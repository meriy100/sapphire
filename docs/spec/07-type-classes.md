# 07. Type classes and higher-kinded types

Status: **draft**. Subject to revision as the prelude (M6), module
system (M4), and the Ruby-evaluation type (M7/M8) land.

## Motivation

The 2026-04-18 direction pivot (see `docs/roadmap.md` §方針転換
メモ) set two goals:

- Sapphire targets **Haskell-class expressiveness**.
- Sapphire has a **general `Monad`** as a language feature, not a
  single bespoke `RubyEval` type.

Both goals converge on the same machinery: ad-hoc polymorphism
through **type classes**, and a **kind system** that admits type
constructors of kind `* -> *` so that `Monad m` can quantify over
`m`.

This document introduces:

- A kind system extending 03's implicit kind-`*`-only treatment.
- Type-class declarations, instance declarations, and constrained
  type schemes.
- A resolution discipline for instances.
- A set of standard classes (`Eq`, `Ord`, `Show`, `Functor`,
  `Applicative`, `Monad`) whose shape and laws are fixed here;
  whose actual prelude-side instances are the concern of M6.
- `do` notation as surface sugar over `Monad`.

It also revises earlier drafts. Document 03 stops being kind-`*`-
only. Document 05's equality and ordering operators become `Eq` /
`Ord` class methods (their Int-only types become the prelude
instance `Eq Int` / `Ord Int`). Document 04's row-polymorphism
question is unaffected — it remains an orthogonal axis.

## Kind system

A **kind** classifies types. At this layer kinds are given by:

```
κ ::= '*'
    | κ '->' κ
```

`*` is the kind of types that classify values (the only kind 03
used implicitly). `κ₁ -> κ₂` is the kind of type-level functions
from `κ₁` to `κ₂`. Kinds are right-associative, mirroring the type
arrow.

Examples:

- `Int`, `Bool`, `String` — kind `*`.
- `Maybe`, `List`, `IO`-like types — kind `* -> *`.
- `Either`, `(->)` the function arrow — kind `* -> * -> *`.
- Records are kind `*` (they classify record values).

Well-formedness of a type `τ` under an environment `Δ` of kind
assumptions is a judgement

    Δ ⊢ τ :: κ

whose rules are the natural ones:

- `Δ ⊢ T :: κ` if `Δ(T) = κ` (type-constructor lookup).
- `Δ ⊢ a :: κ` if `Δ(a) = κ` (type-variable lookup).
- `Δ ⊢ τ₁ τ₂ :: κ₂` if `Δ ⊢ τ₁ :: κ₁ -> κ₂` and `Δ ⊢ τ₂ :: κ₁`.
- `Δ ⊢ τ₁ -> τ₂ :: *` if `Δ ⊢ τ₁ :: *` and `Δ ⊢ τ₂ :: *`.
- Record types `{ f₁ : τ₁, ..., fₙ : τₙ }` are `::*` when every
  `τᵢ :: *`.

Only types of kind `*` may appear as value types (the type of a
Sapphire expression) or as argument types (inside a `->`). Type
constructors of higher kind appear only at quantified positions in
schemes and as heads of type applications.

**Kinds are inferred**, not annotated. Kind inference follows the
standard pattern (fresh kind variable per type variable, solve by
unification from usage sites). No source syntax for kinds is
admitted at this layer; whether to admit `(a :: * -> *)` in source
is OQ 5.

A `data` declaration `data T a₁ ... aₙ = ...` (03) gives `T` kind
`κ₁ -> ... -> κₙ -> *`, where each `κᵢ` is inferred from the uses
of `aᵢ` in the constructor argument types.

## Abstract syntax (BNF)

Extending 01, 03, 06:

```
decl   ::= ...                                     -- (01, 03)
         | 'class' context? TCON TVAR 'where' class_body
         | 'instance' context? TCON type 'where' instance_body

context   ::= constraint_list '=>'
constraint_list ::= constraint                      -- singleton or
               | '(' constraint (',' constraint)* ')'
constraint ::= TCON type                            -- class applied to a type

class_body ::= layout_block_of(class_item)
class_item ::= IDENT ':' scheme                     -- method signature
             | clause                               -- default method body

instance_body ::= layout_block_of(instance_item)
instance_item ::= clause                            -- method implementation

clause     ::= IDENT apat* '=' expr                 -- function-clause
                                                    -- definition, shorthand
                                                    -- for IDENT = \apat... -> expr

scheme ::= 'forall' TVAR* '.' context? type         -- (01 extended)
         | context? type                            -- surface shorthand

expr   ::= ...                                      -- (01, 03, 04, 05, 06)
         | 'do' do_block

do_block   ::= layout_block_of(do_stmt)
do_stmt    ::= pat '<-' expr                        -- monadic bind
             | 'let' IDENT '=' expr                 -- let binding (no 'in')
             | expr                                 -- monadic expression
```

