# 08. `sapphire-runtime` gem レイアウト

本文書は R1（ランタイム gem の scaffold）で確定するレイアウト上
の設計判断を記録する。ランタイムの **契約** は
`docs/build/03-sapphire-runtime.md` が与え、本文書はその契約を
Ruby gem の具体ファイル配置に落とし込むための実装判断を扱う。

状態: **active**。R2〜R6（`docs/impl/06-implementation-roadmap.md`
§Track R）が本レイアウト上にモジュール本体を積み増していく。

## スコープ

- gem の identity（名前・バージョン・Ruby バージョン要件）の固定。
- リポジトリ内の配置ディレクトリの固定。
- ファイル構造（`lib/` 下のモジュール分割、gemspec、Gemfile、
  rspec 雛形）の固定。
- テストフレームワーク・formatter の採否判断。

以下は **本文書の対象外**:

- モジュールの中身（R2〜R6 で実装する）。
- CLI (`sapphire build/run/check`) とランタイムの連携（I8）。
- ランタイムの配布パッケージング（D1〜D3）。

## Gem identity

| 項目                    | 値                                        |
|-------------------------|-------------------------------------------|
| Gem 名                  | `sapphire-runtime`                        |
| Top-level Ruby モジュール | `Sapphire::Runtime`                     |
| `require` パス          | `sapphire/runtime`                        |
| バージョン              | `0.1.0`（pre-release、R6 着地までは bump しない） |
| `required_ruby_version` | `~> 3.3`（= `>= 3.3, < 4.0`）              |
| ランタイム依存          | なし（build 03 §Gem identity） |

`docs/build/03-sapphire-runtime.md` §Gem identity と整合
（build 03 が `~> 3.3` を proposed value に挙げている）。
`docs/project-status.md` の devcontainer pin（Ruby 3.3）とも合致。
3.3 未満、および 4.x は明示的に reject する（Ruby 4.x 対応は
10-OQ6 で watching）。

バージョン規則:

- `0.1.0` は **内部骨組み** の印。R2〜R6 がすべて満たされて M9 の
  例題が通った時点で `0.1.x` の patch を上げる。メジャー bump
  (`1.0.0`) は `docs/build/03-sapphire-runtime.md` §Versioning and
  the calling convention の意味での「破壊的変更なし」が連続した
  状態で判断する。

## リポジトリ配置

リポジトリルート直下に **`runtime/`** ディレクトリを置き、その中
を 1 つの gem として完結させる。

```
sapphire/
  Cargo.toml                 # I2 で作成予定（Rust コンパイラ）
  src/                       # I2 で作成予定
  runtime/                   # 本文書で確定
    sapphire-runtime.gemspec
    Gemfile
    Gemfile.lock             # .gitignore（後述）
    lib/
      sapphire/
        runtime.rb
        runtime/
          version.rb
          adt.rb
          marshal.rb
          ruby.rb
          ruby_error.rb
          errors.rb
    spec/
      spec_helper.rb
      version_spec.rb
    README.md
    .rspec
  docs/
```

Cargo workspace（I2 で決定）は Rust 側のみをメンバーとし、
`runtime/` は含めない（Rust の workspace メンバーではないため）。
Ruby gem は `runtime/` 内で `bundle install` を走らせて閉じる。

### なぜ `runtime/` なのか

- `docs/impl/06-implementation-roadmap.md` §トラック が `runtime/`
  を**提案**していた。I2 がリポ全体 Cargo.toml を切る段階で `src/`
  / `runtime/` の並置になるため、ディレクトリ名の慣用も踏襲できる。
- `gem/` や `ruby/` でなく `runtime/` を採るのは、将来コンパイラを
  gem 配布する際の **distribution gem**（例: `sapphire` gem、D1 で
  設計）と混同しないため。`sapphire-runtime` は **生成された Ruby
  コードが依存する** gem であり、コンパイラ本体を配布する gem とは
  別物。

### ライセンス / README

- `runtime/README.md` は gem の overview を置き、`docs/build/03-
  sapphire-runtime.md` へのリンクで詳細契約に飛ばす。
