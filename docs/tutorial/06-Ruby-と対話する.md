# 06. Ruby と対話する

最終章。Sapphire の signature feature である **Ruby インタロップ**
に踏み込む。前章で導入した `Monad` と `do` 記法の上で、Sapphire
プログラムから Ruby のコードを呼び出し、結果を pure な Sapphire
の世界に流し戻す方法を扱う。

主に文書 10（Ruby インタロップのデータモデル）、11（Ruby 評価
モナド）、12（例題プログラム集）を背景にしている。

## 大きな絵

Sapphire のコードは最終的に Ruby モジュールを生成する
（文書 10 §Generated Ruby module shape）。そして言語の中には、
**書かれた Ruby コードの断片を、Ruby スレッド上で評価して結果を
返す** 仕組みが備わっている。これが `Ruby` モナドである。

ふたつの方向の境界を意識すると整理しやすい。

1. **Sapphire から Ruby を呼ぶ。** `:=` で Ruby コードを埋め
   込み、`Ruby a` 型の値を作る。これを `do` で連鎖させ、最後に
   `run` を呼ぶと Ruby スレッドが走り、結果が `Result RubyError a`
   として戻ってくる。
2. **Ruby から Sapphire を呼ぶ。** Sapphire モジュールは
   `Sapphire::ModuleName` 形式の Ruby クラスにコンパイルされ、
   公開されたトップレベル関数はクラスメソッドとして呼び出せる。
   こちらは「Ruby アプリから Sapphire モジュールを require して
   使う」という普段使いの形。

このチュートリアルでは主に (1) の方向 — Sapphire 側から Ruby を
呼ぶ — に集中する。

## `Ruby a` 型

`Ruby a` は「実行すると Ruby を介して `a` 型の値を返す **保留
された計算**」である（文書 11 §Type signature）。

| 型 | 意味 |
|----|------|
| `Ruby Int` | 実行すると Int を返す Ruby 計算 |
| `Ruby String` | 実行すると String を返す Ruby 計算 |
| `Ruby {}` | 実行すると「特に値を返さない」Ruby 計算（副作用専用） |

`{}` は前章で出てきた **空レコード** の型で、値も `{}` の一つ
だけ。Haskell の `()` (unit) に相当する位置づけ
（文書 09 §Utility functions の余談）。

重要: `Ruby a` は **値を持っていない**。あくまで「実行すれば
Ruby を介して `a` を取り出せる手順書」を持っているだけである。
手順書を実際に走らせるのは `run` の仕事。

## 埋め込みの基本: `:=`

```
rubyUpper : String -> Ruby String
rubyUpper s := "s.upcase"
```

これが Sapphire 文法上の **新しい束縛の形** で、`=` の代わりに
`:=` を使い、右辺は Sapphire 式ではなく **Ruby ソースコードの
文字列** を書く（文書 10 §The embedding form）。

ルール:

- `:=` を使う束縛には型シグネチャが **必須**。
- 結果の型は `Ruby τ` の形でなければならない（途中に `->` が
  挟まっていても最後が `Ruby τ` であればよい）。
- 引数（ここでは `s`）は Ruby 側で **同じ名前のローカル変数**
  として見える（文書 10 §The embedding form 末尾）。

つまりこの `rubyUpper` は

- Sapphire 側からは `rubyUpper "hello"` のように呼べる関数。
- 呼ぶと `Ruby String` 型の値（保留された計算）が返る。
- その計算を `run` すると、Ruby インタプリタが `s = "hello"` の
  状態で `s.upcase` を評価する。成功すれば `Ok "HELLO"` が、Ruby
  側で例外が飛んでいれば `Err e` が Sapphire 側に返る（詳しくは
  下の「実行: `run`」節）。

### 複数行の Ruby コード

複数行を埋め込みたいときは三連引用符 `"""..."""` を使う
（文書 10 §Triple-quoted string literals）。

```
rubyReadLines : String -> Ruby (List String)
rubyReadLines path := """
  File.readlines(path).map(&:chomp)
"""
```

中身はそのまま Ruby として評価される。改行をそのまま含められる
点だけが、ふつうの文字列リテラルとの違い。

## データの行き来 (marshalling)

Ruby 側と Sapphire 側を行き来する値は、文書 10 §Data model が
定めるルールに従って **自動的に変換** される。

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

ADT が **タグ付きハッシュ** に変換される点が要所。Ruby 側で
ADT に対応する値を作って返したいときは、この形のハッシュを
組み立てればよい（文書 12 の例題 4 を参照）。

`Ordering` だけは特別扱いで、シンボル `:lt` `:eq` `:gt` に
対応する。Ruby の `<=>` 慣用に合わせるためである。

## まとめ役: `do` で連鎖する

Sapphire 側から `Ruby a` を組み立てるには、前章でやった `do`
記法をそのまま使える。`Ruby` は `Monad` のインスタンス
（文書 11 §Class instances）だからである。

`Ruby` モジュールは `Prelude` と同じく **暗黙にインポートされる**
（文書 09 §The prelude as a module、文書 11 §Interaction with
earlier drafts）ので、明示的な `import Ruby` は普通は要らない。

