# 10. LSP scaffold（L1）

Track L の L1 タスクとして、`tower-lsp` ベースの最小 LSP サーバと
最小 VSCode extension を入れる。本文書は **コード変更と同じ commit
に載せる設計メモ** であり、後続 L2〜L6 実装時に「L1 時点で何を
決めたか」を遡れるようにする。

L0 の決定は `docs/impl/07-lsp-stack.md` にある。本文書はその続編で、
**実際にコードを置く際の具体** を固定する。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| LSP crate | **`tower-lsp` 0.20.x**（本家） |
| `lsp-types` の扱い | `tower-lsp` が引き込む版（0.94.x）に **追随**。workspace level では再 pin しない（I-OQ6 を DECIDED にする） |
| 非同期ランタイム | `tokio` 1.x、features = `rt-multi-thread` + `macros` + `io-std` |
| ロギング | `tracing` + `tracing-subscriber`（`env-filter` + `fmt`） |
| JSON-RPC transport | **stdin/stdout のみ**（I-OQ10 は引き続き DEFERRED-LATER） |
| バイナリ名 | `sapphire-lsp`（`[[bin]]` として `crates/sapphire-lsp/src/main.rs`） |
| CLI 表面 | 引数なしで serve / `--version` / `--help`。他フラグは未定義（L2 以降で追加） |
| 初回 capabilities | `textDocumentSync = Full` のみ |
| VSCode extension 配置 | `editors/vscode/`（repo ルート直下、Cargo workspace 外） |
| `node_modules` | **コミットしない**。`package-lock.json` はコミット |
| smoke test | `src/server.rs` のユニットテストで `InitializeResult` を検査（`#[tokio::test]`） |

## `tower-lsp` バージョン方針（I-OQ6 / I-OQ7）

本家 `tower-lsp` は近年のリリースペースが落ちているが、0.20.0 時点
で LSP 3.17 の主要メッセージ型は `lsp-types` 0.94 経由で揃ってい
る。L1 は initialize / shutdown / textDocument sync しか触らず、
未対応の新規メッセージに依存しないため **本家を採用** する。fork
（`tower-lsp-server` 等）への移行は、L2 以降で本家に欠けている
capability が必要になった時点で再評価する（I-OQ7 は引き続き
DEFERRED-IMPL）。

`lsp-types` は `tower-lsp` が再公開する版をそのまま使う。workspace
の `Cargo.toml` に `lsp-types = "..."` を明示しない。理由：

1. `tower-lsp::lsp_types::*` で参照できるので LSP 層から別途
   import する動機が薄い。
2. `tower-lsp` が依存バージョンを上げた時に、workspace 側も追随
   する工数が不要。
3. 明示 pin が必要になるのは、`tower-lsp` の型 API に足りない
   フィールドを外部で足したいときだが、L1 ではそのケースがない。

したがって **I-OQ6 は "`tower-lsp` の依存に追随する" で DECIDED**。

## capabilities の現状と今後

L1 で返す `ServerCapabilities` は以下のみ：

```rust
ServerCapabilities {
    text_document_sync: Some(TextDocumentSyncCapability::Kind(
        TextDocumentSyncKind::FULL,
    )),
    ..ServerCapabilities::default()
}
```

拡張計画：

- **L2 (diagnostics)**: `publishDiagnostics` を使う（capability 宣言
  は不要）。解析結果をキャッシュするドキュメントストアを server に
  持たせる。
- **L3 (incremental sync)**: `TextDocumentSyncKind::INCREMENTAL` に
  切り替え、差分適用を実装する。同時にデバウンス（連続 change の
  圧縮）も入れる。
- **L4 (hover)**: `hover_provider: Some(HoverProviderCapability::Simple(true))`。
- **L5 (goto-definition)**: `definition_provider: Some(OneOf::Left(true))`。
- **L6 (completion)**: `completion_provider` を宣言し、
  `CompletionOptions` の trigger characters を埋める。

capability を足すたびに `InitializeResult::capabilities` を拡張
し、対応する `LanguageServer` trait メソッドを実装する。`tower-lsp`
の trait 実装ベースの設計により、差分実装が自然な粒度になる。

## ログ出力先（I-OQ8）

