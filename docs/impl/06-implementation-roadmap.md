# 06. 実装フェーズ・ロードマップ

本文書は 2026-04-19 の I1（Rust 決定）以降、第一版完成までの全ト
ラック・全タスクを **living** に追跡する。

## 完成の定義

`CLAUDE.md` §Phase-conditioned rules および
`docs/project-status.md` §Current phase より：

> **第一版完成 = M9 例題プログラムが end-to-end で動く最初の
> コンパイラ + ランタイム、および VSCode で動く Language Server**

具体化：

1. Sapphire ソース（`.sp`）→ Ruby モジュール（`.rb`）のコンパイル
   が動く。
2. 生成された Ruby モジュールを `sapphire-runtime` gem が動かす。
3. `docs/spec/12-example-programs.md` の 4 つの例題が `sapphire
   build && sapphire run` で実行できる。
4. CLI（`sapphire build` / `run` / `check`）が整う。
5. VSCode で開いた `.sp` ファイルに対し、診断（parse error）・
   hover（型情報）・goto-definition が返る Language Server が動く。

## トラック

| トラック | 概要 | 言語 / 配置 |
|---|---|---|
| **I** Implementation | 中核コンパイラ | Rust, repo root `Cargo.toml`, `src/` |
| **R** Runtime | `sapphire-runtime` gem | Ruby, `runtime/`（提案、I2 で確定） |
| **L** Language Server | LSP 実装 + editor extension | Rust（LSP 本体）+ TypeScript（VSCode extension）。**初回は VSCode のみ** |
| **T** Tutorial | T2 ペダゴジー書き直し | JA, `docs/tutorial/` |
| **S** Spec | 13 C-amendment 反映 | EN + JA, `docs/spec/` |
| **D** Distribution | gem packaging / CI / release | repo 全域 |

## 依存グラフ

```
              ┌────────────────────────────────────── Track T (独立)
              ├────────────────────────────────────── Track S (独立、小)
              ├─── Track R (spec 10/11 のみ依存、I と独立に進行可)
              │
Start ── I2 ──┬── I3 Lex ── I4 Parser ── I5 NameRes ── I6 Type ── I7 Codegen ── I8 CLI ── I9 M9統合
              │                │                │             │              │
              │         (Track L は I の analysis stack を再利用)
              │                │                │             │              │
              └── L0 ──┬── L1 ─┼── L2 Diag ─────┤             ├── L4 Hover ──┤
                       │        │                │             │              │
                       │        └── L3 Sync ─────┤             └── L5 GotoDef─┤
                       │                                                       │
                       │                          (I6 後) L6 Completion ─────┤
                       │                                                       │
                       │                           (I7 以降) L7 VSCode ext ──┤
                       │                                                       │
              Track D (I7 以降) D1 gem packaging ── D2 CI ── D3 release ────┘
```

## タスク一覧（31 件）

### Track I (10 task)

| ID | 内容 | 依存 |
|---|---|---|
| **I2** | `Cargo.toml` workspace、`src/` レイアウト、`rustfmt.toml`、CI（`cargo check / fmt / clippy / test`） | — |
| **I3** | レキサ（spec 02 字句） | I2 |
| **I4** | パーサ + AST（spec 01/03/04/05/06/09/10 具象構文） | I3 |
| **I5** | モジュール解決 + 名前解決（spec 08） | I4 |
| **I6a** | HM 中核型推論（spec 01） | I4 |
| **I6b** | ADT + レコード（spec 03/04） | I6a |
| **I6c** | 型クラス + MTC（spec 07） | I6b |
| **I7a** | Codegen 中核：式 → Ruby | I6a |
| **I7b** | Codegen ADT/レコード → タグ付きハッシュ（spec 10） | I6b, R2 |
| **I7c** | Codegen `Ruby` monad（spec 11） | I6c, R4 |
| **I8** | CLI（`build / run / check`、`sapphire.yml`） | I7c |
| **I9** | M9 例題 4 本の end-to-end 通し | I8, R6 |

### Track R (6 task)

| ID | 内容 | 依存 |
|---|---|---|
| **R1** | `sapphire-runtime.gemspec`、`lib/sapphire/runtime.rb`、Gemfile、rspec 雛形 | — |
| **R2** | `Sapphire::Runtime::ADT`（`.make` / `.match` / `.tag_of` / `.values_of`） | R1 |
| **R3** | `Sapphire::Runtime::Marshal`（Sapphire ↔ Ruby 双方向） | R2 |
| **R4** | `Sapphire::Runtime::Ruby`（`primReturn` / `primBind` / `run`、スレッド管理） | R3 |
| **R5** | `Sapphire::Runtime::RubyError` + 境界 rescue（`StandardError` scope） | R4 |
| **R6** | 生成コードのロード契約（`require` 順序、ランタイムバージョン検証） | R5 |

### Track L (8 task) — **VSCode only**

| ID | 内容 | 依存 |
|---|---|---|
| **L0** | LSP crate 選定（tower-lsp vs lsp-server vs async-lsp）、`docs/impl/07-lsp-stack.md` | — |
| **L1** | LSP サーバ skeleton（initialize / initialized / shutdown / exit） | L0, I2 |
| **L2** | parse error → `textDocument/publishDiagnostics` | L1, I4 |
| **L3** | `didOpen` / `didChange` / `didClose` + インクリメンタル再解析 | L2 |
| **L4** | `textDocument/hover`（型情報） | L3, I6 |
| **L5** | `textDocument/definition` | L3, I5 |
| **L6** | `textDocument/completion`（スコープ内の名前） | L4, L5 |
| **L7** | VSCode extension（TypeScript、LSP client 皮、`.sp` の言語登録） | L1（基本）、以後 L2-L6 に追随 |

