# 15. リゾルバ設計メモ

I5 で `crates/sapphire-compiler/src/resolver/` に導入した名前解決
パスの設計メモ。正本は `docs/spec/08-modules.md`（§Abstract syntax
/ §Visibility / §Name resolution）および
`docs/spec/09-prelude.md`（§The prelude as a module）。本文書は
**CLAUDE.md §Phase-conditioned rules** の「Rust 固有の実装判断は
コード変更前に `docs/impl/` に記録する」方針に則って、I5 で下した
判断の根拠を残しておくもの。

## 入出力の契約

- 入力：`sapphire_core::ast::Module` のリスト。I4 パーサが
  吐くそのまま。`examples/sources/04-fetch-summarise/` のような
  マルチモジュール構成では `Vec<Module>` を一度に渡す。
- 出力：`ResolvedProgram { modules: Vec<ResolvedModule> }`。
  - `ResolvedModule.ast` には元の AST を **そのまま** 保存する。
  - `ResolvedModule.env` に `ModuleEnv`：トップレベル宣言表、
    unqualified scope、`as` alias、export 表。
  - `ResolvedModule.references` は `HashMap<Span, Resolution>`：
    AST 内の参照箇所（`Expr::Var` / `Expr::OpRef` / `Expr::BinOp`
    の op / `Pattern::Con` / `Type::Con` / クラス制約の class
    名など）を `Local { name }` / `Global(ResolvedRef)` に写す
    side table。

エラーは `Vec<ResolveError>` を一度に返し、best-effort で蓄積する。
一件目でピクるのではなく、同じモジュール内で複数の undefined /
ambiguous を束ねて報告したい（最終的には L2 診断層と接続する）
ため。

## resolved AST の扱い：既存 AST + side table

もっとも自然な設計は `resolved` モジュールを別に切って `Expr`
/ `Pattern` / `Type` の並行木を新設し、各参照ノードに
`ResolvedRef` を埋め込むことだが、**今回は採らなかった**。

- parser 側の `ast::Expr` と 1:1 で対応する mirror を維持する
  コストが高い（いずれ field が増えるたびに両側を直す）。
- 後続の I6（型検査）でまたノードを増やすことになり、AST が
  3 種類存在する状態は整理がつかない。
- L2 診断層は parser AST の span を握って位置解決するだけなので、
  resolved form を別建てにしても嬉しさが薄い。

採ったのは **既存 AST をそのまま保持し、`HashMap<Span, Resolution>`
で参照解決結果を脇に持つ** 方式。span は parser 段で既に全ノード
に入っているので識別子として流用できる（`Expr::Var::span` など）。
LSP から `Goto Definition` を書くときもこの表を引けば足りる。

もし将来 span 衝突（同じ span に複数の参照が載る）が起きた場合は、
`Ref` 用の専用 ID 型を導入して AST ノード側に `Option<RefId>` を
足す経路に切り替える。現状 M9 例題の範囲では衝突はなく、
`Expr::BinOp` の op 位置は `left.span().merge(right.span())`
という合成 span で避けている（ここだけ I-OQ43 として未解決）。

## prelude の暗黙 import テーブルの置き場所

spec 09 §The prelude as a module は「`import Prelude` が暗黙に
先頭に差し込まれる」とする。I5 では prelude 自身をまだ `.sp` で
書いていないため、**静的テーブル** として `resolver/prelude.rs` に
ハードコードした。

- `PRELUDE_VALUES: &[(&str, bool)]`：値側。真偽値はコンストラクタ
  か否か（`True` / `Nothing` / `Cons` / ... は `true`、関数・
  演算子は `false`）。
- `PRELUDE_TYPES: &[(&str, bool)]`：型側。真偽値はクラスか否か
  （`Eq` / `Ord` / ... は `true`、`Bool` / `Maybe` / `Int` /
  `String` / `Ruby` などは `false`）。
- `Int`、`String` のような spec 01 のビルトイン primitive 型は
  spec 09 には明示列挙されていないが、I5 が `Int` を undefined と
  して蹴るわけにはいかないので PRELUDE_TYPES に載せてある。
  `Ruby` は spec 10 / 11 の `Ruby a` モナドの頭。いずれも spec 09
  の「prelude が暗黙 import 対象」の傘に入れている。

spec 09 が更新されて prelude が増えたら、このテーブルも同じ commit
で追随する。将来 `lib/Prelude.sp` を書いて user module と同じ
パイプラインで compile する段になったら、このテーブルを削って
user module としての `Prelude` を参照させる（I-OQ44 で追跡）。

## 名前空間分離の実装方式

spec 08 は 2 つの名前空間を想定する：

- `Value`：値束縛、関数、値コンストラクタ、クラスメソッド、
  `:=` 形式の Ruby 埋め込み。
- `Type`：データ型、型別名、クラス名。

parser は位置で判断せず `Type::Con` / `Expr::Var` / `Pattern::Con`
のように node 種別で位置を既に分けているので、I5 は単純に lookup
時に `Namespace` を添えるだけでよい。`ModuleEnv.top_level_index`
は `(String, Namespace) -> usize` のキーを取るし、
`ModuleEnv.exports` は `Value` / `Type` ごとの map を持つ。

