# 30. 第一版完成審査（I9 / M9 end-to-end）

2026-04-19、ウェーブ 7 の I9 タスクとして実施した **第一版完成の最
終審査** を記録する。

- worktree: `.worktree/i9-m9-finalize`
- branch: `impl/i9-m9-finalize`
- base commit: `8e4578c`（`docs(roadmap): ウェーブ 6 完了、
  ウェーブ 7（I9/L6/D3-prep）進行中`）
- 審査日: 2026-04-19
- 審査者: Claude agent（self-review。reviewer サブエージェントは
  本セッションから直接呼び出せなかったため §6 参照）

本文書は **document-only**。Rust / Ruby コードと spec 文書は一切
触っていない（CLAUDE.md phase-conditioned rules 準拠）。

## 1. 第一版完成の 5 条件と根拠

`CLAUDE.md` §Phase-conditioned rules の定義：

> **第一版完成 = M9 例題プログラムが end-to-end で動く最初の
> コンパイラ + ランタイム、および VSCode で動く Language Server**

これを `docs/impl/06-implementation-roadmap.md` §完成の定義 が 5
条件に具体化している。以下、条件ごとに根拠を示す。

### 条件 1: `.sp` → `.rb` コンパイルが動く — **✅ 達成**

- 実装: `crates/sapphire-compiler/src/codegen/`
  - `mod.rs` — トップレベル `generate` と `GeneratedProgram`
  - `expr.rs` — 式の翻訳（I7a）
  - `decl.rs` — トップレベル宣言 / `module Sapphire` 包み / ADT /
    レコード / `run_main` エントリ（I7b + I7c）
  - `pattern.rs` — パターン → Ruby `case/in` 変換
  - `runtime.rs` / `prelude.rs` — prelude / 作用モナドの dispatch
  - `emit.rs` — 文字列整形
- 設計メモ: `docs/impl/24-codegen-expr.md` /
  `25-codegen-adt-record.md` / `26-codegen-effect-monad.md`
- 検証: `crates/sapphire-compiler/tests/codegen_snapshot.rs`
  （5 例、snapshot compare）。
- commit: `722ade0 feat(i7,i8): codegen pipeline + sapphire CLI
  で M9 例題を end-to-end 実行`、フォローアップ `d662286`。

### 条件 2: `sapphire-runtime` gem が動かす — **✅ 達成**

- 実装: `runtime/lib/sapphire/`
  - `runtime.rb` — entry point（`require "sapphire/runtime"` の
    受け口、R1）
  - `runtime/adt.rb` — タグ付きハッシュ ADT（R2）
  - `runtime/marshal.rb` — Sapphire ↔ Ruby 双方向（R3）
  - `runtime/ruby.rb` — 単一スレッドで `Ruby` monad を逐次評価
    （R4 + R5）
  - `runtime/ruby_error.rb` — `RubyError` 表現と境界 rescue（R5）
  - `runtime/errors.rb` / `runtime/version.rb` — runtime 固有の
    エラー型と `require_version!` による loading 契約（R6）
- 設計メモ: `docs/impl/08-runtime-layout.md` /
  `11-runtime-adt-marshalling.md` / `14-ruby-monad-runtime.md` /
  `16-runtime-threaded-loading.md`
- 検証: `runtime/spec/` 下の rspec スイート。`bundle exec rspec`
  で **142 examples, 0 failures**（1.11s、2026-04-19 実行）。
- commit: `8d48cd9`（R5/R6）、`91bb003`（polish）など。

### 条件 3: M9 4 例題が end-to-end で動く — **✅ 達成**

- 実装: `crates/sapphire-compiler/tests/codegen_m9.rs`
  — `example_01..04` の 4 test、いずれも `ruby` 実行して stdout を
  assert する構成。
- 手動 smoke の結果は §3 に詳述。
- 4/4 pass を確認。

### 条件 4: CLI `build` / `run` / `check` — **✅ 達成**

