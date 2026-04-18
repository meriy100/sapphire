# 03. Data types

Status: **draft**. Subject to revision as pattern matching (M3), modules
(M4), and the prelude (M6) land and surface constraints that should flow
back.

## Motivation

Document 01 fixed the core expression language over a closed set of
ground types (`Int`, `Bool`, `String`). This document introduces
**algebraic data types** (ADTs) as the single mechanism by which
Sapphire gains new types beyond those ground constants, and closes
document 01's open question 1 (recursion at `let`) along the way.

Everything downstream leans on ADTs:

- Pattern matching (M3) will destructure data values.
- The prelude (M6) defines `Bool`, `Maybe`, `Result`, and list types
  using this layer's mechanism.
- Ruby interop (M7) will model Ruby-side error and result shapes as
  ordinary Sapphire data types.

The following programs anchor what this layer must accept. The surface
lexical forms (`data`, `|`, `upper_ident`) follow document 02
unchanged.

```
-- An enumeration
data Bool = False | True

-- A parametric sum, and one of its values
data Maybe a = Nothing | Just a
let x = Just 42 in x

-- A recursive type, and a value of that type
data List a = Nil | Cons a (List a)
let xs = Cons 1 (Cons 2 (Cons 3 Nil)) in xs

-- Mutual recursion at the type level
data Even = ZeroE | SuccE Odd
data Odd  = SuccO Even
let e = SuccE (SuccO ZeroE) in e

-- Let-polymorphism still works with constructors
let pair = \a -> \b -> Cons a (Cons b Nil) in
let _xs = pair 1 2 in
pair "hello" "world"
```

None of the examples above can be *taken apart* with the rules of this
document alone; destructuring is the concern of M3. This layer
accounts only for introducing data types and constructing their
values.

Deliberately deferred at this layer:

- **Pattern matching and destructuring.** M3.
- **Record-shaped constructors** (named fields on the right of `=`).
  Anonymous records and named fields are the concern of M2.
- **Higher-kinded types.** Every type in this layer has kind `*`; type
  constructors must be fully applied.
- **Generalized algebraic data types (GADTs), existential
  quantification, and phantom-type tricks.**
- **A `deriving` clause** or any other automatic typeclass / trait
  generation.

## Abstract syntax (BNF)

Extending document 01's BNF monotonically:

```
decl      ::= IDENT ':' type                     -- type signature     (01)
            | IDENT '=' expr                     -- definition         (01)
            | 'data' TCON TVAR* '=' data_alts    -- data declaration   (new)

data_alts ::= data_alt ('|' data_alt)*
data_alt  ::= TCON atype*                        -- constructor with 0+ args
                                                 -- each arg is atomic;
                                                 -- parenthesise for arrows
                                                 -- or applied constructors

type      ::= btype                              -- (revised from 01)
            | btype '->' type                    -- right-associative

btype     ::= atype
            | btype atype                        -- type application,
                                                 -- left-associative

atype     ::= TVAR
            | TCON
            | '(' type ')'
```

The `type` nonterminal from document 01 is refined: the ground
constants `Int`, `Bool`, `String` are ordinary `TCON`s of arity 0, and
type application `T τ₁ ... τₙ` is parsed as repeated left-associative
application at the `btype` layer (same shape as term application).

`upper_ident` (02 §Identifiers) serves both type constructors and
value constructors. Which role a given occurrence plays is resolved by
position: in type position it is a type constructor, in term position
it is a value constructor.

`data` declarations are **top-level only** at this layer. The `decl`
production is reached from `program` (01), never from inside `expr`;
there is no local `data` inside a `let` body. `data` joining the
keyword set is covered by document 02's additive-growth clause (02
§Keywords); 02's §Keywords table is extended in the same change that
publishes this document.

Static well-formedness of a group of `data` declarations requires:

- No two top-level `data` declarations introduce the same `TCON`.
- No `data_alt` reuses a constructor name from another top-level
  declaration.
- Each `τᵢⱼ` in a `data_alt` references only: the left-hand side's
  `TVAR`s, type constructors in scope (including the declaration's
  own `TCON` and any other `TCON`s from the same mutually-recursive
  group, see §Recursion), and nothing else. A free type variable on
  the right-hand side that is not bound on the left is a static
  error (no implicit quantification).

## Types

Kinds at this layer are trivial: every type classifies values, and
every type constructor has a fixed arity n ≥ 0. A TCON application
`T τ₁ ... τₙ` is well-formed only when exactly `n` arguments are
supplied. Partial application of type constructors is not admitted;
its role in higher-kinded polymorphism is deferred.

A declaration

    data T a₁ ... aₙ = C₁ τ₁₁ ... τ₁ₖ₁ | ... | Cₘ τₘ₁ ... τₘₖₘ

