# 11. `sapphire-runtime` R2/R3 — ADT helpers と境界 marshalling

本文書は R2（`Sapphire::Runtime::ADT`）と R3
（`Sapphire::Runtime::Marshal`）の実装で行った設計判断を記録する。
契約は `docs/build/03-sapphire-runtime.md` と
`docs/spec/10-ruby-interop.md` に既にあるので、**本文書は契約を
どう Ruby コードに畳み込んだか** の実装側の判断に限定する。

状態: **active**。R4（`Ruby` monad）および R5（`RubyError`）で
本実装の上に更に積み上げる。

## スコープ

- `ADT` モジュールの Ruby 表現選択（継承クラス階層 vs. タグ付き
  ハッシュ DSL）と、その理由。
- `Marshal.from_ruby` / `to_ruby` が R3 時点で担う範囲と、意図的
  に R3 から外した範囲。
- `Errors` 階層のうち R3 で初めて raise 側が埋まる点の整理。
- R1 の `docs/impl/08-runtime-layout.md` が敷いたレイアウト上に
  R2/R3 がどう載るか。

対象外:

- `Ruby` monad 評価器（R4）と、その `run` の `Result RubyError a`
  出力（R5）。Marshal の型引数版 `to_ruby(value, type)` /
  `to_sapphire(value, type)` の完全実装も R4 以降に送る。
- 演算子メソッドの mangle 方式（10-OQ2）。R3 の Marshal は
  演算子コンストラクタ名を扱わない。

## ADT の Ruby 表現

### 採用: タグ付きハッシュ + ファクトリ DSL

`ADT` モジュールの中核は `ADT.make(tag, values)` が返す

```ruby
{ tag: :K, values: [ruby(v1), ..., ruby(vk)] }.freeze
```

という frozen なハッシュである。これは `docs/spec/10-ruby-
interop.md` §ADTs がそのまま示している形で、§Design notes の
「Tagged-hash ADTs, not classes」の規定にも合致する。spec は
**「クラス per コンストラクタは採らない」** と明示しているので、
`Red < Color < Struct` 的な継承階層は検討せず却下。

ergonomic のために `ADT.define(mod, :Just, arity: 1)` という DSL
を足した。これは spec 10 §Generated Ruby module shape が示す

```ruby
module Sapphire
  class Prelude
    def self.Nothing      = { tag: :Nothing, values: [] }
    def self.Just(x)      = { tag: :Just,    values: [x] }
  end
end
```

のコード片を生成コード外でも再利用可能にするための薄いラッパで、
R7b（生成コードの ADT/レコード codegen）で出す `def self.Just(x)`
が本 DSL を呼び出す形にできる。DSL を噛ませることで、万一 ADT
の内部表現を将来差し替える場合にも生成コードを touch せず済む
（`docs/build/03-sapphire-runtime.md` §ADT helpers の最後の段落が
想定するマージン）。

### なぜ frozen か

spec 10 §ADTs は値が immutable である旨を直接書いていないが、
Sapphire の pure value semantics を Ruby 側にそのまま持ち込むのが
最も safe な既定である。`freeze` により:

- 生成コードが `value[:values] << x` 的な破壊を書いても即 raise
  する（契約違反を早期発見）。
- frozen なシンボルキー Hash は `Ractor.make_shareable` との相性も
  良い（11-OQ1 の Ractor 採用メモに将来効いてくる）。

### 構造的等価

タグ付きハッシュ表現の副産物として、Ruby の `Hash#==` と
`Hash#hash` がそのまま「タグ一致 + `values` 配列一致」の構造的等価
になる。`04-OQ2 DECIDED` の「位置引数のみ」とも整合し、`==` /
`hash` を独自実装する必要はない。`adt_spec.rb` が 3 ケースで明示的
に検査している。

## Marshal の責務範囲

### R3 の shape-driven サブセット

`docs/build/03-sapphire-runtime.md` §Marshalling helpers では
`to_ruby(value, type)` / `to_sapphire(value, type)` の **型引数版**
がスケッチされているが、型引数のエンコードは `B-03-OQ2` が未決で
R3 着地時点では決めたくない。そこで R3 では型引数を取らず、
**shape で routing する `from_ruby(value)` / `to_ruby(value)` の
対** を実装し、以下の範囲を担保する:

| 入力 Ruby 値の shape | 判定 | 出力 |
|---|---|---|
| `true` / `false` | そのまま Bool | 値自身 |
| `Integer` | `Int` | 値自身 |
| `String` | `String`（UTF-8 再エンコード + freeze） | frozen UTF-8 |
| `Array` | `List a` | 要素を再帰 marshal、frozen |
| `Hash` with `{:tag, :values}` | ADT | `ADT.make` で frozen |
| `Hash` with symbol keys のみ | record | 各値再帰、frozen |
| `:lt` / `:eq` / `:gt` | `Ordering` | 値自身 |