`layout_block_of(…)` is the layout form already established by 02
/ 06 for `of`-style blocks, with explicit-braces-and-semicolons as
the escape hatch.

Notes on the BNF:

- `class_item` signatures use `IDENT ':' scheme` — the `:` type-
  annotation separator from 01. Operator-style methods use the
  parenthesised operator form `(==)` from 05, which is still a
  `IDENT` at this layer (treating `(==)` as an identifier with
  surface spelling given by the parenthesised operator).
- The `clause` form `IDENT apat* '=' expr` is a surface sugar used
  in class defaults and instance bodies (and, going forward, in
  ordinary top-level definitions): `f x y = body` is equivalent to
  `f = \x -> \y -> body`. 01's `decl ::= IDENT '=' expr` is the
  zero-parameter case (`apat* = ε`); this document generalises it
  uniformly for all definition sites. `apat` is the atomic-pattern
  class from 06.
- Operator-style class methods admit an additional surface form:
  infix-LHS clauses `x op y = body` for a method `(op)`. This is a
  convenience on top of the base `clause` grammar and is
  equivalent to `(op) = \x -> \y -> body`. The examples in
  §Class declarations and §Instance declarations below use this
  form for `(==)`, `(/=)`, and `::` patterns.
- `instance_item` bodies are `clause`s only; they have no type
  signatures because those come from the class.
- `scheme`'s extension adds an optional `context` before the type,
  expressing constraints like `Eq a => a -> a -> Bool`.
- `do_block` bodies use the same layout-sensitive block form as
  `case ... of`: reference column on the first `do_stmt`, new
  items at the reference column, semicolons separate in the
  explicit-brace form. `do` is a block-opening keyword; document
  02's layout list is extended accordingly.

### `<-` as reserved punctuation

Document 02's reserved-punctuation table is extended with `<-` for
the `do`-notation monadic bind. Like every reservation in 02, this
is additive and does not break any existing document.

## Constrained type schemes

A **type scheme** at this layer has the form

    ∀ a₁ ... aₙ . C₁ τ₁,₁, ..., Cₘ τₘ,kₘ => τ

where each `Cⱼ τⱼ,₁ ... τⱼ,kⱼ` is a class constraint on the
quantified variables. Generalization at `let` (01) and at the top
level (03) is extended: when generalizing a monotype `τ` under Γ,
the resulting scheme includes every constraint on `ftv(τ) \ ftv(Γ)`
that was collected during elaboration.

The constraint scopes over the entire type to its right: the type
`Eq a => a -> a -> Bool` is parsed as
`(Eq a) => (a -> a -> Bool)`, with the constraint binding the
entire arrow-shaped type that follows it.

