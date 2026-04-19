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

`InferCtx::generalize_excluding(sub, ty, exclude_name)` を使う
(薄いラッパ `generalize(sub, ty)` は `exclude_name = None`)。手順：

1. `sub.apply(ty)` して最終形を得る。
2. `ty` の free var ids を列挙し、ENVIRONMENT の free var ids を引
   いて "一般化してよい var" を確定。
3. `InferCtx::wanted` のうち、"一般化してよい var" を含む制約だけ
   を promoted にする。他は wanted に残す (let 上位の binding が
   受け取る)。
4. 一般化 var を rigid に rename して scheme の `vars` にする。

一般化は let の `in` 到達時と top-level binding 完了時の二箇所
で起こる。

### `exclude_name` が必要な理由 (self-slot pinning)

`let f x = x in ...` の型付けは、spec 03 の暗黙再帰に合わせて「`f` を
自分自身の body の中で使える」状態で処理する。実装は先に
`bind_local("f", Scheme::mono(α))` で provisional な monotype を
locals に入れ、その状態で body = `\x -> x` を推論してから α を
一般化する。

ところが単純な `generalize(sub, α)` では、generalize 計算時に
locals に残った `f: mono(α)` 自身が `env_fvs` の計算対象になり、
α が env に束縛されているとみなされて一般化できない。`let f x = x
in (f 1, f "a")` のような 2 箇所利用で、f が実際は `α → α` の
monotype のままになり、1 箇所目の `f 1` が α を `Int` に unify した
後、2 箇所目の `f "a"` が α を `String` に unify しようとして
clash する——というのが本来の挙動。現状はこの直後の
`Subst::compose` が "先勝ち" なので clash を silent に落とす
(後発の binding で先行値を上書きしない) が、これは偶然成立して
いるだけで正しい HM ではない。

`generalize_excluding(sub, α, Some("f"))` は env_fvs 計算から
`f` の self-slot を取り除く。同じ保護は top-level 値バインディングの
`check_value_binding` にも必要で、こちらは phase C で
`Scheme::mono(Var(tv))` を `globals` に入れる実装なので、該当する
`(current_module, name)` エントリを env_fvs から除外する。

### 具体的なトレース例

ソース: `let f x = x in (f 1, f "a")` (タプルは記述用、実装では
record `{ a = f 1, b = f "a" }` で pair 代替)。

1. push_locals、α = fresh。bind `f: mono(α)` を locals 最深フレームに
   入れる。
2. body `\x -> x` を infer: param β = fresh、body `x` = β。lambda ty =
   `β → β`。ボトムアップ unify: unify(β → β, α) → sub = `{α ↦ β → β}`。
3. `generalize_excluding(sub, α, Some("f"))`:
   - `sub.apply(α) = β → β` なので ty_fvs = {β}。
   - locals フレームを walk: `f` は exclude で skip、globals は Prelude
     以外 `main: mono(γ)` (γ は top-level main 用の fresh) だけ。β は
     どちらにも現れない。
   - env_fvs = {γ}、gen_vars = {β}。β を rigid にして
     `forall β. β → β` を返す。
4. locals の `f` を `Scheme forall β. β → β` で置き換える。以降の使用は
   instantiate で fresh なコピーを得る。
5. body で `f 1`: lookup f → forall β. β → β、instantiate → `β₁ → β₁`、
   unify with `Int → res` → β₁ := Int、残 `Int`。
6. `f "a"`: 別 instantiate → `β₂ → β₂`、unify with `String → res'` →
   β₂ := String、残 `String`。
7. Record 内に {a = Int, b = String} を構成。ここで β₁ と β₂ は独立な
   tvar なので clash しない。

`exclude_name = None` の方は既存の意味論を保持しており、呼び出し元
(signature 付き binding) は excluded 指定をしない。

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