simultaneously introduces:

- A type constructor `T` of arity `n`.
- Value constructors `C₁, ..., Cₘ`, each receiving a type scheme
  (see §Typing rules).

Within the declaration's right-hand side, the following are in scope:
`T` itself, `a₁, ..., aₙ`, and any other type constructor introduced
by the surrounding mutually-recursive group of declarations.

### Recursion and mutual recursion (types)

A `data` declaration's right-hand side may mention `T` itself
(directly recursive types, e.g. `data List a = Nil | Cons a (List a)`).

Multiple top-level `data` declarations form a single mutually-recursive
group: each declaration's right-hand side may mention any other
declaration's type constructor, regardless of textual order.
§Top-level declarations below extends this group to include value
bindings as well.

Local `let` in this document remains single-binding (as in 01), so
local mutual recursion at the type level is not reachable at this
layer; it is deferred together with multi-binding `let`.

## Typing rules

### Constructor schemes

Given

    data T a₁ ... aₙ = C₁ τ₁₁ ... τ₁ₖ₁ | ... | Cₘ τₘ₁ ... τₘₖₘ

each value constructor `Cᵢ` is introduced into the environment with
the scheme

    Cᵢ : ∀ a₁ ... aₙ. τᵢ₁ → ... → τᵢₖᵢ → T a₁ ... aₙ

Nullary constructors (`kᵢ = 0`) receive
`Cᵢ : ∀ a₁ ... aₙ. T a₁ ... aₙ`.

Every `Cᵢ` is quantified over **all** of `a₁ ... aₙ`, even over type
parameters that do not appear in any of `Cᵢ`'s argument types — for
example `Nothing` in `data Maybe a = Nothing | Just a` receives
`Nothing : ∀ a. Maybe a`, not `Nothing : Maybe a₀` for some implicit
monomorphic `a₀`. Concrete `a`s are recovered from context at use
sites via `instantiate`.

A value constructor is an ordinary term-level identifier after
elaboration. The use of a constructor as an expression is typed by
document 01's (Var) rule applied to its scheme; no new judgment form
is needed, and no "undersaturated constructor" special case is
required. In particular `Just` (partially applied, or used as a first-
class function) has type `∀ a. a → Maybe a`, while `Just 1` has type
`Maybe Int`.

### Recursive let (revising document 01's (Let))

Document 01 left open (OQ 1) whether a `let` binding may mention
itself. This document closes that question: **`let` bindings are
implicitly recursive.** Document 01's (Let) rule is replaced by:

```
Γ, x : τ₁ ⊢ e₁ : τ₁     Γ, x : generalize(Γ, τ₁) ⊢ e₂ : τ₂
—————————————————————————————————————————————————————————————   (Let)
              Γ ⊢ (let x = e₁ in e₂) : τ₂
```

The binding `x : τ₁` is in scope during the elaboration of `e₁` (with
a monotype, so `e₁` sees `x` at a single type), and the generalized
scheme is in scope during `e₂`. This is the standard declarative
Hindley–Milner rule for implicitly recursive `let`, implementable by
algorithm W or J by allocating a fresh unification variable for `x`
before elaborating `e₁`.

Recursive uses of `x` inside `e₁` are at the same monotype `τ₁`:
**polymorphic recursion is not admitted** at this layer. Support for
polymorphic recursion (which is independently undecidable without a
type annotation) is not considered in this document.

### Top-level declarations

A program is a sequence of top-level declarations
`decl₁ ... declₙ`. **All top-level declarations — `data` declarations
and value bindings alike — form a single mutually-recursive group.**
Concretely:

- Every `data` declaration's type and value constructors are in scope
  for every other `data` declaration's right-hand side, every
  top-level type signature, and every top-level value binding's
  right-hand side.
- Every top-level value binding is in scope for every other top-level
  value binding's right-hand side, regardless of textual order.

A corresponding elaboration:

1. Gather the type constructors and constructor schemes from all
   `data` declarations, yielding an environment `Γ_data`.
2. For each top-level value binding `xⱼ = eⱼ`, bind `xⱼ` to a fresh
   monotype variable `αⱼ`, yielding
   `Γ₀ = Γ_data, x₁ : α₁, ..., xₙ : αₙ`.
3. For each binding, derive `Γ₀ ⊢ eⱼ : αⱼ` (solving whatever
   unification constraints arise, as in 01).
4. Generalize each solved `αⱼ` **under `Γ_data` only** — the other
   `xⱼ` are members of the same mutually recursive group and are
   deliberately excluded from each other's generalization context.
   This yields each binding's scheme for use by later modules or
   downstream queries.

This is the usual Damas–Milner elaboration of a mutually recursive
group and is intended to be implementable by a standard algorithm-W
variant.