01's (Var) rule is **replaced** (not merely augmented) by (Var')
below. Every rule of 01 now threads an ambient constraint set `Q`
(the constraints already known — either from the enclosing scheme
or from earlier (Var') firings in the same elaboration);
instantiating a constrained scheme at a use site produces not only
a monotype but also a constraint set that `Q` must discharge:

```
  σ = Γ(x)    instantiate(σ) = (τ, C)    Q ⊨ C
  —————————————————————————————————————————————    (Var')
                    Γ; Q ⊢ x : τ
```

`Q ⊨ C` means each constraint in `C` is either an element of `Q`
(i.e. already an assumption in the surrounding scheme) or has a
matching instance in scope by the rules of §Instance resolution.
The ambient `Q` is not made explicit elsewhere in this document's
rules to keep the notation close to 01's; conceptually all
judgements thread it through unchanged.

## Class declarations

```
class Eq a where
  (==) : a -> a -> Bool
  (/=) : a -> a -> Bool
  x /= y = not (x == y)
```

A class `class C a where ...` declares:

- A name `C`, added to the class environment.
- Method signatures `mᵢ : σᵢ`. Each method's *visible* scheme
  outside the class is `∀ a. C a => σᵢ` — the class parameter is
  quantified and constrained at the use site.
- Optional **default methods**: `mᵢ = eᵢ`. A default provides the
  method's behaviour when an instance omits it. Defaults may
  call sibling methods at the same class.

Superclass constraints come before the class head, using the same
`context '=>'` form as types:

```
class Eq a => Ord a where
  compare : a -> a -> Ordering
  ...
```

An `instance Ord T where ...` may presuppose `Eq T`: the compiler
either finds an in-scope `instance Eq T` or rejects the `Ord T`
declaration.

**Single-parameter** classes only at this layer. Multi-parameter
type classes (`class C a b where ...`) are OQ 1.

## Instance declarations

```
instance Eq Int where
  x == y = <primitive Int equality>
  -- /= not defined here; class default applies

instance Eq a => Eq (List a) where
  Nil       == Nil       = True
  Cons x xs == Cons y ys = x == y && xs == ys
  _         == _         = False
```

An `instance C T` declares that `T` (a type, possibly with free
variables) supports class `C`. The instance body provides
definitions for the class's methods; any method omitted receives
the class's default, or — if no default exists — is a static error.

Instance declarations may themselves carry a context (the `Eq a =>`
above). The context constrains the free type variables appearing
in the instance head.

**Instance heads** are restricted: the type applied to `C` must be
either

- a type constructor applied to distinct type variables
  (`Eq (List a)`, `Monad Maybe`, `Show (Tree a)`), or
- a ground type (`Eq Int`, `Show String`).

Bare type variables (`instance Eq a where ...`) and arbitrary
shapes (`instance Eq (List Int) where ...` with a concrete argument
inside a constructor) are not admitted at this layer. This is the
"Haskell 98" instance shape; relaxations are OQ 2.

### Orphan instances

An instance `instance C T` is an **orphan** if neither `C` nor the
outermost type constructor of `T` is defined in the same module as
the instance. Orphan instances are **forbidden** at this layer:
every instance must live in the module that defines either its
class or its type.

This is the strict Haskell policy. It preserves the "instance is
automatically in scope whenever both the class and the type are
in scope" invariant that downstream code (including M8's `Monad`
instance for the Ruby-evaluation type) depends on.

### Overlapping instances

Two instances `instance C T₁` and `instance C T₂` **overlap** if
some type `T` matches both (ignoring constraints). Overlapping
instances are **forbidden** at this layer: each class / type-head
pair must have at most one instance in scope.

Relaxations (the Haskell `OverlappingInstances` family) are OQ 3.

## Instance resolution

Given a constraint `C τ` at a use site, resolution proceeds:

1. Look up every instance `instance ctx => C T` in scope.
2. Find the unique one whose head `C T` unifies with `C τ`. If none
   unifies, the constraint is unsolved (a type error unless the
   surrounding scheme quantifies it into the context).
3. Recursively solve the instance's context `ctx` under the
   instantiating substitution.
4. Each solved constraint produces a **dictionary** at the
   implementation level (not user-visible). Dictionaries are
   threaded through method calls; this document does not fix
   the implementation strategy but mentions dictionaries to pin
   the complexity of resolution.

Resolution is **coherent** by construction under the no-orphan /
no-overlap policy: for any given use site and constraint, there is
at most one resolution path.

## Standard classes

These are fixed in MTC as **shapes** (member signatures and laws);
their prelude instances are the concern of M6. Laws are stated
non-normatively but are expected to be upheld by every instance.

### `Eq`

```
class Eq a where
  (==) : a -> a -> Bool
  (/=) : a -> a -> Bool
  x /= y = not (x == y)
```

Minimal complete definition: `(==)`. An instance must define
`(==)`; `(/=)` is provided by the default.

Laws: `==` is an equivalence relation (reflexive, symmetric,
transitive). `x == y` iff `not (x /= y)`.

### `Ord`

```
class Eq a => Ord a where
  compare : a -> a -> Ordering
  (<)  : a -> a -> Bool
  (>)  : a -> a -> Bool
  (<=) : a -> a -> Bool
  (>=) : a -> a -> Bool
  -- derived defaults in terms of `compare`; implementations omitted here
```

where `Ordering = LT | EQ | GT` is a prelude ADT (M6). `<`, `>`,
`<=`, `>=` all have default definitions in terms of `compare`;
**minimal complete definition is `compare`**, and an instance that
provides `compare` inherits the four comparison methods.

Laws: `compare` is a total ordering consistent with `==` from
`Eq`.

### `Show`

```
class Show a where
  show : a -> String
```

Laws: none (purely conventional for display / debugging).

### `Functor`

```
class Functor f where
  fmap : (a -> b) -> f a -> f b
```

Here `f :: * -> *`, inferred from its use in `f a` and `f b`.

Laws:
- `fmap id = id`
- `fmap (g . h) = fmap g . fmap h`

### `Applicative`

```
class Functor f => Applicative f where
  pure  : a -> f a
  (<*>) : f (a -> b) -> f a -> f b
```

Laws (identity, composition, homomorphism, interchange) — stated
as in Haskell; see Haskell's `Control.Applicative` documentation
for the full list.

### `Monad`

```
class Applicative m => Monad m where
  (>>=)  : m a -> (a -> m b) -> m b
  return : a -> m a
  return = pure
```

Laws:
- `return a >>= f ≡ f a`               (left identity)
- `m >>= return ≡ m`                    (right identity)
- `(m >>= f) >>= g ≡ m >>= \x -> f x >>= g`   (associativity)

`return` has a default in terms of `Applicative`'s `pure`; an
instance may still override it for clarity.

The `(>>=)` operator is added to 05's operator table at tier 1,
left-associative. Its type scheme under MTC is
`∀ m a b. Monad m => m a -> (a -> m b) -> m b`. (Operator table
amendment is applied as a forward reference from 05 to 07 —
05's table rows for `>>=` and friends will be filled in once M6
fixes the final prelude name set.)

## `do` notation

The `do` form is **surface sugar** over `Monad`. Its three stmt
forms desugar as follows. Let the block be

    do
      s₁
      s₂
      ...
      sₙ

(with `sₙ` being the final statement, which must be an `expr`
form — no bind or let is allowed as the last statement).

- A `pat <- e` statement followed by the rest of the block `rest`
  desugars to `e >>= \pat -> desugar(rest)`.
- A `let x = e` statement followed by `rest` desugars to
  `let x = e in desugar(rest)`.
- A standalone `expr` statement `e` followed by `rest` desugars to
  `e >>= \_ -> desugar(rest)`.
- The final statement `e` desugars to `e` itself.

The resulting expression has some monadic type `m τ`; the `m` is
determined by the statements in the block (all binds and
standalone `expr` statements must share the same `m`).

Worked example:

```
do x <- readFile "a"
   y <- readFile "b"
   pure (x ++ y)
```

desugars to

```
readFile "a" >>= \x ->
  readFile "b" >>= \y ->
    pure (x ++ y)
```

assuming `readFile : String -> m String` for some `Monad m`.

**Patterns in `<-` are irrefutable.** A `do` bind
`Just x <- mx` is a type error at this layer (pattern is refutable
against `Maybe a`). Integrating refutable binds requires either a
`MonadFail`-shaped escape (Haskell's path) or a desugaring to
`case`; both are OQ 4.

## Interaction with earlier drafts

- **03 (data types).** The "kinds at this layer are trivial: every
  type classifies values" statement is superseded. Type constructor
  kinds become `κ₁ -> ... -> κₙ -> *` for an n-parameter `data`.
  Well-formedness of type applications now checks kind (MTC §Kind
  system). 03 OQ covering "higher-kinded types later" is **closed**
  by this document's kind system.

- **05 (operators and numbers).** Equality and ordering operators
  (`==`, `/=`, `<`, `>`, `<=`, `>=`) become methods of `Eq` / `Ord`.
  Their types in 05's table should be read as **the prelude
  instance** `Eq Int` / `Ord Int`, with `a -> a -> Bool` the
  general class scheme. 05 OQ 2 (polymorphic equality) is **closed**:
  yes, via `Eq` / `Ord`.

  Arithmetic operators (`+`, `-`, `*`, `/`, `%`) stay
  monomorphically `Int -> Int -> Int` at this layer: promoting them
  to a `Num` class is a larger commitment (Haskell's `Num` is
  notoriously over-populated). See OQ 6.

  The `(>>=)` operator joins 05's table at a new tier (tier 1,
  left-assoc). `>>` (monadic sequencing) may accompany it; see
  M6's prelude.

- **06 (pattern matching).** Unchanged. Pattern matching is
  orthogonal to type classes. `case` works identically with
  constrained scrutinee types; instance resolution for any
  constraints on pattern-bound variables happens at the use sites
  within each arm.

- **04 (records).** Unaffected. Row polymorphism (04 OQ 1) is an
  orthogonal axis; MTC does not force its resolution.

- **01 (core expression language).** The (Let) rule's scheme
  generalization now includes constraint collection (see §Constrained
  type schemes). (Var) is replaced by (Var') to enforce
  constraint-solvability at use sites. The `decl` production's
  `IDENT '=' expr` form is generalised to the `clause` form
  `IDENT apat* '=' expr` (see §Abstract syntax), making
  `f x y = body` admissible at the top level as well as inside
  class and instance bodies. All three revisions are monotonic
  extensions that behave identically in code that uses no
  constraints and only nullary definitions.

## Design notes (non-normative)

- **Single-parameter classes in this draft.** Multi-param type
  classes (MPTCs) open significant design space: functional
  dependencies, type-family workarounds, instance resolution
  complications. Keeping draft single-param makes `Eq`, `Ord`,
  `Functor`, `Applicative`, `Monad` all expressible and matches
  Haskell's 98 feature set. OQ 1 reopens this after MTC settles.

- **Haskell-98-shaped instance heads.** Instance heads restricted
  to `C T` where `T` is either a ground type or a saturated type
  constructor applied to distinct type variables is the "flex
  instances off" regime. This keeps resolution decidable and
  coherent. Relaxations (`FlexibleInstances`, etc.) are OQ 2.

- **No overlapping, no orphans.** These two restrictions combine
  to make resolution deterministic and import-order-independent.
  Relaxing either invites subtle coherence issues. OQ 3 revisits.

- **Dictionary passing is a conceptual model.** The spec does not
  mandate dictionary passing at the implementation level — a
  whole-program specialisation / inlining scheme is also sound.
  Mentioning dictionaries nails down the *decidability* of
  resolution, not the implementation.

- **Kind system is second-class.** Kinds appear only in the kind
  well-formedness judgement, not as first-class types. No kind
  polymorphism (à la Haskell's `PolyKinds`), no kind variables
  in source. OQ 5.

- **`do` for monads only.** The `do` block desugars through
  `(>>=)` and therefore imposes a `Monad m` constraint on the
  whole block. Applicative-only `do` (Haskell's `ApplicativeDo`)
  is not admitted; if expressive `Functor` / `Applicative`-only
  forms become desirable, extend via a separate sugar rather
  than overloading `do`. Not currently an OQ.

- **No derived instances syntax.** The `deriving` clause (from
  03's OQ 4) remains deferred. Instances must be written by hand
  at this layer.

- **Relationship to Ruby evaluation (M8).** The Ruby-evaluation
  type becomes a concrete `Monad` instance. Its `return` / `>>=`
  are threaded through the same resolution machinery as any
  other monad. M8's naming decision is about *which concrete type*
  — the monad machinery itself is furnished by MTC.

## Open questions

1. **Multi-parameter type classes.** Admit `class C a b where ...`
   and the attendant machinery (possibly including functional
   dependencies)? Draft: single-parameter only.

2. **Flexible instance heads.** Admit instance shapes beyond
   Haskell-98 — constructor applied to non-distinct variables,
   nested constructors, type-synonym heads? Draft: forbidden.

3. **Overlapping instances and orphans.** Relax the no-overlap /
   no-orphan policies, and if so under what rules? Draft: both
   forbidden.

4. **Refutable `do` binds.** How should `Just x <- mx` inside a
   `do` block be handled — `MonadFail`-shaped escape, desugaring
   through `case` with an error on match failure, or disallowed?
   Draft: disallowed.

5. **Explicit kind annotations in source.** Admit `(a :: * -> *)`
   in type expressions, and `type T (a :: * -> *) = ...` in
   declarations? Draft: no; kinds are inferred only.

6. **`Num` class for arithmetic.** Promote `+`, `-`, `*`, `/`,
   `%` to methods of a `Num` class so arithmetic becomes
   polymorphic across `Int` / `Float` / user-defined numerics?
   Draft: no; arithmetic stays monomorphic on `Int`, matching
   05's decision. Interacts with 05 OQ 1 (`Float` / `number`).

7. **Associated types and type families.** Haskell's
   `TypeFamilies` / `AssociatedTypes`? Draft: none.

8. **`deriving` for automatic instances.** Haskell-style
   `data T = ... deriving (Eq, Show)` for mechanical generation
   of common instances? Draft: none; instances are written by
   hand. 03 OQ 4's "should the surface admit `deriving`" is now
   cross-referenced by this document; final answer is expected
   to fall out of M6 or a follow-up.

9. **Instance chain resolution for higher-kinded constraints.**
   Constraints like `Monad m => Functor m` are well-formed (every
   `Monad` is an `Applicative` by the `Applicative m => Monad m`
   superclass, and every `Applicative` is a `Functor` by the
   `Functor f => Applicative f` superclass). Draft intent: the
   resolver **does** chain through superclasses automatically, so
   a `Functor m` constraint with a `Monad m` assumption is solved
   without requiring the user to write an explicit `Functor`
   instance. A worked example of the two-step chain is deferred to
   a follow-up document rather than included here.
