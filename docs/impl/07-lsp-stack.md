# 07. LSP クレート選定（L0）

Sapphire の Language Server を Rust で実装するにあたり、**どの
LSP crate スタックに乗るか** を決める。本文書は Track L の L0
タスクであり（`06-implementation-roadmap.md`）、コードは書かず、
設計判断のみを確定する。

## スコープと前提

- **初回エディタターゲットは VSCode のみ**（user 指示）。Neovim /
  JetBrains / Emacs 等の対応は本フェーズ外。
- **ホスト言語は Rust** で確定済（`05-decision.md`）。コンパイラ
  本体と LSP は同じ Rust 処理系を共有し、レキサ・パーサ・名前解
  決・型検査のコードを両者で共用する。
- LSP 経路でサポートする機能は Track L の L2〜L6（diagnostics /
  sync / hover / goto-def / completion）。これらを素直に書ける
  スタックを選ぶ。
- JSON-RPC 転送は **stdin/stdout** を前提（VSCode の既定経路で
  あり、LSP 仕様の事実上の標準）。TCP / socket / pipe 等は本
  フェーズで対応しない。

## 候補クレート

### 候補 A: `tower-lsp`

- **概要**: `tower`（Rust の service 抽象）と `tokio` を土台に
  した async 型の LSP フレームワーク。`LanguageServer` trait を
  `#[tower_lsp::async_trait]` で実装するのが典型。
- **実績**: Rust で書かれた OSS LSP の中で最も採用例が多い部類
  で、個人プロジェクトから中規模サーバまで広く使われている。
  crates.io の累計 DL 数も多い。なお本家がメンテ不在気味な時期
  が続いた結果、API 互換の fork（`tower-lsp-server` 等）が
  複数登場している。
- **エルゴノミクス**: trait 実装ベース。initialize / hover /
  definition などを async メソッドとして書く。ボイラープレートは
  比較的少ない。
- **同期 / 非同期**: async（tokio ランタイム必須）。
- **ドキュメント**: README と docs.rs の example が基本。
  rust-analyzer ほどの内部ドキュメンテーションはないが、採用例が
  豊富なのでコピー元に困らない。
- **懸念**: 近年メンテが停滞気味との声があり、fork（`tower-lsp-
  server`）が複数出ている。LSP 3.17 の追従や `lsp-types` 依存の
  更新ペースは注意対象。

### 候補 B: `lsp-server` + `lsp-types`

- **概要**: rust-analyzer チームが公開している低レベル
  JSON-RPC / LSP プリミティブ。`lsp-server` は転送層と
  request/response のディスパッチ、`lsp-types` は LSP メッセージ
  の型定義を提供する。両方とも rust-analyzer 本体が使っている。
- **実績**: rust-analyzer 自身（Rust エコシステムで最も大規模な
  LSP 実装）、および `rust-analyzer` を参考にした多数の個人実装。
  メンテナは rust-analyzer / rustc チーム。
- **エルゴノミクス**: 低レベル。イベントループ（`for msg in
  &connection.receiver`）を自分で書き、メソッド名で手動ディス
  パッチする。ボイラープレートは多いが、その分プロトコル挙動が
  透明。
- **同期 / 非同期**: 同期（crossbeam channel ベース）。非同期仕事
  を走らせたい場合は自前でスレッド管理する（rust-analyzer も
  この方針）。
- **ドキュメント**: rust-analyzer のソースそのものが事実上の
  リファレンス実装。"どう書くべきか" 迷ったときに参照できる。
- **懸念**: 低レベル過ぎるので、L1 サーバスケルトン構築で素朴
  に数百行書くことになる。小規模 LSP では冗長。

### 候補 C: `async-lsp`

- **概要**: より新しい async LSP フレームワーク。`tower`
  ミドルウェアスタックを `tokio` ベースで組み立てる設計で、
  `tower-lsp` より柔軟なレイヤリングを目指す。
- **実績**: 後発の async LSP crate として徐々に採用例が増えつつ
  あるが、サーバ側採用の大規模実例は `tower-lsp` や `lsp-server`
  に及ばない。参考にしたい既存 OSS 実装の数では明確に劣後する。
- **エルゴノミクス**: trait ベースのサーバ実装 + tower ミドル
  ウェア（ロギング、concurrency、catch-unwind 等）を layer で
  差し込む設計。`tower-lsp` より柔軟だが、tower の語彙を覚える
  必要がある。
- **同期 / 非同期**: async（tokio ランタイム）。
- **ドキュメント**: docs.rs + リポジトリの examples。実例は
  まだ限定的。
- **懸念**: エコシステム成熟度が `tower-lsp` / `lsp-server` に
  劣る。L0 時点で採用すると、トラブル時の検索ヒットが少ない。

