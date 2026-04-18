# 04. 起動と設定

状態: **draft**。`docs/spec/08-modules.md`（モジュール DAG）に
対するパイプライン水準の伴走文書。本文書は Sapphire コンパイラ
のユーザ向け CLI、プロジェクト設定ファイルの名前とスキーマ、ビ
ルド順、incremental compilation についての見通しの注記を固定す
る。

本文書は `docs/build/04-invocation-and-config.md` の日本語訳で
ある。英語版が規範的情報源であり、節構成・サブコマンドフラグ・
スキーマ・番号付き未解決の問いは英語版と一致させて保つ。

## 守備範囲

範囲内：

- CLI 実行ファイル名と 3 つの主要サブコマンド（`build`、`run`、
  `check`）。
- `sapphire.yml` 設定ファイル：場所、名前、スキーマ、既定値。
- ビルド順：モジュール DAG（08 §循環インポートに従う）の
  topological 走査。
- v0 が採るか延期するかの incremental compilation の注記。

範囲外：

- コンパイラ内部アーキテクチャ（パーサ、型検査器、コードエミッ
  タ） — `docs/impl/` へ前倒しで委譲。
- ソース／出力ツリー形 — 02 を参照。
- ランタイム gem の振舞い — 03 を参照。
- テストランナー統合 — 05 を参照。

## 実行ファイル名

CLI 実行ファイルは **`sapphire`**。インストール機構はホスト言
語フェーズ（`docs/impl/`）が後に選ぶもの。本文書はインストー
ル機構を規定しない。

ユーザがプロジェクトルート（または `--project-root <dir>` で上
書き）で

```
$ sapphire <subcommand> [args] [options]
```

と打つと、CLI は 3 つのサブコマンドのいずれかにディスパッチす
る。

引数なしの `sapphire`（サブコマンドなし）はヘルプ要約を出力し、
非ゼロステータスで終了する。`sapphire --help` は同じ要約を出力
し、ゼロで終了する。

## サブコマンド

### `sapphire build`

プロジェクトのすべての Sapphire モジュールを compile し、出力
ツリーを書く（02 §出力ツリーに従う）。

```
$ sapphire build [--project-root DIR] [--config FILE]
                 [--clean] [--verbose]
```

挙動：

- 設定を読む（既定：プロジェクトルートの `sapphire.yml`）。
- `src_dir:`（既定 `src/`）下のソースファイルを発見。
- モジュール DAG を計算する（08 に従う）。
- 各モジュールを compile；モジュール 1 つにつき 1 つの `.rb`
  を `output_dir:`（既定 `gen/`）下へ書く。
- 成功で exit status `0`、任意の compile エラーで非ゼロ。

フラグ：

- `--project-root DIR` — 設定中の相対パスの錨。既定は現在のワー
  キングディレクトリ。
- `--config FILE` — `sapphire.yml` 以外の設定ファイルへのパス。
- `--clean` — まず `output_dir:` ツリーを削除してからビルド。
  `rm -rf gen/ && sapphire build` と等価。CLI フラグとして提供
  される；ビルド実行中にユーザが `gen/` を直接削除することをラ
  ンタイムが禁じる場合があるため。
- `--verbose` — モジュールごとのタイミングと依存解決診断を出
  力。既定 off；既定出力は compile されたモジュールごとに 1 行
  （あるいはエラー要約）。

### `sapphire run`

プロジェクトを（必要なら）ビルドし、指定されたエントリを呼ぶ。

```
$ sapphire run [<entry>] [--project-root DIR] [--config FILE]
               [--no-build] [--] [arg...]
```

挙動：

- `--no-build` が与えられない限り、まず `sapphire build` を実
  行する。
- `<entry>` を Sapphire モジュール + 束縛のペアに解決する。既
  定エントリは `Main.run` — モジュール `Main`、束縛 `run`。明示
  的 `<entry>` も同じ `Module.binding` 形（例：`App.serve`）を
  取る。モジュールセグメントは `upper_ident`（PascalCase、文書
  02 §識別子）、束縛名は `lower_ident`。
