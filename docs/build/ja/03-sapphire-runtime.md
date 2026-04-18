# 03. Sapphire ランタイム gem

状態: **draft**。`docs/spec/10-ruby-interop.md`（データモデル、
`RubyError`）と `docs/spec/11-ruby-monad.md`（`Ruby` モナド、
`run`、スレッド）に対するパイプライン水準の伴走文書。

本文書は `docs/build/03-sapphire-runtime.md` の日本語訳である。
英語版が規範的情報源であり、節構成・サブモジュール対応・番号付
き未解決の問いは英語版と一致させて保つ。

## 守備範囲

本文書はすべての compiled Sapphire プログラムが依存する **Ruby
側支援ライブラリ**を固定する。これを Ruby gem（提案名
`sapphire-runtime`）として包装し、生成コード（02 に従う）とホス
ト Ruby アプリ（05 に従う）が消費する公開モジュール構造を規定す
る。

具体的には：

- タグ付きハッシュ ADT ヘルパ（10 §ADT に従う）。
- 11 のプリミティブ（`primReturn`、`primBind`）と `run` を実装す
  る `Ruby` モナド評価器、11 §実行モデルに従うスレッドモデルを
  含む。
- 10 §例外モデルに従う `RubyError` 形の値を生成する境界例外捕捉。
- 10 §データモデルが固定する全数対応を扱うマーシャリング補助
  （`to_sapphire` / `to_ruby`）。
- gem 包装 metadata：gem 名、名前空間、依存、`required_ruby_version`。

範囲外：

- Sapphire モジュールごとの*生成* Ruby の形（それは 10 §生成
  Ruby モジュール形で、02 §ファイル内容の形で概観している）。
- 生成コードをビルドする CLI（04）。
- テスト統合（05）。

ランタイム gem は**コンパイラではない**。compiled 出力とホスト
Ruby コードが実行時にリンクする pure-Ruby ライブラリである。コ
ンパイラ自身（ホスト言語は `docs/impl/` に委譲）は、本 gem を
呼び出す生成コードを出力する。

## gem アイデンティティ

| フィールド               | 提案値                                    |
|--------------------------|-------------------------------------------|
| gem 名                   | `sapphire-runtime`                        |
| トップレベル Ruby module | `Sapphire::Runtime`                       |
| `require` パス           | `sapphire/runtime`                        |
| `required_ruby_version`  | `~> 3.3`（01 OQ 1 に従う）                |
| 依存                     | なし（v0 では third-party ランタイム gem なし） |

トップレベル Sapphire 名前空間 `Sapphire::*` は、生成ユーザコー
ド（`Sapphire::Main`、`Sapphire::Data::List` 等、10 §生成 Ruby
モジュール形に従う）とランタイム（`Sapphire::Runtime::*`）で共
有される。両者は予約により共存する：ランタイム gem が
`Sapphire::Runtime` サブ名前空間を予約し、生成コードは決して
`Runtime` という名の Sapphire モジュールを発行しない（ユーザモ
ジュールがそうしようとすればコンパイラは静的エラーを報告する）。
その予約がコンパイラチェックより強いメカニズムに値するかは 03
OQ 1。

ユーザの `Gemfile` は通常通り依存を追加する：

```ruby
# Gemfile
gem 'sapphire-runtime', '~> 0.1'
```

正確なバージョン制約はバージョン互換問題の一部（01 OQ 2）。

## サブモジュール対応

ランタイム gem の公開表面は名前付きサブモジュール群に分割され
る。すべての消費者（生成コード、ホスト Ruby）はこれらの名前を
通じてランタイムを参照する：

- `Sapphire::Runtime::ADT` — タグ付きハッシュ ADT ヘルパ。
- `Sapphire::Runtime::Ruby` — `Ruby` モナド評価器（`run` とプリ
  ミティブ）。
- `Sapphire::Runtime::RubyError` — 捕捉した Ruby `Exception` か
  ら 10 §例外モデルに従う Sapphire 側 `RubyError` タグ付きハッ
  シュ値を構築するヘルパ。
- `Sapphire::Runtime::Marshal` — 10 §データモデルに従う
  `to_sapphire` / `to_ruby` 補助。