- ライセンスは repo root の `LICENSE` をそのまま使う（gemspec から
  `license` フィールドで参照）。独立した `runtime/LICENSE` は置か
  ない。

## ファイル構成

### gemspec

`runtime/sapphire-runtime.gemspec` に以下を置く:

- `spec.name = "sapphire-runtime"`
- `spec.version = Sapphire::Runtime::VERSION`（`lib/sapphire/
  runtime/version.rb` を require）
- `spec.authors = ["meriy100"]`, `spec.email = ["kouta@meriy100.com"]`
- `spec.summary` = 1 行要約、`spec.description` = 3〜5 行
- `spec.required_ruby_version = "~> 3.3"`（build 03 §Gem identity
  の規範と gemspec 実体を揃える。`>= 3.3, < 4.0` と等価）
- `spec.files` は `git ls-files -z` ベース（gem bundler の慣習）
- `spec.license = "MIT"`（repo root の LICENSE と一致）
- 開発依存: `rspec ~> 3.13`（後述）

`spec.required_rubygems_version` や signing key は v0 では設定
しない（D1 の配布設計で決める）。

### `lib/` 階層

`lib/sapphire/runtime.rb` を唯一のエントリポイントとし、残りを
**`lib/sapphire/runtime/<name>.rb`** に分割する:

| ファイル                                    | 中身                      | 実装タスク |
|---------------------------------------------|---------------------------|------------|
| `lib/sapphire/runtime.rb`                   | 全 sub-module を `require` するエントリ | R1（本タスク） |
| `lib/sapphire/runtime/version.rb`           | `VERSION = "0.1.0"` 定数 | R1 |
| `lib/sapphire/runtime/adt.rb`               | タグ付きハッシュヘルパ    | R2 |
| `lib/sapphire/runtime/marshal.rb`           | Sapphire ↔ Ruby 変換     | R3 |
| `lib/sapphire/runtime/ruby.rb`              | `Ruby` monad 評価器      | R4 |
| `lib/sapphire/runtime/ruby_error.rb`        | `RubyError` 構築ヘルパ   | R5 |
| `lib/sapphire/runtime/errors.rb`            | ランタイム内部例外階層    | R1 で最小実装、R3 で拡張 |

R1 では `errors.rb` のうち `Base` / `MarshalError` / `BoundaryError`
の 3 クラスだけを空のサブクラスとして定義する（`build 03 §Errors
namespace` に列挙された最小集合）。これは stub ではなく **正式定義**
で、R3 が raise 側を実装するまで意味のある使われ方をしない。それ
以外のファイル（`adt.rb` / `marshal.rb` / `ruby.rb` / `ruby_error.rb`）
は `# TODO: implement in RN` コメント付きの空モジュールを置く。

### `lib/sapphire/runtime.rb` の中身方針

```ruby
require "sapphire/runtime/version"
require "sapphire/runtime/errors"
require "sapphire/runtime/adt"
require "sapphire/runtime/marshal"
require "sapphire/runtime/ruby_error"
require "sapphire/runtime/ruby"

module Sapphire
  module Runtime
  end
end
```

R2〜R6 の実装が進んでも、この entrypoint が `require` する順序は
変えない（`docs/build/03-sapphire-runtime.md` §Loading and `require`
order より、順序は契約の一部ではないが、依存関係が明らかになる
ファイル配置にしておくと読み手に親切）。

## テストフレームワーク

**`rspec` を採用する。**

理由:

- Ruby コミュニティの de facto スタンダード。AI エージェントを含め
  て参照資料・事例が最も豊富。
- `sapphire-runtime` は gem 単体で完結するテストスコープ（コンパイラ
  側とのリンクは M9 統合で別に見る）なので、`minitest` vs `rspec` の
  実行モデル差は有意な判断要素にならない。
- `docs/spec/` / `docs/build/` に書かれている境界契約（marshalling
  の全ケース網羅、例外 boundary 挙動）は `describe`/`context`/`it`
  で自然に表現できる。

`runtime/spec/spec_helper.rb` は最小構成（`require "sapphire/
runtime"` と `RSpec.configure` に disable monkey patching）。
`runtime/.rspec` には `--require spec_helper` と `--format
documentation` を置く。