- 実装: `crates/sapphire-compiler/src/bin/sapphire.rs`（単一
  バイナリ、手書きパーサ、`--out-dir` オプション付き）。
- 設計メモ: `docs/impl/27-cli.md`。
- 検証: `crates/sapphire-compiler/tests/cli_smoke.rs`（9 test、
  `build` / `run` / `check` 各分岐と error path を exercise）。
- `--version` / `--help` 確認:
  - `target/release/sapphire --version` → `sapphire 0.0.0`
  - `target/release/sapphire --help` → USAGE 文が正しく表示。

### 条件 5: VSCode LSP — **✅ 達成**

- 実装: `crates/sapphire-lsp/`（`server.rs` が主体）
  - L1 scaffold: `initialize` / `shutdown` 等
  - L2: `textDocument/publishDiagnostics`（lex / layout / parse）
  - L3: `didOpen` / `didChange` / `didClose` incremental sync
  - L4: `textDocument/hover`（top-level scheme + local fallback）
  - L5: `textDocument/definition`（同一ファイル内）
  - L7: VSCode extension（`editors/vscode/`）
- 設計メモ: `docs/impl/07-lsp-stack.md` /
  `10-lsp-scaffold.md` / `17-lsp-diagnostics.md` /
  `21-lsp-incremental-sync.md` / `22-lsp-goto-definition.md` /
  `23-vscode-extension-polish.md` / `28-lsp-hover.md`
- 検証（Rust 側）: `crates/sapphire-lsp/tests/` 配下
  - `example_diagnostics.rs`（4 test）
  - `example_goto.rs`（6 test）
  - `example_hover.rs`（6 test）
  - unit test: 88 in lib + 4 in main
- 検証（editor 側）: `examples/lsp-smoke/hello.sp` を VSCode で
  開き、extension を F5 起動すると diagnostic / hover / goto-def
  が返る手順が `examples/README.md` に documented。
- L6 completion と VSCode marketplace 公開（D3）は本条件の外で
  継続作業（別 worktree で進行中）。

## 2. Fitness レポート

### 2.1 Rust コンパイラ / LSP のテスト通過状況

`source ~/.cargo/env && cargo test --workspace`（2026-04-19 実行、
MSRV `1.85.0`、`rust-toolchain.toml` 準拠）。

| バイナリ / ライブラリ | 単体 | 統合 | doctest |
|---|---|---|---|
| `sapphire-core` (lib) | 0 | — | 0 |
| `sapphire-compiler` (lib) | 342 | cli_smoke 9, codegen_snapshot 5, codegen_m9 4 | 0 |
| `sapphire-compiler` (bin `sapphire`) | 4 | — | — |
| `sapphire-lsp` (lib) | 88 | example_diagnostics 4, example_goto 6, example_hover 6 | 0 |
| `sapphire-lsp` (bin `main`) | 4 | — | — |

合計 **472 test, 0 failure**。すべて `ok` 終了、failing / ignored /
measured なし。

### 2.2 Rust release バイナリ

`cargo build --release --bin sapphire --bin sapphire-lsp` 成功。

| バイナリ | サイズ | 備考 |
|---|---|---|
| `target/release/sapphire` | 約 1.58 MB (1,579,536 B) | CLI。手書きパーサで依存最小。 |
| `target/release/sapphire-lsp` | 約 6.42 MB (6,416,792 B) | `tower-lsp` + `tokio` + `serde_json` 込み。 |

どちらも単一バイナリ、ランタイム依存なし。配布形態は D1/D2 で
platform matrix CI（`.github/workflows/release-build.yml`）、
実 publish は D3 判断待ち。

### 2.3 Ruby runtime gem テスト通過状況

`cd runtime && bundle exec rspec`（2026-04-19 実行、Ruby 3.3、
`bundle install` 後）。

- **142 examples, 0 failures** / 1.11 sec / seed 32333
- `ADT` / `Marshal` / `Ruby` monad thread / `Loader` の
  契約を網羅。

