# 05. Operators and numbers

Status: **draft**. Subject to revision as M3 (pattern matching), M6
(prelude), and any later ad-hoc resolution mechanism land.

## Motivation

Documents 01, 03, and 04 all assumed operators without pinning them
down. 01's motivating examples already use `>` and `+`; 03's ADT
values cannot be exercised without arithmetic; 04's field-selection
examples are more useful alongside numeric operations. This document
fixes:

- The set of **built-in operators** admitted at this layer, their
  **precedences**, **associativities**, and **types**.
- The treatment of **unary minus**.
- The **numeric tower** at this layer.
- How the `::` token from 02 is resolved.

Closes:

- 01 OQ 4 (numeric tower) — partially: Int only at this layer; Float
  and a unified `number` variable are re-posed as 05 OQ 1.
- 01 OQ 5 (built-in operators) — operators are prelude-bound
  primitives with the monomorphic types fixed below.
- 02 OQ 2 (unary minus) — unary `-` is surface sugar for `negate`.
- 02 OQ 3 (operator table) — fixed Elm-style table; user-declarable
  fixity is re-posed as 05 OQ 3.
- 02 OQ 6 (`::` disambiguation) — `::` is list cons (to be realized
  in M6); any pattern-level type annotation will use a different
  spelling in M3.

## Abstract syntax (BNF)

Extending 01 (and orthogonally compatible with 03, 04):

```
expr   ::= ...                                 -- (01, 03, 04)
         | expr op expr                        -- binary application,
                                               -- parsed per §Operator
                                               -- table
         | '-' expr                            -- unary minus, prefix
         | '(' op ')'                          -- operator as function

op     ::= '+' | '-' | '*' | '/' | '%'
         | '<' | '>' | '<=' | '>=' | '==' | '/='
         | '&&' | '||'
         | '++'
         | '::'
```

The `op` set above is a **strict subset** of document 02's `op`
nonterminal: 02 fixes how runs of `op_char`s tokenize (maximal munch),
and this document fixes which of those tokens actually name an
operator at the expression layer. An `op_char` run that tokenizes to
anything outside this set is a parse-time error in expression
position at this layer; reservation of other sequences for future
documents is unchanged.

The binary-application production `expr op expr` is **ambiguous** as
a context-free grammar; it is disambiguated by the precedence and
associativity table in §Operator table. A literal expression
production like `e₁ + e₂ * e₃` has exactly one parse under that
table (here, `e₁ + (e₂ * e₃)`).

The `-` token appears in both the binary `expr op expr` production
(as a member of `op`) and the unary `'-' expr` production. The two
uses do not conflict: §Unary minus below fixes exactly which
syntactic positions make the parser take the unary arm, and every
other position makes it take the binary arm.

Function application binds tighter than every binary operator and
tighter than unary minus. `f 3 + g 5` parses as `(f 3) + (g 5)`.

## Operator table

| Precedence | Associativity | Operators               | Type                           |
|------------|---------------|-------------------------|--------------------------------|
| (app)      | left          | function application    | —                              |
| 9          | prefix        | `-` (unary)             | `Int -> Int` (as `negate`)     |
| 7          | left          | `*`  `/`  `%`           | `Int -> Int -> Int`            |
| 6          | left          | `+`  `-`                | `Int -> Int -> Int`            |
| 5          | right         | `++`                    | `String -> String -> String`   |
| 5          | right         | `::`                    | `∀ a. a -> List a -> List a` * (see prose below) |
| 4          | none          | `==`  `/=`  `<`  `>`  `<=`  `>=` | `Int -> Int -> Bool`  |
| 3          | right         | `&&`                    | `Bool -> Bool -> Bool`         |
| 2          | right         | `\|\|`                  | `Bool -> Bool -> Bool`         |

`::` (asterisked in the table) reserves its precedence and type at
this layer; the `List` type and the actual binding are introduced
in M6. Until M6 lands, `::` is tokenized and reserved but not usable
in an expression. This asymmetry with pipe operators (05 OQ 5,
which reserves no precedence entry yet) is deliberate: pipe
operators have not yet been promised, whereas list cons is a
near-certain M6 export.

