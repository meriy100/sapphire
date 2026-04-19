# 06. Ruby と対話する

前章では `Monad` と `do` 記法で、`Maybe` / `Result` のような
「失敗するかもしれない計算」を軽快に書く道具を手に入れました。
本章では同じ `do` 記法の上で、Sapphire の signature feature で
ある **Ruby インタロップ** に踏み込みます。主に文書 10（Ruby
インタロップのデータモデル）、11（Ruby 評価モナド）、12（例題
プログラム集）を背景にしています。

## なぜ専用の型が要るのか

Sapphire では `puts` や HTTP リクエストのような **副作用** を、
**型に必ず現れる** 形で扱います。普通の `String -> String` では
なく `String -> Ruby String` と書けば、「呼ぶと Ruby を介して
`String` を返す」ことがシグネチャから見て取れます。

この `Ruby` は、前章の `Maybe` や `Result e` と同じく **`Monad`
のインスタンス**（文書 11 §Class instances）で、`do` / `<-` /
`pure` がそっくり流用できます。違うのは **動機** で、`Maybe` /
`Result` が「失敗の短絡」を担っていたのに対し、`Ruby` は
「Ruby 側で副作用を起こしつつ結果を受け取る」を担います。

こうして **同じ `Monad` のかたちを副作用の記述に使う** のが
Sapphire の流儀で、文書 11 ではこの役割を **作用モナド**（英：
*effect monad*）と呼びます。散文で「作用モナド」と書いていても、
型位置では従来どおり `Ruby a` と書きます（文書 11 §Role term）。
`Ruby` モジュールは `Prelude` と並んで **暗黙にインポート** される
（文書 09 §The prelude as a module）ので、`import Ruby` は普通
書きません。

## `Ruby a` の読み方

| 型 | 意味 |
|----|------|
| `Ruby Int` | 走らせると Ruby を介して `Int` を返す作用モナド |
| `Ruby String` | 走らせると `String` を返す作用モナド |
| `Ruby {}` | 値は持たず副作用だけを起こす作用モナド |

`{}` は **空レコード**（文書 04）で、型も値も `{}` の一つきり。
Haskell の `()` (unit) と同じ位置づけで、「副作用はあるが返す値
はない」場面で使います。

重要な注意: `Ruby a` の値を手に入れても、**Ruby はまだ動いて
いません**。これは「走らせれば `a` を取り出せる手順書」を持った
値で、走らせるのは後述の `run` の仕事です。`Maybe a` が `Just x`
か `Nothing` を「持っている値」だったのに対し、`Ruby a` は
「実行したら値が出る予定の値」だ、と覚えておいてください。

## 最小の埋め込み: `:=`

Ruby コードを 1 つの式として埋め込む、最小の形は次のとおりです
（文書 10 §The embedding form）。

```
rubyUpper : String -> Ruby String
rubyUpper s := "s.upcase"
```

`=` の代わりに **`:=`** を使い、右辺は Sapphire 式ではなく
**Ruby ソースコードの文字列** を書きます。ルール:

- `:=` 束縛には型シグネチャが **必須**。
- 結果型は `Ruby τ` の形でなければならない（最後が `Ruby τ` で
  あれば途中に `->` が並んでもよい）。
- 左辺に並べられるのは **単純な名前だけ**（分解パターン不可）。
  引数は Ruby 側で **同じ名前のローカル変数** として見える。

`rubyUpper "hello"` は `Ruby String` 型の値（まだ実行していない
手順書）を返し、後で `run` したときに Ruby インタプリタが
`s = "hello"` の状態で `s.upcase` を評価します。

複数行を埋め込むときは三連引用符 `"""..."""` を使います（文書
10 §Triple-quoted string literals）。

```
rubyReadLines : String -> Ruby (List String)
rubyReadLines path := """
  File.readlines(path).map(&:chomp)
"""
```

単一行の文字列リテラルには生の改行を書けない（文書 02）ので、
複数行の Ruby スニペットは必ず三連引用符形になります。

## Ruby と Sapphire のあいだで値が移る

