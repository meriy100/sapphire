# 12. Example programs

Status: **draft**. Subject to revision as M10 (the spec freeze
review) cross-checks examples against normative content.

## Motivation

Every spec document 01–11 fixed a layer of Sapphire's language
machinery. This document exercises them together. Each example
below is a **self-contained Sapphire program** that compiles to a
Ruby module and runs under the machinery document 10 / 11 pin
down. The examples are the "touch" of the language: they show
what idiomatic Sapphire code looks like after every layer lands.

The examples are deliberately small (30–80 lines each) and
non-contrived: each picks a concrete task that an ordinary user
might bring to Sapphire.

In scope:

- Four programs that together exercise:
  - Pure expressions and pattern matching (01, 03, 06).
  - Records and structural typing (04).
  - Operators and numeric arithmetic (05).
  - Type classes and `do` notation (07).
  - Modules and imports (08).
  - Prelude bindings (09).
  - Ruby interop via `:=` and the `Ruby` monad (10, 11).

Out of scope:

- Benchmarks or performance tuning.
- Full end-to-end test harnesses (M10 may add a "how to run
  examples" appendix).
- The devcontainer / build pipeline that compiles these examples
  to runnable Ruby (that belongs to the implementation phase,
  not the spec-first phase).

## Conventions

Each example lists, in order:

1. The program's intent in one or two sentences.
2. The Sapphire source, as it would appear in one or more
   `.sp` files.
3. A brief reading guide, noting which spec documents the code
   exercises.

Code blocks use Sapphire syntax from the drafts. Comments inside
code are Sapphire comments (`--` line, `{- block -}`), except for
embedded Ruby inside `:=` bindings, which use Ruby's `#` comments.

## Example 1. Hello, Ruby

**Intent.** A one-module program that prints a greeting using
the `Ruby` monad. The minimal non-trivial end-to-end touch.

```
module Main
  ( main )
  where

-- Main action: greet two names in sequence.
main : Ruby {}
main = do
  greet "Sapphire"
  greet "world"

-- A pure function produces the greeting string.
greet : String -> Ruby {}
greet name = rubyPuts (makeMessage name)

-- Pure Sapphire builds the message, no Ruby involved.
makeMessage : String -> String
makeMessage name = "Hello, " ++ name ++ "!"

-- Ruby side: `rubyPuts` is the one embedded snippet.
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

**Reading guide.**

- `Main` is a single-module program (08).
- `main : Ruby {}` uses the `Ruby` monad from 11 with the empty
  record `{}` from 04 as the trivial success result.
- `do` notation (07) sequences two calls to `greet`.
- `greet` is a pure Sapphire function; it produces a `Ruby {}`
  value without itself being a `:=` binding.
- `rubyPuts` is the sole Ruby-interop touch — a `:=` binding
  (10) wrapping `puts`.
- Running the program goes through `run main`, returning
  `Ok {}` on success or `Err e` if `puts` fails.

## Example 2. Parse and sum a number file

**Intent.** Read a file of numbers (one integer per line), parse
them, and print the sum. Exercises Ruby file I/O, pure
`Result`-based parsing, and `List` folds. The Ruby side is
confined to the "read lines" and "print result" I/O edges;
parsing itself is pure Sapphire.

```
module NumberSum
  ( main )
  where

-- | Read a file, parse integers per line, sum them, print the result.
main : Ruby {}
main = do
  raw <- rubyReadLines "numbers.txt"
  case parseAll raw of
    Ok ns  -> rubyPuts (show (sumOf ns))
    Err e  -> rubyPuts ("parse failed: " ++ e)

-- Parse a list of strings into a list of ints, failing fast on any
-- non-integer line. Pure `Result`-monadic.
parseAll : List String -> Result String (List Int)
parseAll []       = Ok []
parseAll (s::ss)  = do
  n  <- parseInt s
  ns <- parseAll ss
  pure (Cons n ns)

-- Pure parse of a single string. Relies on a prelude primitive
-- `readInt : String -> Maybe Int` that 09's minimum set does not
-- yet ship; this example assumes it as a forthcoming addition.
parseInt : String -> Result String Int
parseInt s = case readInt s of
  Nothing -> Err ("not an integer: " ++ s)
  Just n  -> Ok n

-- Fold a list of ints.
sumOf : List Int -> Int
sumOf = foldl (+) 0

-- Ruby bridge: read a file as a list of chomped lines.
rubyReadLines : String -> Ruby (List String)
rubyReadLines path := """
  File.readlines(path).map(&:chomp)
"""

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

**Reading guide.**

- The `case` scrutinee is a pure `Result String (List Int)`
  value, so no `do` is needed at the outermost `main` except for
  the `rubyReadLines` action (06, 09).
- `parseAll` uses `do` **inside `Result`** (07, 09 §"Functor /
  Applicative / Monad instances"): `Result String` is a `Monad`,
  so `Err e >>= f = Err e` short-circuits parsing on the first
  failure.
- `parseInt` is a pure Sapphire function — no Ruby crossing.
  It delegates to a `readInt : String -> Maybe Int` that this
  example takes as a prelude addition. See OQ 6 below.
- `sumOf` uses point-free style: `foldl (+) 0` is
  `foldl (+) 0 xs`.
- Constructors `Ok`, `Err`, `Cons`, `[]` and the literal-list
  desugaring of 09 all show up.

## Example 3. Filter and group a list of records

**Intent.** Demonstrate records and pattern-bindings over records,
plus higher-order list processing. No Ruby interop at all — the
program is pure.

```
module Students
  ( topScorersByGrade )
  where

-- A student's score row. `type` here follows 09 OQ 2's proposed
-- Haskell-style alias syntax; 09 has not yet decided whether to
-- admit it (see Open question 4).
type Student = { name : String, grade : Int, score : Int }

-- Returns the highest-scoring student per grade, as a pair list.
-- (Shows multi-step pattern matching and record-field access.)
topScorersByGrade : List Student -> List { grade : Int, top : Student }
topScorersByGrade students =
  let grades = uniqueGrades students in
  map (\g -> { grade = g, top = bestIn g students }) grades

-- Extract the distinct grades in the list.
uniqueGrades : List Student -> List Int
uniqueGrades = foldr addGradeIfAbsent []

addGradeIfAbsent : Student -> List Int -> List Int
addGradeIfAbsent s gs =
  if member s.grade gs
    then gs
    else Cons s.grade gs

-- Best scorer inside a single grade. The list is assumed non-empty
-- for grades returned by `uniqueGrades`.
bestIn : Int -> List Student -> Student
bestIn g students =
  let inGrade = filter (\s -> s.grade == g) students in
  foldr1 pickBetter inGrade

pickBetter : Student -> Student -> Student
pickBetter a b =
  if a.score >= b.score then a else b

-- Simple membership.
member : Int -> List Int -> Bool
member _ []       = False
member x (y::ys)  = if x == y then True else member x ys

-- foldr1 is not in the minimum prelude of 09; it's defined here for
-- clarity. Sapphire users who want it in their own prelude can
-- re-export from a utility module.
foldr1 : (a -> a -> a) -> List a -> a
foldr1 f (x::xs) = foldr f x xs
-- foldr1 _ [] is not reachable for the cases this module produces.
```

**Reading guide.**

- `type Student = ...` is the `type` alias form proposed by 09
  OQ 2; the alias itself is not yet normatively admitted (Open
  question 4 flags this).
- Record field access via `s.grade` (04).
- Pattern matching over cons / nil (06, 09 list sugar).
- Helper functions (`addGradeIfAbsent`, `pickBetter`) are
  top-level rather than nested because function-local `where`
  clauses are not yet specified (02 reserves `where` only for
  `module` / `class` / `instance` headers). Nested scopes will
  ride with a future `where` or `let`-with-multi-binding
  extension.
- Note the intentional incompleteness of `foldr1` on the empty
  list: 06's exhaustiveness rule would reject the code as
  non-exhaustive. A real spec-conformant version would return
  `Maybe a` or accept a seed value. The example is kept as-is
  to make the exhaustiveness point visible.

## Example 4. Fetch-and-summarise with Ruby interop

**Intent.** Use Ruby to fetch a remote payload, parse it, and
print a summary. Exercises `do` notation crossing multiple `:=`
snippets, error handling through `Result RubyError`, and module
imports.

Two modules: `Fetch` exports the high-level entry; `Http` holds
the Ruby-interop primitives.

### `src/Http.sp`

```
module Http
  ( get, HttpError(..) )
  where

data HttpError
  = NetworkError String
  | StatusError  Int String
  | DecodeError  String

-- | Fetch a URL, returning the body as a `String` or a classified error.
get : String -> Ruby (Result HttpError String)
get url := """
  require 'net/http'
  require 'uri'

  begin
    uri = URI.parse(url)
    res = Net::HTTP.get_response(uri)
    if res.is_a?(Net::HTTPSuccess)
      { tag: :Ok, values: [res.body] }
    else
      msg = res.message || "unknown"
      { tag: :Err, values: [
        { tag: :StatusError, values: [res.code.to_i, msg] }
      ] }
    end
  rescue => e
    { tag: :Err, values: [
      { tag: :NetworkError, values: [e.message] }
    ] }
  end
"""
```

### `src/Fetch.sp`

```
module Fetch
  ( main )
  where

import Http (get, HttpError(..))

main : Ruby {}
main = do
  res <- get "https://example.com/"
  case res of
    Ok body -> do
      n <- stringLength body
      rubyPuts ("fetched " ++ show n ++ " bytes")
    Err httpErr -> rubyPuts (explain httpErr)

explain : HttpError -> String
explain err = case err of
  NetworkError m     -> "network error: " ++ m
  StatusError  c msg -> "HTTP " ++ show c ++ ": " ++ msg
  DecodeError  m     -> "decode error: " ++ m

-- Ruby bridge: ask Ruby for the string's byte length.
-- 09's prelude does not (yet) ship String-length; the Ruby side
-- handles it here.
stringLength : String -> Ruby Int
stringLength s := """
  s.bytesize
"""

rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

**Reading guide.**

- Two-module program; `Fetch` imports specific names from
  `Http` using 08's selective-import form.
- `HttpError` is a 3-constructor ADT; the exported `HttpError(..)`
  form brings both the type and all constructors into `Fetch`'s
  scope.
- `get` returns `Ruby (Result HttpError String)` rather than
  just `Ruby String`. This is the "explicit error channel"
  shape: the `Result` captures domain-level failures (HTTP
  status errors, decode errors) while Ruby exceptions (thrown
  by the `net/http` gem's internals, for instance) are still
  caught by the `Ruby` monad and surface via `run`'s `Err`
  alternative.
- The tagged-hash ADT representation of 10 §ADTs shows up
  explicitly in the Ruby body: `{ tag: :Ok, values: [...] }` /
  `{ tag: :StatusError, values: [...] }`.
- `Result HttpError String` is destructured by `case` in pure
  Sapphire, then each arm calls `rubyPuts` — a clean split
  between "effectful fetch" and "pure classification".

## Design notes (non-normative)

- **Examples as the language's elevator pitch.** Examples 1 and
  4 together are the smallest Sapphire-as-specified reading:
  "pure functional core with a principled Ruby boundary via a
  dedicated monad". A newcomer skimming just these two
  programs should see what Sapphire is for.

- **Deliberate rough edges.** Example 3 uses the `type` alias
  form (09 OQ 2, not yet normatively admitted) and a
  non-exhaustive `foldr1` (reject per 06). These are called
  out in the reading guide rather than silently fixed, so the
  examples double as a "here is what the spec has not yet
  closed" checkpoint.

- **No benchmarks, no long-running demos.** The point is that
  the spec's machinery composes end-to-end, not that it wins
  any speed test. Performance characteristics are an
  implementation-phase concern.

- **Ruby exceptions vs domain errors.** Example 4 draws the
  boundary between "truly exceptional" (caught by `Ruby`'s
  exception channel, surfaced via `Err RubyError` at `run`
  sites) and "expected failure modes" (modelled as
  `Result HttpError _`). Both modes are available, and real
  Sapphire code will mix them.

- **Prelude-only where possible.** The examples lean on 09's
  minimum set: `map`, `filter`, `foldr`, `foldl`, `show`, and
  constructors from 09's ADTs. Example 3's user-defined
  `foldr1`, `member`, `addGradeIfAbsent`, and `pickBetter` are
  the rare deviations — and each is labelled as such.

## Open questions

1. **Additional example: long-running Ruby computation.** A
   program that uses `Ruby` for a CPU-bound Ruby task and
   demonstrates the single-thread-per-`run` model is not
   included. Worth adding to M10's pre-freeze checklist if
   thread behaviour becomes a user-facing concern.

2. **An example that would break without 07.** The current
   examples exercise 07 (`do`, `Show`, `Monad`), but none
   *requires* polymorphism in a way that would fail a pre-MTC
   Sapphire. A small example that takes a `Monad m => m a` or a
   `Show a =>` argument non-trivially would harden 07's
   evidence in the example set.

3. **A Ruby-calls-Sapphire example.** All four examples treat
   Sapphire as the host calling into Ruby snippets. An example
   of a Ruby program consuming the generated `Sapphire::...`
   module (per 10 §Generated Ruby module shape) would close
   the other half of the boundary. Deferred to M10.

4. **`type` alias in example 3.** The example uses
   `type Student = ...` following 09 OQ 2's proposed Haskell-
   style alias form. If 09 OQ 2 lands as `no`, example 3 should
   be rewritten with the record type inlined at every use site.
   If it lands as `yes` with this spelling, the example becomes
   canonical usage; if `yes` with a different spelling (e.g.
   Elm-style `type alias`), the example's `type` keyword needs
   adjusting.

5. **An all-pure example.** Example 3 is the closest, but it
   has no `main`. A program that compiles to a runnable
   Ruby module and does interesting pure work inside a single
   `Ruby` action wrap would be a good addition.

6. **Example 2's `readInt` prelude dependency.** The pure
   `parseInt` in Example 2 calls `readInt : String -> Maybe
   Int`, which 09's minimum prelude does not ship. Options:
   (a) amend 09 to include `readInt` (and a corresponding
   `readFloat`, etc.); (b) restructure Example 2 to take
   integers from Ruby via a `:=` binding of type
   `String -> Ruby Int`, letting Ruby's exception channel
   handle malformed input via `run`'s `Err RubyError`; (c)
   leave the dependency visible and let the user supply their
   own `readInt`. Current draft takes (c) with an explanatory
   comment; M10's pre-freeze checklist should revisit.
