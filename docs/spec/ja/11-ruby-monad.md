# 11. Ruby 評価モナド

状態: **draft**。M9 例題プログラムが `run` / `>>=` 機構を使い込む
過程で改訂されうる。

本文書は `docs/spec/11-ruby-monad.md` の日本語訳である。英語版が
規範的情報源であり、BNF 生成規則・規則名・番号付き未解決の問いは
英語版と一致させて保つ。

## 動機

`docs/project-status.md` は Sapphire の signature feature として
`RubyEval` 風モナドを挙げる — 別スレッドで埋め込み Ruby スニペット
を走らせ、結果を pure なパイプラインに戻すモナド。文書 10（Ruby
相互運用データモデル）は境界契約を定めたが、モナドの命名、
`Monad` インスタンス、実行モデルは意図的に opaque のままだった。本
文書がそれらを決着させる。

範囲内：

- モナド型の **命名**（ロードマップの「★」命名マイルストーンを
  決着）。
- 型に対する `Functor` / `Applicative` / `Monad` インスタンス。
- スレッド実行意味論：連鎖した `>>=` アクションが Ruby 側でどう
  スケジュールされるか。
- `run` 関数 — アクションを完走まで駆動し `Result` を返す pure 側
  の唯一の入口。
- モナドと、10 で導入された `:=` 束縛形式との関係。

本文書は 10 のデータモデル契約を再掲しない。それを前提に、10 が
導入した opaque な `Ruby` 型の上にモナドクラスインスタンス、スレ
ッドモデル、`run` 関数を埋める。

## 命名：`Ruby`

検討した候補（ロードマップ命名マイルストーンより）：

- `RubyEval` — 明示的だが各シグネチャサイトで冗長。
- `Rb` — 短いが汎用的に見え、文脈外では Ruby だと分かりにくい。
- `Ruby` — 既にモジュール名（10）であり、型名として自然。Haskell
  の `Data.Map.Map` パターンに倣う。
- `Eval` — 汎用すぎ。`Reader` 風モナドなどと混同されうる。
- `Host` / `Embed` — Ruby を抽象化する含みを持つが、Sapphire は
  Ruby 特化。
- `Script` — スクリプト的響きだが「任意のスクリプト言語」と重な
  る。

**決定：`Ruby`。** モナド型は `Ruby` モジュール（10 で導入）に住
み、`Ruby` と名付ける。完全修飾は `Ruby.Ruby a`。09（10 による拡
張後）の暗黙 `Ruby` インポートのもと、非修飾使用は `Ruby a`。

文書 10 は本型を opaque として既に `Ruby` と呼んでいる。11 はその
クラスインスタンスと `run` 関数を埋める。

## 型シグネチャ

```
-- module Ruby の内部
data Ruby a    -- opaque、コンストラクタは実行時非公開
```

`Ruby` は 07 の `Monad` クラスが要求する通り種 `* -> *` を持つ。型
は **不透明**：ユーザは `data` パターン経由で `Ruby a` の値を直接
構築しない。値は次の経路でのみ流入する：

- `pure : a -> Ruby a`（`Applicative Ruby` インスタンスから）。
- `:=` 束縛（文書 10）で本体が Ruby スニペットのもの。
- 以上の合成を `>>=` で繋いだもの。

## クラスインスタンス

3 つのインスタンスはすべてモジュール `Ruby` に型と同居する（08 孤
児なし）：

```
instance Functor Ruby where
  fmap f ra = ra >>= (\x -> pure (f x))

instance Applicative Ruby where
  pure  = primReturn
  mf <*> ma = do
    f <- mf
    a <- ma
    pure (f a)

instance Monad Ruby where
  (>>=) = primBind
```

### プリミティブ（実行時提供）

```
primReturn : a -> Ruby a
primBind   : Ruby a -> (a -> Ruby b) -> Ruby b
```

どちらも実行時プリミティブ。`primReturn x` は pure 値のモナド包装
を構築し、`primBind ra f` は次の遅延計算を構築する：走らせると
`ra` を走らせ、結果を Sapphire へマーシャルバックし、`f` を適用し、
結果のアクションを順に走らせる。プリミティブはユーザから不可視 —
クラスメソッドの実装である。