R1 では **smoke test** 1 本のみを入れる:

- `runtime/spec/version_spec.rb` — `Sapphire::Runtime::VERSION` が
  `"0.1.0"` の文字列であることを assert。`require "sapphire/
  runtime"` が raise しないことの事実上の確認も兼ねる。

R2 以降はファイル単位の spec（`adt_spec.rb`, `marshal_spec.rb`,
…）を追加していく。

## Rubocop / formatter

**R1 では採用しない。** 根拠:

- `docs/impl/` 側で Ruby コードスタイルに関する OQ が発生していな
  い。early bike-shedding を避けたい。
- Rust 側は I2 で `rustfmt` / `clippy` が入るため、Ruby 側も似た
  規格を入れたくなる気持ちは分かる。ただし R2 以降でモジュール本体
  が書かれてからのほうが「どこが style の焦点か」の判断が付きやすい。
- rubocop を入れると `.rubocop.yml` の初期値（rules on/off）で
  ディスカッションが長引きがち。不要な摩擦を R1 段階では避ける。

採否は OQ として `I-OQ12` で登録し、R2 の PR レビュー時に再評価する。

## Bundler 使用

`runtime/Gemfile` は以下のパターンを採る:

```ruby
source "https://rubygems.org"
gemspec
```

`gemspec` ディレクティブにより、開発依存（rspec）は gemspec 側で
宣言したものが bundler に自動で引き継がれる。`Gemfile` に gem を
重複宣言しない。

`runtime/Gemfile.lock` は `.gitignore` に追加しない（repo root の
`.gitignore` 内 `/.bundle/` は Bundler の設定ディレクトリを除外する
意図で、lockfile は別問題）。ただし **R1 では `Gemfile.lock` を
commit しない**方針でスタートし、D1（gem 配布設計）の際に再度方針を
確認する。これは「`sapphire-runtime` gem は本体が配布されるのでは
なく、開発 repo では lockfile を固定しないほうがバージョン幅を
探索しやすい」からで、gem としてリリースする立場ならば lockfile
を commit しないのが Ruby 慣習に沿う。

## CI 連携

R1 時点では CI（`.github/workflows/`）を追加**しない**。理由:

- I2（Rust scaffold）側で CI を整える計画（`docs/impl/06-
  implementation-roadmap.md` §Track I I2）。Ruby 側 CI step は
  I2 の CI が出来てから、その YAML の中に `bundle install` +
  `bundle exec rspec` を 1 ジョブとして追加するほうが集約しやすい。
- R1 の scaffold 完了時点では smoke test が 1 本のみで CI の
  coverage 的恩恵が薄い。

Ruby 側 CI の整備は **R2 完了時点** で行う（OQ 不要。R2 で
Sapphire::Runtime::ADT の本体が入ってテスト数が増えてから CI 化
する）。

## 新規 OQ

本文書の判断から派生する OQ を `docs/open-questions.md` §1.5 に
`I-OQk` で登録する:

- `I-OQ12`: rubocop / standard-ruby 等 formatter の採否。R1 で保留、
  R2 再評価。
- `I-OQ13`: `runtime/` を Rust の Cargo workspace の別 crate 扱い
  にすべきか（現状は workspace 外）。D1 の配布設計で再訪。

（上記以外は `docs/build/03-sapphire-runtime.md` §Open questions
の B-03-OQ1〜OQ7 にすでに整理済みなので重複登録しない。）

## 他文書との関係

- **`docs/build/03-sapphire-runtime.md`**: ランタイムの **契約**。
  本文書はその契約を Ruby gem のディレクトリ配置に落とす。契約自体
  は変更しない。
- **`docs/impl/06-implementation-roadmap.md`**: R1 の完了条件を
  give している。本文書は R1 の中で決めるべき「運営判断」を文書化
  する役目。
- **`docs/impl/07-lsp-stack.md`**: L0 で作成される、別 track の
  同格文書。番号は 06 (roadmap) / 07 (LSP stack) の後ろに連番で
  08 を取る。
