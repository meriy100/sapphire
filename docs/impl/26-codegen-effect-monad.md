# 26. Codegen I7c — 作用モナドと `:=` 埋め込み

Status: **draft**（I7c と同時に着地）。 spec 11 §Execution model と
runtime の `Sapphire::Runtime::Ruby`（R4/R5）契約を target とする。

## スコープ

- `name p₁ ... pₙ := "ruby_source"` の Ruby class method 化
- `do` 記法の `>>=` 鎖への展開（再掲；I7a でも触れる）
- `pure` / `return` の monad 解決
- `>>=` / `>>` の runtime dispatch
- `run : Ruby a -> Result RubyError a` の I-OQ40 対応（`[:ok, v]`
  タプル → `{tag: :Ok, values: [v]}` tagged hash への包装）
- `main : Ruby {}` をエントリーポイントとして実行する helper

## `:=` binding

```
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

は

```ruby
def self.rubyPuts
  ->(s) {
    Sapphire::Runtime::Ruby.prim_embed do
      puts s
    end
  }
end
```

として emit。Ruby snippet は **Ruby の block 本体として埋め込む**
（B-03-OQ3 草案に従い、`eval`'d string ではなく literal proc 形）。
parameter はクロージャー外の lambda parameter として引き回され、
block 内で普通の Ruby local variable として見える。

snippet 本体は `"""` で囲まれた文字列の先頭/末尾の空行だけ落とし、
インデントを調整して emit する（生成された `.rb` が読めるよう
に）。

### 戻り値の marshal

`prim_embed` は block の戻り値を `Marshal.from_ruby` で Sapphire 値
化するので、snippet が tagged hash を返せば Sapphire ADT として扱
われる（spec 10 契約）。`String`/`Integer`/`Array`/`Hash` も同
様。

## `do` 展開

I7a で触れた通り、codegen 段で `Expr::Do` をその場で `>>=` 連鎖に
再構築してから翻訳する。desugar 規則（spec 07 §`do` notation）：

```
do { e }                        = e
do { x <- e; rest }             = e >>= (\x -> do { rest })
do { let x = e; rest }          = let x = e in do { rest }
do { e; rest }                  = e >> do { rest }
```

右結合なので、runtime の `prim_bind` は iterative loop で stack を
消費しない（spec 11 §Execution model §Bind-spine iterativity）。

## `pure` / `return` / `>>=` の dispatch

型クラス dictionary passing を避けるため **runtime shape dispatch**
を採る：

- `>>=` / `>>` は `Sapphire::Prelude.monad_bind(m, k)` / `monad_then
  (m, thunk)` に翻訳。`>>` の第 2 引数は必ず zero-arg lambda
  （`-> { n }`）にする。LHS が `Err _` / `Nothing` 等で短絡する
  monad では thunk が force されず、`n` が評価されない。`monad_then`
  の Ruby 側契約は「第 2 引数は callable zero-arg」に fix されて
  いる。runtime 側は `m` の shape で分岐：
  - `Sapphire::Runtime::Ruby::Action` → `prim_bind(m, &k)`
  - `{ tag: :Ok, values: [v] }` → `k.call(v)`
  - `{ tag: :Err, values: [_] }` → `m`（短絡）
  - `{ tag: :Just, values: [v] }` → `k.call(v)`
  - `{ tag: :Nothing, values: [] }` → `m`
  - それ以外 → raise `Sapphire::Runtime::Errors::BoundaryError`
- `pure` は **enclosing top-level binding の return-type head** に
  依存（`do` block 単位ではない — I-OQ82 参照）。I6 の
  `TypedProgram` を codegen に渡し、各 top-level binding の
  return-type head を見て `pure` を次のいずれかに解決する：
  - head が `Ruby` → `Sapphire::Runtime::Ruby.prim_return(x)`
  - head が `Result` → `{ tag: :Ok, values: [x] }`
  - head が `Maybe` → `{ tag: :Just, values: [x] }`
  - head が `List` → `[x]`
  - 判別できない（多相すぎる、あるいはネストした lambda 内で違
    う monad に入っている）→ **`Sapphire::Prelude.pure_polymorphic
    (x)` に fall back**。runtime 側は警告や例外で知らせる（I-OQ82
    予約）。

この shortcut は M9 の 4 例題では十分：

- Example 1: `main : Ruby {}` → Ruby monad
- Example 2: `main : Ruby {}` / `parseAll : List String -> Result
  String (List Int)` / `parseInt : String -> Result String Int` →
  Result monad の中で `pure` が使われる
- Example 3: pure Sapphire（monad なし）
- Example 4: `main : Ruby {}` → Ruby monad

`pure`/`return` がネストした `do` の中で違う monad に属するケース
は M9 にはない（ただし仕様上は起こり得る）。

## `run` の Result ADT 包装（I-OQ40）

runtime の `Ruby.run(action)` は `[:ok, v]` / `[:err, e]` の 2 要
素タプルを返す（R5 時点の契約、I-OQ40）。codegen 段が `run` 呼び
出し位置でこのタプルを spec 11 §`run` の `Result RubyError a` 形
に包み直す：

```ruby
tuple = Sapphire::Runtime::Ruby.run(action)
case tuple
in [:ok, v]
  Sapphire::Runtime::ADT.make(:Ok,  [v])
in [:err, e]
  Sapphire::Runtime::ADT.make(:Err, [e])
end
```

このロジックは `Sapphire::Prelude.run_action(action)` helper とし
て emit し、codegen は `run e` を `Sapphire::Prelude.run_action(e)`
に翻訳する。

## Main エントリー

spec 11 §`run` に従い、`main : Ruby {}` を定義した module は

```ruby
module Sapphire
  class Main
    def self.run_main
      tuple = Sapphire::Runtime::Ruby.run(main.call)
      case tuple
      in [:ok, _]
        0
      in [:err, e]
        # RubyError = [class_name, message, backtrace]
        warn "[sapphire run] #{e[:values][0]}: #{e[:values][1]}"
        e[:values][2].each { |line| warn "  #{line}" }
        1
      end
    end
  end
end
```

を emit する。`sapphire run` CLI は `ruby -r <generated> -e
'exit Sapphire::<Main>.run_main'` の形で呼ぶ。

## 除外

- 並列 `parallel : Ruby a -> Ruby b -> Ruby (a, b)`（11-OQ1）
- timeout / cancel（11-OQ2）
- 真の class dictionary passing
- tail call 最適化

## 今後の拡張

- runtime-shape dispatch を本物の dictionary passing に昇格
- ネストした do 内の monad 切り替えに対応（現状 fall back にな
  るケース）
- `Ruby` 以外の user-defined effect monad（現行の runtime shape
  dispatch で十分回るが、`Sapphire::Prelude.monad_bind` の分岐を
  ユーザ拡張可能にする API を設計する必要あり）