法則。07 の 3 つのモナド法則が成立することを期待する：

- `pure a >>= f ≡ f a`
- `ra >>= pure ≡ ra`
- `(ra >>= f) >>= g ≡ ra >>= \x -> f x >>= g`

## 実行モデル

`Ruby a` 値は Sapphire 側の **遅延計算** である。下の `run` が適用
されるまで評価されない。

実行モデルのもとでは：

1. `run` が発火すると単一の **Ruby 評価スレッド** が spawn される。
   仕様上の契約は「`run` 呼び出しごとに新鮮なスレッド」である。
   実装が内部でスレッドをプールしてよいのは、状態隔離（各 run が
   新鮮な Ruby 側スコープを見、先行 run のローカル・グローバル・
   ロード済定数が漏れない）を保証できる場合に限る。Sapphire 側呼
   び出し元は Ruby スレッドが完了を通知するまでブロックする。

2. 各葉の `Ruby a` アクション — `pure` 包装された Sapphire 値か、
   `:=` 束縛の Ruby スニペットか — は Ruby スレッド上のサブステッ
   プになる。`pure` サブステップは自明。`:=` サブステップは、10
   のデータモデルに従って束縛ごとのローカルを populate した上で
   Ruby ソースを走らせる。

3. `>>=` で直列化されたアクションは Ruby スレッド上で **逐次** に
   走る。2 番目のアクションは 1 番目が完了して結果が Sapphire 値
   へマーシャルバックされ継続に渡されるまで開始しない。

4. **サブステップごとのスコープ隔離。** 各 `:=` 束縛の Ruby 本体
   は **新鮮な Ruby 局所スコープ** で実行し、パラメータをマーシャ
   ルして受け取り、結果をマーシャルして返す。あるスニペットで設
   定された Ruby 側ローカルは次のスニペットでは不可視。サブステ
   ップ間で持ち越されるのは、アクションの結果としてマーシャル・
   アウトされて `>>=` 経由で継続へ渡される値のみ。

5. いずれかのサブステップが Ruby 例外を上げると、残りのサブステ
   ップはスキップされ `run` は `Err` を返す（下の §`run` 参照）。

並行性。Ruby アクションの並列合成は本層では認めない — 各 `Ruby a`
アクションは Ruby 側で単一スレッドである。将来の拡張として
`parallel : Ruby a -> Ruby b -> Ruby (a, b)` 形のプリミティブを
加えることはありうる。11 未解決の問い 1。

`docs/project-status.md` の「別スレッド」表現は保持する：Ruby 評
価スレッドは Sapphire 側呼び出し元とは別の OS レベルスレッドであ
り、これが `run` をブロック操作とする理由である。分離により、
Sapphire の pure パイプラインが Ruby の VM 状態と絡まずにすむ。

## `run`

```
run : Ruby a -> Result RubyError a
```

`run` は `Ruby a` アクションを完走まで駆動する **pure 側唯一の入
口**：

- 成功時（Ruby 例外なし）、`run` は `Ok a` を返す。`a` はアクショ
  ンの最終 Ruby 側結果から Sapphire 値へマーシャルされたもの。
- 失敗時（任意のサブステップが raise）、`run` は `Err e` を返す。
  `e : RubyError` は 10 に従い例外の `class_name`・`message`・
  `backtrace` を運ぶ。

`run` は **pure** — 内部で Ruby スレッドを spawn しつつも、
Sapphire の型レベルでは決定的な関数を呈する：同じ `Ruby a` 値に対
して同じ `Result` を返す（Ruby 側自身がスニペット内で時刻・乱数・
外部状態を使えば非決定的になりうるが、その作用は `Err` / `Ok` の
選択肢として現れるのであって、`run` が同じ入力に異なる返り値を出
すわけではない）。

実装メモ。スレッド spawn、マーシャリング、例外捕捉、結果配送は
すべてコンパイラ／ランタイムの責任。仕様はここで *表層型* と、
`run` が `Ruby a` から `Result` への排他的経路であるという *不変
条件* のみを固定する。

