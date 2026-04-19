# 06. スキャフォールディング設計メモ

本文書は I2（実装フェーズ最初のタスク）で導入する Rust プロジェクト
構造の設計メモである。`docs/impl/05-decision.md` で決定した「ホスト
言語 = Rust」を受け、具体のリポジトリ構成・ツールチェーン・CI を
確定する。

ここでの判断は **コード変更前に `docs/impl/` に記録する** という
`CLAUDE.md` §Phase-conditioned rules に従う。

## 決定サマリ

| 項目 | 決定 |
|---|---|
| ワークスペース構成 | **Cargo workspace**（repo ルート `Cargo.toml` に `[workspace]`） |
| クレート配置 | `crates/sapphire-core/` / `crates/sapphire-compiler/` / `crates/sapphire-lsp/` |
| Rust edition | **2024** |
| MSRV | **1.85.0**（I-OQ1 を DECIDED にする） |
| `Cargo.lock` | **コミットする**（バイナリ配布プロジェクトのため） |
| ツールチェーン pin | `rust-toolchain.toml` で `1.85.0` + `rustfmt` + `clippy` |
| フォーマッタ | `rustfmt` 既定 + `rustfmt.toml` に最小設定 |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` |
| CI | GitHub Actions (`check / fmt / clippy / test`) |
| ライセンス | 既存の `LICENSE`（MIT）を維持（I-OQ11 として dual-license を user 確認に上げる） |

## ワークスペース vs 単一クレート

`docs/impl/05-decision.md` §リポジトリ構造 および
`docs/impl/06-implementation-roadmap.md` の配置メモでは、当初
「repo ルート `Cargo.toml` + `src/`」という単一クレート想定で書いて
いた。I2 着手時に再評価した結果、以下の理由で **workspace 構成に
倒す** ことにする。

1. **LSP が別バイナリになる必要がある**。`docs/impl/06-
   implementation-roadmap.md` Track L が示すとおり、Language Server
   は VSCode extension から起動する独立プロセスで、コンパイラ CLI
   (`sapphire`) とはエントリポイントが別。単一クレートだと
   `[[bin]]` を複数持つことになり、後で LSP-only の依存
   （`tower-lsp` 等、L0 で決定）がコンパイラ CLI のビルド時間を
   引っ張る。workspace に切れば LSP 専用依存を LSP クレートに隔離
   できる。
2. **AST・型・診断を両者で共有する必要がある**。roadmap 依存グラフ
   の「Track L は I の analysis stack を再利用」はこの共有を前提
   にしている。workspace 内の共有クレート（`sapphire-core`）に
   据えれば、`sapphire-compiler` と `sapphire-lsp` が一方向に依存
   する形で整理できる。
3. **将来の gem packaging（Track D）と相性が良い**。`sapphire` gem
   には `sapphire-compiler` 由来のバイナリのみを入れ、LSP は別
   バイナリ（VSCode extension が起動）として扱える。クレートの境界
   がそのまま配布単位になる。
4. **テストの切り分けが楽**。`cargo test -p sapphire-core` のよう
   に、型検査器だけ・レキサだけ、といった単位で回せる。

単一クレートでも同じことは最終的には達成できるが、workspace への
切替は後になるほど痛い（依存再構成・import 書き換え）ため、I2 の
時点で入れておく。

05-decision.md / 06-implementation-roadmap.md の「`src/`」という
記述はこの更新で historical reference になる。両文書は決定記録で
あり、I2 着手時の具体化で構成が進化したことは本文書で吸収する
（両決定記録は書き換えない）。

## クレートレイアウト

```
.
├── Cargo.toml                 # workspace root manifest
├── Cargo.lock                 # committed (binary distribution)
├── rust-toolchain.toml        # pin 1.85.0 + rustfmt + clippy
├── rustfmt.toml               # minimal config
├── crates/
│   ├── sapphire-core/         # 共有型: AST, 型, 診断, スパン
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── sapphire-compiler/     # CLI バイナリ + レキサ/パーサ/型/コード生成
│   │   ├── Cargo.toml
│   │   └── src/lib.rs         # I3 以降で src/bin/sapphire.rs を追加
│   └── sapphire-lsp/          # LSP サーバ（L0 以降で中身が入る）
│       ├── Cargo.toml
│       └── src/lib.rs
├── runtime/                   # Ruby sapphire-runtime gem（R1 で populate）
│   └── .gitkeep
└── editor/                    # VSCode extension（L7 で populate）
    └── .gitkeep