### Track T (3 task)

| ID | 内容 | 依存 |
|---|---|---|
| **T2a** | `docs/tutorial/05-*.md` 書き直し（具体→抽象） | — |
| **T2b** | `docs/tutorial/06-*.md` 書き直し（Monad 比喩整流） | — |
| **T2c** | `docs/tutorial/07-発展篇-型クラス.md`（HKT 隔離） | T2a, T2b |

### Track S (1 task)

| ID | 内容 | 依存 |
|---|---|---|
| **S1** | 13 C-amendment（02-OQ4/5、05-OQ6、08-OQ1/2/5、10-OQ1/3）の本文反映 | — |

### Track D (3 task)

| ID | 内容 | 依存 |
|---|---|---|
| **D1** | Rust バイナリを `sapphire` gem として配布する設計（platform-specific gem） | I7a 以降でいつでも |
| **D2** | CI クロスコンパイル（Linux x86_64 / macOS arm64 / macOS x86_64） | D1 |
| **D3** | 初回リリースプロセス（tag → gem push → GitHub Release） | D2, I9 |

## ウェーブ（並列実行計画）

同時 worktree 数は 4〜5 を上限とする。エージェントは background で
起動し、完了通知を待って main で順にレビュー・マージする。

### ウェーブ 1（2026-04-19 開始）🟢 **完了（2026-04-19）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/i2-scaffold` | **I2** 🟢 |
| 2 | `impl/r1-runtime-scaffold` | **R1** 🟢 |
| 3 | `impl/s1-spec-cleanup` | **S1** 🟢 |
| 4 | `impl/l0-lsp-selection` | **L0** 🟢 |

### ウェーブ 2（I2/R1/L0/S1 着地後）🟢 **完了（2026-04-19）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/i3-lexer` | **I3** レキサ 🟢 |
| 2 | `impl/r2-r3-runtime` | **R2** ADT helpers → **R3** Marshalling 🟢 |
| 3 | `impl/l1-lsp-scaffold` | **L1** LSP protocol scaffold 🟢 |
| 4 | `impl/t2a-tutorial-ch5` | **T2a** tutorial ch5 🟢 |
| 5 | `impl/d1-packaging` | **D1** gem packaging 調査 🟢 |

### ウェーブ 3（I3 着地後）🟢 **完了（2026-04-19）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/i4-parser` | **I4** パーサ + AST 🟢 |
| 2 | `impl/r4-ruby-monad` | **R4** Ruby monad primitives 🟢 |
| 3 | `impl/t2b-tutorial-ch6` | **T2b** tutorial ch6 🟢 |

### ウェーブ 4（I4 着地後）🟢 **完了（2026-04-19）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/i5-resolver` | **I5** 名前解決 🟢 |
| 2 | `impl/r5-r6-runtime` | **R5** thread 管理 → **R6** loading 契約 🟢 |
| 3 | `impl/l2-diagnostics` | **L2** parse-error diagnostics 🟢 |
| 4 | `impl/t2c-tutorial-advanced` | **T2c** tutorial 発展篇（HKT 隔離章） 🟢 |

### ウェーブ 5（I5/L2 着地後）🟢 **完了（2026-04-19）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/i6-typecheck` | **I6a** HM → **I6b** ADT/Record → **I6c** Type classes 🟢 |
| 2 | `impl/l3-sync` | **L3** document sync（incremental 対応の基礎） 🟢 |

### ウェーブ 5.5（機会先取りで並走させた polish）🟢 **完了（2026-04-19）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/l5-goto-def` | **L5** `textDocument/definition` 🟢 |
| 2 | `impl/l7-vscode-polish` | **L7** VSCode extension 拡充 🟢 |
| 3 | `impl/rp-runtime-polish` | **RP** R4/R5 reviewer suggestion 束ね適用 🟢 |

### ウェーブ 6（I6 着地後）🟡 **進行中（2026-04-19〜）**

| # | worktree branch | タスク |
|---|---|---|
| 1 | `impl/i7-i8-codegen-cli` | **I7** codegen（式→Ruby / ADT / 作用モナド）→ **I8** CLI 🟡 |
| 2 | `impl/l4-hover` | **L4** hover（型情報） 🟡 |
| 3 | `impl/d2-ci-cross` | **D2** CI cross-compile matrix 🟡 |

### ウェーブ 7（I7 + R6 着地後、統合）⬜ **未着手**

| # | タスク |
|---|---|
| 1 | **I9** M9 end-to-end |
| 2 | **L6** completion |
| 3 | **D3** 初回リリース |

（L5 / L7 / T2c は先行着地済。）

## 進捗トラッキング

本文書は living で、以下を継続更新：

- 各タスクの完了印（🟢 完了 / 🟡 進行中 / ⬜ 未着手）を左端に付
  ける運用を次回更新から採用する。
- 新しい依存や OQ が発生したら `docs/open-questions.md` に `I-OQk`
  で登録。
- ウェーブ実行途中で dependence が変わった場合、ここを更新して
  `docs/roadmap.md` にも変更を反映する。

## 運用メモ

- **worktree は `.worktree/<name>` 規約**（`CLAUDE.md` §Parallel
  work）。完了後 `git worktree remove` で片付ける。
- **コミット identity**：`meriy100 <kouta@meriy100.com>`（env vars
  で指定、`git config` は変更しない）。
- **reviewer 運用**：各サブエージェントは自分の worktree で自己
  レビューしつつ commit。main マージ前に main セッションから
  reviewer を各 worktree に当てる。
- **Rust 固有の設計判断**（crate 選定、MSRV 確定、エラー型等）は
  コード変更前に `docs/impl/` に追記する（CLAUDE.md §Phase-
  conditioned rules）。