- `Sapphire::Runtime::Errors` — マーシャリング補助が、入力形が
  期待 Sapphire 型と食い違うときに raise する境界エラーサブクラ
  ス（`MarshalError`、`BoundaryError` 等）。

`require 'sapphire/runtime'` で 5 つすべてが利用可能になる。サ
ブパス（`require 'sapphire/runtime/adt'`）も admissible だが契
約では要求されない。

## ADT ヘルパ

10 §ADT に従い、Sapphire ADT 値 `K v₁ ... vₖ` は Ruby ハッシュ
`{ tag: :K, values: [ruby(v1), ..., ruby(vk)] }` にマーシャルさ
れる。`Sapphire::Runtime::ADT` は、生成コードがそれらのハッシュ
を構築・検査するために使う小さなヘルパモジュールである。

スケッチ（draft 時点で正確な API は固定しない；シグネチャは例
示と扱う）：

```ruby
module Sapphire
  module Runtime
    module ADT
      # タグ付きハッシュ値を構築する。
      def self.make(tag, values)
        { tag: tag, values: values }
      end

      # パターンマッチ風：tag と values を yield する。
      # 生成コードでは tag に対する case/when と組み合わせる
      # ことを想定。
      def self.match(value)
        unless value.is_a?(Hash) && value.key?(:tag) && value.key?(:values)
          raise Errors::BoundaryError, "expected tagged ADT hash, got #{value.inspect}"
        end
        yield value[:tag], value[:values]
      end

      # 生成された case 式コードが用いる簡易アクセサ。
      def self.tag(value)    = value[:tag]
      def self.values(value) = value[:values]
    end
  end
end
```

生成コードはこれらヘルパを厳密に**必要としない** — 各所で
`{ tag: :Just, values: [x] }` をインライン展開してもよい — が、
構築を `ADT.make` に集約することで、ランタイムが表現を進化させ
る（例：frozen-hash ラッパを追加、10 OQ 7 に従い `Struct` へ
移行）際に、各プロジェクトでコンパイラを再実行せずに済む。

`Sapphire::Runtime::ADT.match` は例示；生成コードが `ADT.match
{ ... }` を使うか、`ADT.tag(v)` に対するインライン `case` を使
うかは、コンパイラのコード発行選択である。ランタイム契約は：
§ADT ハッシュ形を満たす任意の値は admissible；満たさない任意
の値は `BoundaryError`。

### `Ordering` 特例

10 §`Ordering`（特例）に従い、Sapphire の `LT` / `EQ` / `GT` は
**タグ付きハッシュ表現を使わない**；そのまま Ruby シンボル
`:lt` / `:eq` / `:gt` にマーシャルされる。ランタイムは
`Ordering` 用の `ADT.make` 風ヘルパを露出しない；生成コードは直
接シンボルを発行し、消費者（Sapphire 側 unmarshal）は
`is_a?(Symbol)` をチェックして 3 つの妥当値に intern する。

## マーシャリング補助

`Sapphire::Runtime::Marshal` は 10 §データモデルが要求する 2 つ
の境界越し補助を提供する：

- `to_ruby(sapphire_value, type)` — Sapphire 側値表現と静的
  Sapphire 型を与え、Ruby 値を生成する。生成コードが `:=` 束縛
  Ruby スニペット（10 §埋め込み形に従う）に Sapphire 値を渡す
  ときに使う。
- `to_sapphire(ruby_value, type)` — Ruby 値と期待 Sapphire 型を
  与え、Sapphire 側値を生成する（または形不一致で
  `Errors::MarshalError` を raise）。Ruby スニペットの結果が
  Sapphire に再進入するときに生成コードが使う。

両補助は**型に駆動される**：型引数が、どのマーシャリング規則を
適用するかについての権威ある oracle である — まさに 10 §ADT が
要求する通り（「境界は Ruby ハッシュを検査するのではなく期待
Sapphire 型でマーシャリング規則を選ぶ」）。

`type` 引数の表現（文字列符号化？シンボルタグ AST？生成定数 lookup？）
はランタイムの問題である。ここでの契約は「コンパイラはランタイ
ムが期待するものを一貫して発行する」；実際の符号化は 03 OQ 2。

