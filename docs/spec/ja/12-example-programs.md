# 12. 例題プログラム集

状態: **draft**。M10（仕様凍結レビュー）で例題を規範内容と照合する
過程で改訂されうる。

本文書は `docs/spec/12-example-programs.md` の日本語訳である。英語
版が規範的情報源であり、コードブロックと Sapphire ソースは英語版と
一致させて保つ。コード中のコメントは Sapphire ソースの一部として
英語版と同じ。

## 動機

仕様文書 01〜11 は Sapphire の言語機構を層ごとに固めた。本文書は
それらを同時に走らせる。以下の各例題は **単独で動く Sapphire プロ
グラム** であり、Ruby モジュールへコンパイルされ、文書 10 / 11 が
定める機構の下で走る。例題は言語の「触感」である：各層が着地した
後、idiomatic な Sapphire コードがどう見えるかを示す。

例題は意図的に小さく（30〜80 行）、作為的でない：普通のユーザが
Sapphire に持ち込みそうな具体タスクを扱う。

範囲内：

- 以下を合わせて exercise する 4 本のプログラム：
  - 純粋式とパターンマッチ（01・03・06）。
  - レコードと構造的型付け（04）。
  - 演算子と数値算術（05）。
  - 型クラスと `do` 記法（07）。
  - モジュールとインポート（08）。
  - Prelude 束縛（09）。
  - `:=` と `Ruby` モナドによる Ruby 相互運用（10・11）。

範囲外：

- ベンチマークや性能調整。
- 完全な end-to-end テストハーネス（M10 で "例題の実行方法" 付録
  を追加するかもしれない）。
- 例題を実行可能な Ruby にコンパイルする devcontainer / ビルド
  パイプライン（これは spec-first フェーズではなく実装フェーズ
  の範疇）。

## 規約

各例題は順に以下を列挙する：

1. プログラムの意図を 1〜2 文で。
2. 1 つ以上の `.sp` ファイルに現れる Sapphire ソース。
3. 簡単な読解ガイド：どの仕様文書を exercise しているか。

コードブロックは draft の Sapphire 構文を使う。コード内コメントは
Sapphire コメント（`--` 行、`{- ブロック -}`）。`:=` 束縛中の埋め
込み Ruby は Ruby の `#` コメントを使う。

## 例 1. Hello, Ruby

**意図。** `Ruby` モナドで挨拶を印字する 1 モジュールプログラム。
最小の非自明な end-to-end の触感。

```
module Main
  ( main )
  where

-- Main action: greet two names in sequence.
main : Ruby {}
main = do
  greet "Sapphire"
  greet "world"

-- A pure function produces the greeting string.
greet : String -> Ruby {}
greet name = rubyPuts (makeMessage name)

-- Pure Sapphire builds the message, no Ruby involved.
makeMessage : String -> String
makeMessage name = "Hello, " ++ name ++ "!"

-- Ruby side: `rubyPuts` is the one embedded snippet.
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

**読解ガイド。**

- `Main` は単一モジュールプログラム（08）。
- `main : Ruby {}` は 11 の `Ruby` モナドと 04 の空レコード `{}`
  を自明な成功値として使う。
- `do` 記法（07）が `greet` の 2 回の呼び出しを順序化する。
- `greet` は純粋な Sapphire 関数で、`Ruby {}` 値を生むが自身は
  `:=` 束縛ではない。
- `rubyPuts` が唯一の Ruby 相互運用 — `puts` を包む `:=` 束縛（10）。
- 実行は `run main` を経由し、成功時は `Ok {}`、`puts` が失敗した
  ら `Err e` を返す。

## 例 2. 数値ファイルを解析し合計する

**意図。** 1 行 1 整数のファイルを読み、解析し、合計を印字する。
Ruby のファイル I/O、純粋な `Result` ベースの解析、`List` の畳み
込みを exercise する。Ruby 側は「行を読む」と「結果を印字する」
I/O の縁に留め、解析自体は純粋 Sapphire。

```
module NumberSum
  ( main )
  where

