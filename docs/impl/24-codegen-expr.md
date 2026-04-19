# 24. Codegen I7a — 式 → Ruby

Status: **draft**（I7a と同時に着地）。spec 10 §Generated Ruby module shape
と spec 11 §Execution model をターゲット契約とし、I5 が出す
`ResolvedModule` と I6 が出す `TypedProgram` を入力に受けて、モジュー
ルごとに 1 本の `.rb` を吐く。

## スコープ

- 式カテゴリの Ruby への翻訳方針（`Expr::Lit` / `Var` / `App` /
  `Lambda` / `Let` / `If` / `Case` / `BinOp` / `Neg` / `ListLit` /
  `RecordLit` / `RecordUpdate` / `FieldAccess`）。`Do` は I7c で詳
  述するが、ここでは「Expr::Do は codegen 時点でその場で `>>=` 連
  鎖に展開する」方針を規範として記録する。
- Pattern matching の `case ... in` への翻訳方針。
- トップレベル binding の Ruby class method 形への翻訳。

## 基本方針

### 1 モジュール 1 ファイル

spec 10 §Generated Ruby module shape と build 02 §Output tree に従
い、Sapphire モジュール `Foo.Bar` は `sapphire/foo/bar.rb` に出力
し、`Sapphire::Foo::Bar` という Ruby 階層の末端 `class` に束ねる。
末端が単一セグメントのときは `Sapphire::Bar` の class 1 個でよい。

### Curried lambda 生成

Sapphire の関数は全 curry（spec 01 §Core expressions）。コード生成
は関数 `f x y = body` を

```ruby
def self.f
  ->(x) { ->(y) { body } }
end
```

として吐く。呼び出し `f 1 2` は `f.call(1).call(2)`。値位置での関
数参照 `f` は `Sapphire::Mod.f` という値（lambda）を返す。

多 clause 関数 (`f 0 = ...; f n = ...`) は 単一の lambda 本体で
`case` にまとめる（パターン照合は下記）。

### 識別子マングリング

M9 例題では operator 名のユーザー定義が現れないため、v0 は

- `lower_ident` そのまま Ruby method 名（`foo_bar` など snake_case
  も Sapphire 側が許容）
- `upper_ident`（コンストラクタ）は下記 ADT codegen に逃げる

で足りる。operator の mangling（spec 10-OQ1 / I-OQ80 予約）は後続。

### 定数 / 基本値

- `Int` リテラル → Ruby `Integer` リテラル
- `String` リテラル → Ruby `"..."`（エスケープは `String::escape`
  helper で済ませる）
- `[]` → `[]`（spec 10 §Lists）
- `[a, b, c]` → `[a, b, c]`
- `x :: xs` → `[x, *xs]`（cons 演算子も同じ）

### `if` / `case`

`If { cond, then, else }` → `cond ? then : else` または `if ... else
... end`。ネストが深い場合は後者。

`Case { scrutinee, arms }` → Ruby 3.3 の `case/in` pattern match に
変換。パターン:

| Sapphire | Ruby |
|---|---|
| `_` | `_` |
| `x` | `x`（binding） |
| `42` | `42` |
| `"s"` | `"s"` |
| `Cons x xs` | `[x, *xs]`（List 専用特別扱い、spec 10 §Lists） |
| `Nil` / `[]` | `[]` |
| `Just x` | `{ tag: :Just, values: [x] }` |
| `Ok a` | `{ tag: :Ok, values: [a] }` |
| `{ f = p, ... }` | `{ f:, ... }` or `{ f: p_translated, ... }` |
| `pat :: type_ann` | 内側パターンのみ emit（type は捨てる） |
| `p1 :: p2` | `[_head, *_tail]` として展開（より具体的に） |

### レコード

- `RecordLit { f = v }` → `{ f: v }`（symbol-keyed hash、spec 10）
- `FieldAccess e.f` → `e[:f]`
- `RecordUpdate { e | f = v }` → `e.merge(f: v)`

### 演算子（BinOp）

M9 で必要な範囲をインライン化（型クラス dispatch なし、Ruby の多
相演算子で済む）：

```
+ - * / % == /= < <= > >=    → 対応 Ruby operator
&& ||                         → Ruby short-circuit
++                            → String concatenation / List への
                                 一般化は concat ではなく、spec 09
                                 で `++` は `String -> String ->
                                 String`（ground type）なので `+`。
::                            → [head, *tail]
>>= >>                        → Sapphire::Prelude.monad_bind /
                                 Sapphire::Prelude.monad_then
```

`==` / `/=` は Ruby の `==` / `!=` に翻訳。Int / String / Bool /
Record / ADT（タグ付き Hash）全て Ruby の構造的等価性で OK。

`show` のような class method は runtime dispatch（§26-codegen-
effect-monad.md 参照）。

## トップレベル binding

`data` / `type` / `class` / `instance` 宣言は spec 10 の Generated
Ruby module shape を踏み、data は constructor factory method を、
value binding は class method を emit。

```
data Maybe a = Nothing | Just a
```

は

```ruby
module Sapphire
  class Prelude   # or Main, depending on home module
    def self.Nothing = Sapphire::Runtime::ADT.make(:Nothing, [])
    def self.Just(x) = Sapphire::Runtime::ADT.make(:Just, [x])
  end
end
```

ここでの data 登録詳細は `docs/impl/25-codegen-adt-record.md`。

### prelude の取り扱い

spec 09 の Prelude 値（`+`, `map`, `foldr`, …）は **codegen が emit
する生成コードの一部** として `Sapphire::Prelude` に束ねる。runtime
gem は prelude を提供しない（CLAUDE.md の制約：runtime は変更禁
止）。I8 CLI は各 build で `Sapphire::Prelude` を 1 ファイル
`sapphire_prelude.rb` として emit する。

## Do 記法の扱い

`Expr::Do` は codegen 段でその場で `>>=` 連鎖に展開する（I-OQ58 の
選択肢のうち「codegen 時展開」）。

```
do
  n <- parseInt s
  ns <- parseAll ss
  pure (Cons n ns)
```

は

```
parseInt s >>= (\n ->
  parseAll ss >>= (\ns ->
    pure (Cons n ns)))
```

として Expr を再構築してから通常の expr codegen を通す。`>>=` /
`pure` / `return` の実体は runtime dispatch（I7c §26 参照）。

## 除外

- source map（I-OQ3 DEFERRED）
- operator mangling（spec 10-OQ1, I-OQ80 予約）
- 型クラス dictionary passing（v0 は runtime shape dispatch で回す）
- 巨大 case の decision-tree 最適化
- tail call 最適化

## 今後の拡張

- M10 以降で operator 定義をユーザー側に許すなら mangle を導入
- `Show` / `Eq` などを proper dictionary passing に置き換えるかは
  I-OQ80（予約）で判断
- exhaustiveness 警告（I-OQ60）は case → if-else のフォールスルー
  判定と連動