- 解決された束縛の型はある `a` について `Ruby a` と単一化しな
  ければならない（11 §`run` に従う）。パイプラインは
  `Sapphire::Runtime::Ruby.run(entry_action)` を起動し、結果の
  `Result` を検査する。`Ok a` で `sapphire run` はゼロで終了す
  る。`Err e`（`RubyError`）で `sapphire run` はエラー（クラス
  名、メッセージ、バックトレース）を出力し、非ゼロで終了する。
- `--` の後の引数はランタイム供給機構を介してエントリ束縛の
  Ruby スニペットへ転送される。正確な機構（ランタイム側
  `Sapphire.argv : List String` グローバル？CLI-arg Sapphire
  束縛？）は 04 OQ 1。

`sapphire run` サブコマンドは、仕様 11 の `Ruby a -> Result
RubyError a` exit point が暗黙に必要とするものを実体化した便利
ラッパである。実装が in-process compile + require + invoke か、
Rake タスクラッパかは 01 OQ 4。

### `sapphire check`

すべてのモジュールを型検査するが出力を書かない。

```
$ sapphire check [--project-root DIR] [--config FILE]
                 [--verbose]
```

挙動：

- `build` と同じパース・型検査を行うが、コードエミッタを起動せ
  ず、出力ツリーに触れない。
- 成功でゼロ、任意のエラーで非ゼロ。
- 用途：エディタ／pre-commit フック／CI チェック — full ビル
  ドより速くエラーを掴む。

`check` サブコマンドの存在は使い勝手の便宜である。エディタ統合
向けに daemon として動かすべきか（LSP の精神で）は 04 OQ 2。

## 共通 CLI 慣習

- すべてのサブコマンドは `--project-root` と `--config` を受
  ける。それぞれ既定は現在のディレクトリと `sapphire.yml`。
- すべてのサブコマンドは進捗・エラー出力を stderr に書き、「主
  要産物」出力を stdout に書く（`build` と `check` の主要産物
  はファイルツリー；成功時の stdout は空。`run` の主要産物はエ
  ントリの効果でランタイムが実行する；stdout はエントリが書く
  もの）。
- すべてのサブコマンドは `--help` で自身の usage 要約を出力する。
- exit status は POSIX 慣習：成功で `0`、エラーで非ゼロ。より
  細かい status 体系（例：`2` パースエラー vs `3` 型エラー）
  は 04 OQ 3。

## 設定：`sapphire.yml`

プロジェクト設定ファイルは既定名 `sapphire.yml` でプロジェク
トルートに住む。書式に YAML を選んだのは、Ruby 開発者（聴衆）
に広く理解されており、可読で、Ruby 標準ライブラリでサポートさ
れているから。代替書式として JSON（`sapphire.json`）も追加設
定なしで admissible；他の書式（TOML、plain Ruby DSL）を加える
かは 04 OQ 4。

### スキーマ（v0）

```yaml
# sapphire.yml — Sapphire プロジェクト設定

# プロジェクト名。現状情報的；将来的に gem 包装で利用される
# （05 §gem として公開を参照）。
name: my-project

# プロジェクトバージョン。`name` と同状態。
version: 0.1.0

# プロジェクトルートを基準にしたソースツリーのルート。
# 既定：src/
src_dir: src/

# プロジェクトルートを基準にした出力ツリーのルート。
# 既定：gen/
output_dir: gen/

# `sapphire run` の既定エントリ、`Module.binding` 形。
# 既定：Main.run
entry: Main.run

# 生成コードが由来ヘッダに埋め込むランタイム gem バージョン
# 制約。
# 既定：コンパイラバージョンのランタイム依存と一致。
runtime: '~> 0.1'

# `sapphire run` が生成コードを評価するために使う Ruby 実行
# ファイル。既定はホストの `$PATH` 上の `ruby`。パスやバージョ
# ンマネージャ仕様（'asdf:3.3.0'、'rbenv:3.3.0'）は後で
# admissible になり得る。
ruby: ruby

# （見通し）モジュールごとの compile フラグ；v0 では空マップの
# み admissible。
modules: {}
```

すべてのキーに既定がある。既定を使うプロジェクトは `name:` と
`version:`（あるいは one-off スクリプトなら空ファイル — パイプ
ラインは依然そのディレクトリを Sapphire プロジェクトルートとし
て扱う）だけのほぼ空 `sapphire.yml` を出荷できる。

