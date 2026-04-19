# examples/lsp-goto

Sapphire LSP（L5 goto-definition）の動作確認用サンプル。`hello.sp`
を VSCode で開き、識別子の上で **F12**（または **右クリック → Go to
Definition**）を叩くと、同じファイル内の定義位置へ飛ぶ。

L1（scaffold）から L3（incremental sync）までの経路はそのまま使い、
`textDocument/definition` を追加で受ける形。サーバは
`initialize` で `definition_provider = true` を宣言している。

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

5. たとえば `main = do ... greet "Sapphire"` の `greet` にキャレ
   ットを置いて **F12** を叩くと、`greet : String -> Ruby {}` の
   行にジャンプする。

## 実機で動くケース

- **関数間参照**：`main` → `greet`, `greet` → `rubyPuts` /
  `makeMessage` の相互 goto。
- **型名の goto**：`pick : T -> Int` の `T` → `data T`。
- **constructor の goto**：`pick` の case-arm `A` / `B` →
  `data T = A | B` の ctor 位置。
- **let 束縛**：`makeMessage` 内の `let greeting = ...` の
  `greeting` 参照 → let 行。
- **関数パラメータ**：`greet name = ...` の `name` 参照 → 左辺
  パラメータ。

## 動かないケース（L5 の制約）

- **別ファイルの定義へは飛ばない**。LSP が開いている同じ
  `hello.sp` 内のみ解決する。複数ファイル管理は後続マイル
  ストーンで対応（`docs/open-questions.md` I-OQ72）。
- **Prelude 名**（`+`, `map`, `Int`, `String`, `Ruby` など）
  には飛ばない。Prelude は現状静的テーブル（`resolver/
  prelude.rs`）で実体の `.sp` ファイルが無いため。I-OQ73。
- **resolver エラーが 1 件でもあるファイル** では goto が
  抑止される（例：未定義名が 1 つあると resolver が失敗し、
  reference side table が得られなくなる）。L5 は resolver
  成功を前提にしている。resolve エラー時に部分 reference
  を返せるようにするのは将来の改善点。

## ログで経路を確認

`SAPPHIRE_LSP_LOG=trace` を付けて VSCode の Extension Host を
起動すると、sapphire-lsp は stderr に以下の trace を吐く：

```
TRACE sapphire_lsp::server: textDocument/definition
  uri=file:///…/hello.sp line=23 character=12
```

F12 を連打するとそのたびに 1 行増える。Response の中身は
tower-lsp が自動で flush するので別途ログ出力は無い。

## バイナリのパスを変えたいとき

`SAPPHIRE_LSP_PATH` 環境変数で上書きできる。未設定なら
`sapphire-lsp` を `$PATH` から解決する。詳細は
`editors/vscode/README.md` を参照。
