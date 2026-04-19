# 25. Codegen I7b — ADT とレコード

Status: **draft**（I7b と同時に着地）。

## スコープ

- `data T a₁ ... aₙ = C₁ τ₁ | C₂ τ₂ | ...` 宣言の Ruby 出力
- コンストラクタ呼び出し（`Just 3`）の Ruby 出力
- コンストラクタ・パターン（`Just x`）の `case/in` への反映
- Record リテラル / update / field access の再確認

spec 10 §Data model（tagged hash、symbol-keyed record）、 runtime
契約 `Sapphire::Runtime::ADT.make` を踏襲。

## ADT 宣言

```
data Maybe a = Nothing | Just a
```

は以下を emit：

```ruby
Sapphire::Runtime::ADT.define_variants(
  self,
  [[:Nothing, 0], [:Just, 1]]
)
```

これでモジュールのクラスに `Nothing` / `Just(x)` が生えて、それぞ
れ `{ tag: :Nothing, values: [] }` / `{ tag: :Just, values: [x] }` を
返すようになる（runtime の `ADT.define` 契約）。

Prelude の Bool / Maybe / Result / List / Ordering は codegen が
emit する `Sapphire::Prelude` クラス内に register する。
**`Ordering` は 10 §Ordering により特別扱い**：tagged hash ではな
く Ruby の bare symbol `:lt` / `:eq` / `:gt` を使う。codegen は
`Sapphire::Prelude::LT = :lt` のように定数として emit する。

**`Bool` も特別扱い**：spec 10 §Ground types により Ruby の
`true` / `false` にマッピング。`Sapphire::Prelude::True = true` の
ように emit。

## コンストラクタ参照

**値位置のコンストラクタ**：

- `Nothing` → `Sapphire::Prelude.Nothing`（上の `ADT.define` の成
  果）
- `Just` 単体（`x : Just`）→ `->(v) { Sapphire::Runtime::ADT.make
  (:Just, [v]) }`。curry された関数値として扱う。
- `Just 3` → `Sapphire::Runtime::ADT.make(:Just, [3])`（直接呼び
  出し形は factory を使うより ADT.make 直呼びが短い）

**パターン位置のコンストラクタ**（Ruby の `case/in` パターン）：

- `Nothing` → `{ tag: :Nothing, values: [] }`
- `Just x` → `{ tag: :Just, values: [x] }`
- `Ok a` → `{ tag: :Ok, values: [a] }`
- `Err e` → `{ tag: :Err, values: [e] }`
- `Cons h t` → `[h, *t]`（List は spec 10 で Ruby Array）
- `Nil` → `[]`

パターンが入れ子の場合は Ruby case/in が再帰的に照合する。

## Record

spec 10 §Records に従い symbol-keyed Hash。

- `{ name = "a", age = 30 }` → `{ name: "a", age: 30 }`
- `r.field` → `r[:field]`
- `{ r | f = v }` → `r.merge(f: v)`
- `{ f = p }` in pattern → `{ f: p_translated }`（Ruby 3.x の
  record pattern として動く）

フィールド名が Ruby 予約語と衝突する場合（`class`, `def`, …）：
Ruby の symbol リテラルは予約語も許すので問題ない。hash deref も
OK。

## List

- `[]` → `[]`
- `[a, b, c]` → `[a, b, c]`
- `a :: xs` → `[a, *xs]`
- `Cons` は Prelude の ADT として register されるが、パターン・値
  位置の特別扱い（Array 展開）を codegen が優先する。

## 除外

- ADT の Ruby class（1 constructor = 1 class）化（spec 10 設計ノー
  トで却下済、tagged hash 固定）
- gadt / existential（spec が admit していない）
- deriving（spec 07 未実装）

## 今後の拡張

- `Show` / `Eq` / `Ord` の runtime helper による自動 dispatch は
  既に `Sapphire::Prelude` レベルで入っている。後で proper
  dictionary passing に昇格するかは I-OQ80 以降の判断。
- `Ordering` 以外の「bare-symbol 最適化」の展開。現状は 10 が明
  示的に `Ordering` のみ特別扱い。
