# 20. 型検査 I6c: 型クラスと higher-kinded types

Status: **draft**. I6c 着地 commit 時点のメモ。

## スコープ

spec 07 (type classes / HKT) と spec 11 (Ruby monad) を支える部
分：

- kind `*` と `* -> *` 程度を最低扱えるようにする。kind annota-
  tion は無し (07-OQ5 DEFERRED-LATER)。
- single-parameter class 宣言 / instance 宣言の登録。
- constraint 付き scheme。
- constraint resolution (instance matching + superclass entailment)。
- `do` 記法の desugar (`>>=`)。
- spec 09 の prelude classes / instances を静的に注入。
- spec 11 の `Monad Ruby` / `Functor Ruby` / `Applicative Ruby`
  instance を同様に注入。

## kind 体系

`Kind::{Star, Arrow, Var}`。Var は未決定 kind のためのプレースホ
ルダ (内部のみ)。

- `Int / String / Bool / Ordering` は `*`。
- `Maybe / List / Ruby` は `* -> *`。
- `Result` は `* -> * -> *`。

kind 検査は弱い: `Ty::App(f, x)` を unify するときは両辺の
`Ty::Con` name が一致するかと structural equality しか見ていない。
mis-arity は `UnknownType` / `Mismatch` として普通の unification 失
敗経由で届く。厳密な kind inference は spec 07 §Kind system の記
述通りだが、M9 の 4 例題は explicit signature ですべて記述されて
いるため、この簡略で通った。

## class / instance の登録

### class 宣言

`ClassDecl { context, name, type_var, items, ... }` を `register_ast_class`
で処理する。

- super class 名は `context` から抽出し `superclasses` に入れる。
  このとき各 superclass の引数が **class 自身の type 変数 1 つ**に
  一致することを検査する。spec 07 §Class declarations が
  「Superclass constraints come before the class head [...] using
  the same `context '=>'` form」と規定するとおり、superclass context
  は class が束縛する tvar のみを constrain できる。
  `class Foo b => Ord a where ...` のように無関係な `b` を使うものは
  `InvalidSuperclassContext { class, expected, got }` で reject する。
- 各メソッド signature はそのまま `ClassInfo.methods` に入れる。
- 加えて、prelude と同じ経路で "`class_constraint + body`" 形式
  の scheme を `TypeEnv.globals` に登録する。これにより `compare x y`
  等の呼び出しが `Ord a => ...` scheme として instantiate される。
- `ClassInfo.home_module` に現在の module 名を記録する (orphan 検査で
  参照)。prelude classes は `"Prelude"` を入れる。

### instance 宣言

`register_instance_head` → `check_instance_body` の 2 段。

- head shape: spec 07 §Instance declarations の "ground or ctor
  applied to distinct tyvars" を `ast_head_vars` で確認。
- 重複検査: 既存 instances の head と unify できたら
  `OverlappingInstance`。
- orphan 検査: spec 07 §Orphan instances を **strict に実装** する。
  instance `instance C T` に対して、`C` の `home_module` あるいは
  `T` の outermost type constructor の `home_module` のいずれかが
  現在の module と一致するときだけ admit。どちらも別 module 由来なら
  `OrphanInstance { class }` で reject。
  - `ClassInfo.home_module` / `DataInfo.home_module` で追跡。Prelude
    由来のものはどちらも `"Prelude"`。
  - 構造的 record のように head constructor を持たない head は、class
    が同一 module のときに限り admit。
  - 実装は `register_instance_head` の orphan 判定パスで行う。overlap
    check より前に走るので、"prelude に既にある instance を user module
    で上書き" という意図のソースは OrphanInstance として弾かれる
    (overlap まで進まない)。
- 各 method clause は class の method scheme に「class tvar ↦
  instance head」substitution を入れて作った期待型に対して
  `check_clause_against` で検査する。

### superclass chain の扱い

`ClassEnv::super_closure(class)` で `class` 自身 + 再帰的な super
を全部集める。assumption → wanted の entail は、assumption の super
closure に wanted.class があれば OK。これにより `Ord a` assumption
から `Eq a` が自動で出る。

## constraint resolution

`classes.rs::resolve_constraint`：

