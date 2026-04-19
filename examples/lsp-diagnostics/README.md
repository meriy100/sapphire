# examples/lsp-diagnostics

Sapphire LSP（L2 diagnostics）の動作確認用サンプル群。各 `.sp` は
**意図的にどこかの段で落ちる／落ちない**ように組んであり、VSCode
拡張越しに開くと赤下線（Diagnostic）が出るかどうか、どの
`code = sapphire/...-error` が付くかを実機で確認できる。

## 含まれるもの

| ファイル | 想定 diagnostic | 検査する経路 |
|---|---|---|
| `good.sp` | **無し**（以前のエラーが残っている場合はクリアされる） | lex / layout / parse すべて通過 |
| `lex_error.sp` | `code = sapphire/lex-error` | `LexErrorKind::NonAsciiIdentStart` |
| `layout_error.sp` | `code = sapphire/layout-error` | `LayoutErrorKind::UnclosedExplicitBlock` |
| `parse_error.sp` | `code = sapphire/parse-error` | `ParseErrorKind::UnexpectedEof` / `Expected { expected: "=", .. }` |

各サンプルがどの段で落ちるかは、
`crates/sapphire-lsp/tests/example_diagnostics.rs` の integration
test で pin している。将来レキサ / パーサが緩くなって「`lex_error.sp`
がパースまで通るようになった」等が起きると、その test が先に赤くなる。

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

4. Extension Host のウィンドウで本ディレクトリの `.sp` ファイルを
   順に開く。`lex_error.sp` / `layout_error.sp` / `parse_error.sp`
   では赤下線が出て、**問題** パネルに該当行と
   `[sapphire] sapphire/<lex|layout|parse>-error` の表示が出る想定。
   `good.sp` では何も出ず、以前のエラーが残っていた場合はクリアされる。

5. `good.sp` を開いた状態で編集して壊すと（例えば `greeting = "hi"`
   の `"hi"` を `"hi` にする）、`didChange` 経由で diagnostic が
   再計算されて赤下線が出る。壊したぶんを戻すと即座に消える。

## バイナリのパスを変えたいとき

`SAPPHIRE_LSP_PATH` 環境変数で上書きできる。未設定なら
`sapphire-lsp` を `$PATH` から解決する。詳細は
`editors/vscode/README.md` を参照。

## L2 の制約

- stdin/stdout 経由の LSP のみ（TCP / pipe は非対応、I-OQ10）。
- 同期モデルは **Full text sync**。`didChange` のたびに全文再解析
  する。incremental sync は L3 で足す（I-OQ9）。
- 1 ファイルにつき **最大 1 件** の diagnostic しか返らない。パーサ
  に error recovery を入れていないため、最初のエラーで止まる。
  詳細と今後の拡張方針は `docs/impl/17-lsp-diagnostics.md` と
  I-OQ52 を参照。
- Resolver / type checker は L2 スコープ外。I5 / I6 が main に
  揃ったら本 crate の `analyze` を拡張し、名前解決・型検査由来の
  diagnostic も返せるようにする。
