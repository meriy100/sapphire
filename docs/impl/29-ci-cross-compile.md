# 29. D2: CI platform matrix と release-build ワークフロー

本文書は Track D の **D2**（`docs/impl/06-implementation-roadmap.md`）
に対応する設計メモである。D1（`docs/impl/12-packaging.md`）が出した
配布設計草案のうち、「platform 毎の Rust バイナリを GitHub Actions
上でどうビルドするか」を具体化する。D3（tag → `gem build --platform`
→ rubygems.org push → GitHub Release）に繋ぐ配送パイプラインの
骨組みを本 D2 で整える。

## スコープ

含む：

- `.github/workflows/release-build.yml` の追加。5 platform の
  native runner matrix で `cargo build --release` を回し、
  生成物を `actions/upload-artifact@v4` で保存する。
- `docs/open-questions.md` の I-OQ5 / I-OQ32 を本 D2 決定に沿って
  更新。
- 本設計メモ（29-ci-cross-compile.md）の追加。

含まない（D3 以降）：

- `gem build --platform` / rubygems.org への push / OIDC trusted
  publisher（D3、I-OQ29 / I-OQ30 / I-OQ31 の決着と連動）。
- GitHub Release への artifact の attach（D3、tag push 時に build
  出力をそのまま release asset 化）。
- `cargo-zigbuild` / `cross` crate を使った非 native cross。
  native runner で賄える範囲に限定する（§ツール選択参照）。
- コード署名 / SBOM（I-OQ31、D3 の領分）。
- CLI gem 自体の gem 化。本 PR では I7 の `sapphire` bin が存在
  しない場合は `sapphire-lsp` を暫定 target として matrix に通し、
  配送パイプラインの骨格だけ確認する（§暫定 target）。

## 既存 CI との役割分担

- `.github/workflows/ci.yml`（既存）は **継続的テスト** 用。
  `push` / `pull_request` 契機で `ubuntu-latest` 1 job のみで
  `cargo check / fmt / clippy / test` と `runtime/` の rspec を
  回す。I-OQ5 I2 時点の DECIDED 方針（`ubuntu-latest` 単独）を
  維持する。PR ごとに 5 platform 回すのは時間とコストがかかる
  ため、日常 CI では採らない。
- `.github/workflows/release-build.yml`（本 D2 で新設）は
  **リリース build** 用。`workflow_dispatch`（手動トリガ）と
  `push: tags: ['v*']`（tag push）で発火し、D1 で draft した
  platform matrix を回す。D3 で追加される「gem build → rubygems
  push / GitHub Release 添付」ステップはこのワークフローに
  継ぎ足していく。

PR 段階で platform 依存の build failure を拾いたい場合は、
`workflow_dispatch` を手動で踏めば任意の branch を platform
matrix に通せる。常時回さないこと自体が選択である。

## Platform matrix

D1 §3 の候補表から、**native runner で回せるもののみ** を本 D2 で
採用する。

| Target triple                | Runner             | 備考 |
|------------------------------|--------------------|------|
| `x86_64-unknown-linux-gnu`   | `ubuntu-latest`    | D1 §3 推奨、Linux first-class。 |
| `aarch64-unknown-linux-gnu`  | `ubuntu-24.04-arm` | 2024-10 GA の GitHub arm64 Linux runner。QEMU / `cross` 不要。 |
| `x86_64-apple-darwin`        | `macos-13`         | Intel Mac runner。`macos-13` は Intel を明示的に選ぶために固定版を指定。 |
| `aarch64-apple-darwin`       | `macos-14`         | Apple Silicon runner。`macos-14` は M1 世代を明示的に選ぶために固定版を指定。 |
| `x86_64-pc-windows-msvc`     | `windows-latest`   | Windows first-class。 |

計 **5 platform**。D1 §6 の Wave 2a / 2b / 2c を本 D2 でまとめて
入れる方針（user 要件：最終射程を一度に揃える）。

D1 §3 で挙げた以下は **本 D2 の matrix から外す**：

- `aarch64-pc-windows-msvc`：GitHub Actions の native arm64 Windows
  runner が 2026-04 時点では一般提供前。best-effort 扱い
  （I-OQ32）。`cargo-zigbuild` で cross build する選択肢は残るが、
  本 D2 は native-only に閉じる。