### Interaction with type signatures

A top-level type signature `x : τ`, if present, annotates the
corresponding top-level binding `x = e`. Instead of a fresh monotype
`αₓ`, step 2 of the elaboration above uses the scheme obtained from
the signature (with the signature's free type variables taken as
universally quantified in the Hindley–Milner sense).

Whether top-level signatures are required or optional is not decided
at this layer; see 01 OQ 2, to be closed in M4 (modules).

## Design notes (non-normative)

- **Pattern matching is deferred.** With only the rules in this
  document, a Sapphire program can build values of a data type but
  cannot take them apart. M3 (pattern matching) completes the story;
  until then, `case ... of` is unavailable and this document's
  examples are "write-only" — none of them can be run end-to-end
  without M3. This is a deliberate layering choice, so that the
  introduction of types (this document) is independent of the
  introduction of their elimination form (M3).

- **`Bool` is just an ADT.** The example `data Bool = False | True`
  is illustrative only. Whether `Bool` is defined by this rule and
  shipped in the prelude (M6), or remains lexically distinct with
  02's `BOOL` token and 01's (LitBool) rule staying primitive, is
  the subject of 02 OQ 1 and closed in M6. Either reading presumes
  this document's `data` mechanism; they differ only on whether
  `Bool` specifically goes through it.

- **Higher-kinded types are deferred.** A `data` declaration with `n`
  parameters forces its type constructor to appear with exactly `n`
  arguments. A partially applied `T τ₁ ... τⱼ` for `j < n` is not a
  well-formed type. Higher-kinded polymorphism, typeclasses / traits
  over type constructors, and `Functor`-shaped abstractions are all
  postponed. Whether Sapphire follows Elm (kinds beyond `*` are
  never admitted) or relaxes this is a later decision tied to how
  ad-hoc overloading lands (01 OQ 5, 02 OQ 3).

- **No implicit quantification.** `data T a = Foo b` is rejected: a
  type variable `b` appearing on the right-hand side without being
  bound on the left is a static error. This matches Elm and matches
  Haskell without `ScopedTypeVariables`.

- **Implicit recursion, not `let rec`.** Making every `let` implicitly
  recursive (rather than requiring a separate `let rec`) is the Elm /
  Haskell default and is what Sapphire's motivating examples already
  quietly assume once `data` types with recursive shape exist. A
  performance-motivated distinction between recursive and
  non-recursive bindings is not exposed at the source level at this
  layer. See question 1.

## Open questions

1. **Implicit recursion vs. explicit `let rec`.** This document adopts
   implicit recursion at `let` (and top-level). Revisiting this —
   splitting out `let rec` for recursive bindings and retaining 01's
   original (Let) for non-recursive ones — remains an option if
   clarity-at-read, or performance-sensitive non-recursive bindings,
   become common concerns. Default revisit point: M10.

2. **Local multi-binding `let` and local mutual recursion.** Local
   mutual recursion requires a multi-binding `let` form
   (`let x = e₁ ; y = e₂ in ...`). This document leaves local `let`
   single-binding, deferring that form and with it any local mutual
   recursion, to a later document. Whether such a form forms a
   single mutually-recursive group or a sequential shadowing chain
   is open.

3. **Strictness.** The typing rules here are evaluation-order
   agnostic. Whether Sapphire's source semantics commits to
   strict-by-default (à la Elm / ML) or lazy-by-default (à la
   Haskell), or leaves this to the implementation-language phase,
   is deferred.

4. **`deriving` / automatic instance generation.** Should the surface
   admit a `data T ... = ... deriving (Eq, Show)` clause, or leave
   all derivations for a later typeclass / trait / convention-driven
   mechanism? This document introduces no derivation mechanism; the
   question is whether a later one should.

5. **Namespace and shadowing of constructor names.** Two related
   sub-questions, both silent in this document. (a) Do value
   constructors share a single namespace with ordinary term-level
   identifiers, or are they a distinct syntactic class (e.g. `Just`
   is unconditionally a constructor and `let Just = 1` is a syntax
   error)? (b) If they share a namespace, may a nested scope rebind
   a constructor name (`let Just = 1 in Just`)? Both resolutions
   interact with M3 (pattern matching), which must distinguish
   constructor patterns from variable patterns syntactically, and
   will likely fix both at once.
   *Closed by document 06*: `upper_ident` is always a constructor
   reference in both expression and pattern position; `lower_ident`
   at pattern head is always a binding. Rebinding a constructor name
   with a value binding is a static error.

6. **Local `data` declarations.** This document makes `data`
   top-level only. Whether a later document introduces local
   `data` (e.g. inside a `let` block) is open; M2/M3 do not obviously
   call for it, but M4 (modules) and M6 (prelude) may. Default
   revisit point: M10.
