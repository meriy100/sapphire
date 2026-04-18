# 09. Prelude

Status: **draft**. Subject to revision as Ruby-interop documents
(M7 / M8) exercise which prelude values the generated Ruby side
needs to expose, and as M9 example programs reveal gaps.

## Motivation

Earlier documents have repeatedly deferred concrete bindings to
"the prelude". This document fixes what that prelude contains at
the draft level: the minimum set of types, constructors, standard
instances, and helper functions every Sapphire program can assume.

In scope:

- The core ADTs: `Bool`, `Ordering`, `Maybe`, `Result`, `List`.
- List surface syntax: `[]` and `[x, y, z]` as sugar for `Nil` and
  `Cons` chains; `::` as binary cons (tier 5, right-assoc).
- Respec of `if ... then ... else ...` as sugar for `case`.
- Prelude instances of the standard classes from document 07 —
  `Eq`, `Ord`, `Show`, `Functor`, `Applicative`, `Monad` — at the
  types this document introduces.
- The arithmetic, comparison, and logical operator bindings that
  document 05 promised to site in the prelude.
- A minimum set of utility functions (`id`, `const`, `compose`,
  `flip`, `not`, `map`, `filter`, `foldr`, `foldl`).

This document closes:

- 02 OQ 1 (`True` / `False` as lexical class vs prelude
  constructors) — **constructors**, via the `Bool` ADT below.
- 01 OQ 3 (`if` as primitive vs. sugar) — **sugar**, via the
  desugaring rule in §Boolean and `if` below.

Deferred:

- An exhaustive enumeration of prelude functions. This document
  fixes the *vocabulary* needed to run the examples in M1–M8 and
  to write M9. Further functions (e.g. `foldMap`, `traverse`,
  `Data.Map`-like structures) accrete incrementally.
