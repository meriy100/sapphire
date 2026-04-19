# 32. D3: 初回リリースプロセス（0.1.0 の tag → gem push → GitHub Release）

本文書は Track D の **D3**（`docs/impl/06-implementation-roadmap.md`）
に対応する **実行手順書** である。D1（`docs/impl/12-packaging.md`）
の配布設計と D2（`docs/impl/29-ci-cross-compile.md`）の release-
build workflow を前提に、初回公開リリース 0.1.0 を実際に出す
一連の user 操作をチェックリスト化する。

本手順は **user が実行する** ことを前提にしている（Claude が
rubygems.org や GitHub Releases に push / publish することはない、
`CLAUDE.md` §Git, GitHub, and out-of-scope systems）。Claude 側の
責務は本文書と `CHANGELOG.md` / `docs/release/RELEASE_NOTES_
0.1.0.md` の整備、および release-build workflow の拡張までに
閉じる。

## スコープ

含む：

- 0.1.0 リリース直前の pre-flight checks（test / build / M9 通し）。
- バージョン番号の整合確認（workspace `Cargo.toml`、`sapphire-
  runtime.gemspec`、`codegen/mod.rs::RUNTIME_VERSION_CONSTRAINT`）。
- tag push → GitHub Actions `release-build.yml` 発火 → artifact
  検証 → rubygems.org への gem push → GitHub Release への attach
  と release note 貼り付け の順序。
- インストール後の smoke（別マシンから `gem install sapphire-
  runtime` + GitHub Release の `sapphire` バイナリを叩く）。

含まない：

- 継続的な CI / release 運用の自動化（本 D3 は **手動実行**
  を前提にする。OIDC trusted publisher / sigstore 等は I-OQ31 で
  0.2.0 以降に punt）。
- `sapphire` CLI gem 自体の rubygems.org 公開（I-OQ29 DECIDED
  だが、0.1.0 ではバイナリを GitHub Release に置く形式に留め、
  `gem build --platform <plat>` × 5 → `gem push` × 5 の自動化は
  0.2.0 以降に回す）。
- VSCode marketplace への publish（I-OQ78 DEFERRED）。

## Pre-flight checks

tag を打つ前に、以下がローカルで green であることを確認する。
GitHub Actions 上でも CI が走るが、ここで先に落としておくと
pipeline のやり直しを避けられる。

```sh
# リポジトリのクリーン性
git status              # working tree clean
git log --oneline -5    # 最新コミットが期待の HEAD であること

# Rust 側（I2 / I3 以降で常用している 4 点セット）
cargo check --all
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace      # 既存 472 pass（D3 時点、必要に応じ更新）

# Ruby 側（R1 以降、runtime/ 直下で走らせる）
cd runtime
bundle install
bundle exec rspec           # 既存 142 pass（D3 時点）
cd ..

# リリース bin の build
cargo build --release --bin sapphire

# M9 例題 4 本の smoke（spec 12）
./target/release/sapphire run examples/sources/01-hello-ruby/Main.sp
./target/release/sapphire run examples/sources/02-parse-numbers/Main.sp
./target/release/sapphire run examples/sources/03-students-records/Main.sp
./target/release/sapphire run examples/sources/04-fetch-summarise/Main.sp
```

`runtime/` の Ruby テストは `runtime/` 配下で `bundle exec rspec`
を呼ぶ運用（R1 scaffolding の既定、`docs/impl/08-runtime-layout.
md`）。

## バージョン番号の整合

0.1.0 では以下 3 箇所の version 文字列がすべて `0.1.0` 同等で
揃っている必要がある（I-OQ33 / I-OQ85）。

| 場所 | 値 | 役割 |
|---|---|---|
| `Cargo.toml` の `[workspace.package].version` | `"0.1.0"` | Rust crate 群（`sapphire-compiler` / `sapphire-core` / `sapphire-lsp`）の version と `sapphire --version` 出力、生成 Ruby ヘッダの `# sapphire X.Y.Z` |
| `runtime/sapphire-runtime.gemspec` が read する `runtime/lib/sapphire/runtime/version.rb` の `Sapphire::Runtime::VERSION` | `"0.1.0"` | gem の `spec.version`、`require_version!` の比較対象 |
| `crates/sapphire-compiler/src/codegen/mod.rs` の `RUNTIME_VERSION_CONSTRAINT` | `"~> 0.1"` | 生成 Ruby の `Sapphire::Runtime.require_version!` に渡す制約 |

