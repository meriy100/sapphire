# M9 例題ソース

`docs/spec/12-example-programs.md` の 4 本の例題を、仕様本文から
抽出した `.sp` ソースとして保管する。**現状のコンパイラではまだ
parse できない**（I3 レキサのみ着地、I4 パーサは未着手）。

これらは以下の用途で生きる：

1. I3 レキサを `.sp` 実入力に走らせる素材（`examples/lexer-
   snapshot/` が先行して `hello.sp` を個別管理するのは別目的）。
2. I4 以降のパイプラインで受理できる範囲を段階的に広げていく際の
   回帰入力。最終的に M9（I9）でこの 4 本が `sapphire build`
   `sapphire run` で end-to-end に動くことが「第一版完成」の
   定義。
3. 読み物として、Sapphire 言語のコード感覚を掴むサンプル。

## 一覧

| ディレクトリ | spec 12 対応 | 特徴 |
|---|---|---|
| `01-hello-ruby/` | Example 1 | 作用モナドの最小 end-to-end |
| `02-parse-numbers/` | Example 2 | `Result` モナドによる pure パース、Ruby 側は I/O の端のみ |
| `03-students-records/` | Example 3 | レコードと高階リスト処理、Ruby 非依存 |
| `04-fetch-summarise/` | Example 4 | 2 モジュール、`:=` による Ruby 埋め込み、`HttpError` ADT |

## 現状の動作

**何も動かない**（コンパイラのフロントエンドが未完成）。動くようになる
マイルストーン：

- I3 レキサ着地後：tokenize できることを `cargo run -p sapphire-compiler
  --example lex_dump -- examples/sources/01-hello-ruby/Main.sp` で確認
  可能（ただしトークン列を眺めるだけ）。
- I4 パーサ着地後：AST までは構築できるようになる見込み。
- I9 着地（M9 例題 end-to-end）時点で全 4 本が `sapphire build &&
  sapphire run` で動く。
