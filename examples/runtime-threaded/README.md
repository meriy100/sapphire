# runtime-threaded

R5 で導入した `Ruby.run` の thread 分離を確認するサンプル。

- `main.rb` — caller thread と `prim_embed` block 内 thread が
  別の `Thread.object_id` を持つことを確認。2 回目の `run` も
  fresh Thread が起こされることを確認。
- `reentrant.rb` — `prim_embed` block の中で更に `Ruby.run` を
  呼ぶネスト構造で、それぞれ独立な evaluator Thread が使われる
  こと、内側 `run` の失敗が `[:err, _]` として外側へ戻ること
  を確認。

## 実行

```sh
ruby -I runtime/lib examples/runtime-threaded/main.rb
ruby -I runtime/lib examples/runtime-threaded/reentrant.rb
```

## 関連ドキュメント

- 規範: `docs/spec/11-ruby-monad.md` §Execution model
- 設計: `docs/impl/16-runtime-threaded-loading.md`
- 契約: `docs/build/03-sapphire-runtime.md` §Threading model
