# lexer-snapshot

`docs/spec/02-lexical-syntax.md` に従う I3 レキサの動作を
手元で確認するための最小サンプル。

## ファイル

- `hello.sp` — 短い Sapphire ソース（モジュール宣言 + 1 関数）。
- `hello.tokens.txt` — 上記を `tokenize` に通した期待出力。

## 実行

リポジトリルートで以下を実行する。

```
cargo run -p sapphire-compiler --example lex_dump -- \
    examples/lexer-snapshot/hello.sp
```

出力が `hello.tokens.txt` と一致すれば合格。差分を確認したい場合：

```
cargo run -p sapphire-compiler --example lex_dump -- \
    examples/lexer-snapshot/hello.sp \
  | diff -u examples/lexer-snapshot/hello.tokens.txt -
```

## 出力フォーマット

1 行に 1 トークン：

```
<start>..<end>  <TokenKind>
```

`<start>` と `<end>` はソースへのバイトオフセット（半開区間）。
`<TokenKind>` は `sapphire_compiler::lexer::TokenKind` の
`Display` 出力（変種名 + 必要ならペイロード）。

`Indent(col)` は論理行頭の先頭トークン直前に出る仮想トークンで、
`col` はその行頭の列（0-based、code point 数え）。
`Newline` は論理行の終端。空行でも出る。レイアウト本体（仮想
`{`・`;`・`}` の挿入）は I4 以降の別パスで行う。
