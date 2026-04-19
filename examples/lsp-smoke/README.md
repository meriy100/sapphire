# examples/lsp-smoke

Sapphire LSP（L1 scaffold）の smoke 用サンプル。**言語機能の
デモではなく、LSP が起動してハンドシェイクまで走るか確認する
ためのもの**。

## 含まれるもの

- `hello.sp` — `language id = sapphire` が付く最小の `.sp`。

## 起動手順

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

4. Extension Host のウィンドウで `examples/lsp-smoke/hello.sp` を
   開く。拡張が activate し、`sapphire-lsp` が子プロセスとして
   立ち上がる。

5. VSCode の **出力** パネルで `Sapphire Language Server` を
   選ぶと、`initialize received` / `initialized` / `textDocument/
   didOpen` といったログが見える。診断・hover・completion は L1
   では出ない（L2 以降で入る）。

## バイナリのパスを変えたいとき

`SAPPHIRE_LSP_PATH` 環境変数で上書きできる。未設定なら
`sapphire-lsp` を `$PATH` から解決する。詳細は
`editors/vscode/README.md` を参照。

## L1 の制約

- stdin/stdout 経由の LSP のみ（TCP / pipe は非対応、I-OQ10）。
- `textDocument/didOpen` / `didChange` / `didClose` は受信して
  ログするだけで、ドキュメントストアは持たない。
- パーサ・型検査・診断の接続は L2 以降（Track L）。
