# 09. レキサ設計メモ

I3 で `crates/sapphire-compiler/src/lexer/` に導入するレキサの設計
メモ。仕様は `docs/spec/02-lexical-syntax.md`（正本）と
`docs/spec/05-operators-and-numbers.md` に従う。本文書は
`CLAUDE.md` §Phase-conditioned rules の「Rust 固有の実装判断は
コード変更前に `docs/impl/` に記録する」に則って I3 の着手前提を
畳んでおくためのもの。

## スコープ

I3 のレキサは **字句列 + 行頭インデント情報** までを出す。
レイアウト本体（`let` / `where` / `of` / `do` の開く / 閉じる /
セミコロン挿入）は **I4 以降の別パス（「layout 変換」）に委譲**
する。

### 含む

- 識別子：`lower_ident` / `upper_ident`（ASCII のみ、02-OQ5 DECIDED）。
- 予約語：spec 02 §Keywords の全語を専用 `TokenKind` に上げる。
  `module`・`import`・`exposing`・`hiding`・`as`・`qualified`・
  `export`・`data`・`type`・`class`・`instance`・`where`・
  `let`・`in`・`if`・`then`・`else`・`case`・`of`・`do`・
  `forall`。spec 02 §Keywords は「これらは reserved であり
  `lower_ident` として現れられない」と規範的に述べているため、
  `forall` / `qualified` / `export` のように現段階のどの
  production にも登場しない語でも、`LowerIdent` として出すと
  この不変条件が破れる。よって I3 の時点で全語を変種に並べる。
  用途がまだ具体化していない語はパーサが reject するだけで
  よい。
- リテラル：`Int(i64)`、`String(String)`（エスケープ展開済み）。
  `Float` は M5 以降（05-OQ1）まで到達しないので取らない。
  **負リテラルは出さない** — 02-OQ2 DECIDED / 05 §Unary minus に
  従い、`-3` は `-` と `3` の 2 トークン。
