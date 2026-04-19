# Sapphire 未決定仕様トラッカー

Sapphire のドキュメント群（`docs/spec/`・`docs/build/`・
`docs/tutorial/`）に散らばる未決定事項 (Open Question) を **ここ
で一元管理する**。本文書は living document であり、新しい OQ が
見つかったら追加し、決定が出たらステータスを更新する。

元の仕様文書の §Open questions セクション自体は背景文脈として残
すが、**本文書と矛盾があれば本文書を優先**する。

---

## ステータス語彙

| Status | 意味 |
|---|---|
| `DECIDED` | 答えを決定。仕様本文への反映は別 commit で（反映済みの場合はその旨）。 |
| `DEFERRED-IMPL` | 実装フェーズ（ホスト言語選定後・コンパイラ実装中）に解決。 |
| `DEFERRED-LATER` | 最初の実装を終えた後の言語マイルストーンで再訪。 |
| `WATCHING` | 外部状況の監視のみ。現時点で action なし。 |
| `OPEN` | 本当に未決。ユーザ判断あるいは追加議論が必要。 |

`DECIDED` のうち仕様本文への反映待ちは「要反映」と注記する。

## 追加するには

1. 元の仕様文書（`docs/spec/...` / `docs/build/...` など）の
   §Open questions に問いを書く。
2. 本文書の該当セクションに 1 行エントリを追加する。ID 命名規則：
   - 仕様文書（`docs/spec/`）由来：`NN-OQk`（例：`10-OQ8`）。
   - ビルド文書（`docs/build/`）由来：`B-NN-OQk`（例：`B-03-OQ5`）。
   - 実装文書（`docs/impl/`）由来：`I-OQk`（例：`I-OQ2`）。
     2026-04-19 の Rust 決定以降、実装時に浮上した OQ はこの
     接頭で §1.5 に登録する。
   - チュートリアル（`docs/tutorial/`）の §仕様への気付き 由来：
     `T-NN-k`（例：`T-05-1`）。OQ ではなく定性フィードバックな
     ので `-OQ` を付けない。
   - どの文書にも紐付かない横断的な問い：`X-NN`。
3. 初期ステータスは `OPEN` か、明確な送り先があれば `DEFERRED-IMPL`
   など。判断に迷う場合は `OPEN` で置く。
4. 決定が出たら Status を更新し、必要なら仕様本文と同期。同期が
   別 commit で走る場合は「要反映」を付ける。

削除はしない。決定後も履歴として残すことで「なぜこう決めたか」が
辿れる状態を保つ。

---

## 1. 仕様コア (docs/spec/01〜12)

13 §統合未解決の問い の監査表を本文書に移植し、処理方針を明確
化したもの。13 の `C/K/L/D/—` 分類を本文書の `DECIDED /
DEFERRED-IMPL / DEFERRED-LATER / — (済)` にマッピングしている。

### 01 Core expressions

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 01-OQ1 | `let` 自己参照 | DECIDED | 03 で暗黙再帰として決着済。 |
| 01-OQ2 | トップレベルシグネチャ必須/任意 | DECIDED | 08 境界規則（エクスポート必須）で決着済。 |
| 01-OQ3 | `if` プリミティブ vs 糖衣 | DECIDED | 09 で `case` の糖衣として決着済。 |
| 01-OQ4 | 数値タワー（`Float` / `Number`） | DEFERRED-LATER | 07 OQ6 と連動。`Int` 専用を維持。`Num` 導入は最初の実装後に再訪。 |
| 01-OQ5 | 組み込み演算子 | DECIDED | 05+07 で prelude 束縛＋`Eq`/`Ord` 経由として決着済。 |

### 02 Lexical syntax

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 02-OQ1 | `True`/`False` 字句 vs prelude 構造体 | DECIDED | 09 で prelude コンストラクタ。 |
| 02-OQ2 | 単項マイナス | DECIDED | 05 で `negate` 糖衣。 |
| 02-OQ3 | 演算子表 固定 vs ユーザ宣言 | DECIDED | 05 で Elm 風固定。緩和は 05-OQ3 送り。 |
| 02-OQ4 | レイアウト位置のタブ | DECIDED | strict（字句エラー）を維持。2026-04-19、02 §Layout 本文に反映し OQ を削除。 |
| 02-OQ5 | 識別子文字集合 (ASCII vs Unicode) | DECIDED | 最初の実装は ASCII 限定。Unicode 拡張は pure monotonic extension として後送り。2026-04-19、02 §Identifiers 本文に反映し OQ を削除。 |
| 02-OQ6 | `::` の用途 | DECIDED | 05/06 でリスト cons、パターン型注記は `(pat : type)` として決着。 |

### 03 Data types

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 03-OQ1 | 暗黙再帰 vs `let rec` | DEFERRED-IMPL | draft 暗黙再帰を維持。実装中に手触りで再評価可。 |
| 03-OQ2 | 局所 multi-binding `let` / 局所相互再帰 | DEFERRED-IMPL | 実装フェーズで必要性を判断。 |
| 03-OQ3 | 評価戦略 (strict vs lazy) | DEFERRED-IMPL | Ruby へのコンパイルは実質 strict。仕様は評価順中立。 |
| 03-OQ4 | `deriving` 構文 | DEFERRED-IMPL | 07-OQ8 と重複。最初のコンパイラには入れない。 |
| 03-OQ5 | コンストラクタ名 namespace / shadowing | DECIDED | 06 で決着済。 |
| 03-OQ6 | 局所 `data` 宣言 | DEFERRED-LATER | 需要が見えない。 |

### 04 Records

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 04-OQ1 | 行多相（拡張可能レコード） | DEFERRED-LATER | 大きな設計増分。最初の実装後に再訪。 |
| 04-OQ2 | レコード形コンストラクタ payload | DECIDED | **位置引数のみ**。2026-04-18 user 承認、04 および 10（`RubyError`）に反映済。 |
| 04-OQ3 | レコード punning (`{ x, y }`) | DECIDED | 認めない。2026-04-18 user 承認、04 に反映済。 |
| 04-OQ4 | 対称的更新（フィールド追加/削除） | DEFERRED-LATER | 04-OQ1 連動。 |
| 04-OQ5 | レコード間フィールド名衝突 | DEFERRED-IMPL | 実務上は 08 のモジュール修飾で解決。 |

