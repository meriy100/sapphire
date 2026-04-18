# 06. Pattern matching

Status: **draft**. Subject to revision as the prelude (M6) settles
`Bool` and list syntax.

## Motivation

This document introduces the elimination form Sapphire has been
lacking since document 03. So far, programs could **build** data
values and records but could not take them apart. `case ... of`
closes that gap and threads pattern-matching through the grammar
for both data constructors and records.

In scope:

- `case ... of` expression syntax, including its layout
  interaction with 02's `of`.
- The pattern language: wildcards, variables, literals,
  constructors, records, `@`-patterns, cons patterns, and
  parenthesised annotations.
- How patterns type, how they extend the environment, and how
  `case` expressions are typed.
- Exhaustiveness and redundancy checking.

Closes:

- 03 OQ 5 (namespace and shadowing of constructor names) —
  `upper_ident` is always a constructor in both expression and
  pattern position; `lower_ident` at pattern head is always a
  binding. Shadowing a constructor name with a value binding is a
  static error.
- 05 "pattern-level type annotation" (05 `::` disambiguation, tail
  half) — confirms the spelling `(pat : type)`.

Partially closes:

- 01 OQ 3 (`if` as primitive vs sugar) — the mechanism for the
  sugar reading is now available, but actually collapsing `if` to
  `case c of { True -> t ; False -> f }` depends on M6's resolution
  of `Bool`; see §Design notes.

Deferred:

- Guards (`pat | guard -> e`). 06 OQ 1.
- List-literal patterns `[p₁, ..., pₙ]` as sugar. Spelling
  reserved; realisation waits for M6 lists. 06 OQ 3.
- Or-patterns (`pat₁ | pat₂ -> e`). 06 OQ 2.
- View patterns and active patterns. Not planned.

## Abstract syntax (BNF)

Extending documents 01, 03, 04, 05:

```
expr    ::= ...                                   -- (01, 03, 04, 05)
          | 'case' expr 'of' case_alts            -- (new)

case_alts  ::= case_alt (';' case_alt)*           -- layout-sensitive;
                                                  -- see §Layout

case_alt   ::= pat '->' expr

pat     ::= apat
          | upper_ident apat+                     -- constructor pattern
                                                  -- with arguments
          | pat '::' pat                          -- cons pattern
                                                  -- (tier 5, right-assoc)

apat    ::= '_'                                   -- wildcard
          | lower_ident                           -- variable binding
          | lower_ident '@' apat                  -- as-pattern
          | literal                               -- literal pattern
          | upper_ident                           -- nullary constructor
          | '{' field_pats? '}'                   -- record pattern
          | '(' pat ')'                           -- grouping
          | '(' pat ':' type ')'                  -- type-annotated

field_pats ::= field_pat (',' field_pat)*
field_pat  ::= lower_ident '=' pat
```

`apat` is the **atomic pattern** class. A constructor argument must
be an `apat`; more complex patterns (constructor patterns with their
own arguments, cons patterns) must be parenthesised to appear as a
constructor argument. Example: `Just (Cons 1 xs)` pattern-matches a
`Just` whose payload is a `Cons`; `Just Cons 1 xs` is a syntax error
because `Cons` in that position is at best a nullary constructor.

Literal patterns in `literal` reuse document 01's literal grammar
(`INT`, `STRING`, `BOOL`). Whether `True` / `False` are literal
patterns or constructor patterns depends on 02 OQ 1's closure in M6;
either reading agrees on surface form.

### Precedence of `::` in patterns

Cons patterns use the same precedence and associativity as the `::`
expression operator (05 §Operator table): tier 5, right-associative.
`x :: y :: zs` parses as `x :: (y :: zs)`. This pattern form is
reserved at M3 but not usable until M6 introduces `List`.

### Type-annotated patterns

`(pat : type)` attaches a type annotation to a sub-pattern. It is
the `::`-alternative spelling promised in 05: because 05 fixed `::`
as list cons, pattern-level type annotation uses the
already-reserved `:` separator inside parentheses. The annotation is
a checking hint; it does not change what values the pattern
matches, only what type the matched binding is given.

### No named-field constructor patterns at this layer

A constructor pattern `K apat₁ apat₂ ...` is positional. Record
syntax appears only in the `{ ... }` record pattern form above, and
only against a value whose inferred type is a record (04). Whether
`data` constructors may be declared with named-field payloads —
`data Shape = Circle { cx : Int, cy : Int, r : Int }` — and
therefore matched with `Circle { cx = p₁, cy = p₂, r = p₃ }`
remains 04 OQ 2. Until that is closed, M3 admits only positional
constructor patterns.

