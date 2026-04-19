# 12. D1: Rust コンパイラ + Ruby ランタイム gem の配布設計調査

本文書は Track D の **D1**（`docs/impl/06-implementation-roadmap.md`）
に対応する **研究メモ** であり、非規範である。最終的な配布形態は
D2（CI クロスコンパイル）および D3（初回リリース）で user 確認の
うえ決定する。ここで扱うのは「どの選択肢があり、どういう軸で比較
できるか」「既存 Ruby 生態系での先例」「決着が必要な残論点」まで。

## スコープと前提

対象：

- Rust で実装されるコンパイラバイナリ `sapphire`（`docs/impl/05-
  decision.md` §リポジトリ構造、I7/I8 で実体化）。
- 既存の Ruby gem `sapphire-runtime`（`docs/impl/08-runtime-layout.
  md` §Gem identity、R1〜R6 で実体化）。
- 生成 Ruby コードが期待する契約（`docs/build/03-sapphire-runtime.
  md` §Versioning and the calling convention、02 §File-content
  shape のヘッダ）。

前提：

- ホスト言語は Rust、MSRV **1.85.0**（`docs/impl/06-scaffolding.md`
  §MSRV）。
- ランタイムの最低 Ruby は **3.3**（`docs/impl/08-runtime-layout.md`
  §Gem identity、B-01-OQ1 / 10-OQ6 の pin 方針に整合）。
- CI ランナは GitHub Actions 既定（I-OQ5）。D1 段階では matrix
  構成は決めず、D2 へ申し送る。
- 本 D1 は **コードを変更しない**。成果物はこの文書と、必要なら
  `docs/open-questions.md` への追記のみ。

非対象：

- Homebrew tap / Docker image 配布、Nix package。これらは将来検討
  （少なくとも D1〜D3 のスコープ外）。
- source build 経路の廃止。ユーザが `cargo install --path
  crates/sapphire-compiler` でソースから構築する経路は残す
  （precompiled binary 配布が主、source は二次経路）。
- `cargo install sapphire` による Rust 単独配布を Ruby ユーザの
  主経路にすること（Ruby ユーザの想定操作から外れる）。
- `ruby-sapphire` のようにコンパイラを Ruby から FFI で呼ぶ形
  態。本プロジェクトのコンパイラは子プロセス呼び出しで十分であり、
  Ruby ABI に縛られる動機がない（§3 で再掲）。

## 1. 目標と制約

### 1-install 体験

user が想定する手順は以下：

```
$ gem install sapphire           # CLI コンパイラ + runtime の両方が入る
$ sapphire build hello.sp        # .sp → .rb
$ sapphire run hello.sp          # compile + run をワンショット
```

この体験を第一要件とする。`gem install` 以外の事前手順（rustup・
cargo・homebrew）が **必要にならない** ことが肝心。Ruby 3.3+ が
入っている Ruby ユーザの手元に追加依存を持ち込まない。

### バージョン整合

- Rust **1.85.0** 以上でビルドされたバイナリを配る。実行時 Rust は
  不要（ネイティブバイナリのため）。
- Ruby **3.3** 以上で `sapphire-runtime` が動く。`required_ruby_
  version = "~> 3.3"`（`docs/impl/08-runtime-layout.md`）。
- 生成された `.rb` が `require 'sapphire/runtime'` したとき、
  runtime gem の version が CLI と整合する必要がある
  （`docs/build/02-source-and-output-layout.md` §File-content
  shape のヘッダ、`docs/build/03-sapphire-runtime.md` §Versioning
  and the calling convention）。

### 対応プラットフォーム（射程）

実装は段階的でよい（D2 での wave 割りは後述 §6）。最終射程：

| OS      | x86_64  | arm64 (aarch64) |
|---------|---------|-----------------|
| Linux   | 必須    | 必須             |
| macOS   | 射程内  | 必須（Apple シリコン） |
| Windows | 射程内  | best-effort      |