`sapphire-lsp` は **stderr に `tracing` ログを出す**。LSP の stdout
transport を汚染しないための必須事項であり、`tracing-subscriber::
fmt()` の `with_writer(std::io::stderr)` で強制している。レベルは
既定 `info`、環境変数 `SAPPHIRE_LSP_LOG` で上書きできる
（`tracing_subscriber::EnvFilter` の文法）。

コンパイラ本体（`sapphire-compiler`）のログ整備は I3 以降のタスク
なので、I-OQ8 は本文書では **`tracing` で確定** とのみ記録し、
compiler 側のログ表面は I3 着手時に改めて揃える。

## バイナリと CLI 表面

`crates/sapphire-lsp/Cargo.toml` に `[[bin]] name = "sapphire-lsp"
path = "src/main.rs"` を追加し、`cargo build --bin sapphire-lsp`
でビルドできるようにした。`[lib]` も保持して、server 構造体を
ユニットテストや将来の integration test から呼べるようにしている。

CLI 表面は L1 では以下 3 つのみ：

- 引数なし — stdin/stdout で serve。`tokio::main` で runtime を
  立てる。
- `--version` / `-V` — `sapphire-lsp <version>` を stdout に印字
  して exit 0。
- `--help` / `-h` — 使い方を stdout に印字して exit 0。

未知の引数は stderr にエラーを出して exit 2。`--stdio` のような
LSP 界隈で慣例の冗長フラグは **採用しない**（将来 TCP / pipe を
足すとき、`--tcp <port>` 等と対称に設計し直す予定。I-OQ10）。

## VSCode extension の配置（`editors/vscode/`）

Track L の初回エディタターゲットは VSCode のみだが、将来 Neovim /
JetBrains / Emacs 等を足す可能性があるため、拡張ソースは
**`editors/<name>/` の命名** で並べる。L1 の時点では
`editors/vscode/` のみが存在する。

代替案：

- `vscode-extension/`：単一エディタ前提が名前に入ってしまうので却下。
- `editor/`（単数）：I2 の scaffold で一時的に置いていたが、
  将来複数エディタを足す前提と矛盾する。本 commit で削除し
  `editors/` に揃える。

TypeScript プロジェクトなので Cargo workspace には含めない。
`package-lock.json` はコミット対象（再現ビルド可能性のため）、
`node_modules/` と `out/` は `.gitignore` + `.vscodeignore` で
除外する。

本 commit 時点では devcontainer に Node/npm が入っていない環境
もあるため、`npm install` を CI から走らせる設定は入れず、手元
で `npm install && npm run compile` を走らせる手順を README に
記述するに留める。Node/npm の devcontainer 組み込みは Track D
（配布）で扱う。

## テスト戦略（L1 スコープ）

- **ユニットテスト**：`src/server.rs` の `initialize_result()` が
  `ServerInfo` 名と `Full` sync を返すことを確認する
  `#[tokio::test]` を 1 本置く。mock LSP client は使わず、構造体
  の関連関数を直接呼ぶ。
- **CLI 引数パーサ**：`src/main.rs` の `parse_args` に `--version`
  / `--help` / 未知フラグ / 引数なしのケースを 4 本。
- **Integration test は書かない**：LSP クライアント stub を作り
  JSON-RPC を流す end-to-end テストは L2 以降（診断の振る舞いを
  検証したくなる時点）で用意する。L1 では `cargo build --bin
  sapphire-lsp` が通ること + 上記ユニットテストで足りる。

## 今後の OQ の登録方針

本タスク中に新規の未決事項は発生しなかったため、`docs/open-
questions.md` に新規 I-OQ を追加しない。既存 I-OQ6（`lsp-types`
のバージョン pin）と I-OQ7（`tower-lsp` 本家 vs fork）は本文書の
内容に基づき DECIDED に更新する（tracker 側もこの commit で
ステータス更新）。

## 参照

- `07-lsp-stack.md` — L0 の crate 選定
- `06-scaffolding.md` — Rust workspace レイアウト
- `06-implementation-roadmap.md` — Track L の L1〜L7 位置付け
- `../open-questions.md` §1.5 — I-OQ6 ほか