### 候補 D（参考）: `lsp-rs` / その他

- `lsp-rs` や類似の小規模 crate は複数あるが、
  いずれも `lsp-server` / `lsp-types` より後発で、かつ採用実績
  が限定的。Sapphire のスコープでは **わざわざ選ぶ動機がない**
  ので候補から落とす。

## 評価軸

`02-criteria.md` のコンパイラ本体選定軸をそのまま使うのではなく、
LSP 実装の文脈に合わせて以下の軸を立てる。重みは参考値（H = 高、
M = 中、L = 低）。

| 軸 | 重み | 説明 |
|---|---|---|
| **L-C1** 成熟度 / 採用実績 | H | crates.io の DL、採用プロジェクト数、更新頻度。トラブル時に既存事例をあたれるか。 |
| **L-C2** エルゴノミクス / ボイラープレート | H | L1 サーバ skeleton の立ち上げから L6 completion 追加までの記述量と見通し。 |
| **L-C3** 同期 vs 非同期モデル | H | Sapphire のインクリメンタル解析（L3 で導入）との相性。cancellation やデバウンスのしやすさ。 |
| **L-C4** 共有解析クレートとの統合 | H | コンパイラ本体（`src/`）のレキサ・パーサ・型検査を再利用できるか。Cargo workspace での crate 分割との噛み合わせ。 |
| **L-C5** テスタビリティ | M | mock LSP client を通じた protocol 単位のテスト、ユニットテストの書きやすさ。 |
| **L-C6** ドキュメント / 例 | M | docs.rs、README、参考実装。学習曲線。 |
| **L-C7** LSP 3.17 追従速度 | M | `lsp-types` の更新が現行仕様にどこまで追いついているか。 |

## 評価

### L-C1 成熟度 / 採用実績

- `tower-lsp`: 成熟、採用例多い、ただしメンテ停滞の兆候あり。
- `lsp-server` + `lsp-types`: rust-analyzer 本体採用で最堅。
- `async-lsp`: 新興、採用例少ない。
- 優位: `lsp-server`（rust-analyzer 実績）> `tower-lsp`（採用数）
  > `async-lsp`。

### L-C2 エルゴノミクス / ボイラープレート

- `tower-lsp`: trait 実装だけで初期化〜ハンドラが書ける。最も
  簡潔。
- `async-lsp`: trait + tower layer。柔軟だが記述は `tower-lsp`
  より多め。
- `lsp-server`: 手動ディスパッチ。サーバ skeleton で 200〜300
  行、ハンドラ追加のたびに match arm を増やす運用。
- 優位: `tower-lsp` > `async-lsp` > `lsp-server`。

### L-C3 同期 vs 非同期モデル（Sapphire のインクリメンタル解析との相性）

ここが本選定の **最大の論点**。Sapphire の LSP は L2〜L6 で
  パース/型検査を走らせるが、Track L のスコープでは：

- **L3 `didChange` でインクリメンタル再解析** — 変更のたびに
  最新ドキュメントを再パースする。デバウンス（短期間の連続変更を
  潰す）が必要。
- **L4/L5/L6 で解析結果のキャッシュを query** — パース済み AST /
  型環境を持ち回る必要がある。

rust-analyzer が同期 + Salsa（インクリメンタル計算ライブラリ）
の組み合わせを採っているのは、**キャンセル可能な計算** と
**データ所有モデル** を同期で書く方が素直なため。async にする
と、Salsa 風のキャンセルと tokio の `select!` の二層になる。

ただし Sapphire の L0〜L7 スコープでは：

- 初期実装で Salsa を入れる計画はない（L3 は naive 再解析から
  始めて良い）。
- デバウンスや cancellation は **protocol 層** で tokio タスク
  を spawn + abort する方が書きやすい。
- VSCode とのやり取りでタイムアウトや並行リクエストを素直に
  さばくのは async の得意分野。

よって **Sapphire のスケールでは async の方がむしろ書きやすい**
と判断する。rust-analyzer 級のインクリメンタル計算基盤を本フェ
ーズで入れるわけではない。

- 優位: `tower-lsp` ≈ `async-lsp`（async）> `lsp-server`（sync）
  — ただし将来 Salsa 相当を入れるなら逆転する。これは L0 では
  punt し、OQ として残す。

### L-C4 共有解析クレートとの統合

いずれの候補も「解析コードを別 crate に出して LSP 側から呼ぶ」
という構造は同じ。`Cargo.toml` の workspace member として
`sapphire-core`（共有解析）と `sapphire-lsp`（LSP 層）を並べれば、
どの crate でも同じように使える。

ただし **async 依存の伝播** には差がある：

