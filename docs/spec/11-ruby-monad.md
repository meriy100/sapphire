# 11. Ruby evaluation monad

Status: **draft**. Subject to revision as M9 example programs
exercise the `run`/`>>=` machinery.

## Motivation

`docs/project-status.md` identifies a `RubyEval`-style monad as
Sapphire's signature feature — a monad that runs embedded Ruby
snippets on a separate thread and threads the result back into
the pure pipeline. Document 10 (Ruby interop data model) set the
boundary contract but intentionally left the monad's name, its
`Monad` instance, and its execution model opaque. This document
closes those.

In scope:

- The **name** of the monad type (closing the roadmap's "★"
  naming milestone).
- The `Functor` / `Applicative` / `Monad` instances for the type.
- The threaded execution semantics: how chained `>>=` actions
  schedule on the Ruby side.
- The `run` function — the single pure-side entry point that
  drives an action to completion and returns a `Result`.
- The relationship between the monad and the `:=` binding form
  introduced in 10.

The document does not redo 10's data-model contract; it presumes
it and fills in the monad's class instances, thread model, and
`run` function on top of the opaque `Ruby` type 10 introduced.

## The name: `Ruby`

Candidates considered (from the roadmap naming milestone):

- `RubyEval` — explicit but verbose at every signature site.
- `Rb` — short, but generic-looking and loses obvious Ruby
  identification outside the context.
- `Ruby` — the working name for the module already (10), natural
  for the type, follows Haskell's `Data.Map.Map` pattern.
- `Eval` — too generic; could be mistaken for a `Reader`-style
  reader monad or similar.
- `Host` / `Embed` — abstract away Ruby, but Sapphire is
  specifically Ruby-targeted.
- `Script` — scriptable vibe, but overlaps with "any scripting
  language".

**Decision: `Ruby`.** The monad type lives in the `Ruby` module
(introduced in 10) and is named `Ruby`. Fully qualified it is
`Ruby.Ruby a`; under the implicit `Ruby` import (see 09 as
amended for 10), unqualified use is `Ruby a`.

Document 10 already names the type `Ruby` throughout (treating
it opaquely); 11 fills in its class instances and the `run`
function.

## Type signature

```
-- inside module Ruby
data Ruby a    -- opaque; constructors are runtime-private
```

`Ruby` has kind `* -> *`, as required by 07's `Monad` class. The
type is **opaque**: users do not construct values of `Ruby a`
directly via a `data` pattern; values flow in only through

- `pure : a -> Ruby a` (from the `Applicative Ruby` instance),
- `:=`-bindings (document 10) whose body is a Ruby snippet,
- and compositions thereof through `>>=`.

## Class instances

All three instances live in module `Ruby` alongside the type
(no-orphans — 08).

```
instance Functor Ruby where
  fmap f ra = ra >>= (\x -> pure (f x))

instance Applicative Ruby where
  pure  = primReturn
  mf <*> ma = do
    f <- mf
    a <- ma
    pure (f a)

instance Monad Ruby where
  (>>=) = primBind
```

### Primitives (runtime-supplied)

```
primReturn : a -> Ruby a
primBind   : Ruby a -> (a -> Ruby b) -> Ruby b
```

Both are runtime-side primitives; `primReturn x` constructs the
monadic wrapper around a pure value, and `primBind ra f` builds
a deferred computation that, when run, runs `ra`, marshals its
result back to Sapphire, applies `f`, and runs the resulting
action in turn. The primitives are not user-visible — they are
the implementation of the class methods.

Laws. The three monad laws from 07 are expected to hold:

- `pure a >>= f ≡ f a`
- `ra >>= pure ≡ ra`
- `(ra >>= f) >>= g ≡ ra >>= \x -> f x >>= g`

## Execution model

A `Ruby a` value is a **deferred computation** on the Sapphire
side. It is not evaluated until `run` (below) is applied.

Under the execution model:

1. When `run` fires, a single **Ruby evaluator thread** is
   spawned. The spec contract is "fresh thread per `run`
   invocation" — an implementation may pool threads internally
   only if it guarantees state isolation (each run sees a fresh
   Ruby-side scope, with no leaked locals, globals, or loaded
   constants from a prior run). The Sapphire-side caller blocks
   until the Ruby thread signals completion.

2. Each leaf `Ruby a` action — either a `pure`-wrapped Sapphire
   value or a `:=`-bound Ruby snippet — becomes a sub-step on
   the Ruby thread. `pure` sub-steps are trivial; `:=` sub-steps
   run the Ruby source with the per-binding locals populated per
   10's data model.