スケッチ：

```ruby
module Sapphire
  module Runtime
    module Marshal
      # Sapphire -> Ruby
      def self.to_ruby(value, type)
        case type
        when :int, :string, :bool then value          # 10 §基底型
        when [:list, _]                                # 10 §リスト
          inner = type[1]
          value.map { |x| to_ruby(x, inner) }
        when [:record, _]                              # 10 §レコード
          # value は Sapphire 側レコード表現；
          # 10 §レコードに従いシンボルキー Hash を生成する。
          ...
        when [:adt, _]                                 # 10 §ADT
          # value は { tag:, values: }；各 value に再帰。
          ...
        when [:ordering]                               # 10 §Ordering
          { lt: :lt, eq: :eq, gt: :gt }.fetch(value)
        when [:fun, _, _]                              # 10 §関数
          # Sapphire 関数を Ruby lambda として包む。
          ->(arg) { to_ruby(value.call(to_sapphire(arg, type[1])), type[2]) }
        when [:ruby, _]
          raise Errors::BoundaryError,
                "Ruby a values do not cross as data; use run"
        else
          raise Errors::MarshalError, "unknown Sapphire type: #{type.inspect}"
        end
      end

      # Ruby -> Sapphire（to_ruby を写し、形不一致で raise）
      def self.to_sapphire(value, type)
        ...
      end
    end
  end
end
```

上のスケッチは**例示のみ**。網羅的な節ごと定義は実装フェーズの
ものであり；ここでの契約は「`Marshal` は 10 §データモデルが固
定する型集合に対し全域であり、不一致時は `MarshalError` を生成
する」。

## `Ruby` モナド評価器

`Sapphire::Runtime::Ruby` は 11 §実行モデルに従う Sapphire 側
`Ruby` モナドを実装する。生成コードが起動する 3 つのプリミティ
ブを露出する：

- `pure(value)` — `value` を即座に得る `Ruby a` アクションを生
  成する（11 §クラスインスタンスに従う；これが `primReturn`）。
- `bind(action, k)` — `Ruby a` と Sapphire 継続 `k : a -> Ruby
  b` を逐次合成する（これが `primBind`）。
- `run(action)` — アクションを完了まで駆動し、11 §`run` に従い
  `Result RubyError a` 形の Sapphire 値を返す。

`primReturn` と `primBind` は Sapphire 側ではユーザに見えない
（11 §プリミティブに従う）；`Monad Ruby` クラスのランタイム側
実装である。生成コードはそれらへの呼出を `do` 記法を desugar す
るときに発行する。

`Ruby a` アクションは Ruby 側では opaque 値（`Sapphire::Runtime
::Ruby::Action`）である。具体表現はランタイムに私的；closure か、
trampoline 風レコードか、compiled bytecode ストリームかもしれな
い。契約は `pure`、`bind`、`run` のみを通じて露出する。

### `:=` 束縛スニペット入口

生成コードが `:=` 束縛（10 §埋め込み形に従う）を発行するとき、
実行時に埋め込まれた Ruby ソースをマーシャル済みパラメータと共
に実行するアクションを構築する Ruby メソッドを生成する。ランタ
イムはその構築のためのヘルパを露出する：

```ruby
# `rubyUpper s := "s.upcase"` の生成コード内部：
def self.rubyUpper(s)
  Sapphire::Runtime::Ruby.snippet(
    params:  { s: s },                # 既に Ruby 側
    body:    proc { |s:| s.upcase }   # 捕捉したスニペット
  )
end
```

`snippet` ヘルパはアクションを生成する。実行時にパラメータを新
しいローカルスコープに代入し（11 §実行モデル項目 4 に従う：ス
テップごとスコープ隔離）、捕捉された `proc` を実行する。上記の
形は Ruby ソースの文字列ではなく実 Ruby `proc` を使う；コンパ
イラがスニペット本体をリテラル `proc` として発行するか `eval`
する `String` として発行するかは 03 OQ 3。

### スレッドモデル

11 §実行モデルに従う：

