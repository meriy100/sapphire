# 27. Implementation I8 — `sapphire` CLI

Status: **draft**（I8 と同時に着地）。build 04 §CLI 契約の
実装。

## スコープ

- `sapphire` binary を `sapphire-compiler` crate の `[[bin]]` とし
  て追加
- サブコマンド `build` / `run` / `check`、および `--version` /
  `--help`
- 入出力 path 方針

## 引数パーサ

追加依存を入れず **手書きパーサ**。理由：

- M9 フェーズで必要なフラグは `<path>` / `--out-dir <dir>` / `--
  version` / `--help` 程度
- MSRV 1.85 は最近だが、追加依存 `clap` を入れると compile time /
  binary size が増える
- 実装コストは小さい（< 100 lines）

将来フラグが増えて手書きが辛くなったら `clap` に移す（I-OQ83
予約）。

## サブコマンド

### `sapphire check <path>`

- `<path>` が file: 単一ファイル
- `<path>` が directory: `*.sp` を再帰的に拾う
- lex → layout → parse → resolve → typecheck を走らせる
- エラーがなければ exit 0、あれば stderr に出力して exit 1

### `sapphire build <path> [--out-dir <dir>]`

- 上記 pipeline に加えて codegen を走らせる
- `--out-dir` 省略時は `gen/` を使う
- 各モジュール → `<out-dir>/sapphire/<snake_case_path>.rb`
- さらに `Sapphire::Prelude` を `<out-dir>/sapphire/prelude.rb` に
  emit（codegen が Prelude 値を提供するため、build 02 の「runtime
  gem が prelude を提供する」方針はここでは取らない：runtime は
  Prelude 非対応、prelude は毎 build で生成）

### `sapphire run <path>`

- `build` を実行した後、`ruby -I runtime/lib -I <out-dir>
  <out-dir>/sapphire/<entry>.rb -e 'exit
  Sapphire::<entry_class>.run_main'` を spawn する
- `<path>` のモジュールが `main : Ruby {}` を export していなけ
  ればエラー
- 終了コードは Ruby subprocess の exit code をそのまま返す

### `--version`

`sapphire 0.0.0` と出力（workspace.package.version を参照）。

### `--help`

usage summary を出力して exit 0。

## 入出力 layout

M9 例題は 1 モジュール 〜 2 モジュールなので、多モジュール
workspace（`sapphire.yml` + `src/` ツリー）は **本 task では未対
応**。build 04 §Configuration schema の `sapphire.yml` は I8 では
読まず、`<path>` 引数のみで動作する。

将来のマルチファイル project layout 対応は I-OQ4（packaging）と
併せて D2/D3 段で（ロードマップ §ウェーブ 7）。

## Runtime へのパス解決

生成コードは `require 'sapphire/runtime'` を出すが、CLI が `ruby`
を spawn するときに `$LOAD_PATH` に `runtime/lib` を通す必要があ
る。v0 は **`-I runtime/lib` を絶対パスで指定**：CLI は repo root
を env 変数 `SAPPHIRE_RUNTIME_LIB` から取るか、`cargo run` 時の
working directory から相対で見つける。

将来 gem packaging（D1）で gem 化したら `gem which sapphire/
runtime` で解決するが、v0 は repo-relative で足りる。

## 除外

- `sapphire.yml` 読み込み（多ファイル project は後続）
- incremental build
- watch mode
- `--clean` フラグ
- parallel compilation

## 今後の拡張

- `clap` 移行（I-OQ83）
- `sapphire.yml` 対応（I-OQ4）
- incremental / watch
