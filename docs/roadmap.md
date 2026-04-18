# Sapphire 仕様策定ロードマップ

`docs/project-status.md` で述べた **spec-first フェーズ** の内側で、
どの順序でどんな文書を書いていくかを列挙する。各マイルストーンの成果
物は原則として `docs/spec/NN-*.md`（英、規範）と
`docs/spec/ja/NN-*.md`（日、翻訳）のペアで出る（`CLAUDE.md` の
writing conventions 参照）。

順序は目安であって絶対ではない。依存関係が許す限り並行して進めてよ
く、後続マイルストーンで遡って書き直すのも前提とする。このロードマッ
プ自体 living document として、マイルストーン完了時・方針変更時に更
新する。

本文書は `docs/spec/` 配下の仕様書ではなく `docs/` 直下の design
note であり、`CLAUDE.md` writing conventions の dual-language 対象外
である（日本語のみ）。

未決定仕様の一覧と処理方針は `docs/open-questions.md`（living
document）で集中管理する。本 roadmap は「今どこまで draft が揃
っているか」を示す index の側面を持つのに対し、open-questions は
各 draft に残っている OQ の個別処理を追う。

文書番号（`03-*` 以降）は完了順に振る。マイルストーンの並行進行中は
番号を予約せず、先に draft が出たものから次の番号を取る。既存の
`01-`、`02-` は発行済みなので遡及して振り直すことはしない。「想定文
書」の名前は目安。

## 方針転換メモ

2026-04-18: user 指示により、Sapphire の表現力目標を **Haskell 相
当** とし、汎用 **monad** を言語機能として導入する。Elm 準拠で機能
を削る方向の既定は採らない。この転換に伴い、以下を追加で約束する：

- **型クラス** と **higher-kinded types** を仕様に入れる（新マイル
  ストーン MTC、下記）。
- 既存 draft のうち型クラス前提で書ける箇所（05 の polymorphic
  equality、03 の kind `*`、05 の演算子多相など）は MTC 着地時に
  正面から解く。それまでは各 draft の OQ を残す。
- **M7 / M8**（Ruby インタロップ／評価モナド）は「唯一のモナド」で
  はなく、汎用 `Monad` の具体インスタンスとして再位置付けする。
- Elm 0.19 由来の「閉じたレコード」「演算子固定表」「pipe 無し」等
  は個別に再評価する。自動的に Haskell 寄りへ振るわけではないが、
  Elm 準拠が理由なら再評価対象。

## 現状

- **01 Core expression language** — draft 済み (`docs/spec/01-core-expressions.md`)
- **02 Lexical syntax** — draft 済み (`docs/spec/02-lexical-syntax.md`)
- **M1 データ型 (03 Data types)** — draft 済み (`docs/spec/03-data-types.md`)。
  01 未解決の問い 1（`let` 自己参照）は本文書で「暗黙再帰」として決着。
- **M2 レコード (04 Records)** — draft 済み (`docs/spec/04-records.md`)。
  クローズドな構造的レコードを draft の第一候補として採用。行多相は OQ。
- **M5 演算子と数値 (05 Operators and numbers)** — draft 済み
  (`docs/spec/05-operators-and-numbers.md`)。Elm 風固定演算子表、単項
  マイナスは `negate` の糖衣、`Int` のみの数値タワー、`::` はリスト
  cons として予約。01 OQ5、02 OQ2/3 は決着、01 OQ4 および 02 OQ6 は
  部分的に決着（残余は 05 OQ1/2、M3 に継承）。
- **M3 パターンマッチ (06 Pattern matching)** — draft 済み
  (`docs/spec/06-pattern-matching.md`)。`case ... of`、パターン文法、
  網羅性・冗長性。03 OQ5 決着、05 の pattern-level type annotation
  を `(pat : type)` で確定、01 OQ3 を部分的に対処（残余は M6 依存）。
- **MTC 型クラスと higher-kinded types (07 Type classes)** — draft 済み
  (`docs/spec/07-type-classes.md`)。種体系（`*` と `κ -> κ`）、単一
  パラメータ型クラス、Haskell 98 形インスタンス、重複なし・孤児なし、
  標準クラス `Eq`/`Ord`/`Show`/`Functor`/`Applicative`/`Monad`、
  `do` 記法。03 の「高階種は後送り」、05 OQ2（多相等価）を決着。
  02 に `<-` 予約と `class`/`instance`/`do` キーワード追加。
