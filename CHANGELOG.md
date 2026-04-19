# Changelog

本ファイルは [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/)
の形式を緩く踏襲し、Sapphire の公開リリース単位で変更点を追記する。
バージョニング方針は `docs/build/03-sapphire-runtime.md` §Versioning
and the calling convention と `docs/impl/12-packaging.md` §5 に従い、
CLI（`sapphire`）と ランタイム gem（`sapphire-runtime`）は原則同じ
major.minor を共有する（I-OQ33）。

## [Unreleased]

（次回リリース候補の差分をここへ追記していく。）

## 0.1.0 — 2026-04-19

Sapphire の **初回公開リリース**。spec-first フェーズ（M1〜M10）
完了後の Rust ホスト実装フェーズで、最小構成のコンパイラ・ランタ
イム・Language Server が揃い、`docs/spec/12-example-programs.md`
の M9 4 例題が end-to-end で動く地点に到達した。

### Added

#### Compiler (`sapphire` CLI / crate `sapphire-compiler`)

- レキサ（I3）、手書き再帰下降 + Pratt 演算子パーサ（I4、I-OQ2
  DECIDED）、名前解決（I5）、Hindley-Milner 型推論 + ADT + 型クラス
  推論（I6a/b/c）、Ruby コード生成（I7a/b/c）。
- CLI サブコマンド `sapphire check <path>`、`sapphire build <path>
  [--out-dir <dir>]`、`sapphire run <path> [--out-dir <dir>]`
  （I8、`docs/impl/27-cli.md`）。`--help` / `--version` 対応、引数
  パーサは追加依存なしの手書き実装（I-OQ83）。
- 生成 Ruby コードは `require 'sapphire/runtime'` に続いて
  `Sapphire::Runtime.require_version!('~> 0.1')` を呼び、CLI と
  ランタイム gem の major.minor 一致を起動時に検証（I-OQ33 /
  I-OQ49 / I-OQ85、定数は `crates/sapphire-compiler/src/codegen/
  mod.rs::RUNTIME_VERSION_CONSTRAINT` に固定）。
- 生成 Ruby 先頭コメントに `# sapphire 0.1.0 / sapphire-runtime
  ~> 0.1` 形式のヘッダを焼き込み（build 02 §File-content shape）。

#### Runtime (`sapphire-runtime` gem)

- `Sapphire::Runtime::ADT` — タグ付きハッシュ `{:tag, :values}` に
  よる代数的データ型ヘルパー（R2、spec 10）。
- `Sapphire::Runtime::Marshal` — Sapphire ↔ Ruby 双方向変換
  （R3、spec 10）。
- `Sapphire::Runtime::Ruby` — `pure` / `bind` / `run` の作用モナド
  evaluator。`run` はネストした再入を独立 Thread として許容
  （R4、I-OQ47 DECIDED、spec 11）。
- `Sapphire::Runtime::RubyError` と境界 `rescue`（`StandardError`
  スコープ。`Interrupt` / `SystemExit` は境界を通り抜ける、B-03-OQ5
  DECIDED）（R5）。
- 生成コードのロード契約（require 順序、`require_version!` による
  ランタイム version 検証）（R6、I-OQ49）。
- 必要 Ruby: `~> 3.3`（B-01-OQ1）。

#### Language Server (`sapphire-lsp` crate + VSCode extension)

- LSP サーバ skeleton（initialize / shutdown、L1）、parse error を
  `textDocument/publishDiagnostics` に返す経路（L2）、`didOpen` /
  `didChange` / `didClose` に対するテキスト incremental sync
  （L3、`LineMap` ベース、真の incremental parse は punt、I-OQ9）。
- `textDocument/hover`（L4、top-level scheme を表示、local binder
  は名前と `(local)` タグのみ、I-OQ96）、`textDocument/definition`
  （L5、同一ファイル内 goto）。
- VSCode extension（TypeScript、`editors/vscode/`、L7）。TextMate
  grammar + language configuration + 基本 snippets を同梱
  （I-OQ76 / I-OQ77 で approximation の既知事項を追跡）。
- `textDocument/completion`（L6）は本 0.1.0 と並走して実装中で、
  merge 状況によっては 0.1.0 リリース時点で未含まれの可能性が
  ある。含まれない場合は `CHANGELOG.md` の次回リリースで Added
  に昇格する。