### 05 Operators and numbers

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 05-OQ1 | `Float` / 数値多相 | DEFERRED-LATER | 07-OQ6 連動。 |
| 05-OQ2 | 多相等価 / 順序比較 | DECIDED | 07 で `Eq`/`Ord` 経由。 |
| 05-OQ3 | ユーザ宣言 fixity | DEFERRED-LATER | 必要性なし。最初の実装は Elm 風固定のまま。 |
| 05-OQ4 | 演算子セクション | DEFERRED-LATER | 利便性のみ。 |
| 05-OQ5 | pipe 演算子 (`\|>`/`<\|`) | DEFERRED-IMPL | 書き味が問題になれば早期追加あり得る。 |
| 05-OQ6 | 冪乗 `^` | DECIDED | 最初の実装に含めない（13 C）。2026-04-19、05 §Operator table 本文に反映し OQ を削除。 |

### 06 Pattern matching

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 06-OQ1 | ガード節 | DEFERRED-LATER | チュートリアル章 3 で「現状はガード無し」を確認済。必要なら後続で。 |
| 06-OQ2 | or パターン | DEFERRED-LATER | 同上。 |
| 06-OQ3 | リストリテラルパターン | DECIDED | 09 で決着済。 |
| 06-OQ4 | `Int`/`String` の網羅性（range） | DEFERRED-LATER | 計画しない。 |
| 06-OQ5 | `let` のパターン束縛 | DEFERRED-IMPL | 実装中に手触りで評価。 |
| 06-OQ6 | 名前付きフィールドコンストラクタパターン | DECIDED | 04-OQ2 の副次効果。2026-04-18 反映済。 |
| 06-OQ7 | 空 `case_alts` | DEFERRED-LATER | 稀なコーナーケース。 |

### 07 Type classes + HKT (MTC)

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 07-OQ1 | 多パラメータ型クラス (MPTC) | DEFERRED-LATER | 大設計空間。最初の実装は単一パラメータ。 |
| 07-OQ2 | 柔軟インスタンス頭 | DEFERRED-LATER | Haskell 98 形を維持。 |
| 07-OQ3 | 重複 / 孤児インスタンス緩和 | DEFERRED-LATER | 両方禁止を維持。 |
| 07-OQ4 | `do` の拒絶可能バインド | DEFERRED-LATER | `MonadFail` 導入は任意。 |
| 07-OQ5 | ソースレベル種注記 | DEFERRED-LATER | 直交する拡張。 |
| 07-OQ6 | `Num` クラス化 | DEFERRED-LATER | 01-OQ4 / 05-OQ1 連動。 |
| 07-OQ7 | 関連型 / 型族 | DEFERRED-LATER | 最初の実装スコープ外。 |
| 07-OQ8 | `deriving` | DEFERRED-LATER | 03-OQ4 と重複。 |
| 07-OQ9 | 高階種制約のインスタンス連鎖解決の例題 | DEFERRED-IMPL | ドキュメント課題。実装後にチュートリアル補遺で。 |

### 08 Modules

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 08-OQ1 | `Maybe(..)` vs `Maybe` エクスポート既定 | DECIDED | 「型のみ」を維持（13 C）。2026-04-19、08 §Visibility の既存記述で十分であることを確認、OQ を削除して本体 OQ リストを renumber。 |
| 08-OQ2 | プライベート型漏洩の診断タイミング | DECIDED | 定義時に拒絶（13 C）。2026-04-19、08 §Visibility に規範的な規則を追加し OQ を削除。 |
| 08-OQ3 | 選択的再エクスポート | DEFERRED-IMPL | 実装中にユースケースで判断。 |
| 08-OQ4 | モジュール相互再帰の脱出 | DEFERRED-LATER | Haskell の `.hs-boot` 相当。例題が要求するまで延期。 |
| 08-OQ5 | `module Main` 糖衣 | DECIDED | 単一ファイルスクリプトは省略可、ライブラリは必須（13 C）。2026-04-19、08 §One module per file に規則を強化し OQ を削除。 |
| 08-OQ6 | モジュールレベル fixity 宣言 | DEFERRED-LATER | 05-OQ3 連動。 |
| 08-OQ7 | メソッド単位クラスエクスポート | DEFERRED-IMPL | 人間工学拡張。 |

### 09 Prelude

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 09-OQ1 | タプル構文 | DEFERRED-LATER | 構造的レコードで代替可能。優先度低。 |
| 09-OQ2 | 型別名 `type T = τ` | DECIDED | **admit**（透明な別名）。2026-04-18 user 承認、09 §Type aliases と 02 予約語に反映済。 |
| 09-OQ3 | `String` を `[Char]` に分解 | DEFERRED-LATER | `String` は opaque。`Char` は無し（09-OQ6 連動）。 |
| 09-OQ4 | `Num` vs Int 専用 | DEFERRED-LATER | 01-OQ4 / 07-OQ6 連動。 |
| 09-OQ5 | `IO` / 具体 Ruby monad retype | DECIDED | 11 で決着済（`Ruby a` 型）。 |
| 09-OQ6 | `Char` プリミティブ | DEFERRED-LATER | 09-OQ3 連動。必要性なし。 |
| 09-OQ7 | 暗黙 prelude インポートの機構 | DEFERRED-IMPL | 実装詳細。 |
| 09-OQ8 | 中置合成演算子 | DEFERRED-LATER | `compose` 関数で当面足りる。`<<`/`>>` 衝突などを精査後。 |
| 09-OQ9 | `<$>` / `=<<` 相当の糖衣 | DEFERRED-LATER | 利便性のみ。 |

### 10 Ruby interop (data model)

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 10-OQ1 | `nil` ↔ `Nothing` 近道 | DECIDED | 採らない（13 C）。2026-04-19、10 §Data model ADT 節で規範的に記述し OQ を削除。 |
| 10-OQ2 | 演算子メソッド mangle 方式 | DEFERRED-IMPL | 実装詳細。 |
| 10-OQ3 | シンボルキー vs 文字列キー hash | DECIDED | シンボルキー（13 C）。2026-04-19、10 §Data model Records 節に合理性を追記し OQ を削除。 |
| 10-OQ4 | 例外 backtrace の構造 | DEFERRED-LATER | `List String` で十分。 |
| 10-OQ5 | `ruby_import` 外部ファイル | DEFERRED-LATER | 需要なし。 |
| 10-OQ6 | Ruby 3.x を超えるバージョン対応 | WATCHING | 3.3 pin 維持。4.x 到来時に再訪。pin を 4.0 に動かす判断が出たら `11-OQ1`（Ractor 方針メモ）も併せて再訪する。 |
| 10-OQ7 | 高 arity ADT の ergonomic | DEFERRED-LATER | Struct/OpenStruct ラッパ等。 |