- **M4 モジュール (08 Modules)** — draft 済み
  (`docs/spec/08-modules.md`)。1 ファイル 1 モジュール、`module` ヘッ
  ダ、明示エクスポートリスト、`import`（修飾・非修飾・`hiding`・`as`）、
  修飾名解決、07 の孤児なし規則を grounding、再エクスポート。01 OQ2
  （トップレベルシグネチャ）を「エクスポート済は必須」で決着。02 に
  `as`/`hiding`/`qualified` キーワード追加。
- **M6 prelude (09 Prelude)** — draft 済み
  (`docs/spec/09-prelude.md`)。中核 ADT（`Bool`・`Ordering`・`Maybe`・
  `Result`・`List`）、リスト構文（`[]`・`[x, y, z]`・`::`）、標準クラ
  スの prelude インスタンス、基本 utility 関数。02 OQ1 と 01 OQ3 を
  最終決着（`Bool` は ADT、`if` は `case` の糖衣）。
- **M7 Ruby インタロップ (10 Ruby interop)** — draft 済み
  (`docs/spec/10-ruby-interop.md`)。`:=` による Ruby 埋め込み、三連
  引用符文字列リテラル、データモデル（Int/String/Bool/record/ADT/
  List/関数）、タグ付きハッシュ ADT 表現、`RubyError` 型、生成
  Ruby モジュール形（`Sapphire::M::N::P` クラス階層）。モナド意味
  論は M8 に委譲（`Ruby` を opaque 型として扱う）。
- **M8 Ruby 評価モナド (11 Ruby evaluation monad)** — draft 済み
  (`docs/spec/11-ruby-monad.md`)。型名を `Ruby` と確定（`Ruby`
  モジュールに同居）。`Functor`/`Applicative`/`Monad Ruby` インス
  タンス、プリミティブ `primReturn` / `primBind`、逐次実行モデル
  （単一 Ruby スレッド、`>>=` で直列化）、`run : Ruby a -> Result
  RubyError a` を唯一の pure 側出口として規定。09 の `print` stub
  を `Ruby {}` に retype。10 の `RubyM` 参照を `Ruby` へ統一。
- **M9 例題プログラム集 (12 Example programs)** — draft 済み
  (`docs/spec/12-example-programs.md`)。4 本の例題で 01〜11 を
  end-to-end に exercise：Hello Ruby、数値ファイル解析、生徒レコ
  ード処理（純粋）、HTTP 取得と分類（2 モジュール・Ruby 相互運用・
  Result エラー処理）。各例題に読解ガイド付き、仕様未決箇所も可
  視化。
- **M10 仕様凍結レビュー (13 Spec freeze review)** — draft 済み
  (`docs/spec/13-spec-freeze-review.md`)。01〜12 の状態要約、全
  OQ を C/K/L/D に分類した統合表、文書間整合チェック（予約語・
  演算子表・予約句読点・暗黙インポート）、`CLAUDE.md` phase-
  conditioned rules の次フェーズ向け改訂案、spec-first フェーズ
  の凍結判断。D 決定（04 OQ2、09 OQ2、12 OQ6）と C 修正はユーザ
  サインオフ後の follow-up commit で着地。

## マイルストーン

### M1. データ型（代数的データ型）

目的：sum type / product type の宣言と、コンストラクタを値として使う
規則を固定する。

主題：

- `data` 宣言の構文（`data Maybe a = Nothing | Just a` のような形）。
- コンストラクタの型スキーム生成と (Var) 経由でのインスタンス化。
- 再帰型、相互再帰型の扱い。
- 型コンストラクタの適用（`Maybe Int`、`List a` のような応用位置）。
- `let` 束縛の自己参照（01 open question 1）もここで決着させる。再
  帰的 `data` を導入する以上、再帰束縛の仕組みを同時に固めるのが自
  然な切り方。

依存：01, 02。
想定文書：`data-types.md`（番号は完了順に採番）。

### M2. レコード

目的：名前付きフィールドの product type を導入し、フィールドアクセス
と更新の構文と型規則を定める。

主題：

- nominal records にするか、構造的（row polymorphism）にするかの判断。
- フィールド選択 `r.f`（02 の `.` 予約との整合）。
- 更新構文（`{ r | f = v }` 等）。
- 行変数 / 部分フィールド参照の可否。

依存：01, 02。M1 とは独立だが、型側の決定が噛み合うので近接して書く
とよい。
想定文書：`records.md`（番号は完了順に採番）。

### M3. パターンマッチ

目的：`case ... of` 式とパターンの型規則、網羅性の扱いを決める。