#### Distribution & CI

- GitHub Actions 2 系統：(1) `.github/workflows/ci.yml` は push /
  pull_request 契機で `ubuntu-latest` 単独 `check / fmt / clippy
  / test` + runtime rspec（I-OQ5 DECIDED）。(2) `.github/workflows/
  release-build.yml` は `workflow_dispatch` / tag `v*` push 契機で
  5 platform matrix（`x86_64-unknown-linux-gnu`、`aarch64-unknown-
  linux-gnu`、`x86_64-apple-darwin`、`aarch64-apple-darwin`、
  `x86_64-pc-windows-msvc`）で `cargo build --release --bin
  sapphire` を回し、tar.gz / zip + SHA-256 アーカイブを生成。tag
  push のときのみ GitHub Release に attach（D3）。
- 配送戦略は D1（`docs/impl/12-packaging.md`）方式 X（素の Rust
  バイナリを gem の `exe/` に同梱、`rb-sys` 非採用、I-OQ30）。
  配送単位は「(A) 単一 `sapphire` + `sapphire-runtime`」方式を
  0.1.0 で採る方針（I-OQ29）。初回リリースの実作業は
  `docs/impl/32-release-process.md` にチェックリスト化。

#### Documentation & Spec

- spec 01〜12 の初回公開版（英語規範 + 日本語翻訳、M10 spec-freeze
  review 済）。実装側 `docs/impl/` には I1〜I9、R1〜R6、L0〜L7、
  D1〜D3 の設計メモを収録。
- tutorial 05〜07 改訂（Haskell 相当の型クラス + HKT 方針を反映、
  T2a/b/c）。

### Known Limitations

初回リリースであり、以下は既知の未対応。実用時の影響と回避策は
`docs/open-questions.md` を参照。

- **パターン網羅性 / 到達可能性検査は未実装**（I-OQ60）。clause に
  漏れがあってもコンパイル時エラーにならない。最低限の「多 clause
  関数の最終 clause が全 pattern をカバー」判定のみ緩くかけている。
- **型置換 `Subst::compose` に "先勝ち" の silent drop がある**
  （I-OQ63）。`let f x = x in let y = f 1 in f "hi"` のような偶然
  通ってしまう let-poly 誤認の余地が残る（soundness hole）。
- **`pure` / `return` fallback は runtime raise**（I-OQ82）。
  specialisation の単位は **enclosing top-level binding の
  return-type head**。`do` block が複数 monad にまたがる書き方
  （M9 例題には無い）では runtime で `Sapphire::Prelude.
  pure_polymorphic` fallback が発火する。
- **resolver エラーが 1 件でもあると reference side table を失う**
  （I-OQ74）。LSP goto / hover が resolve エラー下で沈黙する。
- **cross-file goto は未対応**（I-OQ72）。同一ファイル内のみ。
  Prelude 定義への goto も未対応（I-OQ73、I-OQ44 連動）。
- **VSCode extension の TextMate grammar はブロックコメント nesting
  を 1 段までしか追えない**（I-OQ76）。tree-sitter / semantic
  tokens への昇格は次回以降。
- **VSCode extension marketplace への公開は未実施**（I-OQ78 DEFERRED）。
  `editors/vscode/package.json` の `publisher` は placeholder のまま
  で、本 0.1.0 では VSIX として手動配布 or リポジトリ内ビルド経由
  に留める。
- **gem 署名（`--sign`）/ sigstore / OIDC trusted publisher / SBOM
  は未導入**（I-OQ31 DECIDED for 0.1.0：本リリースでは導入しない。
  将来の 0.2.x 以降で再評価）。
- **Windows arm64 は best-effort**（I-OQ32）。CI matrix から外して
  いる。x86_64 Windows は first-class。
- **ライセンスは MIT 単独**（I-OQ11）。Rust 生態系慣例の `MIT OR
  Apache-2.0` dual 化は 0.2.0 以降で user 判断。

### Breaking Changes

N/A（初回リリース）。

### Contributors

- meriy100 (<kouta@meriy100.com>) — project lead、spec、実装、レビュー。
- Claude (Anthropic) — 実装補助、レビュー、文書整備。詳細は各
  commit の `Co-Authored-By` trailer を参照。
