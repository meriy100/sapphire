# 09. Prelude

状態: **draft**。Ruby 相互運用文書 (M7 / M8) が生成 Ruby 側に露出
すべき prelude の値を特定する過程、および M9 の例題プログラムが抜
けを明らかにする過程で改訂されうる。

本文書は `docs/spec/09-prelude.md` の日本語訳である。英語版が規範
的情報源であり、BNF 生成規則・規則名・番号付き未解決の問いは英語版
と一致させて保つ。

## 動機

これまでの文書は具体的な束縛を繰り返し「prelude で」と先送りして
きた。本文書は draft レベルで prelude の内容を固定する：最小の型・
コンストラクタ・標準インスタンス・補助関数の集合であり、すべての
Sapphire プログラムが前提にできるもの。

範囲内：

- 中核 ADT：`Bool`・`Ordering`・`Maybe`・`Result`・`List`。
- リスト表層構文：`[]` および `[x, y, z]` を `Nil` / `Cons` 連鎖の
  糖衣に、`::` を二項 cons（第 5 位右結合）。
- `if ... then ... else ...` を `case` の糖衣として再規定。
- 文書 07 の標準クラス `Eq`・`Ord`・`Show`・`Functor`・
  `Applicative`・`Monad` を、本文書が導入する型に対する prelude
  インスタンスとして定義。
- 文書 05 が prelude で与えると約束した算術・比較・論理演算子の束
  縛。
- 最小限の utility 関数（`id`・`const`・`compose`・`flip`・`not`・
  `map`・`filter`・`foldr`・`foldl`）。

本文書は以下を決着させる：

- 02 未解決の問い 1（`True` / `False` を字句クラスとするか prelude
  コンストラクタとするか）— 下の `Bool` ADT により **コンストラク
  タ**。
- 01 未解決の問い 3（`if` をプリミティブとするか糖衣とするか）—
  §Boolean と `if` の脱糖規則により **糖衣**。

保留：

- prelude 関数の網羅的列挙。本文書は M1〜M8 の例題を走らせ、M9 を
  書くために必要な **語彙** を固定する。それ以上の関数（`foldMap`・
  `traverse`・`Data.Map` 風構造など）は漸進的に増える。
- Ruby 相互運用の値（`readFile`、具体的 Ruby 評価モナドの
  `run`-shape 関数）。M7 / M8。
- M7 / M8 が最終的に定義する範囲を超える I/O。
- クラスインスタンスの既定メソッド本体は、型検査に必要な場合のみ
  書く。それ以外は 07 の minimal-complete-definition 慣習に従う。

## モジュールとしての prelude

prelude は `Prelude` と名付けられた Sapphire モジュール：

```
module Prelude
  ( ... )
  where
```

他のすべてのモジュールは暗黙に `Prelude` を非修飾でインポートする。
モジュールが明示的な空 prelude インポート（`import Prelude ()`、
08 参照）を宣言する場合を除く。明示形は暗黙形を上書きする — 両者
は同時に発生しない。

本文書の残りは `Prelude` の中身を記述する。上のエクスポートリスト
は末尾の §モジュールエクスポートリスト で埋める。

## Boolean と `if`

```
data Bool = False | True
```

`True` と `False` は `Bool` の通常の値コンストラクタであり、文書
03 の `data` 機構と文書 06 のコンストラクタパターン規則で解決する。

**02 未解決の問い 1 決着。** 文書 01 / 02 の `BOOL` は表層形
`True` と `False` の対であり、これを `Bool` の 2 つの 0 引数コン
ストラクタとして identity 付けする。文書 01 の (LitBool) 規則は
これらのコンストラクタスキームに (Var) を適用することで導出可能と
なる。01 は (LitBool) をその導出の簡略形として扱って差し支えない。

**01 未解決の問い 3 決着。** `Bool` が ADT となったため、条件式

    if c then t else f

は

    case c of
      True  -> t
      False -> f

の **表層糖衣** となる。文書 01 の (If) 規則は、文書 06 の (Case)
と、03 の `True` / `False` に対する (Con) / (Var) から導かれる。
(If) 規則は補題として 01 に残される — 糖衣化前の形で書かれたプロ
グラムは、まったく同じ型で型付けされ続ける。

### `Ordering`

```
data Ordering = LT | EQ | GT
```

