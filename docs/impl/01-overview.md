# 01. impl ツリーの役割

本ツリー `docs/impl/` は、Sapphire **コンパイラ本体を何の言語で書
くか** を決定するための deliberation 資料を置く。spec-first フェ
ーズの完了（M10 / 文書 13）後に立ち上げる、次フェーズの最初のト
ラック。

## このツリーでやること

- ホスト言語の候補を列挙する（`03-candidates.md`）。
- 評価基準を定める（`02-criteria.md`）。
- 候補を基準に沿って比較する（`04-matrix.md`）。
- 議論の結論（どの言語を採るか）を記録する（決定後、`05-decision.md`
  を追加予定）。
- 決定後はプロトタイプ結果や設計メモもここに追加する。

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
| `docs/open-questions.md` | 全体の OQ 追跡。本ツリーで新しい OQ が生じたら `X-NN` 形式で登録 |
| `docs/roadmap.md` | フェーズ全体の進捗。本ツリー着手を「次フェーズ」として記録 |

## 現在の phase-conditioned rule

`CLAUDE.md` の「spec-first phase」節の規則は、**ホスト言語が決定
されるまで** 引き続き有効：

- `docs/spec/` ではホスト言語中立性を維持（「Rust ならこう書く」
  等の記述を spec に入れない）。
- コンパイラ scaffolding はホスト言語選定後まで行わない。

本ツリー `docs/impl/` は上記規則の **適用除外**：トレードオフ比較
の文脈で具体の言語を名指すのが目的だから。

## 決定プロセス

1. `02-criteria.md` に評価基準（現状 10 項目、必要に応じて追加）
   を並べる。
2. `03-candidates.md` に候補言語のプロファイル（強み・弱み・
   事例）を書き下ろす。
3. `04-matrix.md` で側面ごとにスコアを付けて俯瞰する。
4. user と議論して候補を絞り、必要であればプロトタイプ（小さな
   レキサ／パーサ試作）を走らせて手触りを確認する。
5. 決定が固まったら `05-decision.md` を追加し、以下を記録する：
   - 選定した言語と選定理由
   - 選ばなかった候補と除外理由
   - 決定日
   - 後続作業のスタート地点（`src/` 構造、ビルドシステム、初回の
     マイルストーン）

本文書は `05-decision.md` 着地までは living document として運用
する。候補や基準の追加・削除は随時行う。

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