`records` と `tagged ADT hashes` は両方 `Hash` だが、`:tag` と
`:values` の 2 キーちょうどの組を R3 では ADT として先に拾う。
これで spec 10 §ADTs 末尾が触れる「user record のキーが `tag /
values` と偶然一致する曖昧性」は R3 では「ADT と解釈される」側に
倒れる。spec は「expected Sapphire type で routing する」と規定
しているので、最終的な曖昧解消は型付き版（R4 以降）で行う。R3
docstring でこの点を明記して、型引数版が乗ったときの挙動差分を
ユーザが期待できるようにしている。

### 拒否対象（いずれも `MarshalError`）

- `Float` — 07-OQ6（`Num` クラス化）着地まで Sapphire に浮動小数点
  は入らない。silent coerce せず shape error として落とす。
- `nil` — `10-OQ1 DECIDED` で「nil ↔ Nothing の近道は取らない」と
  確定しているので明示拒否。
- `:lt` / `:eq` / `:gt` 以外の `Symbol` — 境界で Ruby シンボルが
  Sapphire 型に対応付くのは `Ordering` の 3 値だけ。
- 文字列キー Hash — `10-OQ3 DECIDED` でシンボルキー契約なので
  string-keyed は record として受けつけない。
- `Ractor`、`Proc`、任意の Ruby オブジェクト — 関数値の境界越え
  は R4 の `Ruby` monad 実装時に lambda 橋渡しを入れる。R3 では
  shape error として拒否して、入り口を絞る。

### Errors の raise 箇所

`runtime/lib/sapphire/runtime/errors.rb` は R1 で以下の 3 クラス
のみが用意されていた:

- `Errors::Base` — ルート。
- `Errors::MarshalError` — Marshal の shape mismatch で raise。
- `Errors::BoundaryError` — ADT 契約違反で raise。

R3 着地で **raise 側を全件埋めた**:

- `Marshal.from_ruby` / `to_ruby` → shape 外入力は `MarshalError`。
- `ADT.make` / `.match` / `.tag` / `.values` / `.define` / その
  引数バリデーション → `BoundaryError`。

R5 で入る「境界 catch が StandardError を `RubyError` に包む」
パスは、実際に `Ruby a` が動き始める R4 以後。本文書ではその
事前配置のみを指摘する。

## 未対応（後続トラックに送る項目）

- **型引数版 `to_ruby(value, type)` / `to_sapphire(value, type)`**
  — `B-03-OQ2` の型エンコード決定後に R4 で導入。R3 の
  shape-driven 版は内部では残るが、境界コードが呼ぶ entry point
  は型引数版になる予定。
- **`Float` 対応** — 07-OQ6 連動。
- **`Proc` / `Lambda` の境界橋渡し** — spec 10 §Functions。R4 の
  `Ruby.bind` / `Ruby.run` が lambda の wrap を必要とする時点で
  追加する。
- **`ruby_eval`（仮称）で出てくる任意 Ruby オブジェクトの opaque
  保持** — 仕様側にまだ入っていない構想。`11-OQ5`（Ruby 側共有状態
  の脱出口）連動で、入るとしても DEFERRED-LATER。
- **`Sapphire::Runtime::RubyError.from_exception(e)`** — R5 トラック。

## R1 レイアウトとの整合

`docs/impl/08-runtime-layout.md` §ファイル構成 が敷いた
`lib/sapphire/runtime/*.rb` 単層のレイアウトをそのまま踏襲する。
R2/R3 では `adt.rb` と `marshal.rb` の 2 ファイルに本体を入れ、
`lib/sapphire/runtime.rb` の `require` 順序は触らない。`errors.rb`
の階層も無変更（R1 で揃えた 3 クラスで十分）。テストは
`spec/sapphire/runtime/<name>_spec.rb` を新規追加。

## 他文書との関係

- **`docs/spec/10-ruby-interop.md`**: 規範。R2/R3 の全判断は spec
  10 §Data model / §Exception model のどこを根拠にしたかを参照
  できるようにしている。
- **`docs/build/03-sapphire-runtime.md`**: Ruby gem としての契約
  を与える。本文書はその契約の R3 スコープを埋める。
- **`docs/impl/08-runtime-layout.md`**: R1 のレイアウト決定。本
  文書は R1 のレイアウト上に R2/R3 を積む方針書。
- **`docs/impl/06-implementation-roadmap.md`**: R2/R3 の完了条件を
  give している。R4 以降のタスクは本文書で言及した未対応項目を
  拾っていく。
