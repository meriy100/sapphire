# sapphire-runtime

Sapphire コンパイル済みプログラムが実行時に依存する Ruby gem。

## 位置付け

- `sapphire-runtime` は **コンパイラではない**。Sapphire コンパイラ
  （別途、リポジトリルートの Rust crate として実装される）が生成
  した Ruby コードが、実行時にこの gem を `require "sapphire/
  runtime"` して利用する。
- ホストアプリケーション側の `Gemfile` にも
  `gem "sapphire-runtime"` を追加する（Sapphire の関数を host Ruby
  から直接呼び出す場合）。

## 正規の契約

この gem の **正規の動作仕様**（モジュール公開面、marshalling 規則、
例外境界挙動、threading モデル、バージョン互換性ポリシー）は：

- [`docs/build/03-sapphire-runtime.md`](../docs/build/03-sapphire-runtime.md)
  — Ruby 側の契約（本 gem の設計文書）
- [`docs/spec/10-ruby-interop.md`](../docs/spec/10-ruby-interop.md)
  — data model および exception model
- [`docs/spec/11-ruby-monad.md`](../docs/spec/11-ruby-monad.md)
  — `Ruby` monad の意味論

に規定されている。この README と gem 内コメントは **二次的な**
説明であり、矛盾があれば上記文書が優先する。

## ディレクトリ配置とファイル構成の判断根拠

本 gem のリポジトリ配置（`runtime/`）・ファイル分割・テスト
フレームワーク採用等の判断は
[`docs/impl/08-runtime-layout.md`](../docs/impl/08-runtime-layout.md)
に記録されている。

## ロードマップ上の位置

`docs/impl/06-implementation-roadmap.md` §Track R を参照。

| ID | 内容 | 状態 |
|---|---|---|
| R1 | gemspec / lib/ 骨組み / smoke test | 本 commit で完了 |
| R2 | `Sapphire::Runtime::ADT` 実装 | 未着手 |
| R3 | `Sapphire::Runtime::Marshal` 実装 | 未着手 |
| R4 | `Sapphire::Runtime::Ruby` （monad 評価器）実装 | 未着手 |
| R5 | `Sapphire::Runtime::RubyError` 実装 + boundary rescue | 未着手 |
| R6 | 生成コードのロード契約 / バージョン検証 | 未着手 |

## 開発

Ruby 3.3 が必要。リポジトリルートの devcontainer をそのまま使う。

```bash
cd runtime
bundle install
bundle exec rspec
```

## ライセンス

リポジトリルートの [`LICENSE`](../LICENSE) に従う（MIT）。