Linux 用 gem は **glibc 依存**でビルドする既定（後述 §3）。musl
（Alpine）は best-effort。

### gem 配布先

**rubygems.org 一択**。独立の "tap" 相当（独自 gem index）は不要。
OIDC trusted publishers による GitHub Actions からの push を想定
（§6）。

### 非目標（再確認）

- Homebrew / Docker / Nix は配らない（需要が出たら個別に検討）。
- source build を省略しない。ユーザが `cargo install --path
  crates/sapphire-compiler` でビルドできる経路は維持する。これは
  将来の self-host 検討 (`docs/impl/05-decision.md` §本決定の見直
  し条件) と、下記 §2 (C) 案の受け皿として効く。

## 2. 配布単位の候補

3 案を比較する。どれも「`sapphire-runtime` gem は platform = ruby
の **pure Ruby gem**」という前提は共通（runtime は Rust バイナリ
を含まない。§4 で再掲）。

### (A) 単一 gem `sapphire` がバイナリを同梱する（precompiled native gem）

gem 名 `sapphire` の platform 毎 variant を複数発行：

- `sapphire-0.1.0-x86_64-linux.gem`
- `sapphire-0.1.0-aarch64-linux.gem`
- `sapphire-0.1.0-x86_64-darwin.gem`
- `sapphire-0.1.0-arm64-darwin.gem`
- `sapphire-0.1.0-x64-mingw-ucrt.gem`（Windows）
- `sapphire-0.1.0.gem`（platform = ruby。source fallback として
  `cargo build` を走らせる extconf.rb 系、あるいは明示的な
  "platform がマッチしない" エラーを出す）

rubygems は `gem install` 時に client platform と一致する native
variant があれば優先的にそれを選ぶ。Bundler も同様。
`sapphire-runtime` への依存は `spec.add_runtime_dependency
"sapphire-runtime", "~> X.Y"` で宣言する。結果、ユーザ 1 コマンド
で両方入る。

#### 先例

- **`nokogiri`**（≥ 1.11）: native gem を platform 毎に発行する
  運用の定番。`rake-compiler-dock` を使って docker 内で cross
  build する。
- **`sqlite3-ruby`**: 同じく precompiled native gems を配る。
- **`google-protobuf`**: C 拡張の precompiled gem。
- **`sorbet`** (`sorbet-static`): Rust でなく C++ だが、platform
  毎 gem で CLI バイナリを同梱する構造。`sorbet-static-and-runtime`
  が meta gem として両方を引き込むパターンを持つ（→ (B) 案に近い
  構造）。
- **`ruby-lsp`**: 一部 native dependency（`prism`、`sorbet-static`）
  を precompiled で引っ張る。
- **`wasmer-ruby`** / **`termbox2-ruby`**: Rust を native
  extension として同梱する gem。`rb-sys` + `rb_sys::ExtConfig` +
  `rake-compiler-dock` の組み合わせで cross build している。本
  プロジェクトは Ruby から FFI する形ではない（§3 参照）が、
  cross build のパイプライン自体は参考になる。

#### ユーザの install 体験

```
$ gem install sapphire
Fetching sapphire-0.1.0-x86_64-linux.gem
Fetching sapphire-runtime-0.1.0.gem
Successfully installed sapphire-runtime-0.1.0
Successfully installed sapphire-0.1.0-x86_64-linux
```

1 コマンドで完結。想定に最も合致する。

#### CI ビルドの複雑さ

高い。platform 毎に Rust target を切り替え、native binary を `gem
build` に取り込んで platform stamp 付き gem を生成する必要がある。
Linux arm64 や Windows は GitHub Actions の runner から直接は作り
にくく、`cross` crate（QEMU 経由）または `cargo-zigbuild` でクロス
build することになる（§3 で掘り下げ）。

#### バージョン整合

`sapphire` と `sapphire-runtime` は **同時リリース** が原則。
`sapphire` の gemspec で `add_runtime_dependency "sapphire-runtime",
"= #{Sapphire::VERSION}"` と書けば、CLI と runtime の version ズレ
は gem resolver が検出する。逆に互換幅を広げたいなら `"~> X.Y"`。

