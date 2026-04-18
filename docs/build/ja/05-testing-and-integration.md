# 05. テストとホスト統合

状態: **draft**。03（ランタイム gem）と 04（CLI）に対するパイプ
ライン水準の伴走文書。本文書はホスト Ruby アプリが compiled
Sapphire コードをどう呼ぶかを扱う：RSpec / Minitest からの生成
クラスのテスト、Rails / Sinatra / 素 Ruby プロジェクトへの生成
ツリーの埋め込み、Bundler 統合、生成ツリーを Ruby gem として公
開する任意の経路。

本文書は `docs/build/05-testing-and-integration.md` の日本語訳
である。英語版が規範的情報源であり、節構成・コードブロック・番
号付き未解決の問いは英語版と一致させて保つ。

## 守備範囲

範囲内：

- Ruby 単体テストフレームワーク（RSpec、Minitest）からの
  `Sapphire::*` クラスの呼出。
- ホスト Ruby アプリ（Rails アプリ、Sinatra サービス、素 Ruby
  スクリプト）への生成ツリーの埋め込み。
- Bundler 統合（`Gemfile`、`bundle install`、`bundle exec`）。
- 任意：下流消費のためのバージョン化された gem としての生成
  ツリーの公開。

範囲外：

- Sapphire コンパイラ自身のテスト実行（コンパイラ実装と共に住
  み、`docs/impl/` に）。
- Sapphire 側のテストフレームワーク（Sapphire が独自の pure 側
  テスト機構を出荷するかは言語水準の問いであり、ビルドパイプラ
  インのものではない）。
- 継続的統合プロバイダ設定（GitHub Actions 等） — Ruby プロジ
  ェクト全体で共通であり、Sapphire 固有ではない。

## Ruby から生成 Sapphire を呼ぶ

10 §生成 Ruby モジュール形（および 02 §ファイル内容の形）に従
い、各エクスポート Sapphire 束縛 `name` は leaf モジュールのク
ラスメソッドになる：

```ruby
# Sapphire 側：src/Data/List.sp
#   module Data.List ( map, sum ) where
#     map : (a -> b) -> List a -> List b
#     map f xs = ...
#     sum : List Int -> Int
#     sum xs = ...

# 生成：gen/sapphire/data/list.rb
require 'sapphire/runtime'

module Sapphire
  module Data
    class List
      def self.map(f, xs)
        ...
      end

      def self.sum(xs)
        ...
      end
    end
  end
end
```

Ruby 呼出元はメソッドを直接起動する：

```ruby
require 'sapphire/data/list'

Sapphire::Data::List.sum([1, 2, 3])
# => 6

doubled = Sapphire::Data::List.map(->(x) { x * 2 }, [1, 2, 3])
# => [2, 4, 6]
```

呼出規約の備忘（これらは 10 で規範であり、ここでは再記述しな
い）：

- 関数値は Ruby `Proc` / `Lambda` として渡る（10 §関数）。カリー
  化された Sapphire 関数は returning-lambda として現れる。
- ADT 値はタグ付きハッシュ形を使う（10 §ADT）；`Maybe Int` は
  `{ tag: :Just, values: [3] }` か `{ tag: :Nothing, values: [] }`。
- `Ordering` は素のシンボル `:lt` / `:eq` / `:gt`（10 §Ordering）。
- `Ruby a` エクスポート束縛は遅延アクションを返す；Ruby 呼出元
  は `Sapphire::Runtime::Ruby.run(...)` で駆動して `Result
  RubyError a` 形のタグ付きハッシュを得る（11 §`run` および 03
  §`Ruby` モナド評価器に従う）。

## RSpec でテスト

Sapphire プロジェクトの典型的な RSpec レイアウト：

```
my-project/
├── sapphire.yml
├── src/
│   └── Data/
│       └── List.sp
├── gen/                   # `sapphire build` で生成
├── spec/
│   ├── spec_helper.rb
│   └── data/
│       └── list_spec.rb
└── Gemfile
```

`spec/spec_helper.rb`：

```ruby
require 'sapphire/runtime'

# 生成ツリーを $LOAD_PATH に追加。
$LOAD_PATH.unshift(File.expand_path('../gen', __dir__))

# 全生成モジュールを事前 load。各 spec ファイルが必要なものだけ
# require する代替もある。
Dir[File.expand_path('../gen/sapphire/**/*.rb', __dir__)].sort.each do |f|
  require f
end
```

`spec/data/list_spec.rb`：

