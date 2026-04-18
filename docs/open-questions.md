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
| 02-OQ4 | レイアウト位置のタブ | DECIDED (要反映) | strict-by-default を維持。02 の本文から OQ を削る必要あり（13 §Interaction with earlier drafts）。 |
| 02-OQ5 | 識別子文字集合 (ASCII vs Unicode) | DECIDED (要反映) | 最初の実装は ASCII 限定。Unicode 拡張は pure monotonic extension として後送り。 |
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
| 05-OQ6 | 冪乗 `^` | DECIDED (要反映) | 最初の実装に含めない（13 C）。 |

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
| 08-OQ1 | `Maybe(..)` vs `Maybe` エクスポート既定 | DECIDED (要反映) | 「型のみ」を維持（13 C）。 |
| 08-OQ2 | プライベート型漏洩の診断タイミング | DECIDED (要反映) | 定義時に拒絶（13 C）。 |
| 08-OQ3 | 選択的再エクスポート | DEFERRED-IMPL | 実装中にユースケースで判断。 |
| 08-OQ4 | モジュール相互再帰の脱出 | DEFERRED-LATER | Haskell の `.hs-boot` 相当。例題が要求するまで延期。 |
| 08-OQ5 | `module Main` 糖衣 | DECIDED (要反映) | 単一ファイルスクリプトは省略可、ライブラリは必須（13 C）。 |
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
| 10-OQ1 | `nil` ↔ `Nothing` 近道 | DECIDED (要反映) | 採らない（13 C）。 |
| 10-OQ2 | 演算子メソッド mangle 方式 | DEFERRED-IMPL | 実装詳細。 |
| 10-OQ3 | シンボルキー vs 文字列キー hash | DECIDED (要反映) | シンボルキー（13 C）。 |
| 10-OQ4 | 例外 backtrace の構造 | DEFERRED-LATER | `List String` で十分。 |
| 10-OQ5 | `ruby_import` 外部ファイル | DEFERRED-LATER | 需要なし。 |
| 10-OQ6 | Ruby 3.x を超えるバージョン対応 | WATCHING | 3.3 pin 維持。4.x 到来時に再訪。 |
| 10-OQ7 | 高 arity ADT の ergonomic | DEFERRED-LATER | Struct/OpenStruct ラッパ等。 |

### 11 Ruby evaluation monad

| ID | 要旨 | Status | 決定 / メモ |
|---|---|---|---|
| 11-OQ1 | 並列合成 | DEFERRED-LATER | 並行性設計。 |
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
| T-06-1 | 章 6 (Ruby monad) | `Monad` の比喩が `Maybe`/`Result` と `Ruby` で別物になり摩擦 | DECIDED | 同上（C）。11 の意味論は触らず、tutorial での `do` 脱糖説明を強化する方向で T2 対応。 |
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

2026-04-18 の対話で 7 件の OPEN を処理し、§1〜§3 に反映済。現在
残っている **OPEN** は無し。

残る「要反映」系タスク：

- 13 で決定済みだがまだ本文反映が完了していない C 項目：
  **02-OQ4 / 02-OQ5 / 05-OQ6 / 08-OQ1 / 08-OQ2 / 08-OQ5 /
  10-OQ1 / 10-OQ3**。いずれも小さな chore なので、別途まとめて
  反映する予定（blocker ではない）。

新しく OPEN が発生したらここで列挙する運用。