同名の値と型は共存できる（例：spec 09 の `fst` 関数と `fst`
field 名が同じ点は 04 §Design notes が触れる。**field 名は名前空間
の外**）。`data Maybe` と関数 `maybe` は型 / 値で別物。

## 08-OQ4（相互再帰）について

spec 08 §Cyclic imports は module DAG を要求する。I5 はこれを
**reject する側** として実装した：`detect_cycles` が DFS で循環を
検出し、`ResolveErrorKind::CyclicImports` を返す。相互再帰は
DEFERRED-LATER（08-OQ4）のままで、将来 Haskell の `.hs-boot` 相当を
導入するなら cycle 検出の手前に boot-signature を read する処理を
挟む想定。

## I4 パーサとのデータフロー

```
  source text
    │
    ▼
  lexer::tokenize
    │
    ▼
  layout::resolve_with_source
    │
    ▼
  parser::parse_tokens   ->   ast::Module
    │
    ▼
  resolver::resolve_program
    │    ├──> ModuleEnv    （top_level / exports / unqualified / qualified）
    │    └──> references    （HashMap<Span, Resolution>）
    ▼
  ResolvedProgram          <-- I6 以降はここから型検査 / elaboration
```

- parser AST は **immutable** に扱う。resolver は借用して読むだけで、
  ノードの再構成はしない。
- 参照解決は span で引ける。I6 は `Expr` を walk しながら必要な
  箇所で `ResolvedProgram.modules[i].references[&span]` を引けば
  その識別子の解決結果（local or global）を得られる。
- 複数モジュール compile は `resolve_program(vec)` に一括で渡す。
  `resolve(m)` は単一モジュール convenience wrapper。

## I6 以降への受け渡し

I6 の型検査が欲しい情報：

- 各トップレベル宣言の可視性（`Visibility::Exported` / `Private`）
  — spec 08 §Top-level signatures 境界規則で「export には
  signature が要る」を検証するのに使う。
- 各参照が local か global か、global ならどのモジュールの何か —
  型環境の lookup と signature の取得に使う。
- prelude export 表 — `Int` / `String` / class 名などを baseline
  の型環境へ突っ込む。

これらは `ResolvedModule.env` と `.references` から引ける。I6 実装
は Rust 構造体の派生型を増やすかどうかも含め、**I5 の出力を型
環境にどう変換するか** を別途 `docs/impl/16-typecheck.md` で決める
（予定）。

## 名前が見つからない / 曖昧な参照の扱い

spec 08 §Visibility は unqualified 参照が複数の import から見える
場合を **ambiguity** と呼び、use site で静的エラーにする。I5 は
`unqualified` を `HashMap<(name, ns), Vec<ResolvedRef>>` に
している点に注意：同じ原定義を指す import を dedup するので、
spec 08 §Re-exports の「re-export が合流しても同じ binding なら
OK」が自然に成立する。

qualified 参照 `Mod.x` は qualified alias 表で resolve するが、
**target モジュール側が `x` を export しているかの厳密チェックは
省略** した。理由は resolver の現状設計が target モジュールの
exports snapshot を import 段でしか保持しておらず、reference
解決段では再引きの仕組みが無いため。M9 例題の範囲では
`(Http, HttpError)` `(Http, NetworkError)` などの全列挙を
`import Http (get, HttpError(..))` 側でチェック済みで、bare
`Http.XXX` を書かないので実害はない。I-OQ45 として追跡。

## 私有型漏洩（08 §Visibility「Leaked private types」）

spec 08 は「exported な signature が private な type / class を
参照したら定義時に reject」とする。I5 は phase 6 で以下を検査：

1. exported な top-level value 宣言の `Decl::Signature` に登場する
   `Type::Con { module: None, name }` が、同モジュール内で private
   な `DataType` / `Class` を指していないか。
2. exported な `ClassDecl` の method signature 同様。
3. exported な `DataDecl` の constructor が private 型を引数に取って
   いないか。

**type alias は transparent** なので、private な alias を public
signature に載せても漏洩とは見なさない（spec 09 §Type aliases の
「alias は type-checker から見ると同じ型」という規定に基づく）。
これは spec 08 の字面では判然としないため、I-OQ42 で追跡する。

## テスト

`crates/sapphire-compiler/src/resolver/tests.rs` に 45 ケース。
主要なカバレッジ：

- local（lambda / let / case / do / 構造マッチ）
- トップレベル登録（value / data / class / type alias / Ruby embed）
- duplicate / undefined / ambiguous / unknown-qualifier / 漏洩
- export list（全公開 / 空 / `(T)` / `(T(..))` / `(class C(..))`）
- import（`(x)` / `T(..)` / `hiding` / `as` / `qualified`）
- prelude（values / types / constructors）
- multi-module + 相互再帰 reject
- M9 4 例題（01 / 02 / 03 / 04）を end-to-end に resolve する smoke

## 関連 OQ

新規 OQ は `docs/open-questions.md` §1.5 に I-OQ42〜I-OQ45 として
追加した。要旨：

- I-OQ42：型別名を私有型漏洩検査で除外することの正当性。
- I-OQ43：参照解決 side table の key（現状 `Span`）の将来。
- I-OQ44：`Prelude` を静的テーブルから `.sp` ソースへ移す時期。
- I-OQ45：qualified 参照の target-export 厳密チェック。