### `unsafeRun` / `runIO` は存在しない

仕様は `Ruby a` アクションから純粋な `a` を直接生む「モナド脱出」
プリミティブを露出しない。抽出はすべて `run` を通り、`Result` を
仲介する。Ruby アクションが全域と分かっていても（例：純粋な文字
列変換）、ユーザは `run (rubyUpper "hi")` と書いて `Ok` をパター
ンマッチする。包まれたモナド形は原理的な Ruby 境界の代償である。

## `:=` と `Ruby` — ループを閉じる

文書 10 は `:=` 束縛を導入し、宣言結果型を `Ruby τ` 形（矢印に包
まれていてもよい）にすることを要求した。`Ruby` が具体化した今、
`:=` 束縛は **ユーザが書くスマートコンストラクタ** となる：埋め
込み Ruby ソースは、囲うアクションが `run` で駆動されたときに走
る Ruby スレッドサブステップの記述である。

`do` 記法（07）は `Monad Ruby` の `>>=` を通して脱糖される。例：

```
rubyGreet : String -> Ruby {}
rubyGreet name := """
  puts "Hello, #{name}!"
"""

helloPipeline : String -> Ruby {}
helloPipeline name = do
  rubyGreet name
  rubyGreet ("again, " ++ name)
```

`helloPipeline` 関数は 2 ステップのアクションを構築する。呼び出
し元が `run (helloPipeline "world")` を適用すると、Ruby スレッド
は 2 回の `puts` を順に走らせ、`run` は `Ok {}` を返す。

## `print`（09 stub）との関係

文書 09 は `print` を stub として残していた：

    print : Show a => a -> Result String {}

M7 / M8 が retype する旨のメモ付き。`Ruby` が具体化した今、retype
後のシグネチャは：

```
print : Show a => a -> Ruby {}
print x = rubyPuts (show x)

-- マーシャル可能なパラメータを持つ下層の Ruby スニペット：
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

`print` は `:=` 束縛された `rubyPuts` の上に合成される通常の純粋
Sapphire 関数である。`print` のクラス制約 `a` は Ruby 境界を越え
ない — `show` によってまず `String` に還元され、マーシャルされる
のは結果の `String` のみ。これにより 10 §データモデル のマーシャ
リング契約（境界越えのパラメータは指定の型集合に属する必要がある）
が保たれる。

09 stub を使ったプログラムは結果に `run` を呼ぶよう書き換える：

```
main : Ruby {}
main = print "hello"