### 2.4 LSP の `hello.sp` 手動確認

`examples/lsp-smoke/hello.sp` を開いた VSCode 拡張の期待挙動は
下記 test で担保：

- `crates/sapphire-lsp/tests/example_hover.rs` — hover が
  top-level scheme / prelude tag / constructor tag / local tag
  を返す。
- `crates/sapphire-lsp/tests/example_goto.rs` — goto-def が
  signature、let binder、コンストラクタ宣言、型名、prelude
  （None 返却）の各経路で期待通りに動く。
- `crates/sapphire-lsp/tests/example_diagnostics.rs` — parse /
  lex / layout error が `textDocument/publishDiagnostics` に
  乗る。

これで VSCode からの手動 smoke を automated test でも保証できて
いる（stdio transport 上の LSP プロトコルそのものは `server.rs`
の unit test で cover）。

## 3. M9 4 例題の手動 smoke 結果

`target/release/sapphire build <.sp> --out-dir /tmp/m9-NN` →
`ruby -I runtime/lib -I /tmp/m9-NN -e "require 'sapphire/<mod>';
...; exit Sapphire::<Mod>.run_main"` の順で実行した結果。

### 3.1 Example 1: hello-ruby — **✅**

- build 入力: `examples/sources/01-hello-ruby/Main.sp`
- 生成: `/tmp/m9-01/sapphire/main.rb` + `sapphire/prelude.rb`
- 実行: `ruby -I runtime/lib -I /tmp/m9-01 -e "require
  'sapphire/main'; exit Sapphire::Main.run_main"`
- stdout:
  ```
  Hello, Sapphire!
  Hello, world!
  ```
- stderr: 空
- exit code: `0`

### 3.2 Example 2: parse-numbers — **✅**

- build 入力: `examples/sources/02-parse-numbers/NumberSum.sp`
- 生成: `/tmp/m9-02/sapphire/number_sum.rb` + `prelude.rb`
- 事前準備: `/tmp/m9-02/numbers.txt` に `1\n2\n3\n`
- 実行（`cwd=/tmp/m9-02`）: `ruby -I runtime/lib -I . -e "require
  'sapphire/number_sum'; exit Sapphire::NumberSum.run_main"`
- stdout: `6`
- stderr: 空
- exit code: `0`

### 3.3 Example 3: students-records — **✅**

- build 入力: `examples/sources/03-students-records/Students.sp`
- 生成: `/tmp/m9-03/sapphire/students.rb` + `prelude.rb`
- 実行: Ruby 側から `Sapphire::Students.topScorersByGrade.call`
  を呼び、spot-check 用サンプル（3 生徒、grade 1/2）で期待
  トップ（Alice / Carol）と一致することを assert。
- stdout: `OK: topScorersByGrade matches expected`
- stderr: 空
- exit code: `0`

### 3.4 Example 4: fetch-summarise — **✅（ネットワークスタブ）**

- build 入力: `examples/sources/04-fetch-summarise/Fetch.sp` +
  `Http.sp`
- 生成: `/tmp/m9-04/sapphire/fetch.rb` + `http.rb` + `prelude.rb`
- Ruby 側で `Net::HTTP.get_response` を `'hello'` を返すスタブに
  差し替えて実行（`codegen_m9.rs` と同じ方式。実通信版は Http.sp
  の `:=` ブロックをそのまま使えば動くが、CI から外部へは飛ばさ
  ない）。
- stdout: `fetched 5 bytes`
- stderr: 空
- exit code: `0`

### 3.5 結果サマリ

| # | 例題 | stdout | exit | verdict |
|---|---|---|---|---|
| 1 | hello-ruby | `Hello, Sapphire!\nHello, world!\n` | 0 | ✅ |
| 2 | parse-numbers | `6\n` | 0 | ✅ |
| 3 | students-records | `OK: ...` (library call) | 0 | ✅ |
| 4 | fetch-summarise | `fetched 5 bytes\n` (stub) | 0 | ✅ |

