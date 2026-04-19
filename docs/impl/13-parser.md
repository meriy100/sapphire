# 13. パーサ設計メモ

I4 で `crates/sapphire-compiler/src/{layout,parser}/` および
`crates/sapphire-core/src/ast.rs` に導入するレイアウト解決パスと
パーサの設計メモ。正本は `docs/spec/01`〜`docs/spec/10` の各
章（具象構文・演算子表・レイアウト規則）。本文書は **CLAUDE.md
§Phase-conditioned rules** の「Rust 固有の実装判断はコード変更前
に `docs/impl/` に記録する」方針に則って、I-OQ2（パーサ戦略）の
決定根拠と、レイアウト解決を独立パスに分離した理由、AST を
`sapphire-core` に置いた判断、パーサの射程を整理しておくもの。

## I-OQ2（パーサ戦略）の決定

**結論：手書き再帰下降 + Pratt 演算子**。`chumsky` / `nom` /
`lalrpop` は採らない。`docs/open-questions.md` I-OQ2 を
`DECIDED` に更新した。

### 却下した選択肢

- **`lalrpop`（LR(1) generator）**。Sapphire は off-side rule を
  前提にする（spec 02 §Layout）ため、LR(1) 文法に直接は載らな
  い。レイアウト解決パスを先に通せば LR(1) 化できるが、そうする
  と lalrpop の「1 つの `.lalrpop` ファイルで文法を宣言的に書
  く」という利点が薄れる。エラー報告も別立てで書くことになる。
- **`nom`（パーサコンビネータ）**。入力がバイト列前提の設計で、
  トークン列を扱うには wrapper を書くコストがある。加えて
  indentation や Pratt 演算子の自然な記述にならない。
- **`chumsky`（抽象的なコンビネータ）**。トークン列も扱えるが、
  エラー型（`chumsky::error::Simple` / `Cheap` / `Rich`）は
  Sapphire 側の `ParseError` / `ParseErrorKind` とマッピングさせ
  る中間層を挟むことになる。L2 で **spec 06 の双方向型付け判定**
  を診断に織り込む方針なので、パース段のエラー表現を自前 ADT で
  握っておいた方が後の接続が素直。また chumsky の reductive な
  API（`.then_ignore` / `.repeated` 等）は Pratt 的な優先順位を
  宣言的に書くのに若干回り道で、自前で書いてもコード量が大差な
  い。

### 採った設計

- レキサ（I3）と同じく **手書き・stdlib のみ**。外部 crate 追加
  なし（`Cargo.toml` 変更なし）。
- トップレベルは再帰下降、式層のみ **Pratt 演算子パース**（spec
  05 §Operator table の tier / assoc 固定表を走査）。非結合の
  tier 4（比較演算子）は `a < b < c` を検出してエラー化する。
- 公開 API は `parse(src: &str) -> Result<Module, ParseError>`
  を lex + layout + parse の一気通貫として提供。`parse_tokens
  (&[Token]) -> Result<Module, ParseError>` をテスト用にも公開。
- パーサ自身は `sapphire_core::ast::{Module, Decl, Expr, Pattern,
  Type, Literal, Scheme, ...}` を構築する。AST ノードは全て
  `Span` を持ち、I3 由来のバイトオフセット系と同じ座標系を共有。

## レイアウト解決を独立パスに切った理由

`crates/sapphire-compiler/src/layout/` は `Vec<Token>` を
`Vec<Token>` に書き戻す純関数。Haskell 98 の off-side rule を
なぞり、仮想 `{`・`;`・`}` を同じ `TokenKind` で注入する（空の
`Span` を anchor として持たせる）。

- **レキサを肥大させない**。字句層は spec 02 の byte-level 仕様
  のみを担当する。spec 02 §Layout は logical な規則（`where` /
  `let` / `of` / `do` の block 開閉）なので、レイヤを分ける。
- **パーサをレイアウト非依存にする**。explicit braces
  （`case e of { a -> b ; c -> d }`）と layout-driven
  （`case e of\n  a -> b\n  c -> d`）は layout 通過後に同形に
  なる。パーサ側の条件分岐が減る。
- **L2（診断 UI）との相性**。不整合インデントをレイアウト段で
  検出し `LayoutError` として surface する余地を残す。今回は
  必要最小限（`UnclosedExplicitBlock` / `MissingEof`）のみだが、
  将来 "expected at column N" 系を足しやすい。