3. `>>=`-sequenced actions run **sequentially** on the Ruby
   thread. The second action does not start until the first
   completes and its result is marshalled back to a Sapphire
   value for the continuation.

4. **Per-step scope isolation.** Each `:=` binding's Ruby body
   executes in a **fresh Ruby local scope**, receiving its
   parameters marshalled in and returning its result marshalled
   out; Ruby-side locals set in one snippet are not visible in
   the next. The only state that persists across sub-steps is
   what gets marshalled out as an action's result and threaded
   into the continuation via `>>=`.

5. If any sub-step raises a Ruby exception, the remaining
   sub-steps are skipped and `run` returns `Err` (see §`run`
   below).

Concurrency. Parallel composition of Ruby actions is **not**
admitted at this layer; every `Ruby a` action is single-threaded
on the Ruby side. A future extension could add
`parallel : Ruby a -> Ruby b -> Ruby (a, b)`-shaped primitives;
this is 11 OQ 1.

The "separate thread" phrasing from `docs/project-status.md` is
preserved: the Ruby evaluator thread is a distinct OS-level
thread from the Sapphire-side caller, which is why `run` is a
blocking operation. The separation lets Sapphire's pure pipeline
not be entangled with Ruby's VM state.

## `run`

```
run : Ruby a -> Result RubyError a
```

`run` is the **single pure-side entry point** that drives a
`Ruby a` action to completion:

- On success (no Ruby exception raised), `run` returns
  `Ok a`, where `a` is the Sapphire value marshalled from the
  action's final Ruby-side result.
- On failure (any sub-step raises), `run` returns
  `Err e`, where `e : RubyError` carries the exception's
  `class_name`, `message`, and `backtrace` per 10.

`run` is **pure** — despite internally spawning a Ruby thread,
it presents a deterministic function at the Sapphire type level:
for the same `Ruby a` value, it returns the same `Result`. (The
Ruby side itself may be non-deterministic if the snippet uses
clock, randomness, or external state; those effects manifest
through `Err` / `Ok` alternatives, not through `run` having a
varying return on the same input.)

Implementation note. Thread spawning, marshalling, exception
catching, and result delivery are all the compiler / runtime's
responsibility. The spec here fixes only the *surface type* and
the *invariant* that `run` is the exclusive route from `Ruby a`
to `Result`.

### There is no `unsafeRun` / `runIO`

The spec does not expose any "escape-the-monad" primitive that
would let a `Ruby a` action produce a pure `a` directly. All
extraction happens through `run`, which mediates via `Result`.
If a Ruby action is known-total (say, a pure string
transformation), users still write `run (rubyUpper "hi")` and
pattern-match on the `Ok`; the wrapped monadic shape is the
price of the principled Ruby boundary.

## `:=` and `Ruby` — the loop closed

Document 10 introduced `:=` bindings whose declared result type
must be `Ruby τ` (possibly under arrows). With `Ruby` now
concrete, a `:=` binding is **a user-written smart constructor**
for a `Ruby a` action: the embedded Ruby source is a description
of the Ruby-thread sub-step that will run when the enclosing
action is `run`-driven.

`do` notation (07) desugars through `>>=` from `Monad Ruby`.
Example:

```
rubyGreet : String -> Ruby {}
rubyGreet name := """
  puts "Hello, #{name}!"
"""

helloPipeline : String -> Ruby {}
helloPipeline name = do
  rubyGreet name
  rubyGreet ("again, " ++ name)
```

The `helloPipeline` function builds a two-step action. When a
caller applies `run (helloPipeline "world")`, the Ruby thread
runs the two `puts` calls in sequence and `run` returns `Ok {}`.

## Relationship to `print` (09 stub)

Document 09 left `print` as a stub

    print : Show a => a -> Result String {}

with a note that M7 / M8 would retype it. With `Ruby` now
concrete, the retyped signature is:

```
print : Show a => a -> Ruby {}
print x = rubyPuts (show x)

-- underlying Ruby snippet with a fully-marshallable parameter:
rubyPuts : String -> Ruby {}
rubyPuts s := """
  puts s
"""
```

`print` is an ordinary pure Sapphire function composed on top of
the `:=`-bound `rubyPuts`. The class-quantified `a` of `print`
never crosses the Ruby boundary — it is reduced to `String` by
`show` first, and only the resulting `String` is marshalled. This
keeps 10 §Data model's marshalling contract (which requires
boundary-crossing parameters to be in the specified type set)
intact.

Programs that used the 09 stub now call `run` on the result:

```
main : Ruby {}
main = print "hello"

-- entry point at the runtime boundary:
--   run main   -- returns Ok {} on success, Err e on failure
```

## Interaction with earlier drafts