-- | Read a file, parse integers per line, sum them, print the result.
main : Ruby {}
main = do
  raw <- rubyReadLines "numbers.txt"
  case parseAll raw of
    Ok ns  -> rubyPuts (show (sumOf ns))
    Err e  -> rubyPuts ("parse failed: " ++ e)

-- Parse a list of strings into a list of ints, failing fast on any
-- non-integer line. Pure `Result`-monadic.
parseAll : List String -> Result String (List Int)
parseAll []       = Ok []
parseAll (s::ss)  = do
  n  <- parseInt s
  ns <- parseAll ss
  pure (Cons n ns)

-- Pure parse of a single string. Relies on a prelude primitive
-- `readInt : String -> Maybe Int` that 09's minimum set does not
-- yet ship; this example assumes it as a forthcoming addition.
parseInt : String -> Result String Int
parseInt s = case readInt s of
  Nothing -> Err ("not an integer: " ++ s)
  Just n  -> Ok n

-- Fold a list of ints.
sumOf : List Int -> Int
sumOf = foldl (+) 0

-- Ruby bridge: read a file as a list of chomped lines.
rubyReadLines : String -> Ruby (List String)
rubyReadLines path := """
  File.readlines(path).map(&:chomp)
"""

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

**読解ガイド。**

- `case` の被検査値は純粋な `Result String (List Int)` 値なの
  で、外側の `main` では `rubyReadLines` アクション以外に `do`
  は不要（06・09）。
- `parseAll` は **`Result` 内側の `do`**（07・09 §"Functor /
  Applicative / Monad instances"）を使う：`Result String` はモ
  ナドであり、`Err e >>= f = Err e` が最初の失敗でパースを短絡
  させる。
- `parseInt` は純粋な Sapphire 関数 — Ruby を跨がない。
  `readInt : String -> Maybe Int` に委譲しており、本例はそれを
  prelude への追加として前提にする。下の OQ 6 参照。
- `sumOf` は point-free：`foldl (+) 0` は `foldl (+) 0 xs`。
- コンストラクタ `Ok`・`Err`・`Cons`・`[]` と 09 のリストリテ
  ラル脱糖がすべて現れる。

## 例 3. レコードのリストをフィルタ・グルーピング

**意図。** レコードと、レコードを対象にしたパターン束縛、および
高階リスト処理のデモ。Ruby 相互運用なし — プログラムは純粋。

```
module Students
  ( topScorersByGrade )
  where

-- A student's score row. `type` here follows 09 OQ 2's proposed
-- Haskell-style alias syntax; 09 has not yet decided whether to
-- admit it (see Open question 4).
type Student = { name : String, grade : Int, score : Int }

-- Returns the highest-scoring student per grade, as a pair list.
-- (Shows multi-step pattern matching and record-field access.)
topScorersByGrade : List Student -> List { grade : Int, top : Student }
topScorersByGrade students =
  let grades = uniqueGrades students in
  map (\g -> { grade = g, top = bestIn g students }) grades

-- Extract the distinct grades in the list.
uniqueGrades : List Student -> List Int
uniqueGrades = foldr addGradeIfAbsent []

addGradeIfAbsent : Student -> List Int -> List Int
addGradeIfAbsent s gs =
  if member s.grade gs
    then gs
    else Cons s.grade gs

-- Best scorer inside a single grade. The list is assumed non-empty
-- for grades returned by `uniqueGrades`.
bestIn : Int -> List Student -> Student
bestIn g students =
  let inGrade = filter (\s -> s.grade == g) students in
  foldr1 pickBetter inGrade

pickBetter : Student -> Student -> Student
pickBetter a b =
  if a.score >= b.score then a else b

-- Simple membership.
member : Int -> List Int -> Bool
member _ []       = False
member x (y::ys)  = if x == y then True else member x ys

-- foldr1 is not in the minimum prelude of 09; it's defined here for
-- clarity. Sapphire users who want it in their own prelude can
-- re-export from a utility module.
foldr1 : (a -> a -> a) -> List a -> a
foldr1 f (x::xs) = foldr f x xs
-- foldr1 _ [] is not reachable for the cases this module produces.
```

**読解ガイド。**

- `type Student = ...` は 09 未解決の問い 2 が提案する `type` 別
  名形。別名自体は規範としてまだ認められていない（未解決の問い 4
  が flag する）。