### 11 Ruby evaluation monad

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 11-OQ1 | 並列合成 | DEFERRED-LATER | 並行性設計。**方針メモ（2026-04-19）**：明示的な `parallel : List (Ruby a) -> Ruby (List a)` 相当のコンビネータ導入時、実装候補として Ruby 4.0 の Ractor を採る。Sapphire ADT は frozen + 構造的等価なので `Ractor.make_shareable` と相性がよい。ただし **default 実行モデルは Ractor にしない**：10 の `:=` 束縛経由で埋め込まれる Ruby スニペットは Ractor 非 safe な gem を呼びうるため、明示 parallel を要求した箇所に限定する。Ractor を default 化する場合は **10-OQ6**（Ruby バージョン pin、3.3 → 4.0）を動かす必要があり、本 OQ と **10-OQ6 は連動**。仕様本決定は最初のコンパイラ完成後。 |
| 11-OQ2 | タイムアウト / キャンセル | DEFERRED-LATER | 同上。 |
| 11-OQ3 | ストリーミング | DEFERRED-LATER | 同上。 |
| 11-OQ4 | 例外クラス粒度 | DEFERRED-LATER | 拡張。 |
| 11-OQ5 | Ruby 側共有状態の脱出口 | DEFERRED-LATER | ユーザフィードバック待ち。 |
| 11-OQ6 | prelude としての `join` | DECIDED | `join : Monad m => m (m a) -> m a` を 09 prelude に追加済（2026-04-18）。 |
| 11-OQ7 | 生成 Ruby クラスのスレッド意味論 | DEFERRED-IMPL | 実装詳細。 |

### 12 Example programs

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 12-OQ1 | 長時間 Ruby 例題 | DEFERRED-IMPL | 実装フェーズで追加。 |
| 12-OQ2 | 多相を要求する例 | DEFERRED-IMPL | 同上。 |
| 12-OQ3 | Ruby → Sapphire 呼び出し例 | DEFERRED-IMPL | 同上。 |
| 12-OQ4 | 例 3 の `type` 別名 | DECIDED | 09-OQ2 と連動して admit。2026-04-18 反映済。 |
| 12-OQ5 | 完全に純粋な例題 | DEFERRED-IMPL | 実装フェーズ。 |
| 12-OQ6 | `readInt` prelude 依存 | DECIDED | `readInt : String -> Maybe Int` を 09 に追加済（2026-04-18）。`readFloat` は 01-OQ4 着地後。 |

### 13 Spec freeze review

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 13-OQ1 | `docs/impl/` 導入タイミング | DECIDED | ホスト言語選定に着手する時点で初めて作成する（lazy 作成）。2026-04-18 user 承認、同日に `docs/impl/` を実作成（I1 トラック開始）。 |
| 13-OQ2 | どの draft を "final" 昇格させるか | DEFERRED-IMPL | 最初のコンパイラが通して受理できてから検討。 |
| 13-OQ3 | `docs/roadmap.md` の扱い | DECIDED | living document として次フェーズも維持。2026-04-18 user 承認。spec-first の節は「完了」マークを付けるが削除はしない。 |

---

## 1.5 実装（Rust ホスト）由来 (docs/impl/)