```ruby
require 'spec_helper'

RSpec.describe Sapphire::Data::List do
  describe '.sum' do
    it 'sums an empty list to 0' do
      expect(described_class.sum([])).to eq(0)
    end

    it 'sums a non-empty list' do
      expect(described_class.sum([1, 2, 3])).to eq(6)
    end
  end

  describe '.map' do
    it 'applies a Ruby lambda to every element' do
      result = described_class.map(->(x) { x * 2 }, [1, 2, 3])
      expect(result).to eq([2, 4, 6])
    end
  end
end
```

### `Ruby a` 束縛のテスト

型が `Ruby a` の束縛（10 §`Ruby a` に従う）は遅延アクションを
返す。テストは `Sapphire::Runtime::Ruby.run` でそれを駆動し、結
果の `Result` をパターンマッチする：

```ruby
RSpec.describe Sapphire::Main do
  describe '.greet' do
    it 'returns Ok on success' do
      action = described_class.greet('world')
      result = Sapphire::Runtime::Ruby.run(action)
      expect(result).to eq({ tag: :Ok, values: [{}] })
    end

    it 'returns Err when the snippet raises' do
      action = described_class.fragile_action
      result = Sapphire::Runtime::Ruby.run(action)
      expect(result[:tag]).to eq(:Err)
      ruby_err = result[:values][0]
      expect(ruby_err[:tag]).to eq(:RubyError)
      class_name, message, _backtrace = ruby_err[:values]
      expect(class_name).to eq('RuntimeError')
      expect(message).to match(/expected/)
    end
  end
end
```

ランタイム gem は使い勝手のための機能として RSpec マッチャ（例：
`expect(action).to evaluate_to(some_value)`）を生やすかもしれな
い；それは 05 OQ 1。

### テスト前のビルド統合

テストは最新の `gen/` に対して走らなければならない。2 つの慣習
が admissible：

- **テスト前ビルドフック。** `before(:suite)` フック（RSpec）
  か `Minitest.before_run` callback（Minitest）を追加し、
  `sapphire build` をシェルアウトする：

  ```ruby
  RSpec.configure do |config|
    config.before(:suite) do
      system('sapphire build') or abort('sapphire build failed')
    end
  end
  ```

  単純で明示的；テスト走るたびに rebuild する（incremental ビ
  ルド、04 §Incremental compilation に従い、これを安く保つ）。
  v0 では推奨。

- **Rake タスク連鎖。** Rakefile で `task default: %w[sapphire:
  build spec]` を定義し、`rake` がテスト前に常にビルドするよう
  にする。すでに Rake で組織化されているプロジェクトに有用。

パイプラインが Sapphire 認識 Rake タスクライブラリを最初から
出荷すべきか（例：`require 'sapphire/rake_task'`）は 05 OQ 2。

## Minitest でテスト

Minitest 等価形は構造的に同じ：

```ruby
# test/test_helper.rb
require 'minitest/autorun'
require 'sapphire/runtime'

$LOAD_PATH.unshift(File.expand_path('../gen', __dir__))
Dir[File.expand_path('../gen/sapphire/**/*.rb', __dir__)].sort.each do |f|
  require f
end
```

```ruby
# test/data/test_list.rb
require_relative '../test_helper'

class TestSapphireDataList < Minitest::Test
  def test_sum_empty
    assert_equal 0, Sapphire::Data::List.sum([])
  end

  def test_sum_nonempty
    assert_equal 6, Sapphire::Data::List.sum([1, 2, 3])
  end

  def test_map
    assert_equal [2, 4, 6],
                 Sapphire::Data::List.map(->(x) { x * 2 }, [1, 2, 3])
  end
end
```

`Sapphire::Runtime::Ruby.run` の起動慣習は RSpec と同一；assertion
形のみが変わる。

## ホスト Ruby アプリへの埋め込み

ここでの「ホストアプリ」は、Sapphire 生成コードを呼びたい任意
の Ruby プロジェクトである：Rails アプリ、Sinatra サービス、素
Ruby CLI 等。統合形はどれでも同じ：

1. ランタイム gem を `Gemfile` に追加：

   ```ruby
   # Gemfile
   gem 'sapphire-runtime', '~> 0.1'
   ```

2. Sapphire プロジェクトの `src/` と `sapphire.yml` をホストプ
   ロジェクトの中か並びに置く（`sapphire build` がプロジェクト
   ルートから走る限り、パイプラインは気にしない）。

3. ホストアプリ起動前にビルド：ホストの deploy / boot スクリ
   プトの一部として、または `gen/` を commit された artefact と
   して扱う（後述 §バージョン管理下の `gen/` を参照）。

4. 生成ツリーを `$LOAD_PATH` に追加してから `require` し呼ぶ：

   ```ruby
   $LOAD_PATH.unshift(File.expand_path('gen', __dir__))
   require 'sapphire/main'

   Sapphire::Main.serve  # またはエントリ束縛が何であれ
   ```

### Rails