- `s.grade` によるレコードフィールドアクセス（04）。
- cons / nil 上のパターンマッチ（06、09 のリスト糖衣）。
- 補助関数（`addGradeIfAbsent`・`pickBetter`）は関数内ではなくト
  ップレベルに置く — 関数内局所 `where` 節はまだ規定されていない
  ため（02 は `where` を `module` / `class` / `instance` ヘッダ用
  にのみ予約）。入れ子スコープは将来の `where` 拡張か多束縛 `let`
  拡張で乗る。
- 空リストに対する `foldr1` の意図的な不網羅性：06 の網羅性規則は
  本コードを非網羅として拒絶する。仕様準拠の版は `Maybe a` を返す
  か種値を受け取る。例題は網羅性の論点を可視化するためそのまま残
  す。

## 例 4. Ruby 相互運用で取得して要約する

**意図。** Ruby でリモートペイロードを取得し、解析し、要約を印字
する。複数の `:=` スニペットを跨ぐ `do` 記法、`Result RubyError`
を介したエラー処理、モジュールインポートを exercise する。

2 モジュール：`Fetch` が高レベルの入口を、`Http` が Ruby 相互運用
プリミティブを持つ。

### `src/Http.sp`

```
module Http
  ( get, HttpError(..) )
  where

data HttpError
  = NetworkError String
  | StatusError  Int String
  | DecodeError  String

-- | Fetch a URL, returning the body as a `String` or a classified error.
get : String -> Ruby (Result HttpError String)
get url := """
  require 'net/http'
  require 'uri'

  begin
    uri = URI.parse(url)
    res = Net::HTTP.get_response(uri)
    if res.is_a?(Net::HTTPSuccess)
      { tag: :Ok, values: [res.body] }
    else
      msg = res.message || "unknown"
      { tag: :Err, values: [
        { tag: :StatusError, values: [res.code.to_i, msg] }
      ] }
    end
  rescue => e
    { tag: :Err, values: [
      { tag: :NetworkError, values: [e.message] }
    ] }
  end
"""
```

### `src/Fetch.sp`

