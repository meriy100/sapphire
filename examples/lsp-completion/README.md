# examples/lsp-completion

Sapphire LSP（L6 `textDocument/completion`）の動作確認用サンプル。
`hello.sp` を VSCode で開き、識別子途中で Ctrl+Space（macOS では
Control+Space）を押すと、スコープ内の候補が popup される。

サーバは `initialize` で
`completion_provider.trigger_characters = ["."]` を宣言している。
英数字入力中は LSP の既定で client 側が自動発火させるので、
`trigger_characters` には `.`（module qualifier / 将来の record
field）だけを載せている。

pipeline は L4 hover / L5 goto-definition と同じで、
`analyze → resolve → typeck::infer::check_module` を full run した
うえで、

- 同一ファイル内の top-level（`ModuleEnv::top_level`）
- `import` / prelude 由来の名前（`ModuleEnv::unqualified`）
- Module qualifier（`ModuleEnv::qualified_aliases` の key）
- Cursor 位置までに入った local binder（lambda / let /
  pattern / do-bind）

を集めて `CompletionItem` のリストに組み立てる。設計詳細は
`docs/impl/31-lsp-completion.md` を参照。

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
   開く。

5. たとえば `main = do ... greet "Sapphire"` の `greet` を消して
   `gr` まで打ち、**Ctrl+Space** を叩くと candidate list が出る：

   ```
   greet       [ƒ] String -> Ruby {}
   greeting    [ƒ] String
   ```

## 実機で動くケース（期待 completion 表示）

- **Top-level value** — `greet`, `greeting`, `makeMessage`,
  `main`, `pick`, `packHalf`, `rubyPuts`。kind は `FUNCTION`、
  detail には推論スキーム。
- **Top-level constructor** — `Alpha`, `Beta`。kind は
  `CONSTRUCTOR`、detail は `Alpha : T` / `Beta : T`。
- **Prelude value** — `map`, `id`, `pure`, `return` など。
  kind は `FUNCTION`、detail は Prelude 側の global scheme。
- **Prelude constructor** — `Just`, `Nothing`, `Ok`, `Err` など。
  kind は `CONSTRUCTOR`、detail は constructor scheme。
- **Module qualifier** — `Main`（本モジュール自身）、`Prelude`。
  kind は `MODULE`。`Main.` まで打つと top-level の各名前が
  候補化される。
- **Local binder** — `greet name = ...` の `name`、
  `makeMessage` 内 `let greeting = ...` の `greeting`、
  `case t of Alpha -> ...` の pattern binder。kind は
  `VARIABLE`、detail は `(local)`。
- **`:=`-embed パラメータ** — `rubyPuts s := "..."` の `s` は
  embed の span 内でのみ visible。本サンプルでは body が Ruby
  文字列なので Sapphire 側の completion は発火しないが、
  他の top-level 位置で `s` が候補化されないことは unit test で
  押さえている。

## 動かないケース（L6 の制約）

- **Auto-import**：未 import の名前は候補化しない。I-OQ108。
- **Fuzzy matching**：server 側は prefix startsWith のみ。
  VSCode 側の fuzzy 検索に委ねる。I-OQ107。
- **Snippet completion**：keyword / 制御構文の雛形は L7 の
  `editors/vscode/snippets/` に分かれている。LSP server 側で
  snippet を返すのは本 milestone の射程外。
- **Record field 補完**：`record.field` の `.` 以降の補完は
  I-OQ109。本 milestone では `.` は module qualifier のみ扱う
  ので、`foo.bar` のような record は補完対象にならない。
- **Resolver エラー中のファイル**：ファイル内のどこかで
  resolver が失敗していると、reference side table が空になる
  ため completion も空を返す。partial resolver API は I-OQ74。
- **Docstring**：現状 `install_prelude` が doc comment を持たない
  ので `CompletionItem.documentation` は空。I-OQ98。

## ログで経路を確認

`SAPPHIRE_LSP_LOG=trace` を付けて VSCode の Extension Host を
起動すると、sapphire-lsp は stderr に以下の trace を吐く：

```
TRACE sapphire_lsp::server: textDocument/completion
  uri=file:///…/hello.sp line=45 character=10
```

キャレットを動かして補完を発火するたびにこの行が積まれる。