```
greetTwice : String -> Ruby {}
greetTwice name = do
  rubyPuts ("Hello, " ++ name)
  rubyPuts ("Bye, "   ++ name)

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

`greetTwice "world"` は `Ruby {}` 型の値で、まだ何も実行されて
いない。中身は

```
rubyPuts "Hello, world" >>= \_ ->
  rubyPuts "Bye, world"
```

と展開される（文書 07 §`do` notation の脱糖規則）。最初の
`rubyPuts` の結果（`{}`）は使わないので `\_ ->` で受けている。

## 実行: `run`

`Ruby a` を実際に走らせるには `run` を呼ぶ
（文書 11 §`run`）。

```
run : Ruby a -> Result RubyError a
```

- 例外が発生せずに終われば `Ok a`。
- Ruby 側で例外が投げられれば `Err e`、`e` には `RubyError` が
  入る。

`RubyError` は文書 10 §Exception model で次のように定義されて
いる:

```
data RubyError = RubyError
  { class_name : String
  , message    : String
  , backtrace  : List String
  }
```

つまり「例外クラスの名前、メッセージ、バックトレース」を持つ
レコードである。Ruby の `rescue => e` で捕まえた `e` を、
Sapphire 側の値として手元で扱える形に詰め直したもの。

### 実行モデル

文書 11 §Execution model が定めている要点だけ:

1. `run` を呼ぶたびに **新しい Ruby スレッド** が立ち上がり、
   そこですべての Ruby スニペットが順次評価される。
2. `>>=` で連結された Ruby アクションは **逐次** 実行される。
   並列はサポートされていない。
3. 各 `:=` スニペットは **独立した Ruby ローカルスコープ** で
   走る。前のスニペットで定義した Ruby のローカル変数は次の
   スニペットからは見えない。状態を引き継ぎたいなら、その値を
   Sapphire 側に戻して `do` の `<-` で受け、次の Ruby スニペット
   に **引数として渡す**。
4. 例外が出たら、以降のステップは飛ばされて `run` が `Err` を
   返す。

> Ruby の `Thread.new { ... }` を毎回作るような重さに見えるが、
> これは「pure な Sapphire 世界と Ruby の VM 状態を切り離す」
> ための意図的な設計（文書 11 §Design notes）。

### 「`Ruby` から逃げる方法は `run` だけ」

Sapphire には `unsafeRun` のような抜け道がない（文書 11
§There is no `unsafeRun` / `runIO`）。`Ruby Int` 型の値から
直接 `Int` を取り出すことはできず、必ず `run` を経由して
`Result RubyError Int` という形にしてから case 分岐で取り出す。

これが Sapphire のうたう「pure な世界と effectful な世界の
明示的な境界」である。Ruby の世界で何が起きても `Result` で
くるんで返ってくる、という保証になる。

## 一通り組んでみる

文書 12 §Example 1 をベースにした最小プログラム。

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

読み方:

- `main : Ruby {}` がエントリポイント。実体は二回 `greet` を
  呼ぶだけの `Ruby` アクション。
- `greet` は **pure な Sapphire 関数** だが、結果が
  `Ruby {}` 型なので副作用を持つ計算を組み立てている。
- `makeMessage` は完全に純粋。Ruby は一切絡まない。
- `rubyPuts` だけが `:=` の Ruby スニペット。

実行されるとき (`run main`) は:

1. Ruby スレッド起動。
2. `puts "Hello, Sapphire!"` を実行（`rubyPuts` の 1 回目の
   呼び出し）。
3. `puts "Hello, world!"` を実行（`rubyPuts` の 2 回目の
   呼び出し。同じ `:=` 束縛を別の引数で再利用している）。
4. 例外なく終わったので `Ok {}` を返す。

文書 10 §Generated Ruby module shape のルールに従い、Sapphire
コンパイラは概ね次のような Ruby コードを吐く（簡略化）:

```ruby
module Sapphire
  class Main
    def self.main
      # do 記法を脱糖した連鎖を、Ruby 側のスレッドに乗せて実行
      # 内部実装は省略
    end
  end
end
```

Ruby アプリケーション側からは

```ruby
require 'sapphire/main'
Sapphire::Main.main
```

として呼べる、という想定。

## エラーチャネルを上手に使う

ふたつの「エラー」を区別する設計が、Sapphire の Ruby
インタロップでは推奨されている（文書 12 §Example 4 の読解
ガイド）:

- **本当の例外**: ネットワーク切れ、ファイル不在、Ruby ライブ
  ラリの内部例外など、想定外の事態。これらは Ruby の `raise`
  経由で発生し、`run` 時に `Err RubyError` として表面化する。
- **想定内の失敗**: HTTP 4xx ステータス、入力のパースエラーなど、
  「失敗パターンとして設計に組み込みたい」もの。これらは
  ドメイン側で `Result MyError String` のような型を定義して
  Ruby スニペットの戻り値に乗せる。

例 (文書 12 §Example 4 から引用整形):

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

`get` の型は `Ruby (Result HttpError String)`。

- 二重に包まれているのがミソで、外側の `Ruby` が「Ruby を呼び
  に行く」、内側の `Result HttpError` が「想定内の失敗を
  分類する」。
- Ruby 側で `rescue` を書いているが、これは想定内の
  `NetworkError` に丸めるためのもの。これを書かなければ、
  ネットワーク例外はそのまま `run` 経由で `Err RubyError` と
  して上に届く。

呼ぶ側は二段で受ける:

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
explain (NetworkError m)    = "network error: " ++ m
explain (StatusError c msg) = "HTTP " ++ show c ++ ": " ++ msg
explain (DecodeError m)     = "decode error: " ++ m

-- String の長さは Ruby 側に聞く（09 の prelude には文字列長が
-- 入っていないので、Ruby スニペットで `bytesize` を呼ぶ）
stringLength : String -> Ruby Int
stringLength s := """
  s.bytesize
"""
```