Sapphire 側と Ruby 側を渡る値は、文書 10 §Data model のルールで
**自動的に変換** されます。

| Sapphire 型 | Ruby 表現 |
|-------------|-----------|
| `Int` | `Integer` |
| `String` | `String` (UTF-8) |
| `Bool` | `true` / `false` |
| `{ x = 1, y = 2 }` | `{ x: 1, y: 2 }`（symbol キーの Hash） |
| `Just 1` | `{ tag: :Just, values: [1] }` |
| `Nothing` | `{ tag: :Nothing, values: [] }` |
| `Ok 1` | `{ tag: :Ok, values: [1] }` |
| `Err "x"` | `{ tag: :Err, values: ["x"] }` |
| `[1, 2, 3]` | `[1, 2, 3]`（Ruby `Array`） |
| `LT` / `EQ` / `GT` | `:lt` / `:eq` / `:gt`（特例） |
| 関数 | Ruby `Proc` (`Lambda`) |

要所は三つ:

- **レコードは symbol キーの Hash**（文書 10 §Records）。
- **ADT はタグ付きハッシュ**（文書 10 §ADTs）。`Just 1` が
  `{ tag: :Just, values: [1] }` という envelope に統一されます。
  Ruby 側で `Maybe a` や `Result e a` を作って返したいときも、
  この形で組み立てればよい。
- **`nil` は使わない**。「Ruby で `nil` を返したら `Nothing` に
  なる」のような近道は **採りません**（文書 10 §ADTs の注記）。
  `Just nil` と `Nothing` が区別できなくなるためで、`Maybe a` を
  返す文脈でも必ずタグ付きハッシュを組み立てます。

`Ordering` だけは特例で、`:lt` / `:eq` / `:gt` の 3 シンボルに
直接対応します（Ruby の `<=>` 慣用に合わせるためで、他の ADT に
波及するルールではありません）。

## `do` で Ruby アクションをつなぐ

前章の `do` 記法がそのまま `Ruby a` にも使えます。Ruby スニペット
を 1 つだけ包んだ小さな `Ruby a` を用意して、あとは `do` で連鎖
するのが基本の形です。

```
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""

greetTwice : String -> Ruby {}
greetTwice name = do
  rubyPuts ("Hello, " ++ name)
  rubyPuts ("Bye, "   ++ name)
```

`greetTwice "world"` は `Ruby {}` 型の値で、**まだ何も実行されて
いません**。`do` ブロックは裏では

```
rubyPuts "Hello, world" >>= \_ ->
  rubyPuts "Bye, world"
```

と脱糖されます（文書 07 §`do` notation、前章 §`Monad` という抽象）。
`rubyPuts` の結果 `{}` は使わないので、続けて書けば十分で、前章の
「値を捨てる形」と同じです。値を受け取りたい行だけ `<-` を使い
ます。

```
shout : String -> Ruby {}
shout s = do
  upper <- rubyUpper s
  rubyPuts upper
```

`upper <- rubyUpper s` で `Ruby String` の中身を `upper : String`
として取り出し、`rubyPuts upper` に渡しています。`Maybe` の `<-`
が「`Nothing` なら短絡」だったのに対し、こちらの `<-` は
「Ruby スレッドで前のステップを走らせて、結果を次に流す」に
読み替わります。比喩は別物ですが、**式の形と脱糖規則は完全に
同じ** です。`pure e` で純粋な値を `Ruby a` に持ち上げる点も
前章と共通です。

## 境界は `run`

作った `Ruby a` を実際に走らせるのが `run`（文書 11 §`run`）です。

```
run : Ruby a -> Result RubyError a
```

- 例外なく終われば `Ok a`。
- Ruby 側で例外が上がったら `Err e`（`e : RubyError`）。

`RubyError` は文書 10 §Exception model で次のように定義されて
います。

```
data RubyError = RubyError String String (List String)
--                         class_name    message   backtrace
```