## Pattern typing

Pattern typing is a **bidirectional** judgement

    Γ ⊢ p ⇐ τ ⊣ Γ'

read "under Γ, pattern `p` checks against type `τ`, producing the
extended environment `Γ'`". The extension `Γ'` is `Γ` together with
fresh bindings for any `lower_ident`s introduced by `p`; no binding
is ever shadowed by a pattern (see §Design notes).

### Atomic patterns

```
——————————————————————————    (PWild)
 Γ ⊢ _ ⇐ τ ⊣ Γ


        x ∉ dom(Γ)
——————————————————————————    (PVar)
 Γ ⊢ x ⇐ τ ⊣ Γ, x : τ


 Γ ⊢ x ⇐ τ ⊣ Γ₁     Γ₁ ⊢ p ⇐ τ ⊣ Γ₂
—————————————————————————————————————    (PAs)
        Γ ⊢ x@p ⇐ τ ⊣ Γ₂


         ℓ has literal type τ₀
—————————————————————————————————————    (PLit)
       Γ ⊢ ℓ ⇐ τ₀ ⊣ Γ
```

`(PLit)` requires that the literal's type equals the type being
checked against; it does not unify the two freely. An integer
literal pattern `3` matches only values of type `Int`; using it to
match a value of type `String` is a type error.

### Constructor patterns

```
  C : ∀ a₁ ... aₙ. τ₁ → ... → τₖ → T a₁ ... aₙ    ∈ Γ
  τ unifies with T σ₁ ... σₙ  (fresh σᵢ for the αᵢ in the scheme)
  for each i ∈ {1..k}:  Γᵢ₋₁ ⊢ pᵢ ⇐ τᵢ[σⱼ/aⱼ] ⊣ Γᵢ
  —————————————————————————————————————————————————————    (PCon)
          Γ₀ ⊢ C p₁ ... pₖ ⇐ τ ⊣ Γₖ
```

where `Γ₀ = Γ` is the starting environment and each `Γᵢ` extends
the previous one monotonically. The constructor's scheme is the one
introduced by 03's `data` declaration.

Nullary constructor patterns (`k = 0`) instantiate the scheme
without consuming any sub-pattern: they are a special case of
(PCon) with zero premises of the form `Γᵢ₋₁ ⊢ pᵢ ⇐ ...`.

### Record patterns

```
  τ unifies with { f₁ : τ₁, ..., fₙ : τₙ }  (all fᵢ distinct)
  { fⱼ₁, ..., fⱼₘ } ⊆ { f₁, ..., fₙ }      (each named fⱼᵢ in τ)
  for each i ∈ {1..m}:  Γᵢ₋₁ ⊢ pⱼᵢ ⇐ τⱼᵢ ⊣ Γᵢ
  —————————————————————————————————————————————————————————   (PRec)
   Γ₀ ⊢ { fⱼ₁ = pⱼ₁, ..., fⱼₘ = pⱼₘ } ⇐ τ ⊣ Γₘ
```

A record pattern names only the fields the pattern cares about. It
**does not need** to mention every field of the record type — this
is a deliberate asymmetry with 04's record **literal** form (which
requires every field). The matched record may carry additional
fields that the pattern ignores; the record type is the full field
set, not a projection.

Because 04 adopted **closed structural records** (no row variables),
(PRec) presupposes that the scrutinee's static type `τ` is already
concrete enough to enumerate the full field set `{ f₁, ..., fₙ }`
by the time the rule fires. A record pattern alone cannot drive
inference of the scrutinee's full shape; the shape must come from a
type annotation, a call site, or other surrounding evidence.
Relaxing this requires row polymorphism (04 OQ 1).

Rationale. Writing every field in every pattern hurts ergonomics
far more than the asymmetry with 04's literal form costs
conceptually; typical pattern use is "I only care about one or two
fields".

### Cons patterns

```
  τ unifies with List α   (fresh α)
  Γ₀ ⊢ p ⇐ α ⊣ Γ₁
  Γ₁ ⊢ q ⇐ List α ⊣ Γ₂
  ———————————————————————   (PCons)
  Γ₀ ⊢ p :: q ⇐ τ ⊣ Γ₂
```

Usable once M6 introduces `List`; reserved at M3.

### Annotation