`Ord` の `compare` が用いる（下の §Ord インスタンス 参照）。payload
を持たない単純な三方向比較結果。

## エラー処理 ADT

```
data Maybe a    = Nothing | Just a
data Result e a = Err e   | Ok a
```

`Maybe` は省略可能な値を表す。`Result` は型 `e` のエラー payload
で失敗しうる値を表す。Sapphire 風の名前 `Result` を Haskell の
`Either` より優先するのは、下流の Ruby 側契約との親和性のため
（Ruby の慣用句は ok/err の語彙を用いることが多い）。

`Result` の型パラメータは `e a`（エラー先、成功末尾）の順序：こ
れにより `Result e` は種 `* -> *` を持つ — `Monad (Result e)` の
要求する形。bind 意味論は §Functor / Applicative / Monad インス
タンス 参照。

## List

```
data List a = Nil | Cons a (List a)
```

**リストの表層構文。** `List a` の値は 2 つの同値な表層形で書ける：

- コンストラクタ形：`Nil`、`Cons 1 (Cons 2 (Cons 3 Nil))`。
- **リテラル形**（糖衣）：`[]` は `Nil`、`[1, 2, 3]` は 3 要素
  `Cons` 連鎖、`x :: xs` は `Cons x xs`（優先度・結合性は 05 の
  演算子表 参照）。

リテラル形は純粋に構文糖衣であり、パース時に脱糖される：

```
[]                       脱糖は   Nil
[x]                      脱糖は   Cons x Nil
[x, y, z]                脱糖は   Cons x (Cons y (Cons z Nil))
x :: xs                  脱糖は   Cons x xs
```

リスト・リテラルパターンも同様に扱う。文書 06 の `apat` 生成規則
は本文書でリストリテラルパターン形を追加するよう拡張される：

```
apat ::= ...                                   -- (06)
       | '[' ']'                               -- 空リストパターン
       | '[' pat (',' pat)* ']'                -- リストリテラルパターン
```

各リテラルパターンは対応する `Nil` / `Cons` 連鎖に脱糖され、その
後に 06 の型付け規則（PCon・PCons など）が適用される。

`[` と `]` は文書 02 でトークンとして予約済みであり、本文書がそれ
を有効化する。他の文書はこれらを消費していないため、追加は綺麗な
単調変更となる。

## クラスインスタンス

文書 07 の標準クラスは本文書で具体インスタンスを受け取る。

### `Eq` インスタンス

- `instance Eq Int` — 原始的な整数等価。
- `instance Eq String` — 原始的な文字列等価。
- `instance Eq Bool` — コンストラクタ等価で定義。
- `instance Eq a => Eq (Maybe a)` — `Nothing == Nothing`、
  `Just x == Just y` iff `x == y`。
- `instance (Eq e, Eq a) => Eq (Result e a)` — 同様。
  `Err x == Err y` iff `x == y`、`Ok x == Ok y` iff `x == y`。
- `instance Eq a => Eq (List a)` — 構造的再帰で定義。

### `Ord` インスタンス

- `instance Ord Int` — 原始的な順序。
- `instance Ord String` — 辞書式。
- `instance Ord Bool` — `False < True`。
- `instance Ord a => Ord (Maybe a)` — `Nothing < Just _`。
- `instance (Ord e, Ord a) => Ord (Result e a)` —
  `Err _ < Ok _`。
- `instance Ord a => Ord (List a)` — 辞書式。

### `Show` インスタンス

- `instance Show Int`・`Show String`・`Show Bool`、および
  `Show a` があれば `Show (Maybe a)`・`Show (List a)`。
  `Show e, Show a` があれば `Show (Result e a)`。

`show` は値の正規な表層形を生成する。`show [1, 2, 3]` は
`"[1, 2, 3]"`、`show (Just 1)` は `"Just 1"`。

### `Functor` / `Applicative` / `Monad` インスタンス

`Maybe` について：

```
instance Functor Maybe where
  fmap f Nothing  = Nothing
  fmap f (Just x) = Just (f x)

instance Applicative Maybe where
  pure = Just
  Nothing  <*> _        = Nothing
  _        <*> Nothing  = Nothing
  Just f   <*> Just x   = Just (f x)

instance Monad Maybe where
  Nothing  >>= _ = Nothing
  Just x   >>= f = f x
```