- 区切り：`(`・`)`・`[`・`]`・`{`・`}`・`,`・`;`・`.`・`..`・
  `=`・`->`・`=>`・`|`・`::`・`:`・`<-`・`_`・`\`。
- 演算子：spec 05 §Operator table の
  `+`・`-`・`*`・`/`・`%`・`==`・`/=`・`<`・`<=`・`>`・`>=`・
  `&&`・`||`・`++` を専用変種に出す。`::`（リスト cons）は
  spec 02 §Reserved punctuation の `DoubleColon` と同じトークン
  として兼用する。`>>=` / `>>` は M7（Monad）まで到達しないので
  専用変種を用意せず、後段の monotonic な拡張に任せる（現状は
  `Op(String)` で受ける）。op_char+ 最大連が表の subset にも
  reserved punctuation にも該当しない場合は `Op(String)` に
  包む（例：`<>`, `:>`, `==>` 等）。
- コメント：`-- 行コメント`・`{- ブロック -}`（nest 可）。
  トークンとしては出さずスキップ。
- 行頭インデント：`Newline` トークン + 次の非空行の先頭で
  `Indent(col)` を出す。`col` は 0-based の列（code point 数）。

### 含まない

- **レイアウト本体**（仮想 `{`・`;`・`}` の挿入）。後続パスへ。
- Float / 16 進 / 8 進 / 2 進整数（02 §Literals 末尾で deferred）。
- 非 ASCII 識別子（02-OQ5）。
- 複数行文字列リテラル（02 §Literals 末尾で deferred）。
- 演算子セクション（05-OQ4）。
- パイプ演算子 `|>` / `<|`（05-OQ5）。

## 設計判断

### 手書き再帰下降で書く

I-OQ2（Parser 戦略）は I4 で決めるが、**レキサ単体は parser 戦略
に対して中立**になるよう、`chumsky` / `nom` / `lalrpop` といった
外部 crate を使わず標準ライブラリのみで書く。

理由：

1. 字句層はもともと状態が浅く、手書きでも複雑度が低い。
2. 外部パーサ crate を入れると I4 の pilot で不採用になった場合
   に剥がすコストが発生する。I3 を parser 戦略から疎結合に保つ。
3. エラー位置（`Span` byte offset）を細かく制御できる。後続で
   診断 UI に流し込む際、手書きのほうが自然。

### Token = `(kind, span)` 分割

`Token` は `kind: TokenKind` と `span: Span { start, end }` を
持つ。`Span` は byte offset（`u32` だと 4GiB 制限が早く来て
しまうため `usize`）。

`TokenKind` は列挙型。文字列ペイロードは `UpperIdent(String)` /
`LowerIdent(String)` / `Str(String)` に持たせる。`Int` は `i64`
直持ち。将来 interner を入れる場合は `Symbol` 型を挟めばよいが、
I3 ではまだ入れない（YAGNI、診断メッセージでそのまま使える
プレーンな `String` のほうが着手コストが低い）。

### `Newline` / `Indent` を独立 token として出す

Haskell / Elm 風のレイアウト変換は後段で走らせる。その時に
「どの物理改行の後か」「列いくつか」が取れる必要があるため、

- **`Newline`** — 論理行の終端。連続する空行は 1 個に畳まない
  （空行もレイアウトの読み取りに必要）。ただし空行のあと次の
  token が出る直前に `Indent` を添える、という形で最終的には
  layout パスが使いやすい列になる。
- **`Indent(col)`** — 論理行頭の非空白 token 直前に出す、その
  token の列位置を運ぶトークン。ファイル先頭にも出す（最初の
  論理行のインデントを 0 ではなく正確に測るため）。

同じ論理行の中にある token には `Indent` を付けない（列情報は
`span.start` から層下で再計算できる）。レイアウトに使う列は
論理行頭のみ必要、という Haskell 2010 と同じ制約に揃える。

別案として「各 token に `column` を乗せる」形もあるが、レイアウト
パスが引き回す情報を最小化するために Indent を専用トークンとして
切り出した。後者は token サイズが大きくなる割に、非レイアウト
トークン（文字列リテラルの中身など）では column を使わないため
ムダが多い。

### エラーは独自型

I-OQ3（Error 型設計）はレイヤごとに揃える前提だが、レキサが最初に
具体のエラーを扱うレイヤとなる。ここでは **独自 ADT `LexError`**
を採り、`anyhow` はレキサの公開 API には出さない。

理由：

1. `LexError` は **位置情報（`Span`）と原因コード（`LexErrorKind`）
   を厳密に分離** する必要がある。`anyhow::Error` は文字列化が
   楽な代わりに、後続の診断 renderer が構造を再取得できない。
2. クラスごとに `match` で列挙できるほうが、後続パスでエラー
   コードを参照した振る舞い分岐（LSP の quick-fix など）を書き
   やすい。
3. レキサは層下の errant パスに比べてエラー種が少ない（一桁）
   ため、ADT を書き下すコストが低い。

`LexError` は `std::error::Error` と `Display` を手で実装する。
I-OQ3 の本決着（`thiserror` vs 手書き vs `anyhow` の線引き）は
パーサ層の実装で再検討する。

### 単項マイナスとリテラル境界

02-OQ2 DECIDED / 05 §Unary minus：**`-` は常に 2 トークン**
として lex し、`-` の unary / binary 区別はパーサが行う。
レキサは spec に反して `-3` を 1 トークンにしない。
また 05 §Design notes「`a - b` / `a-b` / `a -b` は全て同一に
tokenize される」を守るため、演算子の直後の空白は判別材料に
しない。

### ブロックコメントの nest

`{- ... -}` は nest 可。深さカウンタで追跡し、0 に戻ったら閉じる。
unterminated block comment はエラー（`LexError` の `kind`）で
開始位置 `{-` の span を返す。

### タブをレイアウト位置で拒否

02-OQ4 DECIDED / 02 §Layout：論理行の先頭（最初の非空白 token の
前）に現れるタブはエラー。行内のタブは通常の空白。実装では、
改行直後の連続空白を読むループで `\t` を検出したら `LexError`。

### 文字列リテラル

- `"..."` 内で生の `\n`（物理改行）は lex error（02 §Literals）。
- エスケープは `\n`・`\t`・`\r`・`\\`・`\"`・`\u{HEX+}` を許し、
  `\u{HEX+}` は 1〜6 桁 hex、`char::from_u32` に失敗する値
  （surrogate 等）はエラー。
- それ以外の `\X` はエラー。

### 識別子開始文字 ASCII 強制

02 §Identifiers / 02-OQ5 DECIDED：非 ASCII 文字で始まる識別子は
エラー。文字列リテラルの内部やコメント内部の非 ASCII は許す。
エラー位置は問題の code point の span。

### `..` の扱い

`.` と `..` は spec 02 §Reserved punctuation で予約されている。
I3 のレキサでは両方を出す（maximal munch で `.` が 2 つ連続した
ら `..` を 1 トークンに）。用途は後続（レンジ構文・モジュール名
修飾・レコード更新）で fix される。

### `<>` は出さない

spec 05 §Operator table に `<>` は含まれない（Haskell 的な
`Monoid` 付随演算子が Sapphire には入っていない）。ただし 02 の
maximal munch は `<`・`>`・`=` などの `op_char` 連鎖を 1 トークン
にまとめる義務を持つので、`<>` という op_char 連鎖が出現したら
`Op("<>".into())` のような汎用の演算子トークンとして出す。
spec 02 §Operator tokens が許す「op_char の最大連」＝token という
契約をレキサが守り、05 が許す subset へのフィルタリングはパーサ
側の責務とする（指示書の記述と spec 05 §Abstract syntax の
「strict subset」定義に合致）。

結果として `TokenKind` は：

- 個別の予約演算子（`==`・`/=`・`+`・`-` …）を固有ヴァリアント
  として持つ。05 §Operator table に載っている集合がこれ。
- そこに該当しない op_char 連鎖は `Op(String)` で包む。

パーサは固有ヴァリアントと `Op` の両方を読む。

## TokenKind 骨子

```
enum TokenKind {
    // Identifiers
    LowerIdent(String),
    UpperIdent(String),
    Underscore,

    // Keywords
    Module, Import, Exposing, Hiding, As,
    Data, Type, Class, Instance, Where,
    Let, In, If, Then, Else, Case, Of, Do,

    // Literals
    Int(i64),
    Str(String),

    // Punctuation
    LParen, RParen, LBracket, RBracket, LBrace, RBrace,
    Comma, Semicolon, Dot, DotDot,
    Equals, Arrow, FatArrow, Bar, DoubleColon, Colon,
    LeftArrow, Backslash,

    // Operators (spec 05 subset)
    Plus, Minus, Star, Slash, Percent,
    EqEq, SlashEq, Lt, LtEq, Gt, GtEq,
    AndAnd, OrOr, PlusPlus,
    // Fallback for op_char runs not in spec 05
    Op(String),

    // Layout
    Newline,
    Indent(usize),

    // Sentinels
    Eof,
}
```

`Eof` を最後に 1 個だけ出す運用にする。パーサ側が「トークン列の
終わり」を普通の token 比較で検査できる。

## 未対応と I4 以降への引き継ぎ

- **Float リテラル**（05-OQ1 / 01-OQ4）：`TokenKind` に枠を設け
  ず、必要になった時点で `Float(f64)` を追加する。追加は monotonic
  extension（既存の Sapphire プログラムを拒絶しない）。
- **非 ASCII 識別子**（02-OQ5）：同様に、識別子文字集合の拡張は
  monotonic。I3 では `LowerIdent` / `UpperIdent` が ASCII のみを
  受け入れる。
- **演算子セクション**（05-OQ4）：レキサには関係せず、パーサが
  `( op )` の並びを読み取れば足りる。`TokenKind::Op` も含めて既に
  表現可能。
- **負の整数リテラル**（02 §Literals / 05 §Unary minus）：レキサは
  出さない。`-` と `Int` の 2 トークン。
- **レイアウト本体**：`Newline` / `Indent` を根拠に後続パスが仮想
  `{`・`;`・`}` を挿入する。I4 着手時に `crates/sapphire-compiler/
  src/lexer/layout.rs` もしくは別モジュールで実装する。
- **インクリメンタル lex**（LSP 用）：I3 は 1 ファイル全体を一気
  に読む関数 `tokenize(&str) -> Result<Vec<Token>, LexError>` のみ
  提供。ストリーム API / 差分再 lex は L3 で再検討（I-OQ9）。