1. `run` は Ruby 評価スレッドを spawn し、Sapphire 側呼出元はそ
   れにブロックする。
2. アクション内のサブステップは、そのスレッド上で逐次実行される。
3. 各 `:=` スニペットは新しいローカルスコープで実行される（ス
   ニペット間でリークしない）。
4. 任意のサブステップで raise された例外はアクションを短絡；
   `run` は `Err` を返す。

ランタイム契約：

- 各 `run` 起動は**新しい Ruby スレッド**、または、再利用前にラ
  ンタイムが clean なローカルスコープを保証するプール済みスレッ
  ドを得る。プール化は 11 に従う実装選択として admissible；v0
  ランタイムがプールするかは 03 OQ 4。
- Sapphire 側呼出元は通常の thread-join でスレッドにブロックす
  る。タイムアウト・キャンセルは扱わない（11 OQ 2 に従う）。
- スレッドは呼出元と**同じ Ruby プロセス**で動く。プロセス越し
  Ruby 実行（`run` ごとの subprocess）は範囲外。

```ruby
module Sapphire
  module Runtime
    module Ruby
      def self.run(action)
        thread = Thread.new { execute_in_isolation(action) }
        result = thread.value          # スレッド完了までブロック
        # `result` は execute_in_isolation で既に Result 形
        result
      end

      private_class_method def self.execute_in_isolation(action)
        # 新しいローカルスコープ、スニペット eval 用の新しい top-level binding。
        # { tag: :Ok, values: [a] } か { tag: :Err, values: [e] }
        # （Sapphire `Result RubyError a` 形）を返す。
        ...
      rescue => e
        ADT.make(:Err, [RubyError.from_exception(e)])
      end
    end
  end
end
```

### `run` は `Result RubyError a` を返す

11 §`run` に従い、`run` は `Result` を返す。ランタイムは ADT 形
の値を発行する：

- 成功：`{ tag: :Ok, values: [a] }`。`a` はマーシャル済み
  Sapphire 結果。
- 失敗：`{ tag: :Err, values: [e] }`。`e` は `RubyError` 形の
  Sapphire レコード。

これが、呼出側の生成 Sapphire コードがパターンマッチする形であ
る（`run` の Sapphire 側型シグネチャは `Ruby a -> Result
RubyError a` だから）。

### `unsafeRun` なし／脱出口なし

11 §`unsafeRun` / `runIO` は存在しないに従い、ランタイムは
**いかなる** Ruby スニペットの値が `run` を経ずに pure Sapphire
へ脱出することを許すプリミティブも露出しない。ランタイムは故意
にそうしたプリミティブを提供しない；そうしたものが必要になれば、
まず 11 の仕様改訂を必要とする。

## `RubyError` と例外捕捉

10 §例外モデルに従い、実行中の `Ruby a` アクション内で発生した
すべての Ruby 側例外は境界で捕捉され、Sapphire 側 `RubyError`
値に変換される。ランタイムが型を担う：

```ruby
# Sapphire 側型は 10 §例外モデル に従い位置引数（04 OQ 2 の
# 2026-04-18 決着に従う）：
#   data RubyError = RubyError String String (List String)
#                              -- class_name  message  backtrace
# 下の Ruby 表現はフィールド順駆動。

module Sapphire
  module Runtime
    module RubyError
      def self.from_exception(e)
        ADT.make(:RubyError, [
          e.class.name,
          e.message.to_s,
          (e.backtrace || []),
        ])
      end
    end
  end
end
```

例外捕捉点は `Ruby.run` 内部の `execute_in_isolation` 境界（前
述）。11 §実行モデル項目 5 に従い、最初に raise された例外がア
クションを短絡する：継続がまだ始まっていない任意のサブステップ
は飛ばされ、`run` の返す `Result` は `Err` となる。

捕捉は **ユーザレベル Ruby エラーについては広い**：すべての
`StandardError` を捕捉する。システムレベルシグナル — `Interrupt`
（Ctrl-C）・`SystemExit`・`NoMemoryError`・`SystemStackError`、
他の非 `StandardError` な `Exception` サブクラス — は **境界を超
えて伝搬する**。