- `x86_64-unknown-linux-musl`（Alpine）：fallback platform = ruby
  + source build で D3 でも間に合う（D1 §3）。
- `aarch64-linux-android` / BSD 系：Ruby ユーザの主戦場から外れる。
  需要が観測されてから個別に検討。

`macos-13` / `macos-14` を `macos-latest` ではなく固定版にしている
のは、GitHub 側の `macos-latest` が Apple Silicon 側に段階的に
移行する過渡期にあるため、target triple と runner の対応を明示的
に保つ方が壊れにくいという判断による。将来 `macos-latest` が
安定して `arm64-darwin` を指すようになれば統一を検討する。

## ツール選択：native runner 優先

D1 §3 で挙げた 3 択：

1. **native runner のみ**（本 D2 採用）。cross build ツール不要で、
   matrix job の数 = runner 種別の数で済む。Rust toolchain の
   管理も `rust-toolchain.toml` + `rustup show` で統一できる。
2. `cargo-zigbuild`：glibc の target version を明示できる（例
   `x86_64-unknown-linux-gnu.2.17`）。CentOS 7 / Ubuntu 18.04
   相当の古い Linux まで走らせたいときに有用。本 D2 の射程では
   `ubuntu-latest` / `ubuntu-24.04-arm` の glibc で十分。
3. `cross` crate：QEMU ベース。Docker 依存で tool chain 管理の
   負担が大きい。Windows arm64 など native runner が無い target
   でどうしても build したいときの最後の手段。

**本 D2 の方針**：native-only。`cargo-zigbuild` / `cross` の導入
は M9 以降の拡張として保留（I-OQ29 / I-OQ32 の決着と連動）。
古い glibc 対応が必要になった時点で D2 の延長として
`cargo-zigbuild` を足す経路は残る。

## MSRV との整合

- `rust-toolchain.toml` が `1.85.0` + `rustfmt` + `clippy` を pin。
  各 job の `rustup show` で当該 version が materialize される
  （既存 `ci.yml` と同じ方式）。
- matrix job は build だけなので `rustfmt` / `clippy` は呼ばない。
  これらは `ci.yml` 側で PR 契機に既に走っている。release-build
  側で再度回すのは冗長。
- `rustup show` は rust-toolchain.toml の存在を前提に動作する。
  release-build.yml 側では target triple を明示的に install する
  必要がない（native runner 上で native target を使うため）。

cross build を将来入れる場合は `rustup target add <triple>` が
必要になる。現時点では全 job が native build なので不要。

## 暫定 target：`sapphire-lsp`

I7（CLI 実装）は本 D2 と並行作業中で、`sapphire` bin（`crates/
sapphire-compiler/src/bin/sapphire.rs` もしくは相当）は本 D2 の
時点では未実体化。release-build.yml は `cargo build --release
--bin ${SAPPHIRE_RELEASE_BIN}` の形で bin 名を env で切り替え可能
にしており、初期値は **`sapphire-lsp`** とする。

- `sapphire-lsp` を通す理由：配送パイプライン（cargo build →
  staging → archive → hash → upload-artifact）の骨格を
  I7 着地を待たずに確認したいため。LSP バイナリ自体は D2 の
  matrix を回しても配布単位にはならない（VSCode extension は
  別経路）が、Rust の `[[bin]]` として build 可能な対象として
  最も手早く使える。
- I7 着地後：`SAPPHIRE_RELEASE_BIN` の default を `sapphire` に
  差し替える。合わせて `crates/sapphire-compiler` の `[[bin]]`
  セクションが追加されていることを release-build.yml の PR で
  確認する。env 経由なので release-build.yml の変更は 1 行で済む。

`sapphire-lsp` バイナリを artifact として rubygems.org に送る
意図ではない。あくまで **build pipeline の通り道** としての
扱いであり、本 D2 の artifact は開発者が workflow_dispatch で
build pipeline の動作確認をするときの中間成果物という位置付け。

## Artifact の形

各 job は以下を artifact として upload する：

- **Unix (Linux / macOS)**：`sapphire-lsp-<target>.tar.gz`
  （`dist/sapphire-lsp-<target>/` ディレクトリを tar+gzip）
- **Windows**：`sapphire-lsp-<target>.zip`
  （`Compress-Archive` で zip 化）