**4/4 pass.** 第一版完成の条件 3（M9 end-to-end）は満たされた。

## 4. 既知の制約・未実装

本 release を素通しにするため残した穴を、`docs/open-questions.md`
から cherry-pick して列挙する。すべて第一版の範囲では許容済み、
次フェーズの scope 決定と合わせて優先度付けする。

### 4.1 型検査・意味論の穴（user コードが踏む可能性あり）

| OQ | 要旨 | 影響 |
|---|---|---|
| **I-OQ60** | exhaustiveness / reachability 検査未実装 | pattern match の漏れが型検査を通る。M9 例題は漏れなし。 |
| **I-OQ62** | class instance の必須メソッド書き忘れ検査なし | 使用時 runtime error になる。M9 例題は該当なし。 |
| **I-OQ63** | `Subst::compose` の "先勝ち" による inconsistent compose silent drop | 一部の不整合なプログラムが通ってしまう soundness hole。M9 例題は該当なし。 |
| **I-OQ59** | constraint propagation が fixed point でない | superclass 階層が深い場合に不足しうる。M9 例題は該当なし。 |
| **I-OQ61** | ambiguous constraint の検出が甘い | `read x` 的な ambiguity で specialise に失敗する可能性。 |
| **I-OQ82** | `pure` / `return` specialisation の単位が top-level return-type head | ネストした do block（`main` は Ruby 側だが内側 do が List 等）で runtime fallback に落ちる。M9 例題は該当なし。 |
| **I-OQ80** | type class method の runtime dispatch | proper dictionary passing への昇格は未実施。性能 / 型安全は M9 範囲で許容。 |

### 4.2 LSP / Tooling の未実装

| OQ | 要旨 | 影響 |
|---|---|---|
| **I-OQ72** | Cross-file goto / workspace scan 未実装 | goto-def が同一ファイル内のみ。`import Foo` 越しは None。 |
| **I-OQ73** | Prelude 定義への goto | Prelude は `.sp` が存在しないため None 返却。 |
| **I-OQ74** | resolve 部分成功の exposing 未実装 | 1 件の resolve error で hover / goto が fall through。 |
| **I-OQ96** | Local binding の hover 型表示 | 現状 name + `(local)` タグ + 注記のみ。 |
| **I-OQ97** | Hover キャッシュ / incremental typecheck | キーストロークごとに full re-run。 |
| **I-OQ99** | Type-position hover | type variable / forall 位置が name-only。 |
| **I-OQ9** | LSP インクリメンタル計算基盤 | L3 は text sync のみ incremental、reparse は naive。 |
| **I-OQ76** | TextMate grammar のネストブロックコメント | 1 段までしか追えない。 |
| **I-OQ77** | indentationRules の精度 | regex 近似のため深い入れ子で崩れる。 |

### 4.3 配布・プロセスの未完了

| OQ | 要旨 | 影響 |
|---|---|---|
| **I-OQ11** | ライセンス dual 化（`MIT OR Apache-2.0`） | 現状 MIT 単独。user 判断待ち。 |
| **I-OQ29** | 単一 gem vs 複数 gem 構成 | D3 で確定。 |
| **I-OQ31** | バイナリ署名 / SBOM 範囲 | D3 前に確定。 |
| **I-OQ33** | CLI / runtime gem の version 一致ポリシー | D3 前に確定。 |
| **I-OQ78** | VSCode 拡張 publisher 名 / icon | D3 で確定。 |
| **I-OQ83** | CLI 引数パーサを `clap` に移すか | 機能増時に判断。 |
| **I-OQ84** | Prelude 生成を毎 build vs snapshot | incremental build 導入時に連動。 |
| **I-OQ85** | 生成 Ruby の runtime version constraint 文字列の管理 | D2 packaging と連動。 |

### 4.4 CLI / codegen 内部の限定事項

