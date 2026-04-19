# sapphire-vscode

Sapphire 言語用の VSCode 拡張（L1 scaffold）。`.sp` ファイルに
`language id = sapphire` を割り当て、別プロセスの
`sapphire-lsp` バイナリへ LSP で接続するだけの最小構成。

機能は **L1 時点で initialize / shutdown のみ**。診断・hover・
completion などは L2 以降のマイルストーンで足していく（詳細は
`docs/impl/10-lsp-scaffold.md` および
`docs/impl/06-implementation-roadmap.md` の Track L を参照）。

## 前提

- Visual Studio Code 1.85 以上。
- Node.js 20 以上 + npm。
- Rust toolchain（`rust-toolchain.toml` 経由で 1.85.0 が導入される）。

## 開発手順

1. リポジトリルートで LSP バイナリをビルドする：

   ```
   cargo build --bin sapphire-lsp
   ```

   これで `target/debug/sapphire-lsp` ができる。

2. 拡張側の依存を入れてコンパイルする：

   ```
   cd editors/vscode
   npm install
   npm run compile
   ```

   `npm install` は `package-lock.json` を生成する（未コミット時は
   commit してよい）。`npm run compile` は `tsc -p .` を呼び出し、
   `src/extension.ts` を `out/extension.js` に出力する。

3. VSCode で `editors/vscode` フォルダを開き、**F5** で Extension
   Host を起動する。新しく開いたウィンドウで `.sp` ファイルを開くと
   拡張が activate し、`sapphire-lsp` が子プロセスとして起動する。

## バイナリの解決

拡張は起動時に環境変数 `SAPPHIRE_LSP_PATH` を読む：

- セットされていればその絶対パスを直接実行する（開発時は
  `target/debug/sapphire-lsp` を指すのが便利）。
- セットされていなければ `sapphire-lsp` を `$PATH` から解決する
  （リリース配布時はバイナリを PATH に乗せる運用）。

VSCode の launch 設定で `env` を指定するか、シェルから
`code` を起動する前に `export SAPPHIRE_LSP_PATH=...` しておく。

## ログ

`sapphire-lsp` は stderr に `tracing` ログを出す。VSCode の
`出力` パネル → `Sapphire Language Server` に流れる。`SAPPHIRE_LSP_LOG`
環境変数で `tracing_subscriber::EnvFilter` のレベルを上げられる
（例：`SAPPHIRE_LSP_LOG=debug`）。

## 注意

- `node_modules/` と `out/` は `.gitignore` と `.vscodeignore` で
  除外している。`package-lock.json` はコミット対象。
- 本ディレクトリは Cargo workspace の **外** に置いている
  （`editors/vscode/` は TypeScript プロジェクト）。
- Neovim / JetBrains 等、他エディタ向けクライアントを足すときは
  `editors/<name>/` として兄弟ディレクトリを作る想定。