これは 10 §例外モデル と整合する。10 は 2026-04-18 に捕捉規則
を *ユーザレベル* Ruby 例外（`StandardError` 以下）に scope する
よう narrow された。シグナルクラス例外（`Interrupt`・`SystemExit`・
`NoMemoryError`・`SystemStackError`）は設計上境界を通り抜ける。
03 OQ 5 はこの tension が解消された経緯を記録するものである。

捕捉した `e.class.name`、`e.message`、`e.backtrace` が 10 に従
い `RubyError` の 3 フィールドに入る。`backtrace` は Ruby が組
み立てなかった場合 `nil` のことがある（稀）；ランタイムは空リ
ストを代入する。

## エラー名前空間（`Sapphire::Runtime::Errors`）

ランタイムは自身の利用のため小さな Ruby 例外階層を定義する：

- `Sapphire::Runtime::Errors::Base` — 全ランタイムエラーの根。
- `Sapphire::Runtime::Errors::MarshalError` — 入力形が宣言型と
  食い違うときに `Marshal.to_ruby` / `to_sapphire` が raise する。
- `Sapphire::Runtime::Errors::BoundaryError` — 非タグ付き値が
  タグ付き値を要求する地点に到達したときに `ADT.match`（と類似
  もの）が raise する。

これらは**Ruby 側**例外であり、Sapphire 側 `RubyError` ではな
い。ランタイム自身が、compiled-code 契約が防ぐべき何かを問われ
たときにのみ表面化する。正しく compile されたコードを実行する
ユーザは見るはずがない；呼出規約に違反する third-party Ruby 呼
出元は見ることになる。

そうしたエラーが `Ruby a` アクションの実行*内部*で raise される
とき、境界 catch（前述）が他の例外と同様に `RubyError` に再包装
する。`Ruby a` アクションの外、すなわちホストアプリの素 Ruby
コードから呼ばれたときは、通常通り伝搬する。

## ロードと `require` 順

ランタイム gem の `lib/sapphire/runtime.rb` が単一の入口。典型
的な生成ファイルの最初の非コメント行は：

```ruby
require 'sapphire/runtime'
```

その後、ファイルは他の生成 Sapphire モジュールを `require` す
る（02 §モジュール越し require に従う）。ランタイムが先に来る
ことを除いて順は問題ではない；Ruby の load-once 意味論で残りは
処理され、生成 DAG は 08 §循環インポートに従い acyclic。

ホストアプリの `Gemfile` と `$LOAD_PATH` 設定は 05 §埋め込みで
文書化する。

## バージョニングと呼出規約

ランタイム gem のバージョンは生成コードとランタイム間の呼出規
約を pin する。01 §バージョニングと Ruby ターゲットに従う：

- 10 §データモデルへの変更（例：新しい基底型、`:tag` / `:values`
  キーの変更）は**破壊的**ランタイム変更；gem の major バージ
  ョンを bump し、既存の生成コードは再発行が必要。
- 11 §実行モデルへの変更（例：`run` のスケジューリング）でユー
  ザが観察できるものも破壊的。
- 生成コード ↔ ランタイム契約を変えない内部リファクタリングは
  非破壊的。

ランタイムは標準 gem メカニズム（`Sapphire::Runtime::VERSION`）
を通じてバージョンを宣言する；コンパイラは生成ファイルヘッダ
（02 §ファイル内容の形に従う）に対象ランタイムバージョンをスタ
ンプする。ランタイムが load 時に「すべての生成ファイルが互換版
で発行された」ことを*検証*すべきかは 03 OQ 6。

## 他文書との相互作用

- **仕様 10。** 本文書は 10 のデータモデル・例外モデルの Ruby
  側実体である。タグ付きハッシュ形、シンボルキーレコード、
  `Ordering` 特例、`RubyError` の 3 フィールドレコードはすべて
  10 に従う。ランタイムは新しいマーシャリング規則を導入しない；
  10 が固定したものを実装する。
- **仕様 11。** `Ruby.pure` / `Ruby.bind` / `Ruby.run` は 11 の
  `primReturn` / `primBind` / `run` を実体化する。スレッドモデ
  ルは 11 §実行モデルに従う。
