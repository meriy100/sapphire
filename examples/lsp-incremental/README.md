# examples/lsp-incremental

Sapphire LSP（L3 incremental document sync）の動作確認用サンプル。
`hello.sp` を VSCode で開いて **1 文字ずつ編集**すると、

- `initialize` 応答で `TextDocumentSyncKind::Incremental` を宣言している
- `textDocument/didChange` で range-based な差分だけが飛んでくる
- サーバ側は差分を既存バッファに適用し、`analyze` を再走させて
  diagnostic を即時返す

という 3 点を実機で確認できる。L2（Full sync）と違い、client は
**毎回ファイル全文を送らない**。

## 起動手順（VSCode）

1. ワークスペースルートで LSP バイナリをビルド。

   ```
   cargo build --bin sapphire-lsp
   ```

2. VSCode 拡張の依存を入れ、TypeScript をコンパイル。

   ```
   cd editors/vscode
   npm install
   npm run compile
   ```

3. `editors/vscode` フォルダを VSCode で開いて **F5** で Extension
   Host を起動。

4. Extension Host のウィンドウで本ディレクトリの `hello.sp` を
   開く。diagnostics が無いので赤下線は出ない。

5. `greeting = "hi"` の `"hi"` を `"hi` に壊すと（閉じ引用符だけ
   削る）、`didChange` 経由で **その 1 文字分の range** が送られ、
   lex / parse が失敗して赤下線が出る。削った `"` を戻すと、
   再び 1 文字分の range が飛んで diagnostic がクリアされる。

## incremental 経路が走っていることをログで確認

`SAPPHIRE_LSP_LOG=trace` を付けて VSCode を起動すると、
sapphire-lsp は stderr に以下のような trace を吐く：

```
TRACE sapphire_lsp::server: textDocument/didChange range-based
  uri=file:///…/hello.sp version=3 changes=1
```

`range=…` が **特定の行/列を指している** ことが L3 incremental の
シグナル。L2 Full sync では range は付かない（client はドキュメ
ント全体を 1 本の change として送る）。

Extension Host 側では VSCode の **拡張機能** パネルで `sapphire-
vscode` の **出力** を選ぶと同じ trace が追える。

## バイナリのパスを変えたいとき

`SAPPHIRE_LSP_PATH` 環境変数で上書きできる。未設定なら
`sapphire-lsp` を `$PATH` から解決する。詳細は
`editors/vscode/README.md` を参照。

## L3 の制約

- テキスト差分は incremental、**再解析は依然として full reparse**。
  edit が来るたびに `analyze` 全体を走らせる。真の incremental
  parsing（AST 再利用）は I-OQ9 / I-OQ67 で継続 punt。
- `LineMap` は edit ごとに再構築する最小実装。巨大ファイルでの
  最適化は I-OQ53 で扱う。
- `rangeLength` フィールドは無視する（LSP 3.17 で deprecated、
  実装ごとに "UTF-16 / byte" の解釈が割れているため）。詳細は
  `docs/impl/21-lsp-incremental-sync.md` §range_length を参照。
- 1 ファイルにつき **最大 1 件** の diagnostic しか返らないのは
  L2 と同様（parser error recovery の punt は I-OQ52）。