スキーマは**ランタイム／コンパイラバージョンを通じて暗黙にバー
ジョン化される**。後方互換でないスキーマ変更はコンパイラの
major バージョンを bump する。明示的 `schema_version:` キーを
加えるかは 04 OQ 5。

### 検証

パイプラインは起動時に設定を検証する：

- 未知のキーは（警告ではなく）エラー。`sources_dir:` のような
  typo が無音で既定にフォールバックしないため。エラーメッセー
  ジにはランタイム／コンパイラバージョンを含め、ユーザがキー未
  知の理由が typo か古いコンパイラかを見分けられるようにする。
- `src_dir:` と `output_dir:` はプロジェクトルート配下の相対パ
  スでなければならない。絶対パスや `..` 走査はエラー。
- `entry:` は構文上 `Module.binding` 形でなければならない（実
  モジュール集合に対する解決は `sapphire run` 時まで延期）。

## ビルド順

08 §循環インポートに従い、モジュールグラフ（ノードがモジュー
ル、辺が `import` 関係）は acyclic。パイプラインはこの DAG 上
の topological 順序を計算し、その順でモジュールを compile する。

パイプライン水準の順序契約：

- モジュールはボトムアップに compile される：import DAG の葉が
  最初、依存モジュールが最後。あるモジュールの compile は、そ
  れが import するすべてのモジュールが完全に compile される
  （パース、型検査、出力発行）まで始まらない。
- 単一の topological「水準」内（互いに未充足依存のないモジュー
  ル群）では、並列 compile は admissible だが要求されない。v0
  パイプラインは serial に compile してよい；並列フラグは 04
  OQ 6。
- import グラフの循環は、出力が書かれる前にパイプラインが報告
  する静的エラー。

topological sort は 08 §インスタンスとモジュール の孤児なし
不変条件をビルドごとに計算可能にするものでもある：インスタン
スの可視性は推移的インポートに従うため、ボトムアップのコンパ
イル順序によって、各モジュールは自身の import 閉包内のインス
タンスだけを観測する。パイプラインはビルド起動ごとに toposort
を 1 回計算する；incremental ビルドはそれを cache してよい
（後述 §Incremental compilation を参照）。

## Incremental compilation

v0 のコミットメントは**見通しのみ**：パイプラインは**clean ビ
ルドのつもりで**振る舞わなければならない（すなわち
`sapphire build --clean` と同じ出力ツリーを生成する）。実際にあ
らゆるモジュールを再ビルドするか変化のないものを skip するかは
実装選択 — どちらの挙動も v0 で admissible。

v0+ incremental スキームのスケッチは：

1. 各ソースファイルについて、その内容と、それが推移的に import
   するすべてのモジュールの内容のハッシュ（*interface hash*。単
   なるソースハッシュではない — モジュールの*型*シグネチャの
   変化はすべての依存先キャッシュを無効化すべき）を取る。
2. `(コンパイラバージョン, ランタイムバージョン, モジュール名,
   interface hash)` をキー、生成 `.rb` を値とするビルドごと cache
   を永続化。
3. rebuild 時、interface hash を再計算し、`(version, version,
   name, hash)` キーが変化しないモジュールについて発行を skip
   する。

incremental ビルドに関するパイプライン水準のコミットメント：

- cache はユーザに opaque：そのディスク上の形は契約ではない。
  ユーザに見える契約は「`sapphire build` の出力は `sapphire
  build --clean` の出力と一致する」。
- cache はプロジェクトルート配下（提案 `.sapphire-cache/`）に
  住み、`.gitignore` 対象であるべき。
- `sapphire build --clean` は常に clean な出力ツリーを生成する；
  cache は `--clean` の一部として無効化される。

正確な cache メカニズム、ハッシュアルゴリズム、ディスク書式は
`docs/impl/` へ前倒しで委譲。v0 が incremental ビルドなしで出
荷してもそれは契約違反ではない。

## 診断

CLI のユーザに見える主要な失敗モードは「ソースが compile しな
い」。パイプライン水準のコミットメント：

- 診断は stderr に書く。
- 各診断は次を担う：ソースパス、行番号、列範囲、severity
  （`error` / `warning`）、短い要約、選択的な説明段落。