- `tower-lsp` / `async-lsp` を選ぶと、LSP ハンドラは async だが
  **解析関数自体は同期で書ける**（同期関数を `spawn_blocking` や
  素直な呼び出しで包めば良い）。解析 crate が tokio 依存になら
  ない。
- `lsp-server` を選ぶと LSP 層も同期。解析 crate の形には影響し
  ない。

どちらも「解析 crate は sync のまま」で書ける。差は小さい。

- 優位: 引き分け。いずれも workspace で共有 crate を分ければ
  大差なし。

### L-C5 テスタビリティ

- `tower-lsp`: `LanguageServer` trait の実装を直接呼べるが、
  JSON-RPC 経由の end-to-end テストは自前で client stub を書く
  必要がある。
- `async-lsp`: tower layer を差し替えることで mock 挿入が
  しやすい。
- `lsp-server`: `Connection::memory()` で in-process channel に
  よる mock client を作れる。rust-analyzer がこの手法で
  protocol テストを書いている。
- 優位: `lsp-server`（明示的 mock API あり）> `async-lsp`
  （tower で差し替え）> `tower-lsp`（自前で書く）。

### L-C6 ドキュメント / 例

- `tower-lsp`: docs.rs + README + 多数の OSS 採用例。
- `lsp-server`: rust-analyzer 本体がリファレンス。
- `async-lsp`: docs.rs + 同梱 examples。採用例は少なめ。
- 優位: `tower-lsp`（コピペ元の数）≈ `lsp-server`（rust-
  analyzer）> `async-lsp`。

### L-C7 LSP 3.17 追従

- いずれも `lsp-types` を依存として使うことが多い。`lsp-types`
  の版を揃えていれば protocol 追従性に差は出にくい。
- `tower-lsp` は独自 wrap があるぶん追従が遅れるリスクあり
  （実際、3.17 の一部追加は fork 版で対応されている）。
- `lsp-server` は `lsp-types` をそのまま使う文化で追従は素直。
- `async-lsp` も `lsp-types` を直接使う。
- 優位: `lsp-server` ≈ `async-lsp` > `tower-lsp`。

## 推奨

**`tower-lsp` を第一候補とする**。L1 着手時に API 互換 fork の
`tower-lsp-server` に差し替える余地を残す。

### 理由

1. **エルゴノミクスが L1〜L6 で効く**。trait 実装だけで
   initialize / diagnostics / hover / goto-def / completion を
   順に足していける。Sapphire の Track L は 1 機能ずつ足す運用
   なので、ハンドラ追加の摩擦が小さい方が有利。
2. **async モデルが Sapphire の初期 L3〜L6 と噛み合う**。
   デバウンスやキャンセルを tokio で書く方が、L0 時点の知識
   基盤（tokio は Rust コミュニティで common）とも一致する。
3. **採用実績が広い**。L1〜L7 実装中に "こう書くもの" を探す
   場面で、既存 OSS LSP を多数参照できる価値は大きい
   （`rust-analyzer` が `lsp-server` 側の参考実装として抜きん
   でているのと同様、`tower-lsp` 側は小中規模 LSP の実装例が
   厚い）。
4. **解析 crate との結合は後段で自由**。`tower-lsp` を選んでも
   解析コードは sync のまま書けるので、将来 `lsp-server` に
   乗せ替える余地が残る（crate ごと差し替える粒度の話）。

### `async-lsp` を選ばない理由

現時点で採用実績が `tower-lsp` に遠く及ばず、L1〜L6 実装中に
詰まったときの検索可能性が低い。`tower-lsp` から `async-lsp`
への差は "tower layer の柔軟性" が主で、Sapphire 初回実装の
規模ではその柔軟性が活きる場面が少ない。

### `lsp-server` を選ばない理由

rust-analyzer 級の同期 + Salsa モデルを取るならこちらが正解だが、
**Sapphire L0〜L7 のスコープではオーバーエンジニアリング**。
L1 サーバ skeleton を書く時点で数倍のコード量になる。Sapphire
が将来 Salsa 相当を導入する段階に達したとき（現時点では計画に
ない）、改めて乗せ替える可能性を OQ として残す。

### メンテ停滞リスクへの対処

`tower-lsp` 本家のメンテペースが落ちている場合、API 互換の
`tower-lsp-server` fork に乗り換えられるようにしておく。
Sapphire の LSP 層は `tower-lsp` の trait API を直接使うだけ
なので、fork への移行は crate 名差し替え＋ minor adjustment で
済む想定。L1 着手時に "本家 vs fork のどちらを使うか" を
再確認する（I-OQ7）。

## 補助クレート