次回 minor bump（例 0.2.0）では 3 箇所ともに手で書き換える。major
bump（例 1.0.0）のときは `RUNTIME_VERSION_CONSTRAINT` を `~> 1.0`
へ上げるのを忘れないこと。build.rs で自動同期する案は I-OQ85 で
保留中。

### version 確認コマンド

```sh
grep -n '^version' Cargo.toml
grep -n 'VERSION' runtime/lib/sapphire/runtime/version.rb
grep -n 'RUNTIME_VERSION_CONSTRAINT' crates/sapphire-compiler/src/codegen/mod.rs

# 生成 Ruby にも焼かれることを確認
./target/release/sapphire build examples/sources/01-hello-ruby/Main.sp --out-dir /tmp/sp-out
head -3 /tmp/sp-out/Main.rb
# => "# sapphire 0.1.0 / sapphire-runtime ~> 0.1" を含むヘッダであること
```

## tag push と release-build の発火

tag は annotated tag（`-a`）で打つ。message は CHANGELOG の該当
節の要点を転記する。

```sh
git tag -a v0.1.0 -m "Sapphire 0.1.0 — initial release"
git push origin v0.1.0
```

tag push で `.github/workflows/release-build.yml` が発火する。
GitHub Actions の Web UI（Actions タブ → Release build workflow）
で matrix 5 job 全部が green で終わることを確認する：

- `build x86_64-unknown-linux-gnu`
- `build aarch64-unknown-linux-gnu`
- `build x86_64-apple-darwin`
- `build aarch64-apple-darwin`
- `build x86_64-pc-windows-msvc`

5 job それぞれが以下の artifact を upload する（`sapphire-<target>
-0.1.0`）：

- `sapphire-<target>-0.1.0.tar.gz` または `.zip`
- `sapphire-<target>-0.1.0.tar.gz.sha256` または `.zip.sha256`

tag push をトリガにした run では、workflow 末尾の GitHub Release
attach step（`if: startsWith(github.ref, 'refs/tags/')`）が動き、
同じ 5 つの archive + `.sha256` を `v0.1.0` release へ添付する。
既に release が存在する場合は更新、存在しなければ作成される
（`softprops/action-gh-release@v2`）。

### artifact の手動検証

Actions UI からダウンロードするか、`gh run download` で手元に
落とす：

```sh
gh run list --workflow release-build.yml --limit 1
gh run download <run-id> --dir /tmp/sapphire-artifacts
ls /tmp/sapphire-artifacts/
# 各 archive を展開し sha256 を検証
cd /tmp/sapphire-artifacts/sapphire-x86_64-unknown-linux-gnu-0.1.0
shasum -a 256 -c sapphire-x86_64-unknown-linux-gnu-0.1.0.tar.gz.sha256
tar xzf sapphire-x86_64-unknown-linux-gnu-0.1.0.tar.gz
./sapphire-x86_64-unknown-linux-gnu-0.1.0/sapphire --version
# => "sapphire 0.1.0"
```

## rubygems.org への gem push

0.1.0 時点で rubygems.org に push するのは **`sapphire-runtime`
gem のみ**（I-OQ29 の (A) 案に沿うが、0.1.0 では CLI gem 側の
platform matrix を回さず、CLI は GitHub Release 経由の tarball
配布に留める。(C) 案を初回限定で採る）。

```sh
cd runtime
gem build sapphire-runtime.gemspec
# => sapphire-runtime-0.1.0.gem

# MFA が有効な rubygems アカウントから push
gem push sapphire-runtime-0.1.0.gem
# OTP を求められたら入力する
cd ..
```

注意点：

- 現状 gem は **署名しない**（`--sign` 不採用、I-OQ31 DECIDED）。
  sigstore / OIDC trusted publisher は 0.2.0 以降に punt。
- rubygems アカウントの **MFA 必須化**（`mfa_required`）を有効に
  しておくこと（D1 §2 (A) §セキュリティ）。
- `sapphire-runtime` gem 名は初回 push で確保される。既に squatting
  されていた場合は rubygems 運営に yank 依頼するか、別名へ振り替える
  判断（D1 §6 §先行 squatting 回避）を user 側で下す。