- 診断集合は exit 前に*完全*：パイプラインは `build` 時の型検
  査における最初のエラーで止まらない。「いくつエラーで bail す
  るか」ポリシー（毎エラー vs 最大 N）は 04 OQ 7。
- 診断*文面*はホスト言語非依存；wording はパイプライン契約では
  なく、コンパイラ実装の性質。

将来の LSP 風統合（04 OQ 2）は同じ診断形を JSON-RPC チャネルで
消費するであろう；診断書式の機械符号化は v0 契約ではなく見通
しの作業の一部。

## 他文書との相互作用

- **仕様 08。** §ビルド順は 08 の acyclic モジュール DAG 保証
  を、パイプラインが行う topological sort として実体化する。
- **仕様 11。** §`sapphire run` は 11 §`run` に従い
  `Sapphire::Runtime::Ruby.run` を起動する；エントリ束縛の必須
  `Ruby a` 形は 11 の typing から来る。
- **ビルド 02。** すべての `src_dir:` / `output_dir:` の既定は
  02 の既定と一致する；設定はそれらをユーザ上書き可にするだけ。
- **ビルド 03。** `runtime:` が制御するランタイム gem バージ
  ョン制約は、02 §ファイル内容の形が生成由来ヘッダにスタンプ
  するもの。
- **ビルド 05。** テストツール統合は内部で `sapphire build` を
  起動する（05 §Bundler 統合を参照）。

## 未解決の問い

1. **`sapphire run` のエントリへの CLI 引数転送。** `--` の後
   の引数を Sapphire エントリ束縛へ届ける必要がある。選択肢：
   ランタイム側 `Sapphire.argv : List String` 定数；エントリ束
   縛のシグネチャに余分なパラメータ；Ruby スニペットからの
   `ARGV` 読み取り。Draft：`:=` 束縛スニペットが読める ランタ
   イム側 `Sapphire::Runtime::Ruby.argv` アクセサ。実装フェー
   ズへ委譲。

2. **`sapphire check` のエディタ統合向け daemon モード。** パ
   イプラインがエディタ内診断のための LSP 風サーバ（`sapphire
   check --lsp`）を露出すべきか。Draft：v0 ではなし；one-shot
   `sapphire check` で pre-commit と CI には充分。委譲。

3. **きめ細かい exit status 体系。** より細かい exit status マ
   ッピング（`2` パースエラー、`3` 型エラー、`4` link エラー
   等）は CI が失敗種別を自動で見分けられるようにする。Draft：
   v0 では二値の成功／失敗；より豊かな exit code は委譲。

4. **YAML / JSON を超える設定書式。** TOML、Ruby DSL（例：
   `Sapphirefile`）、`package.json` 風ネスト鍵。Draft：v0 で
   は YAML + JSON のみ。委譲。

5. **`sapphire.yml` の明示的 `schema_version:` キー。** 「あな
   たは v1 設定を書いたがこのコンパイラは v2 を読む」と精確に
   エラーを出せるようにするもの。Draft：コンパイラバージョン
   経由で暗黙；実スキーマ破壊が起きたら見直し。委譲。

6. **同 DAG 水準の並列 compile。** 並列 toposort のための
   `--jobs N` フラグ。Draft：v0 では serial；契約は並列発行を
   admissible とする。委譲。

7. **エラー時の bail-out ポリシー。** 「exit 前にすべてのエラ
   ーを報告」 vs 「N エラー後に停止」 vs 「最初のエラーで停止」。
   Draft：exit 前にすべてのエラーを報告（典型的なコンパイラ体験
   に近く、batch 修正ワークフローを楽にする）。あとから調整可。

8. **Watch モード（`sapphire build --watch`）。** ファイルシ
   ステム監視で変化に再発行する long-running ビルド。Draft：
   v0 ではなし；実装フェーズへ委譲。

9. **プロジェクトルートに `Gemfile` があるとき `sapphire run`
   が `bundle exec` を自動で行うか。** Draft：はい — プロジェ
   クトルートに `Gemfile` があれば、ランタイム起動は
   `bundle exec ruby ...` を経由する。実装フェーズへ委譲し、
   ホスト言語の使い勝手と照らし合わせて確認する。