主題：

- `case` の構文と layout（02 の block-opening keyword `of` と整合）。
- パターンの文法：変数・ワイルドカード `_`・コンストラクタパターン・
  リテラルパターン・as パターン（02 で予約した `@`）・レコード
  パターン。
- 網羅性・到達不能性の扱い（警告か エラーか、仕様レベルで定めるか）。
- ガード節の有無。

依存：M1（コンストラクタパターン）、M2（レコードパターン）。
`True`/`False` をリテラルパターンとして扱うかコンストラクタパターン
として扱うかは 02 open question 1 の帰結に従うため、その点に関して
は M6 との相互参照とする。M3 本体は「一般のパターン機構」を draft
として先に書き、`True`/`False` の具体決定は M6 に委譲してよい。

01 の `if` を `case` の糖衣として再規定するかもここで決める（01 open
question 3）。

想定文書：`pattern-matching.md`（番号は完了順に採番）。

### M4. モジュールとインポート

目的：ファイルを跨いだ名前解決、可視性、修飾名の仕様を固定する。

主題：

- `module` 宣言、`import`/`export` リスト。
- 修飾名 `Mod.name`（02 の `.` 予約）。
- 可視性の既定（すべて public か、明示的 export のみか）。
- 再エクスポート、`as` によるリネーム。
- `.sp` ファイル一つ = モジュール一つ か、モジュール階層の切り方。
- トップレベル型シグネチャを必須にするか任意にするか（01 open
  question 2）もここで確定する。モジュール境界での主要型・エラー
  メッセージの話とまとめて扱うのが筋。

依存：01, 02（修飾名 `Mod.name` は 02 で `.` を予約済み）。
想定文書：`modules.md`（番号は完了順に採番）。

### M5. 演算子と数値

目的：組み込み演算子の集合・優先度・結合性、および数値階層を固定する。

主題：

- 演算子表（Elm 風固定、Haskell 風ユーザー宣言のどちらか）。
- 単項マイナスの位置付け（02 open question 2）。
- `Int` のみか、`Float` 追加か、統一 `Number` か（01 open
  question 4）。
- 比較・論理・算術演算子の型と振る舞い。
- prelude に置くか、言語組み込みか。

依存：01, 02。01 open questions 4/5、02 open questions 2/3/6 を同時
に閉じる（数値タワー、組み込み演算子、単項マイナス、演算子表、`::`
の用途）。
想定文書：`operators-and-numbers.md`（番号は完了順に採番）。

### MTC. 型クラスと higher-kinded types

目的：汎用 `Monad` を書ける水準の表現力を言語に与える。2026-04-18
の方針転換（Haskell 相当・monad 導入）の主たる成果物。

主題：

- **kind システム**：少なくとも `*` と `* -> *`（および必要に応じ
  てより高階の種）。03 の「kind `*` のみ」は本マイルストーンで解
  除される。
- **型クラス構文**：`class C a where ...` 風の宣言と、`instance C
  T where ...` 風のインスタンス宣言。メソッドの型スキームは `C a
  => ...` のように制約付きスキームで表現される。
- **制約付き型スキーム**：02 で `=>` を予約済み。ここで実体化する。
- **解決機構**：インスタンス解決の規則、重複インスタンス・孤児イ
  ンスタンスの扱い、辞書渡し（または等価な実装戦略）を spec レベ
  ルでどこまで固めるか。
- **標準クラス**：`Eq`、`Ord`、`Show`、`Functor`、`Applicative`、
  `Monad`（少なくともこの 6 つ）。`Monad` の表層：`return`（また
  は `pure`）・`>>=`・do 記法。
- **do 記法**：`do { x <- e1 ; e2 }` を `e1 >>= \x -> e2` の糖衣
  として規定。02 の `<-` 予約をここで使う（現状未予約なら予約を
  追加）。
- **既存 draft への差分**：
  - 05 の `==` / 順序比較を `Eq` / `Ord` で再規定。
  - 05 の算術演算子を `Num` で多相化する案は二次的（数値タワー
    OQ と合わせて議論）。
  - 03 の constructor schemes は kind 系が入っても不変（`T` の
    arity は `T` の kind と等価）。
  - 04 の行多相 OQ は MTC 後に再訪して可否を決める。
- **`RubyEval` との関係**：M7/M8 は MTC の `Monad` 具象インスタン
  スに書き換えられる。独自プリミティブのまま残すか、`Monad` クラ
  スのインスタンスとして統合するかは MTC + M7/M8 で連携して決め
  る。