```

### クレートの責務

- **`sapphire-core`** — 他 2 クレートから import される共有
  データ型。I2 時点では空（`placeholder()` のみ）。I3 以降で
  `Span` / `Diagnostic` / `SourceId` 等の横断型が入る。AST・型
  表現そのものをここに置くかは I4/I6 時点で判断（`sapphire-
  compiler` 内部にとどめる選択肢もある）。
- **`sapphire-compiler`** — コンパイラ本体。レキサ・パーサ・
  名前解決・型検査・コード生成・CLI。I2 時点では空クレート。
- **`sapphire-lsp`** — Language Server。I2 時点では空。L0 で
  選定された LSP クレート（`tower-lsp` 等）と `sapphire-core` に
  依存する形になる見込み。

### `runtime/` と `editor/` のディレクトリ確保

- `runtime/` は Ruby 側の `sapphire-runtime` gem の置き場。R1
  エージェントが `Gemfile` / `sapphire-runtime.gemspec` /
  `lib/sapphire/runtime.rb` 等を populate する。I2 ではディレク
  トリだけ切り、`.gitkeep` で git に拾わせる。
- `editor/` は L7 で作る VSCode extension の置き場。同じく
  `.gitkeep` のみ。

## MSRV（I-OQ1 の決着）

**`1.85.0` に pin する。**

根拠：

- `docs/impl/05-decision.md` §リポジトリ構造で `edition 2024` を
  採用することが決定済み。edition 2024 は Rust 1.85 で解禁され
  たため、MSRV は 1.85 以上でなければならない。
- 2026-04-19 時点の安定版は 1.85 より新しいが、**MSRV は必要最小
  で置く** 方針（配布先の Rust バージョンを必要以上に要求しない）。
  実装中に新しい stable 機能が必要になった場合、その時点で MSRV
  を前進させる。
- ピン値は `rust-toolchain.toml` で固定し、CI と開発者のローカル
  で同じ toolchain を使わせる。

I-OQ1 を DECIDED（1.85.0）に更新する。

## `Cargo.lock` の方針

**コミットする**。I2 の初回 scaffolding commit 時点では
devcontainer に Rust toolchain が無く `cargo check` を走らせられ
なかったが、main session で rustup 1.85.0 を入れ直し `cargo
check --all` → Cargo.lock 生成 → I2 branch に合流という流れで
本 commit に含めた。

Rust コミュニティの慣例：

- ライブラリクレート（`cdylib`/`rlib` だけを配布）は `Cargo.lock`
  をコミットしない。
- バイナリ配布プロジェクト（`[[bin]]` を持つ・リリース tarball に
  バイナリが入る）は再現可能ビルドのためコミットする。

Sapphire は `sapphire` CLI バイナリを配布物の中心に据える
（`docs/impl/06-implementation-roadmap.md` I8 / Track D）ため、
後者に該当する。

## ツールチェーン設定

### `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.85.0"
components = ["rustfmt", "clippy"]
```

- `rustup` ユーザは `cd` した瞬間に適切な toolchain に切り替わる。
- 非 rustup 環境（後述の配布ビルド等）では参考情報扱い。

### `rustfmt.toml`

最小設定に絞る。edition を固定する以外、`rustfmt` の既定を尊重。

```toml
edition = "2024"
```

強い個人設定（`imports_granularity` / `group_imports` 等）を I2
時点で入れない。理由：

- Rust コミュニティの既定スタイルに合わせれば、外部貢献者が
  馴染みやすい。
- 特殊な設定を入れた後で剥がすのは git history を散らかす。
  必要になった時点で相談して追加する。

### `clippy.toml`

I2 時点では置かない（不要な設定ファイルを増やさない）。必要が出
たら足す。CI 側で `-D warnings` を指定するため、警告はすべてエラー
扱い。workspace レベルの lint グループ（`[workspace.lints]`）も
I2 では導入しない — 空クレートに対して未来の clippy バージョンが
新しい警告を出した場合、`-D warnings` と組み合わさると CI が壊れ
る不確実性を持ち込まないため。必要になったタイミングで追加する。

## エラー処理（stub レベル）

I2 の空クレートには `pub fn placeholder() {}` 以外のコードを入れ
ない。エラー処理方針（I-OQ3：`anyhow` vs カスタム ADT）は I3 以降、
レキサ実装時に具体のエラーを扱い始めるタイミングで確定する。本
文書では I-OQ3 を触らない。

## CI 方針

GitHub Actions、単一ジョブ、以下を順に回す。

1. `cargo fmt --all -- --check` — フォーマット違反で fail（安く
   早く落ちる順に並べる）。
2. `cargo check --all` — 全クレートがビルド可能か。
3. `cargo clippy --all-targets --all-features -- -D warnings` —
   clippy 警告ゼロ。
4. `cargo test --all` — 全クレートのテスト（I2 時点では空）。

OS マトリクスは **当面 `ubuntu-latest` のみ**。macOS / Windows は
Track D（クロスコンパイル）で加える。I2 の CI は「開発者の手元と
同じ環境で lint/test が通るか」を保証するのが目的で、配布ビルドの
matrix はスコープ外。

キャッシュは `actions/cache` を使わず、`Swatinem/rust-cache` の
ような第三者アクションも最初は入れない。必要になったら追加する。
理由：CI を最初から凝らない方が trouble-shoot しやすい。

ワークフローファイルは `.github/workflows/ci.yml` 一本とする。

## ライセンス

既存リポジトリには `LICENSE`（MIT）が存在する。

Rust コミュニティの慣例は **MIT OR Apache-2.0 dual-license**。
これには以下のメリットがある：

- 特許条項（Apache-2.0 §3）を欲しい企業ユーザに対応できる。
- 他の Rust クレートとマージ・再配布する際の compatibility が
  高い。

一方、Sapphire は「Ruby コミュニティ向けに gem として配布する」
プロジェクトであり、Ruby 生態系は MIT が圧倒的に多い。
`sapphire-runtime` gem 側（R1 以降）を MIT で切るなら、コンパイラ
側も MIT に揃えるほうが user facing は一貫する。

**I2 での扱い**：既存の MIT を維持する。dual-license 化は user
判断を要するため **I-OQ11** として `docs/open-questions.md` §1.5
に登録し、後続で決める。I-OQ11 が `dual` 決着した場合は `LICENSE-
MIT` / `LICENSE-APACHE` に分割し、各 `Cargo.toml` の `license`
フィールドを `MIT OR Apache-2.0` にする。

各 `Cargo.toml` の `license` フィールドは I2 では `"MIT"` とする。

## README の役割

`README.md` を repo ルートに置く。**索引のみ**。

- プロジェクトの 1 行説明。
- `docs/spec/` / `docs/impl/` / `docs/build/` / `docs/tutorial/`
  への導線。
- 開発環境（`.devcontainer`）に触れる。
- ビルド方法（`cargo check --all` 等）に 1 行だけ触れる。

`CLAUDE.md` で明示されているとおり、README は map。中身を書く場所
ではない。

## I2 スコープ外（明示）

以下は I2 では **触らない**：

- レキサ・パーサ等の実装コード（I3 以降）。
- パーサ crate の採用（I-OQ2、I3 で piloting）。
- エラー型の具体設計（I-OQ3）。
- `sapphire-runtime` gem の中身（R1）。
- LSP クレート選定（I-OQ5 系、L0）。
- クロスコンパイル CI（Track D）。
- gem packaging 詳細（I-OQ4、D1）。

## I2 で新規登録する OQ

- **I-OQ11 ライセンス dual 化** — MIT 単独 vs `MIT OR Apache-2.0`。
  user 判断待ち。

## 残留する既存 OQ

- I-OQ1：**本文書で DECIDED（1.85.0）**。
- I-OQ2〜I-OQ5：状態変更なし（`DEFERRED-IMPL` 維持）。