`Result e` について（最初の `Err` で短絡）：

```
instance Functor (Result e) where
  fmap f (Err e) = Err e
  fmap f (Ok  x) = Ok (f x)

instance Applicative (Result e) where
  pure = Ok
  Err e <*> _     = Err e
  _     <*> Err e = Err e
  Ok f  <*> Ok x  = Ok (f x)

instance Monad (Result e) where
  Err e >>= _ = Err e
  Ok  x >>= f = f x
```

`List` について（Haskell の「非決定的選択」モナド）：

```
instance Functor List where
  fmap f Nil         = Nil
  fmap f (Cons x xs) = Cons (f x) (fmap f xs)

instance Applicative List where
  pure x = Cons x Nil
  fs <*> xs = concatMap (\f -> map f xs) fs

instance Monad List where
  xs >>= f = concatMap f xs
```

（`concatMap` と `map` は下の §Utility 関数 で定義される。シグネ
チャは通常の prelude 関数。**重要：両関数は `List` のコンストラク
タに対するパターンマッチで直接定義される**（`Functor` / `Monad
List` のディスパッチを介さない）ため、`map → concatMap →
Applicative List → Monad List` の定義順は循環依存なく well-founded
である。）

`Ordering` はパラメータを持たないため、`Eq`・`Ord`・`Show` イン
スタンスは持つが、高階クラスのインスタンスは持たない。

## 算術・比較・論理束縛

文書 05 の演算子表の型は prelude 束縛から来ると約束されていた。こ
こでスキーム形で与える。演算子は 07 の `(op)` 構文の括弧付き前置
名で列挙する。

```
(+), (-), (*), (/), (%)  : Int -> Int -> Int
negate                    : Int -> Int

(<), (>), (<=), (>=)      : Ord a => a -> a -> Bool
(==), (/=)                : Eq a  => a -> a -> Bool
compare                   : Ord a => a -> a -> Ordering

(&&), (||)                : Bool -> Bool -> Bool
not                       : Bool -> Bool

(++)                      : String -> String -> String

(>>=)                     : Monad m => m a -> (a -> m b) -> m b
(>>)                      : Monad m => m a -> m b -> m b
pure                      : Applicative f => a -> f a
return                    : Monad m       => a -> m a      -- 既定は pure
```

宣言された型は文書 05 の演算子表と一致する — 05 の `(==)`・
`(/=)`・`<`・`>`・`<=`・`>=` に対する Int 専用エントリは、07 に
従って `Eq Int` / `Ord Int` の **インスタンス** として解釈し直す。

## Utility 関数

最小の有用集合。各シグネチャはエクスポート済 prelude コードが見る
べきスキーム。

```
id        : a -> a
const     : a -> b -> a
compose   : (b -> c) -> (a -> b) -> (a -> c)
flip      : (a -> b -> c) -> (b -> a -> c)

map         : (a -> b) -> List a -> List b
filter      : (a -> Bool) -> List a -> List a
foldr       : (a -> b -> b) -> b -> List a -> b
foldl       : (b -> a -> b) -> b -> List a -> b
concat      : List (List a) -> List a
concatMap   : (a -> List b) -> List a -> List b
length      : List a -> Int
head        : List a -> Maybe a
tail        : List a -> Maybe (List a)
null        : List a -> Bool

fst         : { fst : a, snd : b } -> a        -- 2 フィールドレコード形
snd         : { fst : a, snd : b } -> b

maybe       : b -> (a -> b) -> Maybe a -> b
fromMaybe   : a -> Maybe a -> a

result      : (e -> b) -> (a -> b) -> Result e a -> b
mapErr      : (e -> e') -> Result e a -> Result e' a

when        : Applicative f => Bool -> f {} -> f {}
unless      : Applicative f => Bool -> f {} -> f {}

show        : Show a => a -> String
print       : Show a => a -> Result String {}   -- stub；M7/M8 で retype
```

関数合成は本層では `compose` と名付ける。合成用の中置演算子
（Haskell / Elm の `.` あるいは Elm の `<<` 相当）は **束縛しない** —
`.` は修飾名（08）とレコード選択（04）のための句読点であり、算術
演算子として導入するとそれらの曖昧性解消を再度開くか、別の綴りを
選ぶかを要求する。中置合成演算子の導入は 09 未解決の問い 8。