```
module Fetch
  ( main )
  where

import Http (get, HttpError(..))

main : Ruby {}
main = do
  res <- get "https://example.com/"
  case res of
    Ok body -> do
      n <- stringLength body
      rubyPuts ("fetched " ++ show n ++ " bytes")
    Err httpErr -> rubyPuts (explain httpErr)

explain : HttpError -> String
explain err = case err of
  NetworkError m     -> "network error: " ++ m
  StatusError  c msg -> "HTTP " ++ show c ++ ": " ++ msg
  DecodeError  m     -> "decode error: " ++ m

-- Ruby bridge: ask Ruby for the string's byte length.
-- 09's prelude does not (yet) ship String-length; the Ruby side
-- handles it here.
stringLength : String -> Ruby Int
stringLength s := """
  s.bytesize
"""

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

**読解ガイド。**

- 2 モジュールプログラム。`Fetch` は 08 の選択的インポート形で
  `Http` から特定の名前をインポートする。
- `HttpError` はコンストラクタ 3 つの ADT。エクスポート
  `HttpError(..)` 形により、型と全コンストラクタが `Fetch` のス
  コープに入る。
- `get` は `Ruby String` ではなく `Ruby (Result HttpError String)`
  を返す。これは「明示的エラーチャネル」形状：`Result` はドメイン
  レベルの失敗（HTTP ステータスエラー、デコードエラー）を捉え、
  Ruby 例外（例：`net/http` gem 内部からの）は依然として `Ruby`
  モナドが捕捉し、`run` の `Err` 選択肢を介して露出する。
- Ruby 本体には 10 §ADT のタグ付きハッシュ ADT 表現が明示的に現
  れる：`{ tag: :Ok, values: [...] }` /
  `{ tag: :StatusError, values: [...] }`。
- `Result HttpError String` は純粋 Sapphire 側で `case` 分解され、
  各アームから `rubyPuts` を呼ぶ — 「作用的な取得」と「純粋な分
  類」のきれいな分離。

## 設計メモ（非規範的）

- **例題は言語のエレベーターピッチ。** 例 1 と例 4 はまとめて
  Sapphire-as-specified の最小読み：「原則的な Ruby 境界を専用
  モナドで持つ純粋関数コア」。この 2 プログラムをざっと読めば新
  参者は Sapphire の目的を把握できる。

- **意図的な荒削り。** 例 3 は `type` 別名形（09 未解決の問い 2、
  規範としてはまだ認められていない）と非網羅的な `foldr1`（06 で
  拒絶される）を使う。読解ガイドで明示するが黙って直さない。例題
  は「仕様がまだ閉じていない箇所」のチェックポイントも兼ねる。

- **ベンチマークなし、長時間実行デモなし。** 焦点は仕様機構が
  end-to-end で合成することであって、速度競争に勝つことではない。
  性能特性は実装フェーズの関心事。

- **Ruby 例外 vs ドメインエラー。** 例 4 は「真に例外的」（`Ruby`
  の例外チャネルが捕捉、`run` サイトで `Err RubyError` として露
  出）と「想定される失敗モード」（`Result HttpError _` としてモ
  デル化）の境界を引く。両モードが利用可能であり、実際の Sapphire
  コードは両者を混ぜる。

- **可能な限り prelude のみ。** 例題は 09 の最小集合に依存する：
  `map`・`filter`・`foldr`・`foldl`・`show`、09 の ADT コンス
  トラクタ。例 3 の自作 `foldr1`・`member`・`addGradeIfAbsent`・
  `pickBetter` は稀な逸脱であり、それぞれラベル付けされる。

## 未解決の問い

1. **追加例：長時間走る Ruby 計算。** CPU バウンドの Ruby タスク
   に `Ruby` を使い、`run` ごとの単一スレッドモデルをデモするプロ
   グラムは含んでいない。スレッド挙動がユーザ向けの関心事になった
   ら M10 の凍結前チェックリストに追加する価値がある。

2. **07 なしでは壊れる例。** 現在の例題は 07（`do`・`Show`・
   `Monad`）を exercise するが、pre-MTC Sapphire では失敗するよ
   う多相を **要求** する例は無い。`Monad m => m a` や `Show a =>`
   の引数を非自明に取る小さな例があれば、例題セットでの 07 の証拠
   が硬くなる。

3. **Ruby が Sapphire を呼ぶ例。** 4 つの例題はすべて Sapphire を
   ホストとして Ruby スニペットを呼ぶ扱い。生成 `Sapphire::...`
   モジュール（10 §生成 Ruby モジュールの形）を Ruby プログラムが
   消費する例は、境界のもう片側を閉じる。M10 へ。

4. **例 3 の `type` 別名。** 本例題は 09 未解決の問い 2 が提案す
   る Haskell 風別名形 `type Student = ...` を使う。09 OQ 2 が
   `no` で着地すると、例 3 は使用箇所にレコード型をインライン展
   開するよう書き直す必要がある。この綴りで `yes` で着地すれば
   canonical な用法になる。別の綴り（例：Elm 風 `type alias`）で
   `yes` なら、例の `type` キーワードを調整する。

5. **完全に純粋な例。** 例 3 が最も近いが `main` を持たない。走
   らせ可能な Ruby モジュールにコンパイルされ、単一の `Ruby` アク
   ション包みの中で興味深い純粋作業をするプログラムは良い追加と
   なる。

6. **例 2 の `readInt` prelude 依存。** 例 2 の純粋 `parseInt` は
   `readInt : String -> Maybe Int` を呼ぶが、これは 09 の最小
   prelude には含まれていない。選択肢：(a) 09 を修正して `readInt`
   （および対応する `readFloat` 等）を含める。(b) 例 2 を再構成し、
   `String -> Ruby Int` の `:=` 束縛で Ruby から整数を取り、不正
   入力は Ruby の例外チャネル経由で `run` の `Err RubyError` に任
   せる。(c) 依存を可視のまま残し、ユーザが自前 `readInt` を供給す
   る。現 draft は (c) を取り注釈で補う。M10 の凍結前チェックリ
   ストで再訪する。