### 具体的アルゴリズム（Haskell-98 風）

- スタックに `Implicit { col, opener, fresh }` と `Explicit` を
  積む。トップレベルは `Implicit { col: 0, TopLevel }` を初期
  に積み、先頭で仮想 `{` を出す。
- `where` / `let` / `of` / `do` が来たら `pending_open` を立て、
  **次の非トリビア・トークン** を見て：
  - その token が `{` なら `Explicit` を push（ユーザー明示の
    ブロック）。
  - そうでなければ仮想 `{` を出し、`Implicit { col = その
    token の column, opener, fresh = true }` を push。column は
    `Indent(col)` が直前にあればその値、同一行オープナー
    （`let a = 1` のように `let` と `a` が同じ行）なら source
    バッファから re-compute（`column_of`）。
- 行頭トークン（直前に `Indent` があったもの）が来たとき、
  `col < ref_col` なら trailing `}` を emit して pop を繰り返し、
  `col == ref_col` かつ block が `fresh` なら fresh を下ろすだけ、
  非 fresh なら `;` を emit。
- `in` は最寄りの `Implicit { opener: Let }` を明示的に閉じる
  （indent がまだ下がっていなくても）。途中に積み重なった他の
  Implicit は順に閉じる。
- EOF で、`pending_open` が立っていたら空の `{}` を補って閉じ
  （`module Foo where` しかないファイルのため）、残りの
  Implicit も全て閉じる。`Explicit` が残っていたら
  `UnclosedExplicitBlock`。

`fresh` フラグは「block の最初のトークンには `;` を付けない」を
実現するための state。Haskell 2010 の L 関数はこれを `m < n`
分岐の初回発火で表現するが、本実装では明示フラグの方が読みやす
い。

`column_of` はソース中のバイトオフセットから 0-based code-point
column を再計算する補助。レキサが `Indent(col)` を出すのは論理
行頭のみなので、**同一行 opener** の column を知るにはソースを
引く必要がある（`let a = 1` の `a` は `Indent` を持たない）。
`resolve_with_source` はソースを渡す版の entry point。`resolve`
はソース無しでも動く（column 不明時は `usize::MAX` を記録し、
次の indent が下がった時に閉じる）。

## AST を `sapphire-core` に置いた理由

- **LSP 共有**。`sapphire-lsp`（L1+）は公開 API として AST を
  露出する可能性がある（completion の候補を AST ノードから生成
  する、hover に型情報を乗せる等）。`sapphire-compiler` に直接
  依存すると lexer/parser 以外の全パイプラインを引き連れる。
- **将来の I5+ の分離**。name resolver / type checker は AST を
  読み書きするが、parser そのものには依存しない。表現型を
  `core` に置いておけば parser と資源を切り離せる。
- **`Span` の単一化**。I3 時点では `Span` は `sapphire-compiler
  ::lexer::Span` にあったが、I4 で `sapphire-core::span::Span`
  に昇格して両方から使う形に統一した。`sapphire-compiler::lexer
  ::Span` は `pub use` の再エクスポートで互換を保つ。

## スコープ

### パーサが扱う具象構文

- 式：`literal`、`lower_ident`、`upper_ident`、関数適用、ラムダ
  `\x y -> e`、`let x = e in body`、`if c then t else f`、
  `case e of ...`、`do { ... }`、レコードリテラル / update /
  field 参照、リストリテラル `[...]`、括弧、`(op)` 演算子参照、
  `-e` 単項マイナス、二項演算子（Pratt）、`Foo.Bar.baz`
  modqual Var、`Foo.Ctor` modqual constructor。
- パターン：ワイルドカード `_`、変数、`x@pat`、リテラル、
  コンストラクタ `C p₁ ... pₙ`（位置引数）、cons `::`、
  リストリテラルパターン `[p, q, r]`、レコード `{ f = p }`、
  type annotation `(pat : type)`。
- 型：`TVAR` / `TCON`（qualified 可）、関数型 `->`、応用、
  レコード `{ l : τ }`、forall 導入 `forall a b. τ`、
  制約 `Ctx => τ`。
- 宣言：`name : scheme` 署名、`name pat... = expr` 値定義、
  `data T a = C₁ τ | ...`、`type T a = τ`、
  `class Ctx => C a where ...`、
  `instance Ctx => C τ where ...`、
  `name pat... := "ruby source"` Ruby 埋め込み（spec 10）。