```ruby
# config/application.rb
module MyApp
  class Application < Rails::Application
    config.autoload_paths << Rails.root.join('gen')
  end
end
```

Rails の Zeitwerk autoloader はファイル名と定数名の一致を期待
する；Sapphire 出力ツリーは構築によりこれを満たす（02 §出力ツ
リーに従う：`gen/sapphire/data/list.rb` は `Sapphire::Data::List`
を定義し、snake_case パス → PascalCase 定数の標準 Zeitwerk 対
応に従う）。v0 では追加設定不要。

`gen/` を commit していない場合、Rails initializer が boot 時
にビルドを引き起こす：

```ruby
# config/initializers/sapphire.rb
unless Dir.exist?(Rails.root.join('gen', 'sapphire'))
  unless system('sapphire', 'build', chdir: Rails.root.to_s)
    raise 'sapphire build failed; cannot start application'
  end
end
```

Sapphire がこれを自動で行う Railtie を出荷すべきかは 05 OQ 3。

### Sinatra / 素 Ruby

```ruby
# app.rb
require 'sapphire/runtime'
$LOAD_PATH.unshift(File.expand_path('gen', __dir__))
require 'sapphire/api'

require 'sinatra'

get '/sum' do
  numbers = JSON.parse(params['numbers'])      # Ruby Array<Integer>
  Sapphire::Api.sum_endpoint(numbers).to_s
end
```

### バージョン管理下の `gen/`

2 つの陣営：

- **`gen/` を Git に commit する。** 利点：ホストアプリ deploy
  に Sapphire コンパイラがデプロイサーバに不要；CI もコンパイ
  ラのインストール不要。欠点：ソース編集ごとに同 commit に生成
  コード diff が出る；reviewer が変更ごとに 2 部見る。
- **`gen/` を `.gitignore`。** 利点：source-of-truth は純粋に
  Sapphire；Git history が clean。欠点：repo の各消費者が clone
  後に `sapphire build` を走らせる必要；deploy パイプラインに
  コンパイラを含める必要。

Draft：推奨プロジェクトテンプレートでは **`gen/` を `.gitignore`
する**（`gen/` は 02 §出力ツリーに従う生成出力）。commit する
代替を望むプロジェクトは単に `.gitignore` から外せばよい。`sapphire
init` 時にパイプラインが `.gitignore` エントリを生成すべきかは
05 OQ 4。

## Bundler 統合

Bundler 関連の関心は 2 つ：

1. **ホストアプリ Bundler。** ホストの `Gemfile` が
   `sapphire-runtime` を列挙；`bundle install` がそれを解決し
   lock する。標準 Ruby 慣行；Sapphire 固有なし。

2. **コンパイラ側 Bundler（該当時）。** コンパイラ自身が Ruby
   gem である場合（`docs/impl/` に委譲された可能性）、自身の
   `Gemfile` を出荷し、ランタイムから独立にインストールされる。
   ホスト言語が Ruby になっても 2 つの gem は別。

`sapphire run` が Bundler 管理プロジェクト内で起動されるとき、
ランタイム起動は `bundle exec` で包まれるべきで、load された
`sapphire-runtime` が `Gemfile.lock` でピン留めされた版になるよ
うにする。パイプラインはプロジェクトルートの `Gemfile` を検出
して内部で `bundle exec ruby ...` を選好する；これは 04 OQ 9。

## gem として生成モジュールを公開する（任意）

プロジェクトは compiled 出力を独立した Ruby gem として公開し、
下流消費者が Sapphire コンパイラを一切必要としないようにできる。
形：

1. `sapphire.yml` で `output_dir: lib/` を設定する（02 §なぜ
   `gen/` を別ツリーにするを参照）。
2. ビルド：`sapphire build` が `lib/sapphire/...` を populate。
3. `lib/**/*.rb` を含み、ランタイム依存として `sapphire-runtime`
   を宣言する標準 `*.gemspec` を書く。
4. `gem build && gem push`。

下流消費者の `Gemfile` は 1 行：

```ruby
gem 'my-sapphire-module'
# （依存経由で sapphire-runtime を推移的に引き込む）
```

下流消費者のコード：

```ruby
require 'sapphire/main'
Sapphire::Main.do_thing
```

この経路は**任意**として文書化する。既定ワークフローは「compiled
コードはホストプロジェクトの `gen/` に住み、ランタイム gem が唯
一の third-party 依存」。同じ compiled モジュールが複数のホス
トプロジェクトで消費されるときに gem 公開は適切。

Sapphire 固有 gem 公開ヘルパ（例：`sapphire gem-build`）は 05
OQ 5。

## 性能と warm-up