```
  Γ ⊢ p ⇐ τ ⊣ Γ'    τ = τ_ann
  ———————————————————————————————   (PAnn)
  Γ ⊢ (p : τ_ann) ⇐ τ ⊣ Γ'
```

## Case expression typing

```
  Γ ⊢ e : τ_s
  for each i ∈ {1..n}:
      Γ ⊢ patᵢ ⇐ τ_s ⊣ Γᵢ
      Γᵢ ⊢ eᵢ : τ_r
  patterns (pat₁, ..., patₙ) are collectively exhaustive over τ_s
  —————————————————————————————————————————————————————————————   (Case)
  Γ ⊢ (case e of pat₁ -> e₁ ; ... ; patₙ -> eₙ) : τ_r
```

Every arm must produce the same result type `τ_r`. The patterns are
checked against the **scrutinee type** `τ_s`; patterns that cannot
match a value of `τ_s` are type errors, not silent dead arms.

Exhaustiveness is a typing premise, not a separate analysis pass.
See §Exhaustiveness below.

## Layout for `case`

02 reserved `of` as a block-opening keyword; this document
activates that reservation. A `case e of` whose following token is
not `{` opens a layout block whose reference column `c` is the
column of the first `pat` token. Each subsequent token at column
exactly `c` opens a new `case_alt`; tokens at columns greater than
`c` continue the current arm; a token at a column less than `c`
closes the block.

Worked example:

```
case maybeX of
  Nothing ->
    0
  Just x ->
    x + 1
```

Here the `case` block's reference column is that of `Nothing`. The
indented `0` and `x + 1` continue their arms. The final arm is
closed by whatever next token sits at a column less than the
reference column (or end-of-input).

The explicit-brace form is always available:

```
case maybeX of { Nothing -> 0 ; Just x -> x + 1 }
```

Inside braces the layout rule is disabled and arms are separated by
semicolons (02 §Layout).

When both are used — an explicit `{` immediately after `of` — the
layout block is not opened; everything up to the matching `}` is
the brace-form block.

## Exhaustiveness and redundancy

A `case` expression is **exhaustive** iff, statically, for every
value of the scrutinee's type there is at least one pattern in the
list that matches it. Non-exhaustive `case` expressions are
**static errors** at this layer, not warnings.

- For a closed ADT `data T ... = C₁ ... | Cₘ ...`, a `case` is
  exhaustive iff every constructor `Cᵢ` is covered by at least one
  pattern (possibly through wildcards or variable patterns).
- For records, exhaustiveness is trivial (a record type has exactly
  one shape); every record value matches a record pattern whose
  named fields resolve to exhaustive sub-patterns.
- For literal types (`Int`, `String`), exhaustiveness requires a
  final catch-all (wildcard or variable pattern). A case listing
  finitely many integer literals with no fallback is not
  exhaustive.

`Bool` sits across the literal / ADT line until M6 closes 02 OQ 1.
If M6 fixes `Bool` as the ADT `data Bool = False | True`, then
`case b of { False -> ... ; True -> ... }` is exhaustive by the
first (closed-ADT) bullet above. If M6 keeps `True` / `False` as a
distinct `BOOL` literal class, then `Bool` is a non-enumerated
literal type under the current wording of the third bullet, and a
catch-all would be required. The ADT reading is preferred; 06 is
drafted against that expectation but the wording does not force
the choice.

An **uninhabited** ADT `data Void = ` (zero constructors) is
vacuously exhaustive at any scrutinee of type `Void`. The grammar
`case_alts ::= case_alt (';' case_alt)*` requires at least one
arm, which forbids writing `case e of { }`; the practical
consequence is that you cannot `case` directly on a `Void` value.
Given that `Void` values cannot be constructed, this is acceptable:
any path that typed-checks as needing to eliminate a `Void` is
already unreachable. Introducing an empty-`case_alts` form for
pedagogical reasons is 06 OQ 7.

A pattern is **redundant** iff every value it could match is
already matched by some earlier pattern in the same `case`.
Redundant patterns are diagnostic-level (warning) at this layer,
not errors; eliminating them is never required for compilation but
is always safe.

Rationale for errors-not-warnings on non-exhaustiveness. Sapphire
targets generated Ruby modules that embed `.sp` logic; a non-
exhaustive match at runtime becomes a silent Ruby-side exception,
which defeats the point of static type checking at the module
boundary. Promoting the non-exhaustiveness check to an error is
worth the friction.

## Design notes (non-normative)

