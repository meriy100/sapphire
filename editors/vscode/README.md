# sapphire-vscode

Sapphire 言語用の VSCode 拡張。`.sp` ファイルに
`language id = sapphire` を割り当て、別プロセスの
`sapphire-lsp` バイナリへ LSP で接続する。

## 同梱機能

- **Syntax highlighting**（TextMate grammar、`syntaxes/sapphire.tmLanguage.json`）
  - キーワード・演算子・識別子・数値・文字列・三連引用符・コメント
    （`--` / `{- -}`）を近似ハイライトする
  - `module Foo.Bar ( ... ) where` / `import qualified Foo as F` の
    モジュール名、`name : Type` のシグネチャ、`name args := """..."""` の
    Ruby 埋め込みを専用スコープに振る
  - 完全な parser ではないので、ネストブロックコメント等は近似どまり
    （詳細は `docs/impl/23-vscode-extension-polish.md`）
- **Snippets**（`snippets/sapphire.code-snippets`）
  - `module` / `main` / `data` / `type` / `case` / `let` / `if` /
    `do` / `class` / `instance` / `ruby` / `record` / `update` の
    13 本。M9 例題 + tutorial ch1-5 の骨格に沿っている
- **Language configuration**（`language-configuration.json`）
  - コメントトークン、bracket / auto-close / surrounding pair、
    word pattern、簡易 indent rule、`--` コメント継続の `onEnter`
- **LSP client**
  - `sapphire-lsp` を stdio transport で起動し、initialize /
    shutdown / textDocument sync（incremental）/ publishDiagnostics
    に追随。`textDocument/definition` は L5 実装中で、L4 hover /
    L6 completion は未着手。
  - 現状は **lex / layout / parse エラーの診断** が動作ライン。

機能カバレッジの全体像は
`docs/impl/06-implementation-roadmap.md` §Track L を参照。

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
   拡張が activate し、syntax highlight / snippets / LSP client が
   有効になる。

## Configuration

拡張が VSCode の設定 UI に公開する項目：

| Setting | Type | Default | 用途 |
|---|---|---|---|
| `sapphire.lsp.path` | `string` | `""` | `sapphire-lsp` バイナリの絶対パス。空なら環境変数か `$PATH` にフォールバック |
| `sapphire.lsp.log` | enum `"error"`/`"warn"`/`"info"`/`"debug"`/`"trace"` | `"info"` | `SAPPHIRE_LSP_LOG`（`tracing_subscriber::EnvFilter`）へ渡るレベル |
| `sapphire.trace.server` | enum `"off"`/`"messages"`/`"verbose"` | `"off"` | client ↔ server 間の LSP JSON-RPC をトレース出力 |

**バイナリ解決の優先順**（高い順）：

1. 環境変数 `SAPPHIRE_LSP_PATH`
2. 設定 `sapphire.lsp.path`
3. `$PATH` 上の `sapphire-lsp`

**ログレベルの優先順**：

1. 環境変数 `SAPPHIRE_LSP_LOG`
2. 設定 `sapphire.lsp.log`

設定変更後は **window reload が必要**。拡張はそれを検知して
reload の提案メッセージを出す。

## ログ

`sapphire-lsp` は stderr に `tracing` ログを出す。VSCode の
`出力` パネル → `Sapphire Language Server` に流れる。

`sapphire.trace.server = "messages"` / `"verbose"` にすると、
LSP の JSON-RPC メッセージが同じ出力チャネルに書き出される
（サーバ側のログと混ざらず、client 側が出す別レイヤのトレース）。

## 既知の制約

- Syntax highlight は TextMate grammar による **近似**。完全な
  parser ではないので、以下のケースは期待通りにならないことがある：
  - ネストブロックコメント `{- {- -} -}` は 1 段しか追えない
  - 複雑な infix 演算子周りで隣接識別子のスコープが揺れる
  - `do` ブロックのインデント機微は `indentationRules` の正規表現
    で近似しているだけなので、`where` を挟んだ深い入れ子は手直しが
    必要になる場合がある
- LSP 側の機能は Track L のマイルストーン進捗に依存。現状は
  parse-error diagnostics と goto-definition までが動作する
  （semantic tokens / completion / inlay hints は未着手）
- アイコン画像は同梱していない（`package.json` に `icon` を指定
  していない）。marketplace 公開時に整える予定

## 注意

- `node_modules/` と `out/` は `.gitignore` と `.vscodeignore` で
  除外している。`package-lock.json` はコミット対象。
- 本ディレクトリは Cargo workspace の **外** に置いている
  （`editors/vscode/` は TypeScript プロジェクト）。
- Neovim / JetBrains 等、他エディタ向けクライアントを足すときは
  `editors/<name>/` として兄弟ディレクトリを作る想定。