2026-04-19 のホスト言語決定（Rust）に伴い発生した OQ。実装着手
時に決める必要があるものを列挙。

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| I-OQ1 | Rust MSRV | DECIDED | **1.85.0** に pin（2026-04-19、I2 着手時に確定）。edition 2024 を使うための最小版。`rust-toolchain.toml` と各 `Cargo.toml` の `rust-version` で強制。詳細は `docs/impl/06-scaffolding.md` §MSRV。 |
| I-OQ2 | Parser 戦略 | DECIDED | **手書き再帰下降 + Pratt 演算子**。2026-04-19 I4 着手時に確定。`chumsky` / `nom` / `lalrpop` はいずれも採らない。レイアウト解決を独立パスに分離してから再帰下降で受ける構成が、spec 02 §Layout の off-side rule・spec 05 の固定演算子表・L2 診断との接続のいずれとも整合する。却下理由および採用構成の詳細は `docs/impl/13-parser.md`。 |
| I-OQ3 | Error 型設計 | DEFERRED-IMPL | `anyhow` ベースかカスタム ADT か。layer ごとに揃える。 |
| I-OQ4 | Ruby へのパッケージング | DEFERRED-IMPL | Rust バイナリを `sapphire` gem 配布する段取り。`sapphire-runtime` gem との配布関係を決める。 |
| I-OQ5 | CI プラットフォーム | DEFERRED-IMPL | GitHub Actions 既定、cross-compilation 等の詳細は実装時。I2 時点では `ubuntu-latest` 単独で `check / fmt / clippy / test` を回す最小構成（`.github/workflows/ci.yml`）。macOS / Windows matrix は Track D（クロスコンパイル）で拡張。 |
| I-OQ6 | `lsp-types` のバージョン pin | DECIDED | `tower-lsp` が引き込む版（現行 0.94.x）に追随し、workspace 側で明示 pin しない。2026-04-19 L1 着手時に決定。`07-lsp-stack.md` / `10-lsp-scaffold.md` 参照。 |
| I-OQ7 | `tower-lsp` 本家 vs fork | DECIDED | 本家 0.20.x を採用。L1 スコープ（initialize / shutdown / textDocument sync）で不足はない。fork への切替は、L2 以降で本家未対応の capability が必要になった時点で再評価。2026-04-19 L1 着手時に決定。`07-lsp-stack.md` / `10-lsp-scaffold.md` 参照。 |
| I-OQ8 | ロギング基盤 | DEFERRED-IMPL | `tracing` 推奨（コンパイラ本体 I2 と揃える）。代替は `log` + `env_logger`。I2 で確定。`07-lsp-stack.md` 参照。 |
| I-OQ9 | LSP のインクリメンタル計算基盤 | DEFERRED-IMPL | L3（`docs/impl/21-lsp-incremental-sync.md`）で **text sync のみ incremental** 化し、reparse は naive 全再走のまま。Salsa 等の導入・AST 再利用・`lsp-server` への乗せ替え判断は引き続き先送り。L4/L5 で AST キャッシュを足す段か、M9 例題で性能問題が観測された段に再訪。`07-lsp-stack.md` / `21-lsp-incremental-sync.md` 参照。 |
| I-OQ10 | LSP の transport 抽象 | DEFERRED-LATER | 初回は stdin/stdout のみ。TCP / pipe は VSCode 以外のエディタ対応時（本フェーズ外）に再検討。`07-lsp-stack.md` 参照。 |
| I-OQ11 | ライセンス dual 化 | OPEN | MIT 単独を維持するか、Rust 生態系慣例の `MIT OR Apache-2.0` dual に切り替えるか。I2 では既存 MIT を維持。user 判断待ち。詳細は `docs/impl/06-scaffolding.md` §ライセンス。 |
| I-OQ12 | `sapphire-runtime` 側 Ruby formatter / linter 採否 | DEFERRED-IMPL | R1 では rubocop / standard-ruby を導入せず scaffold を最小化。`docs/impl/08-runtime-layout.md` §Rubocop / formatter。R2（ADT 実装）着地時に再評価。 |
| I-OQ13 | `runtime/` を Cargo workspace の member にすべきか | DEFERRED-IMPL | 現状 Rust workspace 外。`runtime/` は独立 Ruby gem として閉じ、I2 の Cargo workspace には含めない。D1（配布設計）で再訪。 |
| I-OQ14 | R3 shape-driven marshalling での user record vs tagged ADT の曖昧性 | DEFERRED-IMPL | `{:tag, :values}` 2 キーの Hash は、user record（`{ tag: String, values: List Int }` 等）としても ADT としても有効な shape。R3 は **tagged-first** で倒した（ADT として解釈）。spec 10 §ADTs 末尾は「expected type で routing する」と規定するので、最終解消は型引数版 `to_ruby(value, type)` / `to_sapphire(value, type)` を R4 以降で導入した時点。`B-03-OQ2` の型エンコード決定と連動。 |
| I-OQ15 | 高 arity ADT の ergonomic と Ruby キーワード引数（10-OQ7 連動） | DEFERRED-LATER | `ADT.define(mod, :Config, arity: 7)` のような高 arity コンストラクタは位置引数 7 つを並べる形になり、生成 Ruby コードを読む側にはつらい。10-OQ7 が `Struct` / `OpenStruct` wrap を提案しているが、Ruby 側 keyword arg（例: `mod.Config(host:, port:, ...)`）を許す拡張もあり得る。Sapphire 側は 04-OQ2 で位置引数のみと決まっているので、Ruby 側だけに生やす形の対称性に注意。最初の M9 通し後に再評価。 |
| I-OQ29 | 単一 gem vs 複数 gem 構成 | DEFERRED-IMPL (D2 で確定) | `docs/impl/12-packaging.md` §2。(A) 単一 `sapphire` + native gem、(B) メタ gem + `sapphire-compiler` + `sapphire-runtime`、(C) runtime gem のみ + CLI は別経路、の 3 案。draft は (A) を推奨、移行パスとして (C) を初回限定で採りうる。D2 の cross build 試行結果と user 判断で確定。 |
| I-OQ30 | rb-sys 方式 vs 素の Rust binary 同梱方式 | DEFERRED-IMPL (D2 で確定) | `docs/impl/12-packaging.md` §3。Sapphire CLI は Ruby から FFI しない独立バイナリのため、Ruby ABI に縛られる `rb-sys` / native extension 方式（方式 Y）は合わない。素朴な `exe/sapphire` 同梱 platform gem（方式 X）を draft 採用。native extension が必要になる将来（LSP を Ruby プロセスに embed する設計変更が入った等）があれば再訪。 |
| I-OQ31 | バイナリ署名 / SBOM の範囲 | DEFERRED-IMPL (D3 前に確定) | `docs/impl/12-packaging.md` §6。gem の `--sign`、OIDC trusted publishers、sigstore 署名、`cargo auditable`、`cargo sbom` のどこまでを v0 で含めるか。draft は `--sign` 採用せず、trusted publisher で push、SBOM は D2 で判断。D3 着地前に確定。 |
| I-OQ32 | Windows の first-class / best-effort 線引き | DEFERRED-IMPL (D2 で確定) | `docs/impl/12-packaging.md` §3 / §6。x86_64-pc-windows-msvc は first-class、aarch64 は best-effort を draft。CI matrix に Windows を加えるタイミング（Wave 2b vs 2c）も含めて D2 で確定。 |
| I-OQ33 | CLI と runtime gem の version 一致ポリシー | DEFERRED-IMPL (D3 前に確定) | `docs/impl/12-packaging.md` §5。draft は「major.minor 一致、patch はズレ可（`add_runtime_dependency "sapphire-runtime", "~> X.Y.0"`）」。起動時 version check の厳格さ（warning vs error）も合わせて D3 前に確定。 |
| I-OQ34 | 同一行 block-opener の reference column 計算 | DEFERRED-IMPL | `let a = 1\n    b = 2\n  in a` のように `let` と最初の binding `a` が同一行にある場合、レイアウト解決は `a` の column を知らないと `b` を同一 statement として受けられない。現行実装は `resolve_with_source` がソースを受けて `column_of(byte_offset)` を都度計算する形。これは O(file_size) 的な最悪コストを持つ（現実的には無視できる）が、将来 LSP などインクリメンタル再解析をする場合は事前テーブル化を検討。代替は「ソースに依存せず `usize::MAX` を記録し next dedent で閉じる」挙動。これは `let a = 1 in a` のような 1 行 let では正しく動くが、multi-line 継続形（`let a = 1\n    b = 2`）では `;` が挿入されない。現実装は source を受け取れる場合は column を使い、受け取らない `resolve` 経由では MAX を使うハイブリッド。実装コンパイラ完成後の LSP 組み込み時に再評価。 |
| I-OQ35 | `:=` Ruby 埋め込みの専用 TokenKind | DEFERRED-IMPL | 現行レキサは `:=` を `TokenKind::Op(":=")` として返し、パーサ側で文字列比較する。spec 02 §Reserved punctuation は `:=` を reserve しているので、専用の `TokenKind::ColonEquals` を追加した方が clean。追加は monotonic（既存コード破壊なし）なので I5 以降いつでも入れ替え可。今は lexer の変更を最小化した。 |
| I-OQ36 | infix-LHS method clause の射程 | DEFERRED-IMPL | `class Eq a where\n  x == y = not (x /= y)` のような `pat op pat = body` 形の default method clause を、パーサ段では左辺 apat が `Var` / `Wildcard` / 他のどれかに関わらず単純に「operator を見たら infix-LHS」と解釈する。spec 07 §Abstract syntax は「`(op)` の method に対する便宜記法」と位置付けているので、実質 `Var op Var = body` だけを通す方が忠実。I5（name resolution）で method 名を class membership に照らす際に追加検証する想定で、パーサは緩く通す。 |
| I-OQ37 | record update の 1 トークン先読み判定 | DEFERRED-IMPL | `{ e \| f = ... }` record update と `{ f = e, ... }` record literal の区別を、現行は `{` の後のトークン列を paren-depth を追いながらスキャンして top-level `\|` を `=` より先に見るか否かで決めている。spec 04 §Ambiguity も「パーサが複数トークン先読みする」と述べており運用上の問題はない。ただし 式内にさらに record update / literal / ラムダ（`\`）が入れ子になる場合に heuristic が混乱しうる。M9 例題の範囲では問題ないが、将来的には「`{` の直後のトークンが `lower_ident` かつ次が `=` なら literal、そうでなければ expr を parse して `\|` があれば update」と 2 段で決める方が堅い。 |
| I-OQ38 | 単項マイナスの expression-start positions | DEFERRED-IMPL | spec 05 §Unary minus は `-` が unary になる position を厳密列挙している（`(`、`{`、`=`、`->`、binary op、`if`/`then`/`else`/`in` の直後）。現行実装はシンプル化のため、Pratt `parse_unary_expr()` の入口で `Minus` を unary として受けるのみ。`f (-1)` は `f` の引数として `-1` が parse される（OK）、`a - -b` は left=a, op=-, right=parse_unary(-b)=Neg(b) で OK、`a-b` も `a` 後に binary `-` を探す通常経路で OK、となっており実害は観測されない。spec 忠実化すると「`- b` が unary になるべきかどうか」を position set に対して判定する必要があり、現実装はそれより緩めに受けている。M9 の例題・M1 の tutorial で困らない限り deferred。 |
| I-OQ39 | effect monad の `run` 再入と `prim_embed` block の closure キャプチャ相互作用 | DEFERRED-IMPL (I7c で再評価) | `docs/impl/14-ruby-monad-runtime.md` §effect monad 値の内部表現 / §I7c 生成コードのインタフェース想定、および `docs/impl/16-runtime-threaded-loading.md` §再入。R5 時点で再入自体は admitted（ネストした `run` はそれぞれ独立な evaluator Thread を起こす。I-OQ47 参照）。block が lambda / 関数値をキャプチャした際の referential transparency は spec 11 §`run` の「Ruby 側は非決定的でありうる」許容範囲で吸収されている。残る論点（I7c が出す closure の shape、snippet 間の ADT 共有）は codegen 着手時に再訪する。 |
| I-OQ40 | `Ruby.run` の返却形: タプル vs `Result` ADT | DEFERRED-IMPL (I7c で再評価) | `docs/impl/14-ruby-monad-runtime.md` §`run` の返却形 / `docs/impl/16-runtime-threaded-loading.md` §返却形は現状維持。R5 到達時点で再訪した結果、ランタイム側は `[:ok, a] / [:err, e]` の 2 要素タプルのまま（Ruby からのパターンマッチ利便性 + Rust 側 / 生成コード側での Result ADT 化が 1 行差し替えで済む）。spec 11 §`run` が規定する `Result RubyError a` への昇格は I7c が `ADT.make(:Ok, [v])` / `ADT.make(:Err, [e])` で行う想定。 |
| I-OQ41 | `prim_embed` block 内の `Interrupt` / ensure 責務 | DEFERRED-IMPL | `docs/impl/14-ruby-monad-runtime.md` §新規 OQ。B-03-OQ5 DECIDED により `Interrupt` は境界を propagate するが、ensure で後片付けが必要な Ruby スニペット（開いたファイル / ソケット）が出てきたときにどこまでランタイムが責務を持つか。R5 で evaluator Thread が挟まった後も挙動は不変（`Thread#value` が再 raise する）。11-OQ2（タイムアウト）と境界が重なる。M9 例題で必要性が見えたら spec 11 側で決着。 |
| I-OQ42 | 型別名を私有型漏洩検査の対象に含めるか | DEFERRED-IMPL | `docs/impl/15-resolver.md` §私有型漏洩。現行 I5 は transparent alias（spec 09 §Type aliases）を漏洩対象から除外している。M9 例題 03（`Students.sp`）が `type Student = { ... }` を export list に載せずに public signature に使うため、除外しないと弾いてしまう。spec 08 §Visibility は「type」とだけ書いておりaliasを明示していないので、将来 spec 08 本文に「aliasは透過なので対象外」の一文を足すか、ユーザーに選ばせる（strict mode）かを判断する。M9 全例題で resolver が通ったあと、T2 チュートリアル側の違和感と併せて再評価。 |
| I-OQ43 | 参照解決 side table の key としての `Span` 衝突 | DEFERRED-IMPL | `docs/impl/15-resolver.md` §resolved AST の扱い。現行は `HashMap<Span, Resolution>` で reference site を識別する。`Expr::BinOp` の演算子 span は `left.merge(right)` を合成 span として使っているため、同じ left / right をもつ別ノードが無い限り衝突しない（M9 の範囲では衝突観測なし）。将来 record update が深くネストするなどで span が重複する可能性が出れば、`RefId` 専用型を導入して AST ノード側に `Option<RefId>` を足す経路に切り替える。LSP の `Goto Definition` 組み込み時に再評価。 |
| I-OQ44 | `Prelude` を静的テーブルから `.sp` ソース化する時期 | DEFERRED-IMPL | `docs/impl/15-resolver.md` §prelude の暗黙 import テーブルの置き場所。I5 は `resolver/prelude.rs` に静的テーブルを持つが、いずれ `lib/Prelude.sp` を user module と同じパイプラインで compile させたい。先送りの理由は (a) 現状 compiler に codegen / runtime bind が無く `.sp` の Prelude を実行形まで持っていけない、(b) 静的テーブルのメンテコストが spec 09 の成長に追随する範囲では許容可能、の 2 点。I7 codegen 着地後に `lib/Prelude.sp` を書き下ろし、`builtin_prelude_exports` を削除する判断を下す。 |
| I-OQ45 | qualified 参照の target-export 厳密チェック | DEFERRED-IMPL | `docs/impl/15-resolver.md` §名前が見つからない / 曖昧な参照の扱い。現行 I5 の `resolve_name` は `M.x` を `qualified_aliases` が `M` を解決できるかどうかで判定し、`M` が実際に `x` を export しているかは省略している（import 段で `(x)` を書いていれば import 側のチェックで既に蹴られる）。M9 例題は bare `Mod.x` で書かれた import 外の参照が無いため問題にならないが、I6 以降で厳密化する。実装方針は「各 `ResolvedModule.env.exports` の snapshot を `Resolver` に保持し、qualified lookup 時に引く」。 |
| I-OQ47 | `Ruby.run` 再入の admit 方針 | DECIDED (2026-04-19) | `docs/impl/16-runtime-threaded-loading.md` §再入。`prim_embed` block 内で `Ruby.run` を再度呼ぶのは admitted。ネストした `run` はそれぞれ独立な evaluator `Thread` を起こし、`Thread#value` で join してから戻る。Thread 間で共有される state（global / constants / `$LOADED_FEATURES`）は `run` レベルでも共有されるが、block local / `Thread.current[:...]` は内外で完全独立。I-OQ39 の再入部分はこれで閉じる。 |
| I-OQ48 | `run` スコープ分離の現実的解釈 | DECIDED (2026-04-19) | `docs/impl/16-runtime-threaded-loading.md` §分離の境界。spec 11 §Execution model 項 1 の「fresh Ruby-side scope」は **locals と `Thread.current[:...]` の分離まで** と解釈する。global variables / top-level constants / `$LOADED_FEATURES` / monkey-patch は in-process 実装の制約上 `run` 間で共有される（`fork` は portability 上 / `Ractor` は constants 共有できず generated code が壊れる）。生成コード（I7c）はこれらの process-wide mutable state に依存しない契約を持つ前提。 |
| I-OQ49 | ランタイム version 不整合エラーの shape | DECIDED (2026-04-19) | `docs/impl/16-runtime-threaded-loading.md` §R6 loading 契約。`Sapphire::Runtime.require_version!(constraint)` は `Gem::Requirement` 文字列を受け、不整合なら `Sapphire::Runtime::Errors::RuntimeVersionMismatch` を、構文不正なら `Sapphire::Runtime::Errors::LoadError` を raise する（いずれも `Errors::Base < StandardError`）。CLI 側 version 照合（I-OQ33）は別レイヤ。 |
| I-OQ52 | パーサ error recovery 戦略 | DEFERRED-IMPL | `docs/impl/17-lsp-diagnostics.md` §エラー recovery を今回入れない判断。L2 では 1 ファイルにつき最大 1 件の diagnostic しか返らない（現行 parser は single-error 設計）。panic mode / error productions / FOLLOW set のどれを選ぶかは、resolver / type checker が繋がって "multi-error が自然に欲しい" 状況が来たときに再評価。L2 スコープでは recovery は入れない。 |
| I-OQ53 | UTF-16 変換の最適化 / crate 採用 | DEFERRED-IMPL | `docs/impl/17-lsp-diagnostics.md` §LSP UTF-16 と byte offset の変換。現状は自前の `LineMap` を毎リクエスト作り直す素朴実装。インクリメンタル化（I-OQ9）・ホットリロード・巨大ファイルで coordinate 変換が頻繁になったら、`line-index` / `ropey` ベースへの差し替えを検討。L2 時点では自前実装で十分。 |
| I-OQ54 | LSP diagnostic の `relatedInformation` 設計 | DEFERRED-IMPL | `docs/impl/17-lsp-diagnostics.md` §今後の拡張。lex / layout / parse の単発エラーには `relatedInformation` を付けていない。unification clash など型検査由来の双方向エラー（I6）で "こちら側と矛盾している" を指したくなるので、I6 結合時に `related_information` を埋める方針を決める。 |
| I-OQ55 | LSP `positionEncoding` negotiation | DEFERRED-LATER | `docs/impl/17-lsp-diagnostics.md` §今後の拡張。LSP 3.17 は UTF-8 / UTF-16 / UTF-32 のネゴを許す。Sapphire は spec 02 で UTF-8 を内部表現としているので、サーバが UTF-8 を宣言すればエディタ側対応次第で変換レイヤが消える。ただし VSCode のデフォルトは UTF-16 継続なので、対応クライアントが増えた時点で再評価。 |
| I-OQ56 | LSP ドキュメントストアの TTL / memory budget | DEFERRED-IMPL | `docs/impl/17-lsp-diagnostics.md` §今後の拡張。L2 時点の `DashMap<Url, Document>` は `did_close` で `remove` しているため通常ファイルサイズ以上には膨らまないが、将来 AST / resolver / type info を document ごとにキャッシュするようになったときメモリ管理方針（LRU vs 明示破棄）を決める。 |
| I-OQ67 | LineMap の部分更新 | DEFERRED-IMPL | `docs/impl/21-lsp-incremental-sync.md` §LineMap の差分更新を punt する理由。L3 現状は `apply_change` ごとに `LineMap::new` を呼び直す。巨大ファイル / 高頻度編集で hot path になるなら、`line_starts` を slice で継ぎ合わせる incremental 版に差し替える。`line-index` / `ropey` 採用（I-OQ53）と連動して決める。 |
| I-OQ68 | `rangeLength` の取り扱い | DEFERRED-IMPL | `docs/impl/21-lsp-incremental-sync.md` §range_length を無視する判断。LSP 3.17 で deprecated、実装ごとに UTF-16 / byte 計算が割れているため L3 では完全に無視。将来、client 側 UTF-16 算出壊れを log で可視化したい要望が出れば、`range` との不一致時に WARN を出す方向で再検討。 |
| I-OQ69 | client 再送 (resync) プロトコル | DEFERRED-IMPL | `docs/impl/21-lsp-incremental-sync.md` §エラー処理。`apply_change` が失敗して buffer が drift したとき、`workspace/diagnostic/refresh` / 明示的な `textDocument/didOpen` 再送で client に full 再送を促すフック。LSP 3.17 既存メカニズムで賄えるかの調査を含め、resync が必要になった段で決める。 |
| I-OQ72 | Cross-file goto / workspace scan | DEFERRED-IMPL | `docs/impl/22-lsp-goto-definition.md` §Cross-module / Prelude を扱わない理由。L5 は同一ファイルのみ goto 可能。`import Foo` 先の定義へ飛ぶには workspace ルートから `.sp` を発見・キャッシュする層が必要で、L6 以降で `Document` store を workspace-aware に拡張するときに設計する。 |
| I-OQ73 | Prelude 定義への goto | DEFERRED-IMPL | `docs/impl/22-lsp-goto-definition.md` §Cross-module / Prelude を扱わない理由。現状 Prelude は `resolver/prelude.rs` の静的テーブルで実体の `.sp` が無い。I-OQ44（Prelude の `.sp` 化）が済むと goto 可能になる。それまでは `None` 返却で諦める。 |
| I-OQ74 | resolve 部分成功の exposing | DEFERRED-IMPL | `docs/impl/22-lsp-goto-definition.md` §resolve 失敗時の諦め。現行 `resolve` は `Result<_, Vec<ResolveError>>` なので、1 件でも失敗すると reference side table を失う。`(ResolvedProgram, Vec<Error>)` 形に変えれば goto / hover が resolve エラー下でも動く。resolver 本体の API 改修を伴うので、L4 / L6 で手応えを見てから。 |
| I-OQ75 | Type-position goto の binder 定義 | DEFERRED-IMPL | `docs/impl/22-lsp-goto-definition.md` §Local binding の walk。`Type::Var` を `forall`-quantifier にジャンプさせるか、暗黙 quantifier 位置を指すかが多義的。I6（type inference）で forall / implicit quantifier の扱いが固まってから、L5 の `LocalFinder` を type scope まで拡張する方針で再評価。 |

## 2. ビルド戦略由来 (docs/build/)

これらは 13 執筆時点には存在しなかった（B1 トラックで後から追加）。
多くは実装フェーズに送るが、**spec へ跳ね返る可能性のあるもの** は
個別に記す。

### B-01 overview

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| B-01-OQ1 | ランタイム gem の Ruby バージョン pin 方針 | DEFERRED-IMPL | 10-OQ6 と連動。 |
| B-01-OQ2 | コンパイラ / ランタイムの互換性ポリシー | DEFERRED-IMPL | 実装時に semver 方針を決める。 |
| B-01-OQ3 | source map の Ruby backtrace への伝播 | DEFERRED-IMPL | 実装ごとの判断。 |
| B-01-OQ4 | `sapphire run` は in-process か Rake wrapper か | DEFERRED-IMPL | CLI 実装時に選択。 |
| B-01-OQ5 | コンパイラ self-hosting の将来 | DEFERRED-LATER | ホスト言語選定後の余地。 |

### B-02 source and output layout

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| B-02-OQ1 | 既定出力ディレクトリ (`gen/` vs `lib/`) | DEFERRED-IMPL | `gen/` を draft、実装時再評価。 |
| B-02-OQ2 | ソースツリー埋め込みの variant | DEFERRED-IMPL | プロジェクト構成の好み。 |
| B-02-OQ3 | multi-source-root プロジェクト | DEFERRED-IMPL | workspace 構成需要で判断。 |
| B-02-OQ4 | 出力ツリーを gem 化 | DEFERRED-IMPL | 配布形式の選択肢。 |
| B-02-OQ5 | macOS/Windows のケース非感度 | DEFERRED-IMPL | FS 依存問題。 |
| B-02-OQ6 | 生成ファイルヘッダのフォーマット | DEFERRED-IMPL | 実装裁量。 |

### B-03 sapphire-runtime

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| B-03-OQ1 | `Sapphire::Runtime` 名前空間予約 | DEFERRED-IMPL | 衝突検査の実装方針。 |
| B-03-OQ2 | `Marshal` の型引数エンコード | DEFERRED-IMPL | 実装詳細。 |
| B-03-OQ3 | `:=` スニペット本体: literal `proc` vs `eval` | DEFERRED-IMPL | パフォーマンス＆安全性の trade-off。 |
| B-03-OQ4 | `run` ごとのスレッド fresh vs pool | DEFERRED-IMPL | 11 §Execution model 範囲内で実装が選択。 |
| B-03-OQ5 | **`StandardError` のみ捕捉 vs `Exception` まで** | DECIDED | `StandardError` のみ捕捉で確定。10 §Exception model の文言を同時に narrow（システムレベル例外は境界を通り抜ける）。2026-04-18 user 承認、両文書に反映済。 |
| B-03-OQ6 | ランタイムバージョン検査の load 時 hook | DEFERRED-IMPL | 実装詳細。 |
| B-03-OQ7 | 非 Sapphire Ruby からの公開 API | DEFERRED-IMPL | ホスト統合の流儀による。 |

### B-04 invocation and config

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| B-04-OQ1 | CLI argv の entry への流し込み | DEFERRED-IMPL | 実装時。 |
| B-04-OQ2 | `sapphire check` daemon mode (エディタ統合) | DEFERRED-IMPL | 言語サーバの有無と連動。 |
| B-04-OQ3 | exit status の粒度 | DEFERRED-IMPL | 細分化は実装時に選択。 |
| B-04-OQ4 | YAML/JSON 以外の config 形式 | DEFERRED-LATER | 需要なし。 |
| B-04-OQ5 | `schema_version:` キー | DEFERRED-IMPL | config 進化に備え実装時に導入可。 |
| B-04-OQ6 | 同一 DAG level の並列コンパイル | DEFERRED-IMPL | パフォーマンス動機。 |
| B-04-OQ7 | エラー報告の bail-out ポリシー | DEFERRED-IMPL | 実装方針。 |
| B-04-OQ8 | watch モード | DEFERRED-IMPL | 開発体験向上。 |
| B-04-OQ9 | `sapphire run` の `bundle exec` 自動 wrap | DEFERRED-IMPL | Bundler 統合。 |

### B-05 testing and integration

B-05 の §Open questions に載っている項目（source の OQ1〜OQ8）に
1:1 で対応するエントリのみ。§Bundler integration 節にあるホスト
／コンパイラ Bundler の記述は「本文での提案」であり source §Open
questions に載っていないため、OQ として追跡しない。tracker 化が
必要なら source §Open questions に昇格させる。

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| B-05-OQ1 | RSpec-aware matcher | DEFERRED-IMPL | 拡張 gem 化の余地。 |
| B-05-OQ2 | Sapphire-aware Rake タスクライブラリ | DEFERRED-IMPL | 統合支援。 |
| B-05-OQ3 | Rails Railtie による auto-build | DEFERRED-IMPL | 統合支援。 |
| B-05-OQ4 | `sapphire init` テンプレート | DEFERRED-IMPL | 開発体験。 |
| B-05-OQ5 | `sapphire gem-build` パッケージングヘルパ | DEFERRED-IMPL | 配布支援。 |
| B-05-OQ6 | ランタイムのウォームアップ API | DEFERRED-IMPL | 実装詳細。 |
| B-05-OQ7 | 並行 `Ruby.run` の threadsafety | DEFERRED-IMPL | 11-OQ1 / 11-OQ5 連動。 |
| B-05-OQ8 | `:=` スニペットと Ruby グローバル状態 | DEFERRED-LATER | 11-OQ5 連動。 |

---

## 3. チュートリアルからの設計フィードバック (docs/tutorial/)

T1 トラックが writing 中に拾った定性的なフィードバック。個別の OQ
というより「現行仕様の重さ」「比喩の摩擦」系の信号。**Elm と
Haskell の中間** への揺り戻しを検討する際の一次資料。

| ID | 源 | 要旨 | Status | 決定 / メモ |
|---|---|---|---|---|
| T-03-1 | 章 3 (パターンマッチ) | 双方向型付け判定記法 `Γ ⊢ p ⇐ τ ⊣ Γ'` を入門文脈で見せるのが重い。実装者向け付録へ切り出す余地 | DEFERRED-LATER | 06 の規範的規則はそのまま、tutorial のみ抽象度を下げる方向。 |
| T-04-1 | 章 4 | タプル不在（09-OQ1）が説明コスト | DEFERRED-LATER | 09-OQ1 と同一。実装後に再評価。 |
| T-04-2 | 章 4 | `type` 別名不在が説明コスト | DECIDED (要反映) | 09-OQ2 の「admit」で解消見込み。 |
| T-05-1 | 章 5 (型クラス) | Functor → Applicative → Monad の五本柱が同時立ち上げで入門者に重い | DECIDED | 2026-04-18：(C) 仕様維持、tutorial 章 5 を具体→抽象の順序に書き直す方針。実作業は T2 トラックで別途。 |
| T-05-2 | 章 5 | HKT (`Functor f` の `f`) で読者が詰まる | DECIDED | 同上（C）。HKT の概念導入は発展篇に隔離する方向で tutorial 改訂する。 |
| T-05-3 | 章 5 (型クラス) | `Maybe a → Result e a` のブリッジ関数（`maybeToResult : e -> Maybe a -> Maybe` 相当）が prelude にないので `Result` の `do` 例で手書き変換（`case readInt s of Just n -> Ok n; Nothing -> Err ...`）が入る | WATCHING | 2026-04-19 T2a 章 5 書き直し中に発見。`maybeToResult` / `fromMaybe' : e -> Maybe a -> Result e a` のようなユーティリティを 09 prelude に追加する余地あり。M9 例題で頻出するようなら 09 に正式追加を検討。 |
| T-06-1 | 章 6 (Ruby monad) | `Monad` の比喩が `Maybe`/`Result` と `Ruby` で別物になり摩擦 | DECIDED | 同上（C）。11 の意味論は触らず、tutorial での `do` 脱糖説明を強化する方向で T2 対応。 |
| T-06-2 | 章 6 (Ruby monad) | `Just nil` / `Nothing` を区別するため `nil` を使わない規約が、Ruby 利用者の慣用と逆行しうる。Ruby スニペットを書くとき `Maybe a` の返し方を間違えやすい | WATCHING | 2026-04-19 T2b 章 6 書き直し中に発見。規範は 10 §ADTs のまま（10-OQ1 は DECIDED「近道採らない」）。tutorial 側の注意喚起で賄えるか、M9 例題で頻出する `Maybe a` 返しの書き方ガイドを 09 / 10 の非規範メモに足すかは、例題の手触りで判断。 |
| T-02..06 | 全般 | `仕様への気付き` 節を持つ章が 5 本 | WATCHING | チュートリアルが改訂されるたびに気付きをここへ集約する運用。 |

---

## 4. 今後のメンテナンス

- 仕様文書に新しい `Open question` を追加したら、対応エントリを
  本文書の該当セクションに追加する。
- 本文書の Status が `DECIDED (要反映)` のものは、仕様本文側の
  §Open questions を畳んで本文に決定を反映する別 commit を走らせる。
  反映の単位（1 件ずつ commit するか、まとめて改訂ノートにするか）
  は roadmap 側の判断とする。
- `DEFERRED-IMPL` の集合は `docs/impl/` 設立時に参照される
  「実装中に触る予定の OQ 一覧」になる。
- `DEFERRED-LATER` の集合は最初の実装完了後の「次に着手する言語
  マイルストーン候補」になる。
- `WATCHING` は定期的（例えば依存ライブラリや外部言語のメジャー
  更新時）に見直す。
- `OPEN` は放置しない。月 1 回程度の頻度で棚卸し、user 判断や追加
  議論へ回す。

## 5. 直近で user 判断が要るもの

2026-04-18 の対話で 7 件の OPEN を処理し、§1〜§3 に反映済。
2026-04-19 の I2 着手で **I-OQ11（ライセンス dual 化）** が新規
OPEN として追加。user 判断待ち。

2026-04-19（S1 タスク）で、13 由来の「要反映」C 項目を全件反映
済：**02-OQ4 / 02-OQ5 / 05-OQ6 / 08-OQ1 / 08-OQ2 / 08-OQ5 /
10-OQ1 / 10-OQ3** は本文から OQ を削除し、該当節の規範的記述に
畳み込んだ。Status は `DECIDED (要反映)` から `DECIDED` へ更新。

2026-04-19（D1 タスク、`docs/impl/12-packaging.md`）で、配布設計
由来の **I-OQ29〜I-OQ33** を §1.5 に追加。いずれも `DEFERRED-IMPL`
で、D2（CI cross build）および D3（初回 release）の中で user 判断
を仰ぐ位置付け。現時点では OPEN ではない。

2026-04-19（L3 タスク、`docs/impl/21-lsp-incremental-sync.md`）で、
LSP incremental sync 導入に伴い **I-OQ67〜I-OQ69** を §1.5 に
追加。いずれも `DEFERRED-IMPL`。真の incremental parsing は引き
続き I-OQ9 で punt。

2026-04-19（L5 タスク、`docs/impl/22-lsp-goto-definition.md`）で、
LSP goto-definition 導入に伴い **I-OQ72〜I-OQ75** を §1.5 に
追加。いずれも `DEFERRED-IMPL`。Cross-file goto（I-OQ72）は L6
以降、Prelude への goto（I-OQ73）は I-OQ44 連動、resolve 部分
成功（I-OQ74）は L4 / L6 で再評価、type position goto（I-OQ75）は
I6 完了後に扱う。

新しく OPEN が発生したらここで列挙する運用。