- モジュール：`module Name (exports) where`、`import Name`、
  `import qualified`、`as Alias`、`hiding (...)`、選択的 import
  `(x, Bar(..), class Eq)`。
- レキサへの追加：**triple-quoted string** `"""..."""`（spec 10
  §Triple-quoted string literals）を `TokenKind::Str(String)`
  と同一の kind に畳んで出す。改行・`\` エスケープ等の取り回し
  は spec の通り。

### 意図的に除外したもの

- **演算子セクション** `(+ 1)` / `(1 +)`（05-OQ4, DEFERRED-LATER）。
- **パイプ演算子** `|>` / `<|`（05-OQ5, DEFERRED-IMPL）。
- **複数引数 `let`**（03-OQ2, DEFERRED-IMPL）：パーサは単一
  binding しか受け付けない。
- **`let` の LHS パターン** `let (x, y) = e in ...`（06-OQ5,
  DEFERRED-IMPL）：現状は `let name pat... = e` の形のみ。
  M9 例題がこの形を要求しないので十分。
- **ガード** / **or パターン**（06-OQ1 / 06-OQ2, DEFERRED-LATER）。
- **範囲パターン** `1..10`（06-OQ4, 計画しない）。
- **診断 UI の細かな精緻化**。基本的な `Expected { expected,
  found }` / `NonAssociativeChain` / `UnsupportedFeature` 等の
  ADT は整えたが、「どの親構文を parse していたか」のスタック
  情報は L2 送り。

## 既知の弱点

- **パーサは曖昧な場合 deterministic に早期 branch する**。
  例：`looks_like_context()` は FatArrow を先読みして「制約節が
  ある」と判定するが、ネストしたスコープ / 文字列内の同名記号に
  騙されないよう paren depth を保つだけ。現状これで M9 例題と
  Prelude 書式は通るが、より複雑な境界が出た場合は elaboration
  層で再評価する。
- **infix-LHS method clause**（`x == y = not (x /= y)` 形式）の
  判定は heuristic：`parse_apat()` 一つ読んで次が演算子っぽか
  ったらその形と解釈する。ただし clause の1つ目 apat が `Var`
  でない場合（たとえばリテラル）でもこの分岐に入るので、spec
  07 が許す範囲で "operator method" 判定が緩い可能性がある。
  I5（name resolution）で method 名の妥当性を検査する想定で、
  パーサ段では緩めに通す。
- **レイアウト解決の `column_of` は O(byte offset)**。毎回
  ソースの先頭から直近の改行までスキャンしているため、同一行
  opener が多いファイルでは累積コストがある。M9 規模では無視
  できる。将来は `span.start → column` の事前テーブルを張れば
  O(1) になるが、YAGNI。
- **record update の判定** `looks_like_record_update()` は
  `{` の中で top-level `|` を top-level `=` より先に見るかどう
  かの heuristic。record literal 内のラムダに `|` が現れる可能
  性は `let ... in ...` の一部や `\x -> ...` の式スコープを
  想定すると実質的にない（`|` は ADT 宣言と record update のみ
  で使う）。M9 例題は通るが、相互運用を増やすならカッコ深度に
  加えて「最初のトークンが `lower_ident =` なら literal」の早期
  判定を入れる余地がある。
- **`:=`** は lexer が `Op(":=")` として出すので parser 側で
  照合する。専用の `TokenKind::ColonEquals` を置くほうが綺麗だ
  が、既存の lexer 不変条件を壊さないため fallback 経由にした。
  変更は monotonic なので I5 以降で足せる。

## 新規 Open Questions

レイアウト / パース中に浮上した議論点を `docs/open-questions.md`
§1.5 に追加した。詳細は同ファイルの I-OQ34 以降を参照。

## フィットネス

- `cargo fmt --all -- --check` pass
- `cargo check --all` pass
- `cargo clippy --all-targets --all-features -- -D warnings` pass
- `cargo test --workspace` pass（layout 8 / parser 68 / lexer 44
  / ほか = 127 ケース）
- 4 例題（`examples/sources/01-hello-ruby` 〜
  `04-fetch-summarise`）を `parse_dump` で AST まで出せる。
  スナップショットは `examples/parse-snapshot/` に格納。
