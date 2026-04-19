# Sapphire

Sapphire は Elm に触発された関数型言語で、コンパイラは `.sp` ソース
を Ruby モジュール (`.rb`) へ翻訳する。特徴的な機能として、埋め込み
Ruby スニペットを別スレッドで実行して純粋なパイプラインに結果を返
す `Ruby` 評価モナドを備える（予定）。

本リポジトリは **実装フェーズ**（2026-04-19 開始）にあり、コンパイラ
本体を Rust で、ランタイム gem を Ruby で実装していく。

## ドキュメント索引

- `docs/project-status.md` — プロジェクト全体の現状と、なぜ現在の
  フェーズがそうなっているかの背景。
- `docs/roadmap.md` — フェーズ横断のマイルストーン一覧。
- `docs/open-questions.md` — 未決定仕様 (OQ) の living tracker。
- `docs/spec/` — 言語仕様（英、規範） / `docs/spec/ja/`（日、翻訳）。
  ホスト言語中立。
- `docs/impl/` — 実装側の設計メモ。ホスト言語（Rust）決定以降、
  クレート構成・MSRV・エラー戦略などをここに記録する。
- `docs/build/` — Ruby target 側のビルド契約。
- `docs/tutorial/` — 学習者向けチュートリアル（日）。
- `CLAUDE.md` — リポジトリ内で Claude が従うルール。

## ディレクトリ構成

- `crates/sapphire-core/` — コンパイラと LSP が共有する型。
- `crates/sapphire-compiler/` — コンパイラ本体（CLI バイナリ予定）。
- `crates/sapphire-lsp/` — Language Server 実装。
- `runtime/` — `sapphire-runtime` gem（Ruby）の置き場（R1 以降で
  populate）。
- `editors/vscode/` — VSCode extension（L1 scaffold。`docs/impl/10-
  lsp-scaffold.md` 参照）。
- `examples/` — サンプル `.sp` プロジェクト。`examples/lsp-smoke/`
  は LSP scaffold の動作確認用。

## 開発環境

`.devcontainer/` に定義された devcontainer で作業する（Ruby 3.3 入り）。
Rust ツールチェーンはプロジェクトルートの `rust-toolchain.toml` で
`1.85.0` に pin してある。`rustup` 配下の環境であれば `cargo` を
叩いた瞬間に適切な toolchain が降りる。

## ビルドと検査

```
cargo check --all
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

CI（`.github/workflows/ci.yml`）でも同じコマンドが走る。

## ライセンス

MIT（`LICENSE` 参照）。Rust 生態系慣例の dual-license（`MIT OR
Apache-2.0`）への移行は `docs/open-questions.md` I-OQ6 で追跡中。