`do` の中で `<-` で `Result HttpError String` を受け、`case` で
内側を分解する。`Ok` アームが更に `do` を開いているのは、
`stringLength body` が **`Ruby Int` であって `Int` ではない** から
（`String -> Ruby Int`）。`<-` で剥がして `Int` の `n` を得てから
`show n` に渡す。`Ruby a` から `a` に降りる唯一の経路は `<-` か
`run` の二択 — この章の冒頭からの原則である。`Ruby` の例外は更に
もう一段外側で `run` が受け止めるので、ここでは見えない。

## まとめ

Sapphire を Ruby と組み合わせて使うときの原則を改めて並べると:

- **Pure な部分はとことん pure**。`Ruby` 型を持たない関数は、
  どんなに呼んでも副作用を起こせない。これが推論や reasoning の
  土台になる。
- **副作用は `Ruby` の中に押し込む**。`Ruby a` は単なる値で
  あって、`>>=` / `do` で組み立てられる。
- **境界は `run` 一箇所**。`Ruby` から外に出るには必ず `run` を
  通り、結果は `Result RubyError a` という分類された形で受ける。
- **データの marshalling は自動**。型に書いた通りに Ruby と
  Sapphire の値を相互変換してくれる。ADT はタグ付きハッシュ。

これだけ覚えれば、文書 12 にある例題は素直に読める。

## 仕様への気付き

- 文書 11（Ruby モナド）は仕様としては比較的素直だが、これを
  入門者に説明するためには、前章でやった `Monad` の理解が
  かなり要る。チュートリアルとしては
  「Maybe Monad で `do` の感覚をつかむ → Result Monad で同じ
   形が再利用できる → Ruby Monad はそれの effect 版」
  という三段ロケットでようやく飲み込めるという感触で、
  正直なところ章一つ分の重さが集中している。
- ただ、Ruby 利用者にとっては **`>>=` の翻訳に「早期リターンの
  連鎖」を当てる** のが思いのほか効くため、Maybe / Result までは
  比較的軽快に進める。最後の `Ruby` モナドのところは、`>>=` を
  「Ruby スレッドの上で逐次実行する手順書を組み立てる演算子」と
  説明し直す必要がある。比喩を取り替える接ぎ目があり、ここは
  チュートリアルの摩擦点である。
- 文書 10 のデータモデルは、Ruby 利用者には親しみやすい部類で、
  説明の苦労は少ない。タグ付きハッシュの形が一目で意図と
  繋がるので、ここはむしろ Sapphire の「分かりやすい」側。
- 仕様簡素化の候補としては、Elm-Haskell 中間に倒すなら
  `Functor` / `Applicative` を仕様の表面から薄くして、
  `Monad` 単独で `do` を支える設計にするのが入門コストを
  もっとも下げる。`Ruby` モナドだけが要るなら、汎用 `Monad`
  すら導入せず「Ruby 専用 do 記法」にする選択もあるが、これは
  2026-04-18 の方針転換（汎用 monad を入れる）と逆行するので、
  仕様の方向性自体の判断が要る。
- 全体として、05 章と 06 章は連続して読むことを前提に書いて
  おり、05 章で `Monad` が腑に落ちさえすれば、06 章は素直に
  着地できる。逆に言えば 05 章の出来が全体の重量を決めて
  しまっており、ここを「型クラスを表に出さずに `Ruby` モナド
  だけ説明する」軽量版に作り直すこともできる。判断は仕様の
  目標水準しだい。

## おわりに

ここまでで、Sapphire の draft 仕様を一通り「動かしてみる気分」で
眺めた。Sapphire という言語を一言で表すなら、

> **「Elm の読みやすい関数型コアの上に、Haskell 級の抽象化機構と、
> Ruby を effect として正面から扱う Monad を載せた言語」**

である。仕様策定はこの draft をもって一区切りで、今後は実装言語の
選定 / プロトタイプ作成のフェーズへと進んでいく
（`docs/project-status.md`、`docs/roadmap.md`）。

実例と詳細は文書 12 の四つの例題を眺めるのが手早い。気になった
論点があれば、各仕様書末の Open questions 節に大体まとめて
あるので、そこから議論に持ち込める。
