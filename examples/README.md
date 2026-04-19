# Sapphire 実装サンプル

本ディレクトリは、各トラックの実装マイルストーンが達成されるたび
**動かせる状態** を保って増えていく。spec や設計メモだけでは実感
しにくい「何がどこまで動くのか」をコードで示すのが目的。

各サブディレクトリは独立した example で、`README.md` に実行手順を
持つ。サブディレクトリ間の依存関係はない。

## サブディレクトリ一覧

| ディレクトリ | 由来 | 状態 | 動かし方 |
|---|---|---|---|
| `lexer-snapshot/` | I3 | runnable | `cargo run -p sapphire-compiler --example lex_dump -- examples/lexer-snapshot/hello.sp` |
| `runtime-adt/` | R2+R3 | runnable | `ruby -I runtime/lib examples/runtime-adt/<script>.rb` |
| `lsp-smoke/` | L1 | editor-side | VSCode で `examples/lsp-smoke/hello.sp` を開き `editors/vscode/` extension を F5 起動 |
| `sources/` | spec 12 送り | parse 不可（現状） | コンパイラ完成後に `sapphire build / run` で動かす |

状態ラベル：

- **runnable** — 現行 HEAD のコードで実行できる。
- **editor-side** — エディタ側の手動操作が要る（CLI だけで検証できない）。
- **parse 不可** — まだコンパイラが受理できない。将来のマイル
  ストーンで動く。

## 追加するときのルール

- 新しいサブディレクトリを足すときは本表にエントリを追加する。
- サンプルは **最小** に保つ。チュートリアルの代わりではない。
- 実行に外部サービス・秘匿情報・ネットワークアクセスを要求しない。
- Ruby スクリプトは `runtime/` 直下の gem を `-I runtime/lib` で
  ロードする前提で書く（インストール済みであることを前提にしない）。
- Rust 側の example は `crates/<crate>/examples/<name>.rb` にコー
  ドを置き、ここ `examples/` には input / expected output の素材
  だけを置く、という分離を既定とする（lexer-snapshot がその形）。