依存：01, 02, 03。04 とは独立（行多相 OQ の扱いで相互作用）。

想定文書：`type-classes.md`（番号は完了順に採番）。MTC はロードマ
ップ上は M5 と M6 の間に位置するが、これは依存関係上の位置であって
ファイル番号順を拘束しない：MTC の draft が出る時点での未使用の最
小番号を取る（たとえば 06 の次が draft されれば 07）。

### M6. Prelude の最小セット

目的：実用プログラムが最低限依存する標準束縛の外形を固める。

主題：

- `Bool` とそのコンストラクタ（02 open question 1 をここで確定。01
  の Design notes にある (LitBool) 派生化の選択もこの確定に連動す
  る）。パターン側との接続は M3 で消化される（M3 は本体を一般のパ
  ターン機構として先に書き、`True`/`False` の扱いは M6 の確定を参
  照する）。
- リスト型と `[]`（`::` の字句・構文上の用途は M5 で先に決まってい
  る前提）。
- 文字列関連の最小 API。
- `Maybe` / `Result` のような標準 ADT を prelude に入れるか。
- MTC 着地後は標準クラス（`Eq`・`Ord`・`Show`・`Functor`・
  `Applicative`・`Monad`）のインスタンス宣言を prelude に含める
  かどうか、含めるならどの型でどう定義するかを決める。

依存：M1, M5, MTC（標準クラスインスタンスを含める場合のみ MTC を
待つ。含めない draft を先に出す選択肢もある）。
想定文書：`prelude.md`（番号は完了順に採番）。

### M7. Ruby インタロップ — 埋め込みとデータモデル

目的：`.sp` プログラムが Ruby コードを呼び出す部分の「モナドでない
層」を決める。

主題：

- 埋め込み構文：Ruby スニペットを書く場所と形。文字列リテラル内埋め
  込みか、専用ブロック構文か。02 で `:=` が「Ruby interop bindings」
  用として予約されており、ここがその回収先となる。
- Sapphire 側と Ruby 側の値の対応（整数・文字列・リスト・レコード・
  関数）。
- Ruby 側例外の Sapphire 側でのモデル。
- 生成される Ruby モジュールの形と命名規則。

依存：01, 02, M4。
想定文書：`ruby-interop.md`（番号は完了順に採番）。

### M8. ★ Ruby 評価モナドの命名と意味論

目的：`docs/project-status.md` で **`RubyEval`** と呼ばれているモナ
ドについて、**正式名を確定** し、意味論を規範として書き下ろす。

Sapphire の signature feature であり、命名は言語の顔になる。ここで決
めた名前は prelude・教材・エラーメッセージに刻まれるので、作業名で
引きずらない。

命名に関する出発点（M8 の議論用、確定済みではない）：

- `RubyEval` — 現行の working name。明示的だが冗長。
- `Rb` — Haskell の `IO` に倣った短い名。型シグネチャが軽くなる。
- `Ruby` — 端的だが普通名詞として型名に使うのがくどい。
- `Eval` — 汎用すぎて Ruby に限定した型とわかりにくい。
- `Host` / `Embed` — Ruby 以外の host を抽象化する含みを持つが、
  Sapphire が Ruby に結びついているという性格を薄める。
- `Script` — Ruby コードを「スクリプト」視するニュアンス。
- その他、user が提案するもの。

意味論として固めるべき点：

- 別スレッドで Ruby を評価し、結果を pure 側に戻す仕組み。
- `return` / `pure` / `bind` に相当する演算。MTC が `Monad` を導入
  しているので、M8 の型は汎用 `Monad` クラスのインスタンスとして
  書ける。専用の名前付き演算（`runRuby` のような実行関数）は別途
  固める。
- エラー（Ruby 側例外、スレッド失敗、タイムアウト等）の shape。
- 複数の評価を合成したときのスケジューリング（逐次か並行か）。
- 純粋な値と評価結果の型で区別されること（Haskell の `IO a` と `a`
  の区別に相当）。

依存：M7、MTC。2026-04-18 の方針転換以降、M8 の Ruby 評価型は汎用
`Monad` の具体インスタンスとして位置付ける。命名はインスタンス型名
の話であり、「モナドとは何か」の規定は MTC が引き受ける。
想定文書：`ruby-monad.md`（番号は完了順に採番。ファイル名の
`ruby-monad` 部分は M8 の成果に従って最終名へ差し替える）。

### M9. 目的プログラム集

目的：言語全体の「触感」を示すエンドツーエンド例を揃える。