- **Namespace discipline (closes 03 OQ 5).** In both expression and
  pattern position, an `upper_ident` is always a constructor
  reference. A `lower_ident` at pattern head is always a binding.
  `let Just = 1 in Just` is a static error (the `Just` on the LHS
  is attempting to bind `Just`, which is an `upper_ident` and thus
  reserved for constructors). Rebinding a constructor is not
  admitted at any scope. This closes the sub-question about
  shadowing and about whether `{ x }` in a pattern could mean
  "bind `x`" vs "match against `x`" — it is always the former.

- **Record patterns are structural and subset-able.** Unlike record
  literals, record patterns may mention a subset of the record's
  fields. A pattern `{ x = 0 }` matches any `{ x : Int, ... }`
  whose `x` is `0`, regardless of other fields. Combined with the
  closed-structural-record choice from 04, this means the
  *inferred type* of the matched binding still carries all the
  record's fields — the pattern is a filter over the full type,
  not a projection to a smaller type. Projection to a smaller
  record type would require row polymorphism (04 OQ 1).

- **`if` as sugar for `case` (partially closes 01 OQ 3).** With
  `case` available, the mechanism for
  `if c then t else f` ≡ `case c of { True -> t ; False -> f }`
  is in hand. Whether to actually collapse (If) to this sugar
  depends on the resolution of 02 OQ 1 in M6: if `Bool` is defined
  as a `data` ADT, the sugar reading becomes canonical; if `Bool`
  stays lexically distinct (with `True` / `False` as a dedicated
  `BOOL` token), the sugar is not available and (If) remains
  primitive. M3 does not unilaterally make this call.

- **Literal patterns on `Int` / `String` need catch-alls.**
  Non-exhaustiveness as a static error forces the programmer to
  write a wildcard or variable pattern in any `case` over a
  non-enumerated type. This is the intended ergonomics: either
  match structurally (on an ADT) or provide a default.

- **Guards, or-patterns, and view patterns are deferred.** The
  pattern language at this layer is deliberately small.
  Ergonomic extensions (guards, or-patterns) are OQ; expressive
  extensions (view patterns, active patterns) are not planned.

- **Pattern typing is declarative.** The bidirectional judgement
  `Γ ⊢ p ⇐ τ ⊣ Γ'` reads cleanly as a type-checking algorithm but
  this document does not commit to a specific implementation
  strategy. An algorithm-W-style solver with unification at (PCon)
  and (PRec) is sufficient.

## Open questions

1. **Guards.** Admit `pat | guard -> e` arms, where `guard` is a
   boolean expression evaluated after a successful pattern match
   to conditionally fall through to the next arm? Elm has no
   guards; Haskell has them. Draft: no guards. The `if/else if`
   nesting workaround is acceptable for now.

2. **Or-patterns.** Admit `pat₁ | pat₂ -> e` as a single arm that
   fires on either sub-pattern? Interacts with guards (the syntax
   reuses `|`) and with variable binding (both sides of an or-
   pattern must bind the same set of variable names at compatible
   types). Draft: no.

3. **List-literal patterns.** `[]`, `[x, y, z]`, `[x, y | rest]`
   as pattern sugar. Spelling is reserved (02 §Operator tokens
   reserves `[` and `]`); realisation waits for M6 list syntax.
   Draft: only `::` cons patterns and nullary `Nil` (once M6 lands)
   are available.

4. **Exhaustiveness on `Int` / `String` patterns.** The current
   rule requires a catch-all on non-enumerated literal types.
   A future alternative: admit range patterns (`1..10`) to reduce
   the need for catch-alls in common cases. Not planned now.

5. **Pattern bindings in `let`.** `let pat = e in body` as a
   "let-pattern" form (Elm, OCaml, Haskell). Current grammar
   admits only `let lower_ident = e`. Extending to patterns on
   the LHS of `let` is a natural extension but not required at
   M3; defer to M10 or a follow-up document. Conceptually just
   `case e of { pat -> body }` for a single refutable pattern.

6. **Named-field constructor patterns.** Contingent on 04 OQ 2:
   if `data` constructors gain named-field payloads, pattern
   syntax extends to `K { f = p }`. Draft is positional-only.
   *Closed 2026-04-18*: 04 OQ 2 was decided as "no named-field
   constructor payloads at the first implementation"; this OQ
   collapses. Patterns remain positional.

7. **Empty `case_alts`.** Should `case e of { }` be admitted for
   pedagogical completeness on uninhabited scrutinees? Draft: no.
   The grammar forbids it; `Void` values are unreachable by
   construction, so practical need is nil.