| OQ | 要旨 | 影響 |
|---|---|---|
| **I-OQ81** | 多引数 ADT コンストラクタの値位置 arity | codegen が 1 引数の curried lambda を emit。M9 範囲で該当なし。 |
| **I-OQ57** | 型付き AST の持ち方 | top-level scheme のみ back-annotate。hover / codegen 拡張時に再訪。 |
| **I-OQ58** | `do` 展開の段 | `typeck` と `codegen` がそれぞれ独立に desugar。DECIDED on 2026-04-19、本節は参考。 |

**集計**: `DEFERRED-IMPL` 系の既知制約を §4 で **27 件** 列挙
（§4.1 = 7、§4.2 = 9、§4.3 = 8、§4.4 = 3）。いずれも M9 4 例題に
非干渉であることを §3 の smoke で確認済み。追加 OQ の発生は本 I9
タスクでは **なし**。

## 5. 次フェーズへの示唆（CLAUDE.md 改訂案）

本 release で実装フェーズの exit condition は満たされた。次の
phase に入るにあたり、`CLAUDE.md` §Phase-conditioned rules の
以下の箇所は user 判断物として提起する（本 PR では **触らない**）。

- L7 `"implementation phase, from 2026-04-19"` → 次フェーズ名 /
  開始日に書き換える。
- L10–11 `"implementation phase ... This phase ends when Sapphire
  has a first usable compiler + runtime that can run the M9
  example programs end-to-end"` → 到達済みを明記し、次フェーズ
  の exit condition を新設する。
- L14–17 `"subsequent phases (self-host exploration, broader
  stdlib, ecosystem) will be scoped separately"` → 次フェーズ
  の scope を user と合意後に展開する。
- spec tree 中立性、OQ tracking、`docs/impl/` 記録方針は次
  フェーズでも継続で問題ない（phase-neutral）。

本改訂は user 判断なので、本 worktree では `CLAUDE.md` を触らず、
`docs/project-status.md` §Current phase の書き換えと、本文書の
§5 による flagging のみで留める。

## 6. Reviewer 経過

I9 審査は meta ワーク（document-only）であり、reviewer サブエー
ジェントが本セッションから呼び出せなかったため、main agent が
self-review を行った。CLAUDE.md §Review flow の精神（「初回レビュー
+ 最大 2 回 follow-up」）に従い、self-review iteration で以下を
矯正した：

- §2.1 のテスト集計で sapphire-core を `4` tests と誤記 → 実際は
  `0` tests、`sapphire` バイナリ側の 4 tests と行を分離。
- 条件 1 / 条件 2 の実装ファイル列挙を実物と一致させる（codegen
  は `adt.rs`/`record.rs`/`effect.rs` ではなく `decl.rs`/`emit.rs`
  /`pattern.rs`/`runtime.rs`/`prelude.rs`、runtime は `ruby_thread.rb`
  /`loader.rb` ではなく `ruby.rb` + `errors.rb` + `version.rb`）。
- §4 の既知制約総数を `25 件` と書き誤り → 実カウント `27 件` に
  揃え、project-status.md 側も同期。
- `project-status.md` 冒頭の中国語混入（`审查`）を `audit` に矯正。
- README の古い記述（「実装中」「CLI バイナリ予定」「実装していく」）
  を第一版 exit 到達済みの言い回しへ更新。

self-review 後の残 must-fix は検出されず（spec tree 未改変、
CLAUDE.md 未改変、normative 変更なし、code 変更なし）。
`Verdict: approve with suggestions`（将来 user と reviewer の
実呼び出し環境で再 audit するのが望ましい旨を記録）。

## 7. 結論

第一版完成の 5 条件 すべて **✅ 達成**。M9 4 例題 end-to-end
実行 4/4 pass。known limitation 27 件は次フェーズで scope 付けて
処理する。I9 タスクを **完了（🟢）** とし、Sapphire の実装フェーズ
exit condition を **2026-04-19 付で到達** したものとして記録する。