順に「例外クラス名、メッセージ、バックトレース」の 3 要素で、
Ruby の `rescue => e` で捕まえた `e` を Sapphire 側に詰め直した
もの、と考えるとよいでしょう。`rescue => e` と同じく **捕まえる
のは `StandardError` 系のみ** で、`Interrupt` のようなシステム
レベル例外は境界を素通りします。

`Ruby` から pure な Sapphire 側に値を出す経路は `run` **だけ**
です（文書 11 §There is no `unsafeRun` / `runIO`）。`Ruby Int` から
直接 `Int` は取れず、必ず `run` を通して `Result RubyError Int`
で受け、`case` で分解します。この「出口が 1 つ」の設計が Sapphire
のうたう **pure な世界と副作用の世界の明示的な境界** で、`run`
までの `Ruby a` 値はすべて「作用の **記述**」であって「作用の
**実行**」ではありません。

### 実行モデルの要点

文書 11 §Execution model のうち、入門で意識するとよいのは次の
4 点です。

1. `run` を呼ぶたびに **新しい Ruby 評価スレッド** が立ち上がり、
   そこで手順書のサブステップが順に走る。呼び出し元は完了を待つ。
2. `>>=` で直列化されたステップは **逐次** に走る。
3. 各 `:=` スニペットは **新鮮な Ruby ローカルスコープ** で走る。
   前のスニペットで作った Ruby 側の局所変数は次からは **見えない**。
   状態を引き継ぎたければ Sapphire 側に返して `<-` で受け、次の
   スニペットに **引数として渡す**。
4. どこかで例外が出たら以降はスキップされ `run` が `Err` を返す。

3 が Ruby の感覚と食い違いやすい点です。「スニペット間で Ruby の
状態を共有しない」方が Sapphire 側の推論と噛み合います。

## 小さな一本: hello, Ruby

文書 12 §Example 1 をそのまま引くと、最小の通しは次の形です。

```
module Main
  ( main )
  where

main : Ruby {}
main = do
  greet "Sapphire"
  greet "world"

greet : String -> Ruby {}
greet name = rubyPuts (makeMessage name)

makeMessage : String -> String
makeMessage name = "Hello, " ++ name ++ "!"

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

読みどころ:

- `main : Ruby {}` がエントリポイント。`do` で `greet` を 2 回
  呼ぶ手順書を組み立てているだけで、まだ何も実行していません。
- `greet` は **pure な Sapphire 関数** だが、結果が `Ruby {}` な
  ので「副作用を起こす手順書を作って返す関数」。Sapphire では
  「副作用を起こす関数」と「手順書を作る純粋関数」の区別がなく、
  すべて後者です。
- `makeMessage` は Ruby に触れない完全に純粋な部分。**できる限り
  pure 側を厚く** するのが Sapphire の書き方。
- `rubyPuts` が唯一の `:=` 束縛。

`run main` を呼ぶと Ruby スレッドで `puts "Hello, Sapphire!"` と
`puts "Hello, world!"` が順に走り、例外が出なければ `Ok {}` が
返ります。コンパイラは本モジュールを `Sapphire::Main` という
Ruby クラスに落とす（文書 10 §Generated Ruby module shape）ので、
Ruby アプリ側から `Sapphire::Main.main` として呼ぶ使い方も想定
されていますが、本章では扱いません。

## エラーを二層に分ける

`run` が返す `Result RubyError a` は **想定外の Ruby 例外** を
拾うチャネルです。いっぽう HTTP の 4xx や入力パースの失敗のような
**想定内の失敗** は、Ruby の例外に任せずドメインの型として定義
して `Result MyError a` に載せる、というのが Sapphire の推奨
スタイルです（文書 12 §Example 4）。

```
data HttpError
  = NetworkError String
  | StatusError  Int String
  | DecodeError  String

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
      { tag: :Err, values: [
        { tag: :StatusError, values: [res.code.to_i, res.message] }
      ] }
    end
  rescue => e
    { tag: :Err, values: [
      { tag: :NetworkError, values: [e.message] }
    ] }
  end