- Ruby interop values (`readFile`, the concrete Ruby-evaluation
  monad's `run`-shaped function). M7 / M8.
- I/O beyond what M7 / M8 eventually define.
- Default-method bodies for class instances are given only where
  they matter to the type-checker; all others rely on 07's
  minimal-complete-definition convention.

## The prelude as a module

The prelude is a Sapphire module named `Prelude`:

```
module Prelude
  ( ... )
  where
```

Every other module implicitly imports `Prelude` unqualified,
unless the module declares an explicit empty prelude import
(`import Prelude ()`, see 08). The explicit form shadows the
implicit one — the two never occur together.

Document 10 (Ruby interop) introduces a second implicitly
imported module, `Ruby`, under the same rule. Future documents
that add implicitly imported modules follow the same pattern —
the implicit-import set is not fixed at 09.

The rest of this document describes `Prelude`'s contents; the
export list above is filled in by §Module export list at the end.

## Boolean and `if`

```
data Bool = False | True
```

`True` and `False` are ordinary value constructors of `Bool`,
resolved through document 03's `data` mechanism and document 06's
constructor-pattern rules.

**02 OQ 1 closed.** `BOOL` from document 01 / 02 is the pair of
surface forms `True` and `False`, now identified as the two
nullary constructors of `Bool`. Document 01's (LitBool) rule
becomes derivable from (Var) applied to these constructor
schemes; 01 may treat (LitBool) as a shorthand for the derivation
without incident.

**01 OQ 3 closed.** With `Bool` now an ADT, the conditional
expression

    if c then t else f

is **surface sugar** for

    case c of
      True  -> t
      False -> f

Document 01's (If) rule is derived from (Case) of document 06
together with the (Con) / (Var) rules of 03 applied to `True` and
`False`. The (If) rule is preserved in 01 as a lemma — programs
written against the pre-sugar form continue to type-check with
exactly the same types.

### `Ordering`

```
data Ordering = LT | EQ | GT
```

Used by `compare` from `Ord` (see §Ord instances below). A simple
three-way comparison result with no payload.

## Error-handling ADTs

```
data Maybe a    = Nothing | Just a
data Result e a = Err e   | Ok a
```

`Maybe` represents optional values; `Result` represents values
that may have failed with an error payload of type `e`. The
Sapphire-flavoured name `Result` is preferred over Haskell's
`Either` for familiarity with downstream Ruby-side contracts
(Ruby idioms often use "ok/err" vocabulary).

The type parameters of `Result` are ordered `e a` (error first,
success last) so that `Result e` has kind `* -> *` — the shape
required by `Monad (Result e)`. See §Functor / Applicative / Monad
instances for the bind semantics.

## List

```
data List a = Nil | Cons a (List a)
```

**Surface syntax for lists.** `List a` values can be written in
two equivalent surface forms:

- Constructor form: `Nil`, `Cons 1 (Cons 2 (Cons 3 Nil))`.
- **Literal form** (sugar): `[]` for `Nil`, `[1, 2, 3]` for the
  three-element `Cons` chain, and `x :: xs` for `Cons x xs` (see
  05's operator table for precedence and associativity).

The literal form is purely syntactic sugar and desugars at parse
time:

```
[]                       desugars to   Nil
[x]                      desugars to   Cons x Nil
[x, y, z]                desugars to   Cons x (Cons y (Cons z Nil))
x :: xs                  desugars to   Cons x xs
```

List-literal patterns work the same way. Document 06's `apat`
production is extended by this document with the list-literal
pattern forms:

```
apat ::= ...                                   -- (06)
       | '[' ']'                               -- empty-list pattern
       | '[' pat (',' pat)* ']'                -- list-literal pattern
```

Each literal pattern desugars to the corresponding `Nil` / `Cons`
chain before 06's typing rules (PCon, PCons, etc.) apply.

`[` and `]` are token reservations from document 02, activated by
this document. No other document consumes them, so the activation
is a clean additive change.

## Class instances

The standard classes from document 07 receive their concrete
instances here.

### `Eq` instances

- `instance Eq Int` — primitive integer equality.
- `instance Eq String` — primitive string equality.
- `instance Eq Bool` — defined in terms of constructor equality.
- `instance Eq a => Eq (Maybe a)` — `Nothing == Nothing`,
  `Just x == Just y` iff `x == y`.
- `instance (Eq e, Eq a) => Eq (Result e a)` — analogous, with
  `Err x == Err y` iff `x == y` and `Ok x == Ok y` iff `x == y`.
- `instance Eq a => Eq (List a)` — by structural recursion.

### `Ord` instances

- `instance Ord Int` — primitive ordering.
- `instance Ord String` — lexicographic.
- `instance Ord Bool` — `False < True`.
- `instance Ord a => Ord (Maybe a)` — `Nothing < Just _`.
- `instance (Ord e, Ord a) => Ord (Result e a)` — `Err _ < Ok _`.
- `instance Ord a => Ord (List a)` — lexicographic.

### `Show` instances

- `instance Show Int`, `Show String`, `Show Bool`, and — given
  `Show a` — `Show (Maybe a)`, `Show (List a)`; given
  `Show e, Show a`, `Show (Result e a)`.

`show` produces the canonical surface form of the value. `show
[1, 2, 3]` is `"[1, 2, 3]"`; `show (Just 1)` is `"Just 1"`.

### `Functor` / `Applicative` / `Monad` instances

For `Maybe`:

```
instance Functor Maybe where
  fmap f Nothing  = Nothing
  fmap f (Just x) = Just (f x)

instance Applicative Maybe where
  pure = Just
  Nothing  <*> _        = Nothing
  _        <*> Nothing  = Nothing
  Just f   <*> Just x   = Just (f x)

instance Monad Maybe where
  Nothing  >>= _ = Nothing
  Just x   >>= f = f x
```

For `Result e` (short-circuits on the first `Err`):

```
instance Functor (Result e) where
  fmap f (Err e) = Err e
  fmap f (Ok  x) = Ok (f x)

instance Applicative (Result e) where
  pure = Ok
  Err e <*> _     = Err e
  _     <*> Err e = Err e
  Ok f  <*> Ok x  = Ok (f x)

instance Monad (Result e) where
  Err e >>= _ = Err e
  Ok  x >>= f = f x
```

For `List` (the Haskell "non-deterministic choice" monad):

```
instance Functor List where
  fmap f Nil         = Nil
  fmap f (Cons x xs) = Cons (f x) (fmap f xs)

instance Applicative List where
  pure x = Cons x Nil
  fs <*> xs = concatMap (\f -> map f xs) fs

instance Monad List where
  xs >>= f = concatMap f xs
```

(`concatMap` and `map` are defined in §Utility functions below;
their signatures are ordinary prelude functions. **Crucially, both
are defined directly by pattern matching on the `List`
constructors**, not through `Functor` / `Monad List` dispatch, so
the definition order `map → concatMap → Applicative List → Monad
List` is well-founded with no recursive dependency.)

`Ordering` does not carry a parameter and therefore receives
`Eq`, `Ord`, and `Show` instances but none of the higher-kinded
classes.

## Arithmetic, comparison, and logical bindings

Document 05's operator table's types were promised to come from
prelude bindings. Here they are, in scheme form. Operators are
listed by the parenthesised prefix name that 07's `(op)` syntax
uses.

```
(+), (-), (*), (/), (%)  : Int -> Int -> Int
negate                    : Int -> Int

(<), (>), (<=), (>=)      : Ord a => a -> a -> Bool
(==), (/=)                : Eq a  => a -> a -> Bool
compare                   : Ord a => a -> a -> Ordering

(&&), (||)                : Bool -> Bool -> Bool
not                       : Bool -> Bool

(++)                      : String -> String -> String

(>>=)                     : Monad m => m a -> (a -> m b) -> m b
(>>)                      : Monad m => m a -> m b -> m b
pure                      : Applicative f => a -> f a
return                    : Monad m       => a -> m a      -- default: pure
```

The declared types match document 05's operator table once 05's
Int-only entries for `(==)`, `(/=)`, `<`, `>`, `<=`, `>=` are
reinterpreted as the `Eq Int` / `Ord Int` **instances**, per 07.

## Utility functions

A minimum useful set. Each signature is the scheme that exported
prelude code should see.

```
id        : a -> a
const     : a -> b -> a
compose   : (b -> c) -> (a -> b) -> (a -> c)
flip      : (a -> b -> c) -> (b -> a -> c)

map         : (a -> b) -> List a -> List b
filter      : (a -> Bool) -> List a -> List a
foldr       : (a -> b -> b) -> b -> List a -> b
foldl       : (b -> a -> b) -> b -> List a -> b
concat      : List (List a) -> List a
concatMap   : (a -> List b) -> List a -> List b
length      : List a -> Int
head        : List a -> Maybe a
tail        : List a -> Maybe (List a)
null        : List a -> Bool

fst         : { fst : a, snd : b } -> a        -- 2-field record form
snd         : { fst : a, snd : b } -> b

maybe       : b -> (a -> b) -> Maybe a -> b
fromMaybe   : a -> Maybe a -> a

result      : (e -> b) -> (a -> b) -> Result e a -> b
mapErr      : (e -> e') -> Result e a -> Result e' a

when        : Applicative f => Bool -> f {} -> f {}
unless      : Applicative f => Bool -> f {} -> f {}

show        : Show a => a -> String
print       : Show a => a -> Result String {}   -- stub; M7/M8 will retype
```

Function composition is named `compose` at this layer. An infix
operator for composition (equivalent to Haskell / Elm's `.` or
Elm's `<<`) is **not** bound — `.` is punctuation for qualified
names (08) and record selection (04), and introducing it as an
arithmetic operator would either require re-opening those
disambiguations or picking a different spelling. Introducing an
infix composition operator is 09 OQ 8.

The pseudo-type `{}` is the empty record type from 04. `f {}`
stands for a monadic action that returns the trivial record —
the closest analogue to Haskell's `()` unit type at this layer.
See §Design notes for why 09 does not introduce a distinct unit
primitive.

`head` and `tail` are **total**, returning `Maybe` rather than
trapping. The partial versions (`head'`, `tail'`) are not part of
the minimum prelude.

`print` is a **stub** at this draft: it stands in for I/O until
M7 / M8 fix the concrete Ruby-evaluation-based I/O story. Its
return type `Result String {}` is a placeholder — a failed
`print` "carries an error message, succeeds trivially with the
empty record." M7 / M8 will replace it with the real monadic I/O
type. Programs that depend on `print` today should isolate the
call and expect re-typing when M7 / M8 land.

## Module export list

Sketch of the prelude's export list (not exhaustive; the full
list grows with this document):

```
module Prelude
  ( -- Types
    Bool(..)
  , Ordering(..)
  , Maybe(..)
  , Result(..)
  , List(..)

    -- Classes (all classes export methods)
  , class Eq(..)
  , class Ord(..)
  , class Show(..)
  , class Functor(..)
  , class Applicative(..)
  , class Monad(..)

    -- Operators
  , (+), (-), (*), (/), (%), negate
  , (<), (>), (<=), (>=), (==), (/=)
  , (&&), (||), not
  , (++), (::)
  , (>>=), (>>)

    -- Utilities
  , id, const, compose, flip
  , map, filter, foldr, foldl, concat, concatMap
  , length, head, tail, null
  , fst, snd
  , maybe, fromMaybe
  , result, mapErr
  , when, unless
  , pure, return
  , show, print
  , compare
  )
  where
```

`class X(..)` is the all-methods export form from document 08.
The implicit-prelude rule means ordinary modules receive every
listed name unqualified by default.

## Design notes (non-normative)

- **`Bool` as ADT, `if` as sugar.** The closure of 02 OQ 1 and
  01 OQ 3 via this document unifies boolean reasoning with the
  rest of the ADT story. `True` / `False` become just
  constructors; there is no special case for conditionals in the
  type system. The cost is one level of desugaring indirection
  in a compiler, which is trivial compared to specifying `if` as
  a separate primitive.

- **List literal syntax.** The desugaring `[x, y, z]` →
  `Cons x (Cons y (Cons z Nil))` avoids having to make `[` and
  `]` special in the abstract syntax tree; every list-literal
  expression is just a constructor-chain expression after parsing.
  Pattern-side literals desugar the same way, so `[x, y]` as a
  pattern matches `Cons x (Cons y Nil)`.

- **Why `Result e a`, not `Either e a`?** The generated-Ruby story
  (M7 / M8) will surface result values to Ruby code that uses
  ok/err vocabulary idiomatically. `Result` preserves that
  vocabulary across the language boundary. Users who prefer
  Haskell's `Either` can alias it; `type Either e a = Result e a`
  is a one-line prelude addition if the community asks for it.
  (Type aliases as a language feature are not yet specified.)

- **No infix composition operator in this draft.** Function
  composition is the prelude-bound `compose`. Reserving `.` as
  composition is tempting — Haskell does so — but `.` already
  means record-field selection (04) and qualified-name separation
  (02 / 08), and the prose disambiguations that keep those two
  uses distinct already strain the parser. An infix spelling can
  land via 09 OQ 8 once a dedicated token (e.g. `<<` / `>>`) is
  chosen.

- **Incremental growth.** Beyond this document's core set, the
  prelude will grow as M9 example programs reveal missing
  functions and M7 / M8 land Ruby-interop values. Any addition
  should respect the draft's shape (names, types, class
  instances).

- **Tuples are absent.** `fst` and `snd` are typed over a
  two-field record (`{ fst, snd }`) rather than a built-in
  tuple type. This is a deliberate choice consistent with 04's
  closed structural records. Users who want `(1, 2)` syntax can
  write `{ fst = 1, snd = 2 }`. Introducing tuple syntax as
  separate sugar is not included in the minimum prelude.

  Note the harmless coincidence that the **field names** `fst`
  / `snd` and the **function names** `fst` / `snd` are identical.
  Per 04 §Design notes, field names live in their own
  selection-syntax world — `.f` does not look `f` up in the
  environment — so the function `fst` and the field `fst` coexist
  without ambiguity. `(fst p)` calls the function; `p.fst`
  selects the field.

- **`head` / `tail` are total.** Both return `Maybe` rather than
  failing on the empty list. `tail : List a -> Maybe (List a)`
  in particular is non-standard relative to Haskell (which has
  `tail : [a] -> [a]` partial); Sapphire's totality bias and
  the prelude's "no `undefined` / `error`" policy both point at
  the `Maybe` version.

- **No `undefined` / `error`.** The prelude does not include a
  partial "stuck" function. Total programming is the preferred
  style; failures flow through `Result` or (eventually) an I/O
  monad's error channel. Ad-hoc abort routes can be added if
  Ruby interop demands them.

## Open questions

1. **Tuple syntax.** Should Sapphire admit `(a, b)` and `(a, b, c)`
   tuple syntax as sugar for record types with integer-keyed
   fields (or, alternatively, with positional `fst`/`snd` /
   `first`/`second`/... fields)? Current draft: no, users spell
   out the record. Interaction: 04's closed-records discipline
   makes this a mild ergonomic win, not a type-system necessity.

2. **Type aliases.** `type Either e a = Result e a` style sugar?
   Orthogonal to everything in this document but would be useful
   here. Not yet specified.

3. **String as a list of characters?** Haskell's `type String =
   [Char]` forces a `Char` type. Sapphire's `String` is a
   primitive-layer atomic type (01, 02). Whether `String` should
   be reinterpreted as `List Char` once `Char` exists is open;
   the current draft keeps `String` opaque.

4. **`Num` vs numeric-only `Int`.** 07 OQ 6 asked whether
   arithmetic should become a `Num` class. This document leaves
   it `Int`-only in line with 07's draft. Revisiting would
   require re-writing every arithmetic signature here.

5. **`IO` / concrete Ruby-evaluation monad.** `print` is a stub
   typed `Result String {}`. Once M7 / M8 define the concrete
   Ruby-evaluation type, `print` should be retyped to use it.
   Not a blocker for 09 itself, but a load-bearing forward
   dependency.

6. **`Char` primitive.** The prelude does not include `Char`.
   String indexing, character operations, and JSON-like string
   manipulations might warrant it eventually. Defer until M7 /
   M8 or user programs demand.

7. **Default prelude imports.** "Implicit `import Prelude`" is
   stated, but the exact mechanism (compiler-synthesised import
   vs. a language-defined module) could matter for tooling. This
   document is silent on the implementation path.

8. **Infix composition operator.** Introduce a binary composition
   operator in place of the prefix `compose` function? Candidate
   spellings: Haskell-style `.` (requires re-opening 02/04/08's
   `.` disambiguation story), Elm-style `<<` / `>>` (requires
   checking no collision with 02's `op_char` set and 05's monadic
   `>>`). Draft: no infix form; users call `compose`.

9. **Infix aliases for `map` / `concatMap`.** Haskell's `<$>`
   for `fmap` and `=<<` for `flip (>>=)` are common conveniences.
   Whether to add them as operators (requires adding the tokens
   to 05's operator table) is open. Draft: no.