| 役割 | 採用 | 備考 |
|---|---|---|
| JSON-RPC 転送 | **stdin/stdout**（LSP 仕様の標準、VSCode のデフォルト経路） | `tower-lsp` が提供する `Server` API に stdin/stdout を流し込む形になる（詳細は L1 で確定）。 |
| LSP 型 | **`lsp-types`**（`tower-lsp` が `re-export` する版に追随） | 版の pin は L1 で実施。3.17 以上を想定。 |
| 非同期ランタイム | **`tokio`** | `tower-lsp` が要求。features は `rt-multi-thread` + `io-std` + `macros` 程度から。 |
| ロギング | **`tracing`** を推奨（I-OQ として flag） | コンパイラ本体も `tracing` に寄せれば I との共通化が効く。代替 `log` + `env_logger` は simple だが span が取れない。 |

## 共有 crate レイアウト（I2 との関係）

LSP 層はコンパイラ本体のレキサ・パーサ・名前解決・型検査を
**そのまま再利用** したい。したがって Cargo workspace で以下の
分割を提案する：

```
(workspace root)
├── Cargo.toml                 # [workspace] members = ["crates/*"]
├── crates/
│   ├── sapphire-core/         # 共有解析: lexer / parser / AST /
│   │                           # resolver / type-checker / diagnostics
│   ├── sapphire-compiler/     # CLI + codegen (Ruby 出力)
│   │                           # sapphire build/run/check エントリポイント
│   └── sapphire-lsp/          # LSP サーバ (tower-lsp 依存)
│                               # handlers / document store / sync
└── ...
```

- `sapphire-core` は **同期 API** で公開する。async 依存を引
  き込まない。LSP 側は必要に応じて `tokio::task::spawn_blocking`
  で包む。
- `sapphire-compiler` は CLI binary を持つ crate（あるいは
  workspace の `[[bin]]`）。
- `sapphire-lsp` は `tower-lsp` + `tokio` + `sapphire-core`
  依存。別 binary として `sapphire-lsp` を build する。
- VSCode extension（L7）は別ディレクトリ（例 `editors/
  vscode/`）の TypeScript プロジェクト。Rust workspace 外。

### I2 との整合

本レイアウト案は **Track I2（scaffolding）の決定と整合させる
必要がある**。I2 は本 L0 と並行稼働中であり、詳細解決は両者が
main にマージされる時点で行う：

- I2 が `src/` 単一 crate レイアウト（repo ルート直下の
  `Cargo.toml` + `src/`）を推す場合 — `sapphire-core` / `lsp`
  分割は L1 着手時点で crate 分割 PR を出せば良い。`05-
  decision.md` §リポジトリ構造 は `src/` 配置を述べているが、
  これは単一 crate を強制する表現ではなく、workspace でも
  `src/` 相当の位置に各 crate が並ぶ形で合致する。
- I2 が workspace レイアウトを採用する場合 — 本文書の提案が
  そのまま使える。crate 名の具体（`sapphire-core` 等）は I2
  決定に合わせる。

本 L0 の立場は「**workspace 前提で LSP を足すなら sapphire-
core / sapphire-lsp の 2 crate 構成に倒したい**」という要望
表明であり、I2 が単一 crate で行く決定を下した場合にブロッ
カーにはならない（L1 着手前に再議できる）。

## Open questions（L0 から punt）

以下は本フェーズで決めず、L1 以降に判断する。`docs/open-
questions.md` §1.5 に `I-OQk` として登録する（本 commit で
同時追加）。

- **I-OQ6 `lsp-types` のバージョン pin**: `tower-lsp` の依存が
  暗黙に引き込む版に追随するか、workspace で明示 pin するか。
  L1 着手時に決定。
- **I-OQ7 `tower-lsp` 本家 vs fork（`tower-lsp-server` 等）**:
  本家のメンテ状況を L1 時点で再確認し、必要なら fork に
  切り替える。
- **I-OQ8 ロギング基盤**: `tracing` で確定か、`log` + `env_
  logger` に倒すか。コンパイラ本体（I2）と揃える要件あり。
  現在の推奨は `tracing`。
- **I-OQ9 インクリメンタル計算基盤**: 現 L3 は naive 再解析で
  開始。将来 Salsa / incremental framework を入れる段階に達し
  たとき、**同期ベースの `lsp-server` への乗せ替え** を含め
  再評価する。L3 実装中に手触りで判断。
- **I-OQ10 複数エディタ対応時の transport 抽象**: 初回は
  stdin/stdout のみ。TCP / pipe 等は別エディタ対応時（本
  フェーズ外）に再検討。

## 参照

- `03-candidates.md` — ホスト言語候補（ホスト言語決定の前段）
- `04-matrix.md` — ホスト言語比較マトリクス
- `05-decision.md` — Rust 決定記録
- `06-implementation-roadmap.md` — Track L の L0〜L7 位置付け
- `../open-questions.md` — `I-OQk` での実装由来 OQ トラッキング