## GitHub Release への upload と release note 貼り付け

release-build workflow が attach step まで走っていれば、
`v0.1.0` の draft/published release が GitHub 上に既に存在する。
Web UI の Releases から該当 release を開き：

1. タイトルを `Sapphire 0.1.0 — initial release` に整える。
2. 本文に `docs/release/RELEASE_NOTES_0.1.0.md` の本文を貼る
   （または `gh release edit v0.1.0 --notes-file docs/release/
   RELEASE_NOTES_0.1.0.md` で反映）。
3. `Set as the latest release` を有効化して publish。
4. attach されている 5 platform × 2 ファイル（archive + .sha256）
   を確認。不足があれば `gh release upload v0.1.0 <path>` で追加。

## インストール後 smoke（ユーザ側検証）

初回リリース直後に、**clean な別マシン**（または Docker container）
で以下を手動で試し、1-install の体験が壊れていないことを確認する。

```sh
# Ruby 3.3+ / glibc 2.17+ の Linux x86_64 を例に
ruby --version                       # 3.3 以上
gem install sapphire-runtime         # rubygems.org から
ruby -e "require 'sapphire/runtime'; puts Sapphire::Runtime::VERSION"
# => "0.1.0"

# GitHub Release から CLI をダウンロード
curl -LO https://github.com/meriy100/sapphire/releases/download/v0.1.0/sapphire-x86_64-unknown-linux-gnu-0.1.0.tar.gz
tar xzf sapphire-x86_64-unknown-linux-gnu-0.1.0.tar.gz
./sapphire-x86_64-unknown-linux-gnu-0.1.0/sapphire --version
# => "sapphire 0.1.0"

# M9 例題 1 本で end-to-end
cat > Main.sp <<'EOF'
module Main
  ( main )
  where

main : Ruby {}
main = rubyPuts "hello from sapphire 0.1.0"

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
EOF
./sapphire-x86_64-unknown-linux-gnu-0.1.0/sapphire run Main.sp
# => "hello from sapphire 0.1.0"
```

どれかが失敗した場合は、release を `draft` に戻すか、yank を含む
ロールバック判断を user が下す。0.1.0 のロールバック手順は本文書
の範囲外だが、原則：

- rubygems: `gem yank sapphire-runtime -v 0.1.0`。yank 後の
  再 push は同一 version では不可（0.1.1 に上げる）。
- GitHub Release: 対象 release を draft 化 or delete。tag 自体を
  消すのは非推奨（subsequent fetches が壊れる）。

## 次回（0.2.0 以降）に向けて

本 D3 は **手動リリース** を前提にしている。0.2.0 以降で自動化
の範囲を広げる際の検討項目：

- OIDC trusted publisher を rubygems.org 側で設定し、`gem push`
  を GitHub Actions workflow 内に取り込む（I-OQ31）。
- `sapphire` CLI gem を platform matrix で `gem build --platform`
  → `gem push` し、D1 の (A) 案へ完全移行（I-OQ29）。
- VSCode extension の marketplace publish（I-OQ78）。publisher 名
  / icon を確定し、`vsce publish` を workflow 化。
- `cargo auditable` / `cargo sbom` で SBOM を release asset に
  同梱（I-OQ31）。
- Windows arm64 の first-class 化（I-OQ32）。

## 参照

- `CHANGELOG.md` — 0.1.0 の機能一覧と Known limitations。
- `docs/release/RELEASE_NOTES_0.1.0.md` — ユーザ向け narrative。
- `docs/impl/12-packaging.md` — D1 配布設計。
- `docs/impl/29-ci-cross-compile.md` — D2 CI matrix 設計と
  `release-build.yml` 現行設計。
- `docs/impl/08-runtime-layout.md` — `sapphire-runtime` gemspec。
- `docs/impl/27-cli.md` — CLI 設計と `--version` 出力。
- `docs/build/02-source-and-output-layout.md` — 生成 Ruby の header
  契約。
- `docs/build/03-sapphire-runtime.md` — ランタイム gem の calling
  convention と version 整合ポリシー。
- `docs/open-questions.md` — I-OQ29 / I-OQ30 / I-OQ31 / I-OQ33 /
  I-OQ78 等。