Function application has effective precedence tighter than tier 9
(unary minus); it is shown for comparison.

The tier-4 comparison operators are **non-associative**: `a < b < c`
is a syntax error at this layer. Explicit parentheses are required.

Operators at the same precedence and associativity share a group:
`x :: xs ++ ys` at tier 5 right-associative parses as
`x :: (xs ++ ys)`. This is a parse-time fact regardless of types;
`xs ++ ys` must still have type `List a` to satisfy `::`, which
(given `++`'s `String` type at this layer) produces a type error
for this specific example once `::` is realized in M6. Mixed-tier
grouping is the usual precedence-climbing behaviour.

Every operator at this layer is **monomorphic**. In particular,
`==` and `/=` are typed `Int -> Int -> Bool` only. Extending them
(and the ordering comparisons) to further types is 05 OQ 2.

## Unary minus

Unary `-` is **surface sugar for `negate : Int -> Int`**, a prelude
binding introduced in M6. Wherever the parser recognizes `-` as
unary, `-e` is shorthand for `negate e`.

Syntactic position. `-` is a unary operator iff it is in
**expression-start position**: at the start of the right-hand side
of a top-level definition or a `let` binding, or immediately after
any of `(`, `{` (in the update form `{ e | ... }`, where an expression
follows `{` directly), `=`, `->`, a binary operator, or the keywords
`if`, `then`, `else`, `in`.
Everywhere else, `-` is the binary subtraction operator (tier 6).

Documents published so far (01 / 03 / 04) define the positions above.
Future documents will extend this list monotonically: M6 adds `[`
once list literals land, M3 adds `case`, `of`, and the pattern-branch
arrow `->` inside `case` expressions.

Precedence. Unary `-` sits at tier 9: tighter than every binary
operator but looser than function application. Hence

    -3 * 2   parses as   (negate 3) * 2         = -6
    -f 5     parses as   negate (f 5)
    -a - b   parses as   (negate a) - b

There is no lexical-munge rule that attaches `-` to a following
integer literal: `-3` is two tokens (the `-` operator and the integer
literal `3`). 02's decision that negative integer literals are not a
lexical form is unchanged.

## Numeric tower

At this layer the numeric tower has a single ground type: **`Int`**.
All arithmetic, comparison, and unary-minus operators are
monomorphically typed over `Int` as shown in the operator table.

This closes 01 OQ 4 for M5's purposes and leaves the choice between
"add `Float` as a second ground type" and "unify `Int` / `Float`
under an Elm-style `number` constrained variable" to 05 OQ 1. Both
paths are monotonic extensions of this layer: adding `Float` would
introduce additional `Float`-typed operator entries alongside the
`Int` ones; `number` would require a constraint-resolution
mechanism that this document does not provide.

## Typing rules

Every operator in §Operator table contributes a **primitive binding**
of the listed type to the initial environment Γ₀ supplied by the
prelude. At the core-language level this is enough: the binary form
`e₁ op e₂` is typed as the ordinary application `(op) e₁ e₂`, and
unary `-e` is typed as `negate e`. No new judgment form is required;
01's (Var), (App), and (Abs) suffice.

The wrapped form `(op)` is an atomic expression whose type is the
scheme of the corresponding prelude binding, instantiated by 01's
(Var).

The following two rules are **derived, not primitive** — they are
stated only to make surface-level typing explicit. The normative
content is 01's (Var) + (App) applied to the prelude bindings.

```
   (op) : τ₁ → τ₂ → τ₃ ∈ Γ    Γ ⊢ e₁ : τ₁    Γ ⊢ e₂ : τ₂
   ————————————————————————————————————————————————————————   (BinOp)
                   Γ ⊢ e₁ op e₂ : τ₃


   negate : Int → Int ∈ Γ    Γ ⊢ e : Int
   ——————————————————————————————————————                     (UMinus)
             Γ ⊢ -e : Int
```

## `::` disambiguation

Per the operator table, `::` is **list cons** at tier 5
right-associative, typed `∀ a. a -> List a -> List a`. The `List`
type and the `::` binding itself are introduced in M6; this document
only reserves the token with its precedence and intended type.

Pattern-level type annotation, if needed, will be spelled differently
(a likely candidate, to be fixed in M3, is `(pat : type)` —
parenthesized, reusing the existing `:` annotation separator). This
choice is driven by `::` being more frequently used than pattern-
level type annotation in typical Sapphire programs.

## Design notes (non-normative)

- **Operators are prelude-bound primitives, not lexically primitive.**
  The lexer (02) tokenizes operators; this document restricts which
  tokens are operators at the expression layer and fixes their
  types. The actual bindings — including the symbolic names `+`,
  `*`, `==`, and `negate` — live in the prelude (M6). Treating them
  as ordinary prelude values keeps the core expression language
  uniform: `(+) 3 5` and `3 + 5` are type-equal and (modulo parsing)
  evaluation-equal.

- **Equality is Int-only at this layer.** Admitting `==` / `/=` on
  strings, booleans, and user-defined data types in one step would
  require either a type-class mechanism (01 OQ 5's other branch) or
  a runtime universal-equality primitive. Draft chooses the narrow
  path; 05 OQ 2 reopens it.

- **No operator sections.** Sapphire does not admit `(+ 5)` or
  `(5 +)` as partial applications at this layer. Users write
  `\x -> x + 5`. Sections interact non-trivially with unary minus
  (`(- 5)` is ambiguous between "section of binary `-` with right
  operand `5`" and "the unary-minus application `negate 5`"); see
  05 OQ 4.

- **No user-declarable fixity.** The Elm-style fixed table is
  adopted. Haskell-style `infixl N` declarations are OQ 3.

- **Whitespace around `-` is not semantically significant at the
  lexer level.** The unary / binary distinction is parser-level,
  decided by whether `-` appears in expression-start position.
  `a - b`, `a-b`, and `a -b` all tokenize identically; whether they
  parse differently depends on what precedes the `-` at parse time
  (in all three, `-` is binary because it follows the expression
  `a`).

- **Comparison non-associativity.** `a < b < c` is rejected rather
  than parsing as either `(a < b) < c` (a type error, since
  comparisons return `Bool`, not `Int`) or as a chained comparison
  `a < b ∧ b < c` (which Python admits but Sapphire does not). The
  non-associative tier 4 turns this into a parse error, which is
  clearer than a later type error.

## Open questions

1. **`Float` and numeric polymorphism.** Admit `Float` as a distinct
   ground type, or unify via an Elm-style `number` constrained
   variable? The `number` path requires the constraint mechanism 01
   OQ 5's type-class alternative would introduce; the `Float` path
   only needs extra operator entries. Draft: `Int` only.

2. **Polymorphic equality / ordering.** Should `==`, `/=`, `<`, `>`,
   `<=`, `>=` extend to any type that "admits equality / ordering"
   (Elm's runtime constraint, restricting function types)? A yes
   answer depends on either a type-class mechanism or a runtime
   universal-equality primitive. Any yes answer must exclude types
   containing function arrows from `==` / `/=` (Elm's standing
   rule): functional extensional equality is undecidable, so a
   runtime universal-equality primitive that silently returned
   `False` on function values would be a footgun. Draft: monomorphic
   on `Int`.

3. **User-declarable operator fixity.** Admit `infixl N` / `infixr N`
   declarations (Haskell)? Draft is Elm-style fixed. A yes answer
   interacts with M4 (modules): imported operators need consistent
   fixity at import sites.

4. **Operator sections.** `(+ 5)` / `(5 +)` for partial application.
   Non-trivial interaction with unary minus (`(- 5)`), which pushes
   toward a `no` answer unless a separate spelling is adopted.
   Draft: no.

5. **Pipe operators.** Elm's `|>` (left-to-right) and `<|`
   (right-to-left). Useful; deferred to avoid overloading this
   document. Would be a natural tier-0 / tier-1 right/left entry.

6. **Exponentiation `^`.** Include `^ : Int -> Int -> Int` at a
   higher tier than `*`? Draft omits it; users write iterated
   multiplication or a prelude function.