擬型 `{}` は 04 の空レコード型。`f {}` は自明なレコードを返すモナ
ド動作を表す — 本層における Haskell の `()` 単位型の最も近い類似
物。独立した単位プリミティブを 09 が導入しない理由は §設計メモ
参照。

`head` と `tail` は **全域的** であり、トラップせず `Maybe` を返
す。部分的な版（`head'`・`tail'`）は最小 prelude には含まない。

`print` は本 draft の **stub**：M7 / M8 が Ruby 評価を基盤とする
具体 I/O 物語を固めるまでの当座品。返り値型 `Result String {}`
はプレースホルダ — 失敗した `print` は「エラーメッセージを運び、
成功時は空レコードを自明に返す」。M7 / M8 で真のモナド I/O 型に
置き換わる。今日 `print` に依存するプログラムは呼び出しを分離し、
M7 / M8 着地時の型変更に備えること。

## モジュールエクスポートリスト

prelude のエクスポートリスト（要点、全てではない — 本文書と共に
育つ）：

```
module Prelude
  ( -- 型
    Bool(..)
  , Ordering(..)
  , Maybe(..)
  , Result(..)
  , List(..)

    -- クラス（全クラスはメソッドもエクスポート）
  , class Eq(..)
  , class Ord(..)
  , class Show(..)
  , class Functor(..)
  , class Applicative(..)
  , class Monad(..)

    -- 演算子
  , (+), (-), (*), (/), (%), negate
  , (<), (>), (<=), (>=), (==), (/=)
  , (&&), (||), not
  , (++), (::)
  , (>>=), (>>)

    -- ユーティリティ
  , id, const, compose, flip
  , map, filter, foldr, foldl, concat, concatMap
  , length, head, tail, null
  , fst, snd
  , maybe, fromMaybe
  , result, mapErr
  , when, unless
  , pure, return
  , show, print
  , compare
  )
  where
```

`class X(..)` は文書 08 の「全メソッドエクスポート」形。暗黙
prelude 規則により、通常のモジュールは列挙された全名を既定で非修
飾で受け取る。

## 設計メモ（非規範的）

- **`Bool` は ADT、`if` は糖衣。** 本文書による 02 未解決の問い 1
  と 01 未解決の問い 3 の決着は、真偽値の扱いを ADT 物語の中に統
  一する。`True` / `False` は単なるコンストラクタであり、条件式
  に対する型システムの特別ケースは存在しない。コストはコンパイ
  ラ側の 1 段の脱糖であり、`if` を別プリミティブとして規定する
  のに比べれば自明に小さい。

- **リストリテラル構文。** 脱糖 `[x, y, z]` →
  `Cons x (Cons y (Cons z Nil))` により、`[` と `]` を抽象構文木
  で特別扱いする必要が無くなる。全リストリテラル式はパース後には
  単なるコンストラクタ連鎖式である。パターン側リテラルも同様に脱
  糖される — パターンとしての `[x, y]` は `Cons x (Cons y Nil)`
  にマッチする。

- **なぜ `Result e a`、`Either e a` ではないのか。** 生成 Ruby
  物語（M7 / M8）は、ok/err の語彙を用いる Ruby コードに結果値を
  露出する。`Result` は言語境界をまたいでこの語彙を保つ。Haskell
  の `Either` を好むユーザは別名を付けられる：`type Either e a =
  Result e a` は一行の prelude 追加（型別名は言語機能として未確
  定）。

- **本 draft では中置合成演算子を採らない。** 関数合成は prelude
  束縛の `compose`。`.` を合成用に予約するのは魅力的（Haskell は
  そうする）だが、`.` は既にレコードフィールド選択（04）と修飾名
  区切り（02 / 08）の意味を持ち、両者を区別するための散文的曖昧
  性解消は既にパーサに負担をかけている。中置綴りは、専用のトーク
  ン（例：`<<` / `>>`）が選ばれた時点で 09 未解決の問い 8 として
  着地可能。

- **漸進的成長。** 本文書の中核集合を超えた prelude は、M9 例題プ
  ログラムが抜けを明らかにするとともに、M7 / M8 が Ruby 相互運用
  値を着地させる過程で成長する。追加は draft の形（名前・型・
  クラスインスタンス）を尊重すること。