CLI 起動時に `Gem.loaded_specs['sapphire-runtime']&.version`
（あるいは `Gem::Specification.find_all_by_name('sapphire-runtime')
.first&.version`）を参照してメジャーマイナー一致を確認し、ズレて
いれば警告する、という防御もできる（§5 で詳述）。

#### アップデート経路

`gem update sapphire` で両方更新される（依存に `sapphire-runtime`
が入っているため）。Bundler 利用プロジェクトは `bundle update
sapphire` でロックファイルが動く。

#### セキュリティ（署名 / SBOM）

- gem 署名：v0 では gemspec に signing key を入れない（`docs/
  impl/08-runtime-layout.md` §gemspec の記述と整合）。後続で
  sigstore / OIDC trusted publishing を検討（§6）。
- SBOM：rustc の `cargo auditable` または `cargo sbom` 系を CI で
  回せる。D2 以降で検討。
- rubygems の `mfa_required` policy（organization 単位で 2FA 必
  須化）は rubygems.org 側の設定。push 前提として user 個人 /
  organization の MFA 設定を先に有効化する。

### (B) メタ gem `sapphire` + `sapphire-compiler`（native） + `sapphire-runtime`（既存）

3 gem 構成：

- `sapphire-compiler` — Rust バイナリだけを含む native gem。
  platform variant を持つ。
- `sapphire-runtime` — 既存。pure Ruby。
- `sapphire` — メタ gem。platform = ruby。中身は
  `add_runtime_dependency "sapphire-compiler"` +
  `add_runtime_dependency "sapphire-runtime"` の 2 行だけ。

#### 先例

- **`sorbet`** / **`sorbet-static`** / **`sorbet-runtime`** /
  **`sorbet-static-and-runtime`**：`sorbet` 自身は minimal、
  `sorbet-static` は platform 毎 native、`sorbet-runtime` は pure
  Ruby、`sorbet-static-and-runtime` がメタ。5 層だが構造は本案に
  近い。
- **`grpc`** + **`grpc-tools`**：ライブラリとツールを別 gem に
  切る構造。

#### ユーザの install 体験

(A) と同じく `gem install sapphire` で 3 つ全部入る。差は不可視。

#### CI ビルドの複雑さ

(A) よりわずかに増える。push する gem が 2 系統（compiler の
platform variant 群 + runtime） + メタ 1 本。ただし各 gem の責務
が分かれるので「compiler の platform matrix だけ失敗した」と
いった部分障害の切り分けがしやすい。

#### バージョン整合

3 gem の version を揃える運用が前提。メタ gem が `= X.Y.Z` で両方
ピンすれば整合。一方、**バグ修正を runtime だけ patch リリース**
したいケースで、メタ gem を再リリースしないと依存解決が動かない
のは面倒（= でなく ~> を使えば緩む）。

#### アップデート経路

(A) と同じ。

#### セキュリティ

gem が増えるぶん署名対象が増える。trusted publisher は gem 単位で
設定するので、3 gem それぞれに OIDC の設定が要る。

### (C) `sapphire-runtime` gem のみ + CLI は Rust binary release（GitHub Release / cargo install）

- `sapphire-runtime` gem は既存のまま pure Ruby で配る。
- `sapphire` CLI は **gem として配らない**。`github.com/meriy100/
  sapphire/releases` に tarball を置き、`curl | sh` 型のインス
  トーラ、あるいは `cargo install --git` で入れる。
- Ruby ユーザは `gem install sapphire-runtime` だけする。CLI は
  別手段で入れる。

#### 先例

- Rust 由来 CLI の多くが採る形（`ripgrep`、`fd`、`cargo-nextest`）。
- **`tree-sitter`**: CLI は GitHub Releases / cargo / homebrew。
  Ruby/JS バインディングは別 gem/npm として配る。