-- 実行時境界のエントリポイント：
--   run main   -- 成功なら Ok {}、失敗なら Err e を返す
```

## 既存 draft との相互作用

- **07（型クラス）。** `Monad Ruby` は種 `* -> *` の単一パラメー
  タインスタンス。`do` 記法は追加の機構なしで動く。
- **08（モジュール）。** `Ruby`、`primReturn`、`primBind`、`run`
  はすべてモジュール `Ruby` に住む。クラスインスタンスも同モジュ
  ール（孤児なし）。
- **09（prelude）。** `Ruby` は `Prelude` と並んで暗黙インポート
  される（09、10 による拡張後）。`print` は上記のように retype さ
  れる。
- **10（Ruby インタロップ）。** 10 は `Ruby` 型を opaque として
  扱う。11 は `Functor` / `Applicative` / `Monad` インスタンス、
  `run` 関数、スレッドモデル意味論を埋める。参照整合性を保つため
  10 と 11 は同じ commit で着地する。

## 設計メモ（非規範的）

- **名前の選択。** `RubyEval` ではなく `Ruby` を採ることで、各使
  用箇所のシグネチャが読みやすくなる（`Ruby Int` vs
  `RubyEval Int`）。Haskell 慣習の `Data.Map.Map` パターン（モジ
  ュール名と型名を共有）は命名をコンパクトに保ちつつ曖昧さを生ま
  ない — 08 によりモジュール名と型名は別名前空間に住む。

- **既定で単一スレッド。** 逐次 `Ruby` モナドは推論しやすく、
  Haskell の `IO` に近い。並列合成は魅力的だが、draft 層で仕様に
  忍び込ませるのが望ましくない競合条件の問題を伴う。11 未解決の
  問い 1 で再訪。

- **`run` が唯一の出口。** `run` を唯一の出口に保つことで参照透
  過性の境界を保つ：pure Sapphire コード中にある `Ruby a` 値は
  作用の記述であって作用の実行ではない。pure コードに関するプロ
  グラム正当性の議論は `run` サイトまでは依然有効。

- **`Result RubyError` がエラーチャネル。** 代替の形（Sapphire 側
  エラー投擲、コールスタック巻き戻しなど）は、07 が現在提供しな
  い型システム機構を要する。`Result` チャネルは操作的に最も単純
  で、09 の `Result e a` prelude 型が与える慣用に最も合う。

- **タイムアウトとキャンセルは非モデル化。** 長時間走る `Ruby a`
  アクションは Sapphire 側呼び出し元から見て `run` を無期限に
  ブロックしうる。`Ruby a` の割り込み／タイムアウトは 11 未解決
  の問い 2。

- **Ruby モナドは離散ステップで厳格。** 各 `>>=` ステップは次が
  始まる前に完全に完了する。結果のレイジー／インクリメンタルな
  ストリーミング（例：長大な `List String` を emit する `Ruby`
  アクション）はモデルの一部ではない。ストリーミングは 11 未解
  決の問い 3。

## 未解決の問い

1. **並列合成。** 2 つの Ruby アクションを別の Ruby スレッドで
   スケジュールし結果を joint する
   `parallel : Ruby a -> Ruby b -> Ruby (a, b)` 形のプリミティブ
   を認めるか。draft は否定。

2. **タイムアウトとキャンセル。** 壁時計制限でアクションを包む
   プリミティブ
   `timeout : Int -> Ruby a -> Ruby (Maybe a)` や
   `cancel : Ruby a -> Ruby (Result RubyError a)`。draft は否定。

3. **ストリーミング。** `Ruby` に「インクリメンタル」変種（Haskell
   の `MonadIO` + `StreamingT` のような）を認めるか。draft は否定。

4. **例外クラス粒度。** `RubyError` は Ruby 例外の `class_name` を
   `String` で運ぶ。より豊かなエラー区別（例：Ruby の一般的例外階
   層を映す Sapphire 側 ADT）があれば、文字列比較ではなく特定の
   Ruby 例外クラスをパターンマッチできる。draft は文字列ベース。

5. **連鎖 `:=` スニペット間の Ruby 側共有状態の脱出口。** §実行
   モデル 項目 4 はサブステップごとのスコープ隔離を規範として固
   定する。本 OQ は後続で、いくつかの `:=` スニペットを 1 つの実
   効的な Ruby スコープに束ねる opt-in プリミティブ（例：
   `withSharedScope : Ruby a -> Ruby a`）を提供すべきかという問
   い — 中間可変状態を要求する Ruby idiom のため。draft は否定。

6. **`Ruby` 内部の `Ruby` ネスト。** `Ruby (Ruby a)` は整形な型。
   仕様は `join : Ruby (Ruby a) -> Ruby a` を prelude の便利関数
   として提供すべきか（あるいは標準 `Monad` クラスから導出可能
   として）。Haskell は `join = (>>= id)` で無料。Sapphire prelude
   も露出できる。draft はユーザが `>>= id` を書く。

7. **生成 Ruby クラスとスレッド。** M7 の生成 Ruby クラスは束縛
   をクラスメソッドとして露出する。各メソッド呼び出しは内部で独自
   のスレッドを spawn するのか、それとも呼び出し側 Ruby コードが
   スレッドを所有するのか（すなわち `run` は Ruby 呼び出し元側か
   らは no-op）。draft は実装詳細だが、「Sapphire 側から呼ぶとき
   は Sapphire の `run` ラッパがスレッドを管理し、Ruby から直接呼
   ぶ側は `run` を介さずクラスメソッドを同期的に呼ぶ」寄り。