- **仕様 12。** 12 の例題プログラム群は `Ruby` アクションを
  `run` するときに暗黙にランタイムを参照する；本文書がそれらの
  実呼出先である。
- **ビルド 02。** 出力ツリーの各ファイルの `require 'sapphire/
  runtime'`（02 §ファイル内容の形に従う）が本 gem を load する。
- **ビルド 04。** CLI（04）はランタイムを直接起動しない；ラン
  タイムは*生成コード*が Ruby 実行時に load する、ビルド時パ
  イプラインではなく。
- **ビルド 05。** ホストアプリ統合（`Gemfile` エントリ、
  `$LOAD_PATH`）は 05。

## 未解決の問い

1. **`Sapphire::Runtime` 名前空間の予約。** コンパイラは `Runtime`
   という名のユーザモジュールを静的に拒絶し、`Sapphire::Runtime`
   と衝突しないようにする。より強いメカニズム（例：ランタイム
   gem が名前空間を凍結する）は現在予定なし。Draft：コンパイラ
   側チェックで充分。実装フェーズへ委譲。

2. **`Marshal` 用の型引数符号化。** `to_ruby` / `to_sapphire`
   補助は Sapphire 型を入力に取る；その型をランタイムでどう表現
   するか（plain シンボルタグ、コンパイラ発行定数、AST リテラ
   ル）は未決。Draft：実装が選ぶ。委譲。

3. **`:=` スニペット本体：リテラル `proc` vs `eval` する文字
   列。** コンパイラはスニペットの Ruby ソースを、事前 compile
   された Ruby `proc`（`Proc.new { ... }` でソースを運ぶ）か、
   呼出ごとにランタイムが `eval` する `String` のいずれかとし
   て発行できる。Draft：性能・予測可能性の双方からリテラル
   `proc` を選好；遅延束縛ソースが必要になれば見直し。委譲。

4. **スレッドプール vs `run` ごとの新規スレッド。** 11 §実行モ
   デルに従い、ステップごとスコープ隔離を保証するならランタイム
   はスレッドをプールしてよい。Draft：v0 では `run` ごとに新規
   スレッド；測定コストが要求してくればプール。委譲。

5. **`StandardError` vs `Exception` の捕捉幅。** ランタイムは
   `StandardError` を捕捉し `Interrupt` / `SystemExit` を伝搬さ
   せる。より厳しい「すべての `Exception` を捕捉」ポリシーはユー
   ザの `Ctrl-C` を `RubyError` として表面化させてしまい、これ
   は誤り；より緩いポリシーは `NoMemoryError` を境界外へ脱出さ
   せ、これは 10 が禁じる。Draft：`StandardError`。境界の sanity-
   check としてのみ委譲；blocker ではない。
   *2026-04-18 決定*: `StandardError` のみ捕捉で合意。同じ変更で
   10 §例外モデル の絶対表現「未捕捉のまま伝搬しない」をユーザ
   レベル（`StandardError` 以下）の例外に狭めた。システムレベル
   例外（`Interrupt`・`SystemExit`・`NoMemoryError`・
   `SystemStackError` など）は設計上境界を通り抜ける。ビルド側
   の tension は解消。

6. **load 時のランタイムバージョン検証。** 各生成ファイルの由
   来コメントは対象ランタイムバージョン名を持つ（02 §ファイル内
   容の形に従う）。ランタイムはそれらヘッダを読んで（あるいはコ
   ンパイラがバージョンチェック呼出を発行することで）互換性を
   強制し得る。Draft：v0 ではランタイム側強制なし；Bundler /
   gemspec 制約に頼る。委譲。

7. **非 Sapphire 呼出元向けの公開 Ruby API。** ホストアプリは
   生成コードを経ずに直接 Sapphire 側 ADT 値を構築したい場合が
   ある（例：`Just 3` を Sapphire 関数に渡す）。`ADT.make` 補助
   は技術的にこれを許すが、洗練された API ならコンストラクタを
   シンボリックに名付ける（`ADT.just(3)`）。Draft：
   `ADT.make(:Just, [3])` がサポート形；糖衣はあとから可。委譲。
