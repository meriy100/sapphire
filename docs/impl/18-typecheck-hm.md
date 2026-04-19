# 18. 型検査 I6a: Hindley–Milner コア

Status: **draft**. I6a 着地 commit 時点の設計メモ。以降の ADT (I6b)
/ classes (I6c) で追記 / 更新されうる。

## スコープ

spec 01 §Typing rules と §Type schemes を満たす Hindley–Milner を
Rust で実装する。具体的には：

- unification variables, rigid `forall` vars, let-polymorphism。
- lambda / application / `let` / `if` / `case` / literals。
- signature 付きバインディングは署名のスキームを期待型として採
  用。signature なしは推論 + 一般化。

I6a 単体では ADT / record / class の detail は開けるだけ開けてお
くが (ctor 登録のフックなど)、本体は I6b / I6c で埋める。

## 採用アルゴリズム

古典的な **Algorithm W** + **let 一般化**。教科書 (Pierce TAPL
§22, Sulzmann & Jones 1999) の記述に近い形で書いた。Constraint
ベース (GHC の `inside-out`) は採らない理由は下記 §代替検討。

Unification は Robinson の定型アルゴリズム、occurs check あり。
`Ty::Var(id)` を介してサブスティチュートを重ねる。`Subst::apply`
はチェーンを追って固定点で評価するが、循環 subst が入った時は
visited set で検出して無限ループしないようにしている。

## 型表現

`crates/sapphire-compiler/src/typeck/ty.rs` を参照。

- `Ty`: `Var` / `Con` / `App` / `Fun` / `Record`。`Con` は kind
  フィールドを持つが unification では name のみ比較 (kind は
  diagnostics 用)。
- `Scheme`: `forall vs. (ctx) => body`。vars / ctx が empty の
  とき monotype。
- `Subst`: `HashMap<u32, Ty>`。`compose` は `self ∘ other`。

## 推論の流れ

モジュール単位で次の 6 フェーズを順に走る (`infer::check_module`).

1. **Phase A** (data / alias / class): `data` 宣言は `TypeEnv.datas`
   と ctor scheme を登録。`type` alias は `TypeEnv.aliases` に入
   れて `ty_from_ast` で expand。`class` は class_env に登録 +
   メソッドの global scheme を追加。
2. **Phase B** (signatures): 署名をパースして `Scheme` を作り、
   `TypeEnv.globals` にその名前で事前登録。
3. **Phase C** (provisional): 署名なし値バインディングに fresh
   monotype `Var` を入れておく (相互再帰の準備)。
4. **Phase D** (instance heads): instance 宣言の head / context を
   登録するだけ。bodies は phase F で。
5. **Phase E** (value bindings): 各 binding ごとに `infer_value_clause` →
   既存 scheme と unify → `generalize`。署名あり case は制約が
   署名で許可されているかを検査。
6. **Phase F** (instance bodies): 各メソッド clause を class の
   method scheme に対して check。

Phase D を先行することで、E で `combine 1 2` のような呼び出しに
対して `Semi Int` instance を resolve できる。

## 型抽出 (AST Type → Ty)

- `ty_from_ast(ctx, ast_ty, locals)`。locals は "この scheme の
  `forall` vars" を fresh TyVar に mapping。
- alias 名は expand (spec 09 transparent rule)。
- data 名は `TypeEnv.datas` に見つかれば kind `* -> *` 等を計算し
  てくれる。未知名は `UnknownType` エラー。

## let 一般化

`InferCtx::generalize(sub, ty)` を使う。手順：

1. `sub.apply(ty)` して最終形を得る。
2. `ty` の free var ids を列挙し、ENVIRONMENT の free var ids を引
   いて "一般化してよい var" を確定。
3. `InferCtx::wanted` のうち、"一般化してよい var" を含む制約だけ
   を promoted にする。他は wanted に残す (let 上位の binding が
   受け取る)。
4. 一般化 var を rigid に rename して scheme の `vars` にする。

一般化は let の `in` 到達時と top-level binding 完了時の二箇所
で起こる。

## エラー報告の形

`crates/sapphire-compiler/src/typeck/error.rs` の `TypeErrorKind` が
列挙型で、各 variant が message-layer から簡単にレンダリングでき
る形。`Display` 実装はそのまま CLI で出せる。

- `Mismatch { expected, found }`: unify 失敗時の一本化エラー。
- `OccursCheck { var, ty }`: infinite type。
- `UnknownType`, `UnknownClass`: name lookup 失敗。
- `UnresolvedConstraint`, `AmbiguousConstraint`: class 解決時。
- `MissingField`, `RecordPatternField`, `CtorArity`: データ構造系。
- `OrphanInstance`, `OverlappingInstance`, `InvalidInstanceHead`:
  instance 登録時。
- `InvalidDoFinalStmt`, `EmptyDo`: do-sugar desugar 時。

全エラーは `Span` 付き。LSP (L3) 以降で diagnostic に変換可能。

## I7 codegen との接続インターフェース

I7 はまだ着手前だが、現段階で引き渡せるものを決めておく：

- `TypedProgram { modules: Vec<TypedModule> }` / `TypedModule { id, schemes }` は
  最低限。codegen は binding 名から scheme を引ける。
- I6 layer はまだ AST に型注記を back-annotate しない (side table
  でもない)。codegen はスキーム + AST + resolver の reference table
  で回す想定。evidence dictionary は I7c で instance env から再計
  算する。

I-OQ57 (次章) で「side-table の型付き AST を持つ方がよいか」を OPEN
のまま登録する。

## 代替検討と却下理由

- **Constraint-based (OutsideIn(X))**: 記述力 / エラーメッセージは
  優れるが、実装コストが大きい。Haskell 98 相当の MTC だけを目指
  すなら algorithm W で十分。MPTCs / TypeFamilies を入れる段階で
  再評価。
- **Bidirectional typing + 完全型付け AST**: I6 で全 AST ノード
  に `type_of` を持たせることも可能だが、現状の LSP 程度のユース
  では scheme per top-level があれば十分。I7 codegen 着手時に改
  めて判断。
- **row-polymorphism for records**: spec 04 は closed records を
  選択済み。従って field access には expected な record 型が必
  要。現行は pending_fields で "後で決まるかもしれない" を許容。
  不足なら type annotation を user に書かせる方針 (M9 3 例題は
  この緩和で通った)。

## 現状の既知の緩み (I-OQ57 候補)

- ambiguous constraint (generalize で一般化変数が body から消える
  場合) を現行は wanted に残すだけ。厳密には `AmbiguousConstraint`
  を上げるべき。
- multi-clause binding の clause 間パターン被覆 (exhaustiveness) は
  検査していない。codegen 前に必要になる。
- class method の "minimal complete definition" 違反 (例えば `Eq`
  で `==` も `/=` も書かない) を検査していない。