主題：

- 01 の motivating examples（core expression 層の anchor）はその役
  目で据え置き、本マイルストーンではそれとは別に、30〜80 行程度の短
  いプログラム 3〜5 本を追加する。01 の例の改訂が必要になったら別途
  01 を更新する。
- Ruby インタロップを含む例を少なくとも 1 本。
- 型シグネチャ・モジュール分割・prelude 利用を含む例を 1 本。

依存：M1–M8 の大部分が draft になっていること。
想定文書：`example-programs.md`（番号は完了順に採番）。

### M10. 仕様整合確認と凍結判断

目的：01〜11 までを横断し、open question の棚卸しと相互整合の最終確
認を行い、実装フェーズに進んでよいかを user と判断する。

主題：

- 各文書に残っている open question の一覧化と、未決 / 決定済みの仕
  分け。下流マイルストーンに明示的に割り振られていない低コスト項目
  （02 open question 4 = レイアウト位置のタブ、02 open question 5
  = 識別子の Unicode 許容）を含む、「どのマイルストーンの主題にもし
  なかった残り」の一括整理もここで行う。
- draft のまま残っている文書を正式化するか、それとも実装段階で回収
  するか。
- `CLAUDE.md` の phase-conditioned rules の見直し（spec-first フェ
  ーズを終えて次フェーズに入れるか）。
- 実装言語の選定作業の開始条件を揃える（候補列挙・比較表の枠組み
  はここではなく次フェーズで作る）。

依存：全マイルストーン。
想定文書：`spec-freeze-review.md`（番号は完了順に採番）。

## 次フェーズ関連の並行トラック (2026-04-18 開始)

spec-first フェーズのロードマップ (M1–M10) 完了と並行して、次のト
ラックを進行する：

- **T1 チュートリアル** — 関数型言語を触ったことがない読者（Ruby
  出身の user 自身を含む）向けの段階的チュートリアル群を
  `docs/tutorial/` 以下に整備する。現行仕様（01〜13 draft）を基礎
  としつつ、複雑さは段階的に露出する。user が「現状の仕様は複雑で
  理解が難しいかもしれない」「場合によっては Elm と Haskell の中
  間を目指すかも」と述べているため、チュートリアル側で揺り戻しの
  種が見つかる可能性も見越す。
- **B1 ビルド戦略** — Sapphire は Ruby コードとしてビルドされる
  ため、コンパイルパイプライン（入力・出力・ランタイム・invocation
  モデル・packaging）のドキュメント群を `docs/build/` 以下に整備
  する。文書 10（Ruby interop）／ 11（Ruby monad）の「実装側対応」
  としての位置付け。`docs/impl/`（ホスト言語選定、M10 で提案）と
  は異なる軸で、対象言語 Ruby 側の build 契約を扱う。
- **T2 チュートリアル章 5/6 教育的書き直し** (2026-04-18 発動) —
  T1 で書いた章 5（モジュールと型クラス）と章 6（Ruby との対話）
  が入門者に重い点を user が指摘。spec は維持しつつ（open-questions
  の T-05/T-06 を (C) で決着）、tutorial を **具体→抽象の順序化**
  で書き直す。HKT / `Functor f` / `Monad m` の一般化した視点は
  新設する「発展篇」に隔離する。作業対象は `docs/tutorial/05-*.md`
  と `06-*.md`、および新 chapter（07 以降の発展篇）。

これらは仕様の delta ではない（規範は 01〜13）が、仕様を読み解く
／実装するための支援文書として spec-first フェーズ完了と並行して
走る。

## フェーズ外

- **実装言語の選定**：仕様凍結後のフェーズの主題。`CLAUDE.md` の
  phase-conditioned rules に従い、spec-first フェーズ中は触らない。
- **コンパイラ本体の設計**：同上。
- **パッケージ管理・ビルドツール**：同上。

## 進め方のメモ

- draft 段階では後続文書が先行文書に制約を逆流させることを許す。
  逆流が起きたら先行文書を素直に更新する（commit を分けて履歴上追
  跡可能にする）。
- 各文書は必ず Open questions 節を持ち、未決事項を明示する。
- マイルストーン完了時にはこのロードマップの該当項目に「draft 済み」
  の印を付け、新たに生じた依存関係・方針転換を反映する。
- 「draft 済み」から「正式化」への遷移は M10（仕様整合確認と凍結判
  断）で user と合意して行う。各文書は M10 に入るまで draft 扱いで
  改訂される前提とする。