1. `wanted.arg` が TyVar ならば assumption (or その super) だけで
   entail を試す。だめなら "deferred" として wanted を返す (呼び
   出し側が一般化時に context に昇格する)。
2. concrete type ならば assumption を試した後、instance 一覧を走査。
   head を fresh vars に refresh して unify を試す。複数マッチす
   れば `OverlappingInstance`、0 マッチなら `UnresolvedConstraint`。
3. マッチした instance の context を再帰的に simplify。

`simplify(env, assumed, wanted)` が top-level entry。

## do 記法の desugar

`desugar_do(stmts, span)` を infer_expr 内で呼ぶ。spec 07 §do
notation 通り:

- `pat <- e ; rest`  →  `e >>= \pat -> rest`
- `let x = e ; rest` →  `let x = e in rest`
- `e ; rest`         →  `e >>= \_ -> rest`
- 末尾 `e` は `e` そのまま。

desugar 後は通常の `infer_expr` を再帰呼び出しするだけ。`>>=` は
prelude の `Monad m => m a -> (a -> m b) -> m b` として lookup さ
れるので、constraint resolution が `Monad Ruby` / `Monad Maybe` /
`Monad (Result e)` を決める。

## hint propagation (bidirectional の軽量版)

HM の通常順序 (`func` を先に infer、次に `arg`) では `filter (\s -> s.grade == g) students`
のような "lambda の param に record field access する" パターンで
`s` の型が決まらない。対策として `infer_expr(App { func, arg })`
内で：

- `func` を infer して得た型が `Fun(a, b)` なら、`arg` 推論時に
  `a` を hint として渡す。
- `arg` が lambda の場合、hint の param type を lambda の param
  に直接 assign して body 推論を行う。

これは bidirectional typechecking のごく軽い形。完全な bidirec-
tional ではないので、より複雑なパターンでは type annotation に
頼ることになる。

## prelude / Ruby monad instance

`install_prelude(ctx)` で下記を静的に投入する：

- spec 09 の `Bool / Ordering / Maybe / Result / List` ADT とそ
  の ctor scheme。
- spec 09 / 07 の `Eq / Ord / Show / Functor / Applicative / Monad`
  class 宣言。superclass chain は `Eq ⊂ Ord`, `Functor ⊂ Applicative`,
  `Applicative ⊂ Monad`。
- spec 09 の具体 instance (`Int / String / Bool / Ordering` に
  `Eq / Ord / Show`; `Maybe / List / Result e / Ruby` に `Functor
  / Applicative / Monad`)。
- spec 09 の operator / utility bindings (`+`, `map`, `foldl`, …)。

`Ruby` は spec 11 §The type 通り `Monad` instance として登録。

## エラー報告

- `UnresolvedConstraint { constraint }`: missing instance。
- `OverlappingInstance { class, head }`: 重複 instance。
- `InvalidInstanceHead { class, head }`: Haskell-98 形式違反。
- `UnknownClass { name }`: 型位置で未知 class を参照。
- `InvalidDoFinalStmt` / `EmptyDo`: `do` ブロック構造違反。

## 代替検討と却下理由

- **Evidence dictionary を AST に注入する方式**: I6c 時点では避
  けた。codegen (I7c) が Constraint → dictionary の変換を行う方
  針で、I6c は "instance を type-level で確定する" ところまで。
  これにより I6c のコード量を抑えつつ、I7 着手時に型情報を
  `check_program` の出力から素直に拾える。
- **Functional dependencies / TypeFamilies**: 07-OQ1 / 07-OQ7 と
  もに deferred。MTC 実装は最初の実装後に再訪。
- **OverlappingInstances / IncoherentInstances**: 07-OQ3 で
  `no overlap` 維持。

## 次段 (I7 codegen) との接続

- class / instance 環境は `check_program` 戻り値には現状含めて
  いない。I7 着手時に `TypedProgram` 経由で露出するか、I6c 終了
  時に `ctx.class_env` を保存するかは I7 設計で決める。
- `do` 展開結果の AST は現在 in-place で展開しているが、typed_ast
  化するなら別 AST tree を生成する必要がある。I-OQ57 で記録。