- **タプルは不在。** `fst` と `snd` は 2 フィールドレコード
  （`{ fst, snd }`）上で型付けされ、組み込みタプル型を持たない。
  04 のクローズド構造的レコード方針と一致する意図的な選択。`(1,
  2)` 構文が欲しいユーザは `{ fst = 1, snd = 2 }` と書ける。タプ
  ル構文を別糖衣として導入するかは最小 prelude に含めない。

  **フィールド名** `fst` / `snd` と **関数名** `fst` / `snd` が
  同じなのは無害な偶然である。04 §設計メモ によれば、フィールド
  名は選択構文の独自世界に住み（`.f` は環境中で `f` を参照しない）、
  関数 `fst` とフィールド `fst` は曖昧性なく共存する。`(fst p)`
  は関数呼び出し、`p.fst` はフィールド選択。

- **`head` / `tail` は全域的。** 両関数は空リストで失敗せず
  `Maybe` を返す。特に `tail : List a -> Maybe (List a)` は
  Haskell の慣習（部分関数 `tail : [a] -> [a]`）と異なる。Sapphire
  の全域志向と prelude の「`undefined` / `error` 無し」方針の両方
  が `Maybe` 版を指す。

- **`undefined` / `error` は無し。** prelude は部分的な「stuck」
  関数を含まない。全域プログラミングを好むスタイルであり、失敗は
  `Result` か（最終的に）I/O モナドのエラーチャネルを流れる。
  Ruby 相互運用が要求すればアドホック脱出口を追加できる。

## 未解決の問い

1. **タプル構文。** Sapphire は `(a, b)` や `(a, b, c)` のタプル
   構文を、整数キーのフィールド（あるいは位置的 `fst`/`snd` /
   `first`/`second`/... フィールド）を持つレコード型の糖衣として
   認めるか。現 draft は否定、ユーザはレコードを書き下す。相互作
   用：04 のクローズドレコード規律により、これは軽い人間工学的
   利得であって型システム上の必須ではない。

2. **型別名。** `type Either e a = Result e a` 風の糖衣を採るか。
   本文書と直交だが、ここで役立つ。まだ未規定。

3. **`String` を文字のリストとするか。** Haskell の `type String
   = [Char]` は `Char` 型を要求する。Sapphire の `String` はプリ
   ミティブ層の原子型（01・02）。`Char` が存在した後、`String`
   を `List Char` として再解釈すべきかは未決。現 draft は `String`
   を不透明に保つ。

4. **`Num` か Int のみか。** 07 未解決の問い 6 は算術を `Num` クラ
   スにすべきかを問うた。本文書は 07 の draft に合わせて Int 専
   用のまま。再考すれば、ここの算術シグネチャをすべて書き換える
   ことになる。

5. **`IO` / 具体的 Ruby 評価モナド。** `print` は
   `Result String {}` 型の stub。M7 / M8 が具体的 Ruby 評価型を
   定義したら、`print` はそれを使うよう retyped されるべき。09
   自身のブロッカーではないが、重い forward 依存。

6. **`Char` プリミティブ。** prelude は `Char` を含まない。文字列
   索引・文字操作・JSON ライクな文字列操作でやがて要求されうる。
   M7 / M8 またはユーザプログラムが要求するまで保留。

7. **既定 prelude インポート。** 「暗黙の `import Prelude`」と述
   べたが、厳密な機構（コンパイラ合成インポートか、言語定義モジュ
   ールか）はツーリングに影響しうる。本文書は実装経路について沈黙
   する。

8. **中置合成演算子。** 前置 `compose` 関数の代わりに二項合成演算
   子を導入するか。候補：Haskell 風 `.`（02/04/08 の `.` 曖昧性解
   消を再度開く必要あり）、Elm 風 `<<` / `>>`（02 の `op_char` 集
   合および 05 のモナド `>>` との衝突チェックが必要）。draft は中
   置形を採らず、ユーザは `compose` を呼ぶ。

9. **`map` / `concatMap` の中置別名。** Haskell の `fmap` に対する
   `<$>`、`flip (>>=)` に対する `=<<` は一般的な利便。演算子として
   追加するか（トークンを 05 の演算子表に加える必要がある）は未決。
   draft は否定。