- **hash**：`.sha256` を tarball / zip の隣に置く。`shasum -a 256`
  （Unix）/ `Get-FileHash -Algorithm SHA256`（Windows）で生成。
- **同梱**：`LICENSE` を archive 内に入れる。MIT の条項上
  copyright notice と license text を binary 配布に添える必要
  があるため（`docs/impl/06-scaffolding.md` §ライセンス）。

artifact 名は matrix 行ごとに `sapphire-<target>` で upload-
artifact に渡す。`actions/upload-artifact@v4` は同名 artifact の
duplicate upload を禁じているので、job 単位で一意に分けて命名
する（v3 以前の "複数 job で同じ artifact 名に追記" パターンは
v4 で動かない）。

保持期間（retention）は **14 日**。日常開発で `workflow_
dispatch` を試しに踏んだときの artifact が溜まり続けないよう
短めに設定。D3 で tag push 時の artifact を GitHub Release に
attach する段階では、release 側が永続保持するので artifact の
retention は無関係になる。

## 検証方法

本 PR 段階では GitHub Actions が **実際に回せない**（merge → tag
push / workflow_dispatch が初めてのトリガ）。手元で確認するのは：

- YAML parse：`python3 -c "import yaml; yaml.safe_load(open(
  '.github/workflows/release-build.yml'))"` が parse error ゼロで
  通る。devcontainer に `pyyaml` が入っていない環境では
  `ruby -e 'require "yaml"; YAML.load_file(
  ".github/workflows/release-build.yml")'` を代替として使う
  （Ruby は runtime/ 向けに入っているため常用可能）。本 PR 提出
  時の検証はこの Ruby 経路で実施した（5 matrix entries を確認）。
- 既存 Rust test / lint への影響ゼロ：`cargo check --all` /
  `cargo fmt --all -- --check` / `cargo clippy --all-targets
  --all-features -- -D warnings` / `cargo test --workspace` が
  本 PR 前と同じ結果で green。
- runtime 側の rspec への影響ゼロ：`runtime/` は本 PR で触らない。

merge 後の最初の workflow_dispatch で：

1. 5 platform 全 job が green で終わること。
2. artifact 5 本（`sapphire-<target>` × 5）が upload されて
   ダウンロード可能なこと。
3. 各 archive 内に `sapphire-lsp`（Windows は `.exe`）と
   `LICENSE`、並びに `.sha256` ファイルが正しく入っていること。

いずれかが赤の場合、本 D2 の範囲で修正する（D3 着手前）。

## D3 への申し送り

D3 で rubygems.org に push するとき、本 release-build.yml に
以下を継ぎ足す想定：

- `gem build` job を matrix job の **後** に追加。artifact を
  `actions/download-artifact@v4` で受け取り、`exe/sapphire` に
  配置した上で `gem build sapphire.gemspec --platform <gem-
  platform>` を実行する。`Gem::Platform` 文字列と rustc target
  triple のマッピングは D1 §3 §rubygems 側の platform string の
  表を使う（例：`x86_64-unknown-linux-gnu` → `x86_64-linux`、
  `aarch64-apple-darwin` → `arm64-darwin`、`x86_64-pc-windows-
  msvc` → `x64-mingw-ucrt`）。
- `gem push` job を `gem build` 後に配置。OIDC trusted publisher
  を使い long-lived API key を CI に置かない（D1 §6）。
- `softprops/action-gh-release` 相当で tarball / zip / gem を
  GitHub Release に attach する job を並走。

`workflow_dispatch` と `tags: ['v*']` の二本立てを維持しつつ、
`gem push` / release attach は tag push のときだけ動かす（`if:
startsWith(github.ref, 'refs/tags/v')`）のが素直。

## 参照

- `docs/impl/12-packaging.md` — D1、§3 クロスコンパイル節 / §6
  CI / リリース計画
- `docs/impl/05-decision.md` — ホスト言語 Rust 決定
- `docs/impl/06-scaffolding.md` — Cargo workspace / MSRV 1.85.0 /
  `rust-toolchain.toml`
- `docs/impl/06-implementation-roadmap.md` — Track D の D1/D2/D3
- `docs/impl/08-runtime-layout.md` — `sapphire-runtime` gem 配置
- `docs/open-questions.md` — I-OQ5 / I-OQ29〜I-OQ33