ランタイム評価器（03 §スレッドモデルに従う）は各 `run` で新規
Ruby スレッドを spawn する。ホストアプリが hot path で
`Sapphire::Runtime::Ruby.run` を頻繁に呼ぶ場合、スレッド spawn
コストが効く可能性がある。パイプライン水準のコミットメント：

- ランタイム gem はステップごとスコープ隔離（11 §実行モデル項
  目 4 に従う）を保つ限り、スレッドをプール（03 OQ 4 に従う）し
  てよい。
- 純 Sapphire 計算（`Ruby a` アクションを含まないもの）はラン
  タイム評価器を起動しない；通常の Ruby として走る。Pure 側
  hot path はスレッドオーバヘッドを見ない。
- ランタイムが「warm up」API（スレッドプールを事前 spawn）を
  露出すべきかは 05 OQ 6。

## 他文書との相互作用

- **仕様 10。** 呼出規約備忘（関数値が `Proc`、ADT がタグ付き
  ハッシュ、`Ordering` がシンボル）は 10 で規範；本文書は Ruby
  側の使い勝手を示すのみ。
- **仕様 11。** テストとホストコードの `run` 起動慣習は 11
  §`run` に従う。
- **ビルド 02。** `$LOAD_PATH.unshift('gen')` + `require
  'sapphire/X/Y'` のパターンが効くのは、出力ツリーの形（02 に
  従う）が Ruby の `require`-vs-名前空間慣習と並ぶから。
- **ビルド 03。** ランタイム gem の公開 API（`Sapphire::
  Runtime::Ruby.run`、`Sapphire::Runtime::ADT`、
  `Sapphire::Runtime::Marshal`）がテストコードとホストコードが
  参照する先。
- **ビルド 04。** テストフックが shell out する `sapphire
  build` 起動と `bundle exec` 包みは 04 に従う。

## 未解決の問い

1. **ランタイム gem の RSpec 認識マッチャ。** `sapphire-runtime`
   が `require 'sapphire/runtime/rspec'` モジュールを出荷し
   `evaluate_to`、`raise_ruby_error_of_class` のようなマッチャ
   を露出すべきか。Draft：v0 ではなし — 素の `expect(result
   [:tag]).to eq(:Ok)` で acceptable。委譲。

2. **Sapphire 認識 Rake タスクライブラリ。** `require
   'sapphire/rake_task'` で `Sapphire::RakeTask.new` を露出し、
   `sapphire:build`、`sapphire:check` 等を定義する。Draft：v0
   ではなし；ユーザは手動で `system('sapphire build')` を配線。
   委譲。

3. **自動ビルド向け Rails Railtie。** `sapphire-rails`（または
   統合）gem で、`gen/` がない場合に Rails boot 時に
   `sapphire build` を引き起こす。Draft：v0 ではなし；ユーザは
   initializer を手動で追加。委譲。

4. **`sapphire init` プロジェクトテンプレート。** `sapphire
   init` サブコマンドが `sapphire.yml`、`src/Main.sp`、
   `Gemfile`、`.gitignore`、`spec/spec_helper.rb` を scaffold
   する。Draft：v0 ではなし。実装フェーズへ委譲。

5. **`sapphire gem-build` 包装ヘルパ。** `*.gemspec` を書き、
   gem を build し、release 用に包装するサブコマンド。Draft：
   v0 ではなし。委譲。

6. **ランタイム warm-up API。** Ruby 評価スレッドを事前 spawn
   する `Sapphire::Runtime::Ruby.warm_up(pool_size:)` 呼出。
   Draft：v0 ではなし。委譲。

7. **同時ホストリクエストからの `Sapphire::Runtime::Ruby.run`
   のスレッド安全性。** Rails アプリは複数の Puma worker スレ
   ッドから同時に `run` を呼ぶ可能性がある。11 のランタイム契
   約は「`run` ごとに新規スレッド」だから、同時呼出は独立であ
   るべき — だが、ランタイムのプール化スレッド変種（03 OQ 4）
   は明示的に同時プール checkout を扱う必要がある。Draft：v0
   serial 実装は trivially スレッド安全（共有可変状態なし）；
   プール化変種は追加時に並行性を慎重に文書化する。委譲。

8. **グローバル Ruby 状態に触る `Ruby` スニペットのテスト隔
   離。** `$globals`、autoload された定数、`Thread.current` を
   変異させるスニペットは、ステップごとスコープ隔離（11 §実行
   モデル項目 4 はローカルのみを隔離し、グローバルは隔離しな
   い）に関わらずテスト間でリークする。パイプライン水準の推奨：
   そのようなスニペットは anti-pattern；それに依存するテスト
   は明示的に状態を reset しなければならない。ランタイムが「プ
   ロセス隔離」run モード（`run` ごとに subprocess）を提供で
   きるかは v0 には高すぎる；委譲。