#### ユーザの install 体験

2 ステップになる：

```
$ curl -sSfL https://.../sapphire/install.sh | sh    # もしくは cargo install
$ gem install sapphire-runtime
$ sapphire build hello.sp
```

**user の 1-install 要件に反する**。ただし暫定策として最初のリリー
スでこの形を取り、後に (A)/(B) へ移行する、という移行パスは現実
的（後述 §2 §移行パス）。

#### CI ビルドの複雑さ

低い。Rust バイナリの cross build は必要だが、gem 化の手順がない
ぶん (A)/(B) より素朴。`cargo dist` / `cargo-release` のような
Rust 側エコシステムが揃っており、tarball + checksum + GitHub
Release 作成まで 1 ツールで済む。

#### バージョン整合

gem と CLI の version を連携させる仕組みを自分で実装する必要がある。
CLI 起動時に runtime gem の version を `bundle exec` 下なら
`Gem.loaded_specs` で確認する、など。ズレ検出は可能だが、gem
dependency resolver による自動整合は効かない。

#### アップデート経路

CLI 側と gem 側を別々に更新してもらう必要がある。user experience
として劣る。

#### セキュリティ

GitHub Release の artifact は GitHub Actions の
`id-token: write` + sigstore で署名できる。rubygems 側は runtime
gem のみなので負荷が小さい。

### 比較サマリ

| 軸                          | (A) 単一 `sapphire` | (B) メタ + 3 gem | (C) runtime-only gem |
|-----------------------------|---------------------|------------------|----------------------|
| 1-install 体験              | ◯                   | ◯                | ✕（2 ステップ）      |
| CI の複雑さ                 | 高                  | やや高           | 低                   |
| バージョン整合の自動化      | ◯（resolver）       | ◯（resolver）    | △（自前）            |
| アップデート経路            | `gem update`        | `gem update`    | CLI と gem を個別   |
| runtime のみ patch release  | △                   | ◯                | ◯                    |
| 運用対象 gem 数             | 2（runtime 含む）   | 3                | 1                    |
| 構造の分かりやすさ          | ◯                   | △（5 層類比）    | ◯                    |
| 先例の厚み                  | nokogiri 級         | sorbet 型        | ripgrep 型           |

### 推奨（D2/D3 への申し送り）

**第一候補は (A)。** 理由：

1. 1-install 要件（user の最優先）を最少 gem 数で満たす。
2. 先例 `nokogiri` が確立しており、`rake-compiler-dock` / `rb-sys`
   系エコシステムの資料も豊富。
3. gem 運用対象が 2 本で済む（meta 1 + runtime 1 と比べて 3 本に
   増えるメリットが薄い）。
4. version 整合が resolver で済む。

**移行パスとして (C) を初回リリースにする妥当性もある** — 以下の
場合：

- D2 時点で cross build（特に Windows / Linux arm64）が不安定で、
  "Linux x86_64 の Rust binary だけ先に出す" という小さな一歩を
  踏みたい。
- コンパイラがまだ 0.0.x の段階（I9 未到達に近い段階）で、Ruby
  ユーザ向けのつもりが "early adopter 向けの raw binary" でも
  許される段階。

この場合、`sapphire-runtime` gem（R1 で scaffold 済）だけを先に
rubygems.org に上げ、CLI は GitHub Release で配る。その後 (A) に
寄せ直す。

ただし移行時に `sapphire-compiler` の gem 名が既に別の誰かに
squatting されている可能性は rubygems.org の性質上ある。名前予約
として空の `sapphire` / `sapphire-compiler` gem を早期に push して
占有する判断は D3 の前に必要（後述 §7 OQ）。

**最終選択は本 D1 では凍結しない。** (A)/(B)/(C) のいずれを採る
かは D2 の cross build 試行の結果と、user 判断で決める。

## 3. クロスビルドの仕組み

(A) または (B) を採るとき、platform 毎の native binary をどう作る
かが最大の論点。