"""
```

`get` の型は `Ruby (Result HttpError String)` — 二重に包まれて
いるのがミソ。**外側の `Ruby`** が「Ruby を呼びに行く」作用
（`run` で剥がれると `Result RubyError _` になる）、**内側の
`Result HttpError`** が「想定内の失敗を分類する」ドメイン側の型
です。Ruby 側で `rescue` を書いて、想定内の失敗に分類し直して
います。Ruby の返り値が `{ tag: :Ok, values: [...] }` 等の形で、
タグ付きハッシュ規則どおり envelope を手で書いている点も確認
しておいてください。

呼ぶ側では `<-` と `case` を二段で組み合わせます。

```
main : Ruby {}
main = do
  res <- get "https://example.com/"
  case res of
    Ok body -> do
      n <- stringLength body
      rubyPuts ("fetched " ++ show n ++ " bytes")
    Err httpErr ->
      rubyPuts (explain httpErr)

explain : HttpError -> String
explain err = case err of
  NetworkError m     -> "network error: " ++ m
  StatusError  c msg -> "HTTP " ++ show c ++ ": " ++ msg
  DecodeError  m     -> "decode error: " ++ m

-- 09 prelude に String の長さは入っていないので、ここだけ Ruby に聞く。
stringLength : String -> Ruby Int
stringLength s := """
  s.bytesize
"""
```

読み方:

- `res <- get "..."` で `Ruby (Result HttpError String)` から
  `Result HttpError String` を取り出す。この `<-` は作用モナドの
  `<-` で、「Ruby スレッドで `get` を走らせてから続きへ」と読む。
- 取り出した `res` を **pure な `case` で** 分解。ここから先は
  `Result` の話で Ruby は絡みません。
- `Ok body` の枝が更に `do` を開いているのは、`stringLength body`
  が `Ruby Int` で `Int` ではないため。もう一度 `<-` で剥がして
  から `show n` に渡します。
- `Err httpErr` の枝は `explain` で `String` に直してから
  `rubyPuts`。純粋な分類は pure 関数に閉じ込める。

この「Ruby で外と会話 → Sapphire に戻って型で分類」の往復が、
Sapphire Ruby インタロップの基本リズムです。

## まとめと次へ

作用モナドの周辺で覚えておきたい原則を並べると:

- **Pure な部分はとことん pure**。`Ruby` が付かない型はどんなに
  呼んでも副作用を起こしません。
- **副作用は `Ruby a` に押し込む**。`do` / `<-` / `pure` で自由に
  組み立てられる。
- **境界は `run` 一箇所**。Ruby 側の例外は `Result RubyError a`
  にまとまって上がってきます。
- **想定内の失敗はドメインの型で**。本物の例外は `Err RubyError`、
  想定内の分類は `Ruby (Result MyError _)` と二層に分ける。
- **marshalling は自動**。レコードは symbol キーの Hash、ADT は
  タグ付きハッシュ、`nil` は使わない — の 3 点だけ押さえる。

次に進むなら文書 12 §Example 4 の全文を通しで読んでみるのが
おすすめです。本章で扱った道具はひととおり揃っているので、
`Http` モジュールと `Fetch` モジュールに分かれた二モジュール版
として、同じ形のコードがもう一段大きいスケールで書かれています。

## 仕様への気付き

- 前章の `<-` を「失敗の短絡」と読むモデルから、「Ruby スレッド
  上での逐次」に読み替える橋渡しが本章で一度だけ行なわれます。
  式の形と脱糖規則が不変な点は強調しましたが、比喩がここで切り
  替わる摩擦そのものは残るため、Example 4 の読解時に同じ点を
  もう一度強化する必要があるかもしれません（T-06-1 として既存
  登録済）。
- データモデル側はタグ付きハッシュの形が素直で Ruby 利用者には
  親しみやすい一方、`Just nil` と `Nothing` を区別するために
  `nil` を使わない規約は Ruby 側の慣用と逆行しうるので、Ruby
  スニペットを書くときに間違えやすいと思われます。規範は 10
  §ADTs のまま（T-06-2）。
