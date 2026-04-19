# Sapphire 0.1.0 — 初回リリース

*2026-04-19*

Sapphire は Haskell 相当の表現力（型クラス + higher-kinded types）
を目指す関数型言語で、`.sp` ソースを Ruby モジュール（`.rb`）へ
翻訳して実行する。本リリース 0.1.0 は、2026-04-19 の I1（Rust
ホスト言語決定）から始まった実装フェーズの最初の公開版で、最小限
のコンパイラ・ランタイム gem・VSCode 向け Language Server が一体
として動く地点に到達した。

## ハイライト

- **`.sp` → `.rb` の end-to-end コンパイルが動く**。`sapphire
  check / build / run` の 3 サブコマンドが揃い、`docs/spec/12-
  example-programs.md` の 4 例題（`01-hello-ruby`、`02-parse-
  numbers`、`03-students-records`、`04-fetch-summarise`）を `cargo
  build --release --bin sapphire` したバイナリから素で通せる。
- **ADT + レコード + 型クラス + 作用モナド `Ruby`**。
  `sapphire-runtime` gem が Sapphire ↔ Ruby 双方向のマーシャリン
  グ、タグ付きハッシュ `{:tag, :values}` での ADT、`pure` /
  `bind` / `run` の評価器、境界 `rescue` による `RubyError` を提供。
  spec 10 / 11 の契約を最小実装としてカバーする。
- **VSCode Language Server が動く**。parse 診断、hover（top-level
  scheme）、goto-definition（同一ファイル）を `editors/vscode/` から
  F5 起動で試せる。TextMate grammar / snippets / language
  configuration を同梱。completion（L6）は 0.1.0 と並走実装中で、
  merge タイミングによっては 0.1.0 に含まれる。
- **5 platform release-build workflow**。Linux x86_64 / Linux
  arm64 / macOS x86_64 / macOS arm64 / Windows x86_64 を GitHub
  Actions の native runner で build、tar.gz / zip + SHA-256 で
  artifact 化し、tag push のときのみ GitHub Release に attach。
- **仕様書は English 規範 + 日本語翻訳の 2 トラックで凍結済み**
  （M10 spec-freeze review）。ホスト言語に依存しない語り口を維持
  しており、将来 self-host 検討へ移ったときも契約は揺れない。

## Quick start

0.1.0 時点の推奨インストール経路は **ローカルビルド + 手動 gem
install** である（rubygems.org / GitHub Release への公開は user
判断で本リリース後に実施予定、`docs/impl/32-release-process.md`
参照）。

```sh
# 1. リポジトリを取得
git clone https://github.com/meriy100/sapphire
cd sapphire

# 2. ランタイム gem をビルド & install（Ruby 3.3+ 必須）
cd runtime
gem build sapphire-runtime.gemspec
gem install --local sapphire-runtime-0.1.0.gem
cd ..

# 3. コンパイラをビルド（Rust 1.85.0+ 必須、rust-toolchain.toml で pin 済み）
cargo build --release --bin sapphire

# 4. M9 例題を動かす
./target/release/sapphire run examples/sources/01-hello-ruby/Main.sp
./target/release/sapphire run examples/sources/02-parse-numbers/Main.sp
./target/release/sapphire run examples/sources/03-students-records/Main.sp
./target/release/sapphire run examples/sources/04-fetch-summarise/Main.sp
```

GitHub Release に attach された `sapphire-<target>-0.1.0.tar.gz`
/ `.zip` を展開すれば `sapphire` バイナリが直接使える（tag push
後に user が公開する）。その場合でも `gem install sapphire-
runtime` は別途必要。

詳細な CLI オプションは `sapphire --help`、build pipeline は
`docs/build/` 配下を参照。

## Known limitations

本 0.1.0 は「M9 例題を通すための最小実装」という位置付けで、以下
は既知の未対応。完全な一覧と issue 追跡 ID は `CHANGELOG.md` /
`docs/open-questions.md` を参照。

- パターンの網羅性 / 到達可能性検査は未実装（I-OQ60）。
- 型置換 `Subst::compose` に silent drop の余地（I-OQ63、soundness
  hole）。
- `do` ブロックが複数 monad をまたぐ書き方で `pure` / `return` が
  runtime raise される（I-OQ82。M9 例題は踏まない）。
- resolver が 1 件でもエラーを返すと LSP の goto / hover が沈黙
  （I-OQ74）。
- cross-file goto / Prelude 定義への goto は未対応（I-OQ72 /
  I-OQ73）。
- VSCode extension marketplace 未公開、`publisher` は placeholder
  （I-OQ78）。VSIX 手動配布 or リポジトリ内ビルドから使う運用。
- gem 署名 / sigstore / OIDC trusted publisher / SBOM は未導入
  （I-OQ31）。
- Windows arm64 は best-effort（I-OQ32、CI matrix から除外）。
- **ライセンスは MIT 単独**（I-OQ11）。Rust 生態系慣例の `MIT OR
  Apache-2.0` dual 化は 0.2.0 以降で user 判断。

## 次に来るもの

0.2.0 以降の候補（順不同、個別に優先度は別途決める）：

- 網羅性検査 / `Subst::compose` soundness hole の解消（I-OQ60 /
  I-OQ63）。実装フェーズの exit 条件直近の積み残し。
- `pure` / `return` の specialisation 粒度を do block 単位まで
  下げる（I-OQ82）。
- resolver を部分成功（`(ResolvedProgram, Vec<Error>)`）API に
  移行し、LSP を resolve エラー下でも応答させる（I-OQ74）。
- cross-file goto / workspace-aware な document store（I-OQ72）、
  Prelude の `.sp` 化（I-OQ44、I-OQ73 / I-OQ98 連動）。
- VSCode marketplace への公開（I-OQ78）、publisher 名 / icon 確定。
- OIDC trusted publisher 経由で rubygems.org へ gem push を自動化
  （I-OQ31）。gem 署名 / SBOM の再評価も同時。
- Windows arm64 の first-class 化（I-OQ32、`cargo-zigbuild` or
  native arm64 Windows runner 待ち）。
- dual-license 化の最終判断（I-OQ11）。
- self-host 探索の開始（spec-first phase 後段の topic）。

## 参照

- `CHANGELOG.md` — 機械可読な差分。
- `docs/impl/32-release-process.md` — 初回リリース実行手順
  （user 用チェックリスト）。
- `docs/impl/12-packaging.md` — 配布設計 D1。
- `docs/impl/29-ci-cross-compile.md` — CI cross build D2。
- `docs/open-questions.md` — 全 open question の living tracker。