### 前提：Sapphire CLI は Ruby から FFI しない

重要な切り分け：

- `nokogiri` / `wasmer-ruby` / `termbox2-ruby` は **Ruby プロセス
  内に Rust コードを loadable extension (`.so` / `.bundle` /
  `.dll`) としてロードする**。Ruby の ABI と密結合し、
  `ext/.../extconf.rb` + `rb-sys` で Ruby version 毎の ABI に
  合わせて複数 extension を作る必要がある。
- Sapphire CLI は **独立した Rust バイナリ**。`sapphire build`
  は Ruby プロセスと無関係の `execv` された子プロセス。Ruby ABI
  と結合しない。

このため `rb-sys` / `rake-compiler` / `rake-compiler-dock` の仕組み
は **「不要に複雑」になる可能性がある**。Ruby 毎に別 `.so` を作る
必要がなく、platform 毎に 1 バイナリで済む。

### 方式 X: 素朴に Rust バイナリを gem の `exe/` に置く

gemspec で：

```ruby
spec.platform      = Gem::Platform.new("x86_64-linux")
spec.files         = ["exe/sapphire"]   # 実行可能バイナリ
spec.bindir        = "exe"
spec.executables   = ["sapphire"]
```

GitHub Actions の matrix job で以下を回す：

1. `cargo build --release --target <triple>`
2. 生成された `target/<triple>/release/sapphire` を `exe/sapphire`
   に配置
3. `gem build sapphire.gemspec` で platform variant を作成
4. artifact として保存 / rubygems.org に push

これは **ABI 依存のない "コピーするだけ" native gem** という発想。
Ruby version 非依存のため、1 つの platform variant が全 Ruby
version で動く（3.3 も 3.4 も）。

先例：

- 大手の確立された「CLI-only native gem」は `sorbet-static` が
  代表格（C++ 由来だが構造は同じ：`libexec/` 相当にプラット
  フォーム別 CLI バイナリを同梱、pure Ruby コードを持たない native
  gem）。Rust 由来でこの方式を取っている gem は（`rb-sys` を使わ
  ない形の CLI-only 配布として）数は多くないが、gemspec と CI
  構成の単純さから個人開発者が最初に到達するパターンでもある。
- 本方式が通用する理由は「Ruby ABI に依存しないため Ruby version
  毎 artifact を作る必要がない」点にある（`rb-sys` 方式との主要な
  差別点、方式 Y で後述）。

懸念点：

- `Gem::Platform` のマッチ規則を正しく書く必要がある（Linux で
  x86_64 vs aarch64、macOS で Darwin version、Windows で mingw-ucrt
  / x64-mingw32 の違い）。
- fallback platform variant（platform = ruby）で source build を
  提供するかどうか。提供するなら extconf.rb 相当のフックで
  `cargo build` を走らせることになる。

### 方式 Y: `rb-sys` + `rake-compiler-dock`（Ruby native extension 式）

`sapphire-compiler` gem を `ext/sapphire_compiler/Cargo.toml` 構成
にして、**Rust コードを `.so` / `.bundle` として `require` する**
形。`rb_sys::ExtConfig` が Ruby ABI に橋渡しする。

先例：

- **`wasmer-ruby`**：Wasmer（Rust）の Ruby バインディング。Ruby
  プロセス内から Wasm 実行ランタイムを呼ぶために native extension
  として loadable に作られている。
- **`oxi-png` / `rb_sys` の example crates**：`rb-sys` が想定する
  「Rust を Ruby native extension として配る」用途の代表。

懸念：

- Sapphire CLI は Ruby から load する必要がない。`.so` にする動機
  がない。それを敢えて `.so` にして `sapphire` exe から内部で Ruby
  を embed する、のような捻じれ設計は採るべきでない。
- Ruby version 毎に `.so` を作るコストが発生する（3.3 と 3.4 で別
  artifact）。Sapphire は Ruby から呼ばれないため、このコストは
  払う意味がない。

