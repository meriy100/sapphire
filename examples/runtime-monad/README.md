# runtime-monad 例題

`sapphire-runtime` の R4（`Sapphire::Runtime::Ruby` effect monad
primitives）を Ruby から直接叩く最小サンプル。まだ Sapphire
コンパイラが通せるソースは無い段階なので、生成コードが I7c で
出すことになる呼び出し形を Ruby 側で手書きする構成。

## 前提

- Ruby 3.3 以降（runtime gem の `required_ruby_version = "~> 3.3"`）。
- 追加 gem 依存はない（runtime gem はゼロ依存）。

## 実行

リポジトリのルートから以下を実行する:

```
ruby -I runtime/lib examples/runtime-monad/hello_ruby.rb
ruby -I runtime/lib examples/runtime-monad/chained_bind.rb
ruby -I runtime/lib examples/runtime-monad/failure.rb
```

いずれも標準出力に各ステップの結果を出し、最後に `OK: ...` を
印字して正常終了する。

## 内容

- `hello_ruby.rb` — `prim_embed { puts "hello"; {} }` を `run` で
  駆動し、`[:ok, {}]` が返ることを確認する最小例。
- `chained_bind.rb` — `prim_bind` で 3 段の連鎖を組み、do 記法が
  脱糖された後の形（spec 11 §`:=` and `Ruby` — the loop closed）を
  手書きで再現する。最終値 `"got 30"` を確認。
- `failure.rb` — `prim_embed { raise ArgumentError, ... }` が
  `[:err, RubyError]` で短絡し、続く bind が実行されないこと
  （spec 11 §Execution model item 5）を確認する。

## 参照

- `docs/spec/11-ruby-monad.md` — effect monad の規範。`primReturn`
  / `primBind` / `run` / 実行モデル。
- `docs/spec/10-ruby-interop.md` §Exception model — `RubyError` の
  スキーマ、`StandardError`-only rescue scope（B-03-OQ5）。
- `docs/build/03-sapphire-runtime.md` §The `Ruby` monad evaluator —
  runtime 契約。
- `docs/impl/14-ruby-monad-runtime.md` — 本実装の設計メモ。