- **07 (type classes).** `Monad Ruby` is a single-parameter
  instance at kind `* -> *`. `do` notation works without further
  machinery.
- **08 (modules).** `Ruby`, `primReturn`, `primBind`, `run` all
  live in module `Ruby`. The class instances live in the same
  module (no orphans).
- **09 (prelude).** `Ruby` is implicitly imported alongside
  `Prelude` (09 as amended). `print` retyped as above.
- **10 (Ruby interop).** 10 treats the `Ruby` type opaquely; 11
  fills in the `Functor` / `Applicative` / `Monad` instances, the
  `run` function, and the thread-model semantics. 10 and 11 land
  in the same commit so that references across them stay
  consistent.

## Design notes (non-normative)

- **Name choice.** Picking `Ruby` over `RubyEval` makes signatures
  read better at every site (`Ruby Int` vs `RubyEval Int`). The
  Haskell-tradition `Data.Map.Map` pattern (module and type share
  a name) keeps the naming compact without ambiguity; the module
  name and the type name live in disjoint namespaces per 08.

- **Single thread by default.** A sequential `Ruby` monad is
  easier to reason about and closer to Haskell's `IO`. Parallel
  composition is tempting but introduces race-condition concerns
  that the spec should not smuggle in at the draft layer. 11 OQ
  1 reopens if needed.

- **`run` is the only exit.** Keeping `run` the single exit
  preserves the referential-transparency boundary: any `Ruby a`
  value sitting inside pure Sapphire code is a description of
  effect, not a consummated effect. Program correctness arguments
  about pure code stay applicable up to `run` sites.

- **`Result RubyError` is the error channel.** Alternative
  shapes (throwing a Sapphire-side error, unwinding a call
  stack, etc.) would require more type-system machinery than 07
  currently provides. The `Result` channel is both the
  operationally-simplest and the most idiomatic given 09's
  `Result e a` prelude type.

- **Timeouts and cancellation are not modelled.** A long-running
  `Ruby a` action can block `run` indefinitely from the
  Sapphire-side caller's perspective. Interrupting / timing-out
  a `Ruby a` is 11 OQ 2.

- **The Ruby monad is strict in its discrete steps.** Each `>>=`
  step fully completes before the next begins. Lazy / incremental
  streaming of results (e.g. for a `Ruby` action that emits a
  long `List String`) is not part of the model. Streaming is
  11 OQ 3.

## Open questions

1. **Parallel composition.** Admit a primitive
   `parallel : Ruby a -> Ruby b -> Ruby (a, b)` or similar that
   schedules two Ruby actions on distinct Ruby threads and
   joins the results? Draft: no.

2. **Timeouts and cancellation.** Primitives
   `timeout : Int -> Ruby a -> Ruby (Maybe a)` or
   `cancel : Ruby a -> Ruby (Result RubyError a)` that wrap an
   action with a wall-clock limit. Draft: no.

3. **Streaming.** Should `Ruby` admit an "incremental" variant
   (like Haskell's `MonadIO` + `StreamingT`)? Draft: no.

4. **Exception-class granularity.** `RubyError` carries the
   Ruby exception's `class_name` as a `String`. Richer error
   discrimination (e.g. a Sapphire ADT mirroring common Ruby
   exception hierarchies) would let users pattern-match on
   specific Ruby exception classes rather than comparing
   strings. Draft: string-based.

5. **Escape hatch for shared Ruby-side state across chained
   `:=` snippets.** §Execution model item 4 fixes per-step
   scope isolation as the normative rule. OQ 5 is the follow-up
   question of whether a future extension should provide an
   opt-in primitive (e.g. `withSharedScope : Ruby a -> Ruby a`)
   that lets a user bundle several `:=` snippets into one
   effective Ruby scope, for the sake of Ruby idioms that
   require mutable intermediate state. Draft: no.

6. **Nested `Ruby` inside `Ruby`.** `Ruby (Ruby a)` is a
   well-formed type. Should the spec provide
   `join : Ruby (Ruby a) -> Ruby a` as a prelude convenience
   (or as a standard `Monad`-class derivable)? Haskell has this
   for free as `join = (>>= id)`; Sapphire prelude could expose
   it. Draft: user writes `>>= id`.

7. **Generated Ruby class and threading.** The M7 generated Ruby
   class exposes bindings as class methods; does each method
   call internally spawn its own thread, or does the calling
   Ruby code own the thread (i.e. `run` is a no-op from the
   Ruby-caller side)? Draft: implementation detail, but tilts
   toward "the Sapphire `run` wrapper manages the thread when
   called from the Sapphire side; direct Ruby consumers just
   call the class method synchronously without going through
   `run`."