結論：**方式 Y は Sapphire の構造に合わない**。方式 X を採る。

### 方式 X でのクロスコンパイル実装

GitHub Actions matrix で native runner を使い分ける：

| Target triple              | GitHub runner          | 備考 |
|----------------------------|------------------------|------|
| `x86_64-unknown-linux-gnu` | `ubuntu-latest`        | native |
| `aarch64-unknown-linux-gnu`| `ubuntu-24.04-arm`（GA から提供）もしくは `ubuntu-latest` + `cross` / `cargo-zigbuild` | GitHub の ARM runner はパブリックリポジトリで無料利用可。2025 年以降 GA。 |
| `x86_64-apple-darwin`      | `macos-13`             | native |
| `aarch64-apple-darwin`     | `macos-14` / `macos-latest` | Apple Silicon runner |
| `x86_64-pc-windows-msvc`   | `windows-latest`       | native |

ツール選択：

- **`cargo-zigbuild`**：glibc の target version を明示できる（例
  `x86_64-unknown-linux-gnu.2.17`）。古い Linux distro でも動く
  バイナリが作れる。`cross` より依存が軽い。
- **`cross`**（crate）：QEMU ベースの cross build。Docker 依存。
  target が増えるほど tool chain 管理の負担が増える。
- **native runner のみ**：最も素朴。cross build ツール不要。
  ただし runner 種類が増える。

**推奨**：native runner を第一、arm64 Linux のみ `cargo-zigbuild`
または GitHub の arm64 runner。Windows はまず x86_64 のみ、arm64
は best-effort（§7 OQ）。

glibc 最低 version は **CentOS 7 / Ubuntu 18.04 相当（glibc 2.17）
を目指す** 方針でよい（Ruby 3.3 を動かせる現役 Linux の下限と概ね
一致）。musl (Alpine) 向けは fallback platform = ruby + source
build で対応。

### rubygems 側の platform string

`Gem::Platform.new('x86_64-linux')` のように書く。具体の値：

- Linux: `x86_64-linux`, `aarch64-linux`, `x86_64-linux-musl`（musl
  を別出しする場合）
- macOS: `x86_64-darwin`, `arm64-darwin` （version 番号は外す運用
  が `nokogiri` の既定）
- Windows: `x64-mingw-ucrt`（Ruby 3.2+）/ `x64-mingw32`（古い）。
  Ruby 3.3+ は `ucrt` のみを想定。

## 4. gemspec の形

### `sapphire` (A 案) / `sapphire-compiler` (B 案)

platform native gem の最小 gemspec：

```ruby
# (A) 案の sapphire.gemspec を例示。B 案の sapphire-compiler も構造は同じ
require_relative "lib/sapphire/cli/version"   # 小さな VERSION 定数ファイル

Gem::Specification.new do |spec|
  spec.name                  = "sapphire"       # (B) なら "sapphire-compiler"
  spec.version               = Sapphire::CLI::VERSION
  spec.required_ruby_version = ">= 3.3"         # 3.3 未満を reject
  spec.files                 = Dir["exe/*", "lib/**/*.rb",
                                   "LICENSE", "README.md"]
  spec.bindir                = "exe"
  spec.executables           = ["sapphire"]      # (B) なら同上または分離
  spec.license               = "MIT"
  # platform は CI matrix で gem build 時に --platform で上書き
  # あるいは gemspec 内で ENV["GEM_PLATFORM"] を参照する
  spec.add_runtime_dependency "sapphire-runtime", "~> #{Sapphire::CLI::VERSION}"
end
```

注意点：

- `spec.version` 用に小さな `lib/sapphire/cli/version.rb` を置き、
  `Sapphire::CLI::VERSION = "0.1.0"` を定義する。native binary は
  `exe/sapphire`、Ruby 側は version 定数だけ、という分け方。
  `Sapphire::Runtime::VERSION` とは namespace が異なるので衝突し
  ない。
