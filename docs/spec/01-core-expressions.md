# 01. Core expression language

Status: **draft**. Subject to revision as later layers (ADTs, records, modules,
Ruby interop) land and surface constraints that should flow back into the core.

## Motivation

The core expression language is the layer every later layer (ADTs, records,
modules, Ruby interop, the `RubyEval` monad) is expected to be stated in terms
of. It is intentionally minimal: lambda, application, `let`, `if`, literals,
plus Hindley–Milner types with let-generalization.

The following programs anchor what this layer must accept. The surface syntax
is tentative — exact lexical conventions (operators, reserved words, layout
rules) are the concern of a later document.

```
-- Identity, applied
let id = \x -> x in
id 42

-- Picking the larger
let max = \a -> \b -> if a > b then a else b in
max 3 7

-- Let-polymorphism: the same `id` is used at two different types
let id = \x -> x in
let _  = id 1 in
id "hello"

-- Top-level with an explicit signature
double : Int -> Int
double = \n -> n + n
```

Anything richer than this (data constructors, records, modules, effects) is
deferred to later documents and may extend the grammar and the typing rules
monotonically.

## Abstract syntax (BNF)

Lexical tokens (`IDENT`, `INT`, `STRING`, `BOOL`, `TVAR`, `TCON`) are treated
as given at this layer; their exact regexes are the concern of the lexical
syntax document.

```
program   ::= decl*

decl      ::= IDENT ':' type                     -- type signature
            | IDENT '=' expr                     -- definition

expr      ::= literal
            | IDENT                              -- variable
            | '\' IDENT '->' expr                -- lambda (single parameter)
            | expr expr                          -- application, left-assoc
            | 'let' IDENT '=' expr 'in' expr     -- local binding
            | 'if' expr 'then' expr 'else' expr  -- conditional expression
            | '(' expr ')'

literal   ::= INT | STRING | BOOL

type      ::= TVAR                               -- type variable
            | TCON                               -- Int | Bool | String
            | type '->' type                     -- right-associative
            | '(' type ')'

scheme    ::= 'forall' TVAR* '.' type            -- surface-implicit; internal
```

Deliberately deferred at this layer:

- Multi-parameter lambdas `\a b -> ...`: to be treated as sugar for nested
  single-parameter lambdas once decided.
- Built-in operators (`+`, `-`, `>`, …): not primitive at this layer. They
  are either added later once ADTs and a resolution mechanism exist, or
  introduced as ordinary identifiers whose types live in a prelude.
- `let rec` and mutual recursion: see *Open questions* below.

## Types

At this layer there are exactly three ground type constants:

- `Int`
- `Bool`
- `String`

Function arrow `->` is right-associative.

A **type scheme** carries a (possibly empty) prefix of universally quantified
type variables. Generalization happens at `let` bindings; lambda-bound
variables are never generalized.

## Typing rules

Let Γ be a type environment mapping identifiers to type schemes. The judgement

    Γ ⊢ e : τ

reads "under Γ, expression `e` has type `τ`".

```
                   σ = Γ(x)
——————————————————————————————————————            (Var)
        Γ ⊢ x : instantiate(σ)


       Γ, x : τ₁ ⊢ e : τ₂
————————————————————————————————                  (Abs)
 Γ ⊢ (\x -> e) : τ₁ -> τ₂


Γ ⊢ e₁ : τ₁ -> τ₂     Γ ⊢ e₂ : τ₁
———————————————————————————————————               (App)
       Γ ⊢ e₁ e₂ : τ₂


Γ ⊢ e₁ : τ₁     Γ, x : generalize(Γ, τ₁) ⊢ e₂ : τ₂
———————————————————————————————————————————————————   (Let)
        Γ ⊢ (let x = e₁ in e₂) : τ₂


Γ ⊢ c : Bool     Γ ⊢ t : τ     Γ ⊢ f : τ
——————————————————————————————————————————        (If)
       Γ ⊢ (if c then t else f) : τ


——————————————  (LitInt)    ——————————————  (LitBool)    ————————————————  (LitStr)
 Γ ⊢ n : Int                 Γ ⊢ b : Bool                 Γ ⊢ s : String
```

Auxiliaries:

- `instantiate(∀α₁…αₙ. τ)` substitutes each `αᵢ` with a fresh unification
  variable and returns the resulting monotype.
- `generalize(Γ, τ) = ∀(ftv(τ) \ ftv(Γ)). τ`, where `ftv` denotes the set of
  free type variables.

These are the standard declarative Hindley–Milner rules and are intended to
be implementable by algorithm W (or J) without surprises.

## Design notes (non-normative)

- `if ... then ... else ...` is an **expression**, not a statement. Both
  branches must have the same type. Once `Bool = True | False` becomes
  available as an ADT (see the ADT document), `if` may be respecified as
  sugar for `case` — see question 3 below.
- Top-level type signatures are syntactically permitted but not yet declared
  mandatory. The choice is intentionally deferred; see question 2.
- The surface language chooses explicit `let ... in ...` here. Whether the
  top-level form looks more like `decl*` (current draft) or like one big
  `let ... in main` is a later decision tied to the module and entrypoint
  design.

## Open questions

1. **Recursion and generalization at `let`.** Should every `let` both
   generalize *and* allow self-reference, or should recursion require a
   distinct `let rec` form? (Current draft is silent on self-reference; the
   (Let) rule as written does not admit `e₁` mentioning `x`.)
2. **Top-level signatures: required or optional?** A required-signatures
   policy simplifies error messages and principal-type questions at module
   boundaries. An optional-signatures policy is friendlier but interacts
   with future type-class-like mechanisms.
3. **`if` as primitive vs. sugar.** Keep (If) as a primitive rule, or once
   `Bool = True | False` exists, respecify `if c then t else f` as sugar for
   `case c of { True -> t ; False -> f }` and drop the primitive rule?
4. **Numeric tower.** Stay with a single `Int` at this layer, and decide
   later whether to add `Float` as a separate type or introduce a unified
   `Number`. This document assumes the former is at least possible.
5. **Built-in operators.** Are arithmetic and comparison operators primitive
   constants with fixed monomorphic types in a prelude, or are they resolved
   through a later ad-hoc / type-class-like mechanism? The motivating
   examples use `+` and `>` but this document does not yet assign them types.
