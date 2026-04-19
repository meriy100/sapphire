# 01. impl ツリーの役割

本ツリー `docs/impl/` は、Sapphire **コンパイラ本体を何の言語で書
くか** を決定するための deliberation 資料を置く。spec-first フェ
ーズの完了（M10 / 文書 13）後に立ち上げる、次フェーズの最初のト
ラック。

## このツリーでやること

**フェーズ 1（2026-04-18 ～ 2026-04-19、完了済）**：ホスト言語の
選定。

- 候補言語の列挙（`03-candidates.md`）。
- 評価基準の定義（`02-criteria.md`）。
- 比較（`04-matrix.md`）。
- 決定記録（`05-decision.md` — Rust 選定）。

**フェーズ 2（2026-04-19 以降、継続中）**：実装中の設計メモ置き
場。

- Rust crate 選択・MSRV・エラー型・パーサ戦略等の個別判断の記録
  （`06-*.md`・`07-*.md`...）。
- 実装中に surface した OQ の `docs/open-questions.md` への登
  録と back-reference。
- scaffolding（`Cargo.toml`、ディレクトリ構造、CI）の設計メモ。
  コード本体は `docs/impl/` 外（repo ルートや `src/`）に置く
  が、設計判断の **記録** はここに集約する。

## このツリーでやらないこと

- **仕様の delta**：Sapphire 言語自体の規範は `docs/spec/` が正。
  本ツリーで言語機能を変えようとしない。
- **ビルドパイプラインの規定**：Sapphire コード → Ruby コードの
  コンパイル契約は `docs/build/` が担う。本ツリーは「コンパイラ
  の実装 **側**」の話なので、target (Ruby) の話ではない。
- **実装コードそのもの**：ホスト言語が決まる前は scaffolding し
  ない（`CLAUDE.md` §Phase-conditioned rules）。決定後、コード置
  き場は `docs/impl/` 外の適切なディレクトリ（例えばレポジトリ
  ルート直下）になる。

## 他ツリーとの関係

| ツリー | 関係 |
|---|---|
| `docs/spec/` | 規範。コンパイラが実装する対象 |
| `docs/build/` | target（Ruby）側の契約。コンパイラの出力を規定する |
| `docs/tutorial/` | ユーザ向け学習材料。実装側とは独立 |
| `docs/open-questions.md` | 全体の OQ 追跡。本ツリーで新しい OQ が生じたら `I-OQk` 形式で §1.5 に登録 |
| `docs/roadmap.md` | フェーズ全体の進捗。本ツリー着手を「次フェーズ」として記録 |

## 現在の phase-conditioned rule

2026-04-19 の `05-decision.md` 着地に伴い、`CLAUDE.md` は
「implementation phase（from 2026-04-19）」節に改訂された。現行
規則の要点：

- `docs/spec/` は引き続きホスト言語中立（Rust 固有の記述を入れ
  ない。将来の self-host や別ホスト移行を阻害しないため）。
- コンパイラ scaffolding は **解禁**：`Cargo.toml` や `src/` 構造、
  CI を用意してよい（本ツリー `docs/impl/` は、scaffolding の
  設計メモと実装中の判断記録の置き場を継続する）。
- 実装側の crate 選択・MSRV ピン・エラー型設計・パーサ戦略等は
  コード変更前に本ツリーへ追記し、根拠を残す。

## 決定プロセス（フェーズ 1 のログ、完了済）

ホスト言語決定までの流れ：

1. `02-criteria.md` に評価基準（10 項目）を定義。
2. `03-candidates.md` に 6 候補のプロファイル。
3. `04-matrix.md` で重み付きスコアリング。感度分析も付与。
4. 2026-04-19、user 判断で **Rust** に決定。プロトタイプなしで
   感度分析の安定性を根拠に早期決定（`05-decision.md`）。

**以後の新規判断** は `06-*.md` 以降の連番文書に残す。ID 命名に
特段の規則はないが、テーマが分かる短い英語スラッグを推奨
（例：`06-cargo-layout.md`、`07-parser-strategy.md`）。

## 作業リズム

本ツリーは design note の集合であり、normative spec ではない。
ただし `CLAUDE.md` §Review flow は新規ファイル作成時に reviewer
呼び出しを要求しているため、各文書の初稿時点で通常通りレビュー
を通す。決定文書 `05-decision.md` は規範的帰結を持つので、ここ
でも reviewer による確認を行う。

軽微な更新（候補プロファイルへの補足追記、スコアの微調整等）は
`CLAUDE.md` の「Typo fixes, heading renames, and formatting-only
tweaks do not count and do not require review」に該当すれば
review を省いてよい。
