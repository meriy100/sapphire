# 05. ホスト言語決定

## 決定

**Sapphire コンパイラ本体のホスト言語は Rust とする。**

決定日: **2026-04-19**
決定者: user (kouta@meriy100.com)
根拠資料: `03-candidates.md` / `04-matrix.md`（総合スコアで Rust
が首位、感度分析でも首位を維持）

## 選定理由

`04-matrix.md` の基準で Rust が首位（81 点）。特に効いた軸：

- **C1 ADT / パターンマッチ (5)**: Rust の `enum` と `match` が
  native。exhaustiveness 検査も compile 時に走る。Sapphire
  コンパイラが内部で扱う AST・型・パターン検査器の実装量と可読
  性に直結する。
- **C2 コンパイラ実装エコ (5)**: `chumsky`・`nom`・`pest`・
  `lalrpop` などパーサライブラリの選択肢が豊富。既存のコンパイラ
  実装例（gleam、roc、Rust 自身）から学べる。
- **C5 型システムの強さ (5)**: 所有権と型システムで AST 変換
  時のミスが compile 時に見つかる。バグ吸収力が高い。
- **C6 パフォーマンス (5)**: ネイティブコンパイル・高速起動。
  将来プロジェクト規模が大きくなったときの swap コストが小さい。
- **C7 配布 (5)**: 単一バイナリ・クロスコンパイル・`cargo
  install` で導入できる。

感度分析でも：

- 「Ruby 出身者重視」シナリオ（C4 を ×5 に引き上げ）でも Rust
  は首位を維持（87 点、次点 Ruby 84・TypeScript 80）。
- 「実装品質重視」シナリオでは圧倒的首位（106 点、次点 Haskell
  94・OCaml 94）。

どちらの価値観寄せでも Rust が落ちない、という感度の小ささが選定
の決め手となった。

## 却下した候補と理由

`03-candidates.md` の他候補の却下理由を短く再掲：

| 候補 | 却下理由 |
|---|---|
| **OCaml** | コンパイラ実装の古典解。Rust に対し明確な優位が乏しく、配布（C7）で劣る。コミュニティ勢い（C9）でも見劣り。 |
| **TypeScript** | コミュ最大という利点はあるが、Node.js 依存で Ruby コミュニティとの統合が二重管理になる。C3 Ruby 相互運用の失点が効く。 |
| **Haskell** | Sapphire と血縁が近く技術的魅力はあるが、user が Haskell 経験が浅いため maintainership のハードルが高い。C4 で大きく失点。 |
| **Ruby** | target (Ruby) 同居は魅力的だが、ADT / パターンマッチが native でない（C1=2）ため、コンパイラ実装の網羅漏れバグを言語レベルで防げない。user の maintain しやすさだけでは決め手に欠ける。 |
| **Crystal** | Ruby 風 + 静的型という珍しい立ち位置だが、C2（コンパイラ実装エコ）と C9（コミュ勢い）の薄さが響く。 |

## 実装フェーズに移る帰結

### リポジトリ構造

- 実装コードは **`src/`**（Cargo の慣習）に置く。
- `Cargo.toml` は repo ルートに置く。
- Rust edition: **2024**（2026-04-19 時点で最新 edition）。
- MSRV（minimum supported Rust version）は後続で決める（I-OQ1）。
  edition 2024 解禁の `1.85` 以上は確定、具体の数字は I2 着手時
  に固定。

### `CLAUDE.md` 規則の更新

本決定と同じ変更で `CLAUDE.md` §Phase-conditioned rules を改訂：

- spec-first フェーズの規則は役目を終える（spec 中立性は残し、
  scaffolding 禁止は解除）。
- 実装フェーズ用の規則セットに差し替える。

### ドキュメントツリーの扱い

- `docs/spec/` — ホスト言語中立性を保つ（Rust 固有の記述を入れな
  い）。
- `docs/build/` — Ruby target 側の契約、現状維持。
- `docs/impl/` — 本決定以後は「実装メモ」の置き場として継続使用
  （Rust の選択肢の議論から、具体の設計メモへ役割移行）。
- `docs/tutorial/` — エンドユーザ向け、現状維持。

### 次のマイルストーン候補

- **I2 スキャフォールディング** — `Cargo.toml`、基本のモジュール
  構造（`src/lexer/`・`src/parser/`・`src/ast/` 等）、`cargo
  check` が通る最小状態、CI の雛形。
- **I3 レキサ** — 02 の字句構文を実装。
- **I4 パーサ + AST** — 01〜09 の具象構文を実装。実装は段階的。
- **I5 型検査 / elaboration** — MTC (07) の単一パラメータ型
  クラスを含む。
- **I6 コード生成** — 10 / 11 のタグ付きハッシュ ADT・`Ruby`
  モナドを Ruby コードに落とす。

詳細 roadmap は `docs/roadmap.md` で段階的に更新する。

## 本決定に伴う新規 OQ

実装開始時にほぼ必ず決める必要がある項目。`docs/open-questions.md`
§1.5（実装由来、`I-OQk` 形式）で追跡する：

- **I-OQ1 Rust MSRV**: 実装中に使う Rust の最低バージョン。
  1.85（edition 2024 解禁）以上は確定。具体の数字は着手時に固定。
- **I-OQ2 Parser 戦略**: `chumsky` / `nom` / `lalrpop` / 手書き
  再帰下降 のどれで書くか。実装初期にパイロット実装して決める。
- **I-OQ3 Error 型設計**: `anyhow` ベースかカスタム ADT か。
  コンパイラ内の layer ごとに揃える必要あり。
- **I-OQ4 Ruby へのパッケージング**: Rust バイナリを `sapphire`
  gem として配布する段取り。`sapphire-runtime` gem（`docs/build/`
  参照）と別配布か同梱か。
- **I-OQ5 CI プラットフォーム**: GitHub Actions が既定だが、
  rustup / cargo-chef / cross-compilation の設定は実装時に詰める。

これらは `docs/open-questions.md` に `I-OQk` 接頭で追加する。

## 本決定の見直し条件

Rust が「間違いだった」と判断する材料として、以下のどれかが明確
に顕在化した場合に再評価する（本決定の単調性は保持 — 見直す場合
は `06-reconsideration.md` 等の新文書で記録）：

- Ruby コミュニティからの maintainership 応募が Rust 学習コスト
  を理由に集まらない。
- Rust の配布・クロスコンパイルが Ruby ユーザ（macOS arm64 /
  Linux x86_64 / Windows 等）の導入障壁になる。
- Rust 実装が完了した後、self-host 移行を議論するタイミングで
  現実的な移行計画が立たない。

いずれも **ユーザからの実フィードバックが発生してから** 再評価
する。仮定では再評価しない。
