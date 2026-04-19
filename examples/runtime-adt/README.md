# runtime-adt 例題

`sapphire-runtime` の R2（ADT helpers）と R3（Marshalling）を直接
叩く最小サンプル。まだ Sapphire コンパイラが通せるソースは無い
段階なので、Ruby 側から runtime gem をそのまま呼ぶ構成になっている。

## 前提

- Ruby 3.3 以降（runtime gem の `required_ruby_version = "~> 3.3"`）。
- 追加の gem 依存はない（runtime gem はゼロ依存）。

## 実行

リポジトリのルートから以下を実行する:

```
ruby -I runtime/lib examples/runtime-adt/define_color.rb
ruby -I runtime/lib examples/runtime-adt/maybe_user.rb
ruby -I runtime/lib examples/runtime-adt/marshal_boundary.rb
```

いずれも標準出力に各ステップの結果を出し、最後に `OK: ...` を
印字して正常終了する。

## 内容

- `define_color.rb` — `Color = Red | Green | Blue` 相当を
  `Sapphire::Runtime::ADT.define_variants` で組み立て、frozen
  性・構造的等価性を確認する。
- `maybe_user.rb` — `Maybe (User { name: String, age: Int })`
  相当を Ruby 側で構築し、`case` でパターンマッチして record
  フィールドを取り出す。
- `marshal_boundary.rb` — `Sapphire::Runtime::Marshal.from_ruby`
  と `to_ruby` を使い、Int / Bool / String / Array / record /
  tagged ADT hash の境界往復と、拒否ケース（nil / Float /
  文字列キー Hash）を示す。

## 参照

- `docs/spec/10-ruby-interop.md` §Data model — 境界 marshalling の
  規範。
- `docs/build/03-sapphire-runtime.md` §ADT helpers / §Marshalling
  helpers — gem の契約。
- `docs/impl/11-runtime-adt-marshalling.md` — 本実装の設計メモ。