- `spec.platform = Gem::Platform.new(ENV.fetch("GEM_PLATFORM",
  "ruby"))` のように runtime で差し替える運用にすると、同じ
  gemspec から全 platform variant を作れる（`nokogiri` の手法）。
- `spec.required_rubygems_version` は v0 では省略。必要が出てから。
- `Sapphire::CLI::VERSION` と `sapphire-runtime` の version を
  同期させる仕組み（単一の `VERSION` ファイルを両 gem の build
  script が読む等）を D3 前に確定する（§5、§7 I-OQ33）。

### `sapphire-runtime`

**platform = ruby のまま**。既存の `docs/impl/08-runtime-layout.md`
§gemspec の記述を維持する。Rust バイナリは一切入らない。

### CLI gem と runtime gem の役割分離

- `sapphire` (or `sapphire-compiler`) = CLI バイナリ（native）。
  `Sapphire::VERSION` は自身の version。
- `sapphire-runtime` = 生成コードが依存する Ruby ライブラリ（pure
  Ruby）。`Sapphire::Runtime::VERSION` を持つ。

両 version は同じ source-of-truth（後述 §5）から生成する。

## 5. バージョン整合性

### 基本方針

**CLI と runtime gem は同じ major.minor を常に共有**する。patch
はズレてよい（runtime 側のバグ修正 patch だけを出せる余地を残す）。

- `sapphire` 0.3.2 は `sapphire-runtime ~> 0.3.0` を要求。
- `sapphire-runtime` 0.3.0 も 0.3.5 も許容。
- `sapphire` 0.4.0 は `sapphire-runtime ~> 0.4.0`。

これは `docs/build/03-sapphire-runtime.md` §Versioning and the
calling convention が言う「calling convention（10/11 の data model
/ monad）を破るなら breaking」の原則と整合する。calling convention
を触らない patch は runtime 側だけで出してよく、そのために ~>
を採る。

### 起動時チェック

CLI 起動時に（あるいは `sapphire build` が runtime を呼び出す
段階で）、以下に類する確認を行う：

```
rt = Gem.loaded_specs['sapphire-runtime']&.version ||
     Gem::Specification.find_all_by_name('sapphire-runtime').first&.version
# rt が nil（runtime 未インストール）なら install 案内で exit。
# rt が想定範囲（例: Gem::Requirement.new("~> 0.3.0")）外なら warn / error。
```

CLI が想定する範囲（例えば `Gem::Requirement.new("~> 0.3.0")`）と
合致しない場合は警告、もしくは `--strict` フラグで error。

この check は gem resolver の整合と二重だが、user が素の Ruby で
`sapphire-runtime` を別途 install している場合（Bundler 非経由）
に有効。

### 生成コードへの stamp

`docs/build/02-source-and-output-layout.md` §File-content shape で
既に触れているとおり、生成 `.rb` の先頭コメントに `# sapphire
X.Y.Z / sapphire-runtime ~> X.Y` を焼く。将来の runtime 側 load-
time hook（B-03-OQ6）でこの header を読む設計に拡張できる。

## 6. CI / リリース計画（D2 / D3 への申し送り）

本 D1 では実装しない。D2/D3 のための方向性のみ置く。

### D2 の matrix wave

段階的に拡張する：

1. **Wave 2a**：`x86_64-unknown-linux-gnu` のみビルド + artifact
   保存。I-OQ5 で I2 時点の `ubuntu-latest` 単体 CI と接続する。
2. **Wave 2b**：`aarch64-unknown-linux-gnu`、`x86_64-apple-darwin`、
   `aarch64-apple-darwin` を追加。GitHub の arm64 runner 可用性に
   依存。
3. **Wave 2c**：`x86_64-pc-windows-msvc` を追加。
4. **Wave 2d**（optional）：Windows arm64、Linux musl。

### D3 で rubygems.org に push するもの

(A) 案採用時：

