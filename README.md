# Sapphire

Sapphire は Haskell 並の表現力（型クラス + higher-kinded types）を
目指す関数型言語で、コンパイラは `.sp` ソースを Ruby モジュール
(`.rb`) へ翻訳する。特徴的な機能として、埋め込み Ruby スニペットを
別スレッドで実行して純粋なパイプラインに結果を返す **作用モナド
**（effect monad、型 `Ruby a`）を備える。

本リポジトリでは **実装フェーズ**（2026-04-19 開始、同日 exit
到達）でコンパイラ本体を Rust で、ランタイム gem を Ruby で実装
した。続くフェーズの scope は user と合意の上で定める。

## 状態

- **実装フェーズ exit 到達（2026-04-19）** — M9 例題 4 本の
  end-to-end 動作、CLI `sapphire build / run / check`、VSCode
  LSP（診断 / hover / goto-def）、`sapphire-runtime` gem が揃った。
  根拠は `docs/impl/30-first-release-audit.md`。D3 の実 publish と
  L6 completion は並行で継続、次フェーズ scope は user と合意予定。

## Quick start

devcontainer 内（Ruby 3.3 + Rust 1.85.0）を前提。

```sh
# 1. リリースバイナリをビルド
cargo build --release --bin sapphire

# 2. M9 例題 01（hello-ruby）を直接実行（build + ruby 呼び出し）
./target/release/sapphire run examples/sources/01-hello-ruby/Main.sp \
  --out-dir /tmp/m9-01
# => Hello, Sapphire!
# => Hello, world!

# 生成物だけ欲しいときは `build` → 自分で ruby を呼ぶ
./target/release/sapphire build examples/sources/01-hello-ruby/Main.sp \
  --out-dir /tmp/m9-01
ruby -I runtime/lib -I /tmp/m9-01 \
  -e "require 'sapphire/main'; exit Sapphire::Main.run_main"
```

残り 3 例題（parse-numbers / students-records / fetch-summarise）
の smoke 手順は `docs/impl/30-first-release-audit.md` §3。

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
- `crates/sapphire-compiler/` — コンパイラ本体と `sapphire` CLI
  バイナリ。
- `crates/sapphire-lsp/` — Language Server 実装。
- `runtime/` — `sapphire-runtime` gem（Ruby）。
- `editors/vscode/` — VSCode extension（LSP client 皮）。
- `examples/` — 動作確認用の実装サンプル（各トラックのマイル
  ストーン達成ごとにサブディレクトリが増える）。

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
Apache-2.0`）への移行は `docs/open-questions.md` I-OQ11 で追跡中。
