# runtime-version

R6 で追加した `Sapphire::Runtime.require_version!` のサンプル。

- `ok.rb` — 現在のランタイム (`0.1.0`) を満たす制約（`~> 0.1`、
  `[">= 0.1.0", "< 1.0"]`）を渡して成功ケースを確認。
- `mismatch.rb` — 現ランタイムを満たさない `~> 99.0` を渡し
  `Sapphire::Runtime::Errors::RuntimeVersionMismatch`、構文不正の
  `"not a version"` で `Sapphire::Runtime::Errors::LoadError` が
  raise されることを確認。

## 実行

```sh
ruby -I runtime/lib examples/runtime-version/ok.rb
ruby -I runtime/lib examples/runtime-version/mismatch.rb
```

## 関連ドキュメント

- 契約: `docs/build/03-sapphire-runtime.md` §Versioning and the
  calling convention
- 設計: `docs/impl/16-runtime-threaded-loading.md` §R6 loading 契約