- `sapphire-0.1.0-x86_64-linux.gem`
- `sapphire-0.1.0-aarch64-linux.gem`
- `sapphire-0.1.0-x86_64-darwin.gem`
- `sapphire-0.1.0-arm64-darwin.gem`
- `sapphire-0.1.0-x64-mingw-ucrt.gem`
- `sapphire-0.1.0.gem`（fallback、platform = ruby。内容は README
  のみ + 明示的な "platform not supported" エラーで fail する
  `exe/sapphire`、あるいは source build トリガ）
- `sapphire-runtime-0.1.0.gem`

### push の自動化

**OIDC trusted publishers**（rubygems.org が 2024 以降サポート）
を利用する：

- GitHub Actions の `id-token: write` permission と rubygems 側の
  trusted publisher 設定を紐付け、**長期 API key を CI に持たせ
  ない**。
- tag push（例 `v0.1.0`）をトリガに release workflow を発火し、
  build → sign → push まで自動化。
- 手動 push を完全禁止にするかは運用判断（最初は手動 fallback を
  残す方が安全）。

### 署名 / SBOM

- gem の `--sign` は **採用しない** 方針で進める。rubygems の
  legacy signing は MFA + trusted publisher の時代には価値が薄い。
- sigstore / cosign による artifact 署名は将来検討（§7 OQ）。
- Rust 側で `cargo auditable` を有効にして binary に依存情報を
  埋める、`cargo sbom` で SBOM を別 artifact として release に
  添える、は D2 で判断。

### 先行 squatting 回避

D3 着手前に、`sapphire` および `sapphire-compiler` gem 名を
rubygems.org で予約する（空の 0.0.0 gem を push する or
`gem owner` で事前に押さえる）。rubygems の squatting policy は
運営裁量なので、こちらで先に占有するのが安全。

## 7. 残される open questions

以下を `docs/open-questions.md` §1.5 に `I-OQ29`〜`I-OQ33` として
登録する（本 commit で同時追加）。D1 は調査であり決定ではないた
め、Status はすべて `DEFERRED-IMPL`。

- **I-OQ29 単一 gem vs 複数 gem**：§2 (A) / (B) / (C) の決着。
  D2 の cross build 試行結果と user 判断で確定。
- **I-OQ30 rb-sys 方式 vs 素の Rust binary 同梱方式**：§3 方式 X
  vs 方式 Y。現時点での推奨は方式 X（CLI は Ruby から FFI しない
  ため）。native extension 化の必要が将来出たときに再訪。
- **I-OQ31 バイナリ署名・SBOM の範囲**：§6。gem `--sign`、sigstore、
  `cargo auditable`、`cargo sbom`、OIDC trusted publishers の
  どこまでを v0 で入れるか。D3 着地前に確定。
- **I-OQ32 Windows の first-class / best-effort 線引き**：§3。
  Windows x86_64 は first-class、arm64 は best-effort、という
  draft を D2 で検証する。Windows の CI 追加タイミング（Wave 2b
  vs 2c）もここで決める。
- **I-OQ33 runtime gem と CLI の version 一致ポリシー**：§5。
  「major.minor 一致、patch はズレ可」が draft。初回 release 前
  に gemspec の `add_runtime_dependency` 記法で最終確定する。

## 参照

- `docs/impl/05-decision.md` — ホスト言語 Rust 決定
- `docs/impl/06-scaffolding.md` — Cargo workspace、MSRV 1.85.0
- `docs/impl/06-implementation-roadmap.md` — Track D の D1/D2/D3
- `docs/impl/08-runtime-layout.md` — `sapphire-runtime` gem 配置
- `docs/build/01-overview.md` — pipeline overview（B-01-OQ1 の
  Ruby pin）
- `docs/build/02-source-and-output-layout.md` — 生成コードヘッダ
- `docs/build/03-sapphire-runtime.md` — runtime gem 契約、version
  整合
- `docs/build/04-invocation-and-config.md` — CLI
- `docs/build/05-testing-and-integration.md` — host 統合、Bundler
- `docs/open-questions.md` — I-OQ / B-OQ / 10-OQ6 の統括
