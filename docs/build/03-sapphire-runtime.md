# 03. The Sapphire runtime gem

Status: **draft**. Pipeline-level companion to
`docs/spec/10-ruby-interop.md` (data model, `RubyError`) and
`docs/spec/11-ruby-monad.md` (`Ruby` monad, `run`, threading).

## Scope

This document fixes the **Ruby-side support library** that every
compiled Sapphire program depends on. It packages this library as a
Ruby gem (proposed name `sapphire-runtime`) and specifies the
public-facing module structure that the generated code (per 02) and
host Ruby applications (per 05) consume.

Specifically:

- Tagged-hash ADT helpers (per 10 §ADTs).
- The `Ruby` monad evaluator that implements 11's primitives
  (`primReturn`, `primBind`) and `run`, including the thread model
  per 11 §Execution model.
- Boundary exception catching that produces `RubyError`-shaped
  values per 10 §Exception model.
- Marshalling helpers (`to_sapphire` / `to_ruby`) that handle the
  total mapping fixed by 10 §Data model.
- Gem packaging metadata: gem name, namespace, dependencies,
  `required_ruby_version`.

Out of scope:

- The shape of the *generated* Ruby per Sapphire module (that is 10
  §Generated Ruby module shape, surveyed in 02 §File-content shape).
- The CLI that builds the generated code (04).
- Test integration (05).

The runtime gem is **not** the compiler. It is a pure-Ruby library
that compiled output and host Ruby code link against at run time.
The compiler itself (host language deferred to `docs/impl/`)
produces the generated code that calls into this gem.

## Gem identity

| Field                   | Proposed value                            |
|-------------------------|-------------------------------------------|
| Gem name                | `sapphire-runtime`                        |
| Top-level Ruby module   | `Sapphire::Runtime`                       |
| `require` path          | `sapphire/runtime`                        |
| `required_ruby_version` | `~> 3.3` (per 01 OQ 1)                    |
| Dependencies            | None (no third-party runtime gems in v0)  |

The top-level Sapphire namespace `Sapphire::*` is shared between
generated user code (`Sapphire::Main`, `Sapphire::Data::List`, etc.,
per 10 §Generated Ruby module shape) and the runtime
(`Sapphire::Runtime::*`). The two coexist by reservation: the
runtime gem reserves the `Sapphire::Runtime` sub-namespace, and
the generated code never emits a Sapphire module named `Runtime`
(the compiler reports a static error if a user module would do
so). Whether that reservation deserves a stronger mechanism than a
compiler check is 03 OQ 1.

The user's `Gemfile` adds the dependency in the usual way:

```ruby
# Gemfile
gem 'sapphire-runtime', '~> 0.1'
```

The exact version constraint is part of the version-compatibility
question (01 OQ 2).

## Sub-module map

The runtime gem's public surface is partitioned into named
sub-modules. Every consumer (generated code, host Ruby) addresses
the runtime through these names:

- `Sapphire::Runtime::ADT` — tagged-hash ADT helpers.
- `Sapphire::Runtime::Ruby` — the `Ruby` monad evaluator (`run`
  and primitives).
- `Sapphire::Runtime::RubyError` — helpers for constructing the
  Sapphire-side `RubyError` tagged-hash value per 10 §Exception
  model from a caught Ruby `Exception`.
- `Sapphire::Runtime::Marshal` — `to_sapphire` / `to_ruby`
  helpers per 10 §Data model.
- `Sapphire::Runtime::Errors` — boundary-error subclasses
  (`MarshalError`, `BoundaryError`, etc.) raised by the
  marshalling helpers when input shape disagrees with the
  expected Sapphire type.

`require 'sapphire/runtime'` makes all five available. Sub-paths
(`require 'sapphire/runtime/adt'`) are admitted but not required
by the contract.

## ADT helpers

Per 10 §ADTs, a Sapphire ADT value `K v₁ ... vₖ` marshals to a
Ruby hash `{ tag: :K, values: [ruby(v1), ..., ruby(vk)] }`.
`Sapphire::Runtime::ADT` is the small helper module the generated
code uses to build and inspect those hashes.

Sketch (the exact API is not fixed at draft time; treat the
signatures as illustrative):

```ruby
module Sapphire
  module Runtime
    module ADT
      # Build a tagged-hash value.
      def self.make(tag, values)
        { tag: tag, values: values }
      end

      # Pattern-match style: yield the tag and values.
      # Expected to be paired with a case/when on the tag in
      # generated code.
      def self.match(value)
        unless value.is_a?(Hash) && value.key?(:tag) && value.key?(:values)
          raise Errors::BoundaryError, "expected tagged ADT hash, got #{value.inspect}"
        end
        yield value[:tag], value[:values]
      end

      # Quick accessors used by generated case-expression code.
      def self.tag(value)    = value[:tag]
      def self.values(value) = value[:values]
    end
  end
end
```

The generated code does not strictly **need** these helpers — it
could inline `{ tag: :Just, values: [x] }` everywhere — but
funnelling construction through `ADT.make` lets the runtime
evolve the representation (e.g. add a frozen-hash wrapper, or
move to a `Struct` per 10 OQ 7) without re-running the compiler
on every project.

`Sapphire::Runtime::ADT.match` is illustrative; whether the
generated code uses `ADT.match { ... }` or an inline `case` on
`ADT.tag(v)` is a code-emission choice for the compiler. The
runtime contract is: any value that satisfies the §ADTs hash
shape is admitted; any value that does not is a
`BoundaryError`.

### `Ordering` special case

Per 10 §`Ordering` (special-cased), Sapphire's `LT` / `EQ` / `GT`
do **not** use the tagged-hash representation; they marshal to
the bare Ruby symbols `:lt` / `:eq` / `:gt`. The runtime exposes
no `ADT.make`-style helper for `Ordering`; the generated code
emits the symbols directly, and consumers (Sapphire-side
unmarshalling) check `is_a?(Symbol)` and intern against the
three valid values.

## Marshalling helpers

`Sapphire::Runtime::Marshal` provides the two boundary-crossing
helpers that 10 §Data model requires:

- `to_ruby(sapphire_value, type)` — given a Sapphire-side value
  representation and the static Sapphire type, produce the Ruby
  value. Used by the generated code when it hands a Sapphire
  value to a `:=`-bound Ruby snippet (per 10 §The embedding
  form).
- `to_sapphire(ruby_value, type)` — given a Ruby value and the
  expected Sapphire type, produce the Sapphire-side value (or
  raise `Errors::MarshalError` on a shape mismatch). Used by the
  generated code when a Ruby snippet's result re-enters Sapphire.

Both helpers are **type-directed**: the type argument is the
authoritative oracle for which marshalling rule to apply, exactly
as 10 §ADTs requires ("the boundary picks the marshalling rule by
the expected Sapphire type, not by inspecting the Ruby hash").

The `type` argument's representation is a runtime question
(string-encoded? symbol-tagged AST? generated constant lookup?).
The contract here is "the compiler emits whatever the runtime
expects, consistently"; the actual encoding is 03 OQ 2.

Sketch:

```ruby
module Sapphire
  module Runtime
    module Marshal
      # Sapphire -> Ruby
      def self.to_ruby(value, type)
        case type
        when :int, :string, :bool then value          # 10 §Ground types
        when [:list, _]                                # 10 §Lists
          inner = type[1]
          value.map { |x| to_ruby(x, inner) }
        when [:record, _]                              # 10 §Records
          # value is the Sapphire-side record representation;
          # produce a symbol-keyed Hash per 10 §Records.
          ...
        when [:adt, _]                                 # 10 §ADTs
          # value is { tag:, values: }; recurse on each value.
          ...
        when [:ordering]                               # 10 §Ordering
          { lt: :lt, eq: :eq, gt: :gt }.fetch(value)
        when [:fun, _, _]                              # 10 §Functions
          # Wrap a Sapphire function as a Ruby lambda.
          ->(arg) { to_ruby(value.call(to_sapphire(arg, type[1])), type[2]) }
        when [:ruby, _]
          raise Errors::BoundaryError,
                "Ruby a values do not cross as data; use run"
        else
          raise Errors::MarshalError, "unknown Sapphire type: #{type.inspect}"
        end
      end

      # Ruby -> Sapphire (mirrors to_ruby; raises on shape mismatch)
      def self.to_sapphire(value, type)
        ...
      end
    end
  end
end
```

The sketch above is **illustrative only**. The exhaustive
clause-by-clause definition belongs to the implementation phase;
the contract here is that `Marshal` is total over the type set
fixed by 10 §Data model and produces `MarshalError` on any
mismatch.

## The `Ruby` monad evaluator

`Sapphire::Runtime::Ruby` implements the Sapphire-side `Ruby`
monad per 11 §Execution model. It exposes three primitives that
the generated code invokes:

- `pure(value)` — produces a `Ruby a` action whose execution
  yields `value` immediately (per 11 §Class instances; this is
  `primReturn`).
- `bind(action, k)` — sequentially composes a `Ruby a` with a
  Sapphire continuation `k : a -> Ruby b` (this is `primBind`).
- `run(action)` — drives an action to completion and returns
  a `Result RubyError a`-shaped Sapphire value per 11 §`run`.

`primReturn` and `primBind` are not user-facing in Sapphire (per
11 §Primitives); they are the runtime-side implementation of the
`Monad Ruby` class. The generated code emits calls to them when
it desugars `do` notation.

A `Ruby a` action, on the Ruby side, is an opaque value (a
`Sapphire::Runtime::Ruby::Action`). Its concrete representation
is private to the runtime; it might be a closure, a
trampoline-style record, or a compiled bytecode stream. The
contract is exposed only through `pure`, `bind`, and `run`.

### `:=`-bound snippet entry

When the generated code emits a `:=`-bound binding (per 10 §The
embedding form), it produces a Ruby method that constructs an
action which, when run, will execute the embedded Ruby source
with the parameters marshalled in. The runtime exposes a helper
for the construction:

```ruby
# Inside generated code for `rubyUpper s := "s.upcase"`:
def self.rubyUpper(s)
  Sapphire::Runtime::Ruby.snippet(
    params:  { s: s },                # already Ruby-side
    body:    proc { |s:| s.upcase }   # the snippet, captured
  )
end
```

The `snippet` helper produces an `Action` whose execution
substitutes the parameters into a fresh local scope (per 11
§Execution model item 4: per-step scope isolation) and runs the
captured `proc`. The form above uses a real Ruby `proc` rather
than a string of source code; whether the compiler emits the
snippet body as a literal `proc` or as a `eval`'d source string
is 03 OQ 3.

### Threading model

Per 11 §Execution model:

1. `run` spawns a Ruby evaluator thread and the Sapphire-side
   caller blocks on it.
2. Sub-steps within the action execute sequentially on that
   thread.
3. Each `:=` snippet runs in a fresh local scope (no leaks
   between snippets).
4. A raised exception inside any sub-step short-circuits the
   action; `run` returns `Err`.

The runtime contract:

- Each `run` invocation gets a **fresh Ruby thread**, or a pooled
  thread that the runtime guarantees has a clean local scope
  before reuse. Pooling is admitted as an implementation choice
  per 11; whether the v0 runtime pools is 03 OQ 4.
- The Sapphire-side caller blocks on the thread via a normal
  thread-join. Timeouts and cancellation are not modelled (per
  11 OQ 2).
- The thread runs in the **same Ruby process** as the caller.
  Out-of-process Ruby execution (subprocess per `run`) is not
  in scope.

```ruby
module Sapphire
  module Runtime
    module Ruby
      def self.run(action)
        thread = Thread.new { execute_in_isolation(action) }
        result = thread.value          # blocks until the thread completes
        # `result` is already Result-shaped, per execute_in_isolation
        result
      end

      private_class_method def self.execute_in_isolation(action)
        # Fresh local scope, fresh top-level binding for snippet eval.
        # Returns { tag: :Ok, values: [a] } or { tag: :Err, values: [e] }
        # (the Sapphire `Result RubyError a` shape).
        ...
      rescue => e
        ADT.make(:Err, [RubyError.from_exception(e)])
      end
    end
  end
end
```

### `run` returns `Result RubyError a`

Per 11 §`run`, `run` returns a `Result`. The runtime emits the
ADT-shaped value:

- Success: `{ tag: :Ok, values: [a] }`, where `a` is the
  marshalled Sapphire result.
- Failure: `{ tag: :Err, values: [e] }`, where `e` is a
  `RubyError`-shaped Sapphire record.

This shape is what generated Sapphire code on the calling side
will pattern-match (since the Sapphire-side type signature of
`run` is `Ruby a -> Result RubyError a`).

### No `unsafeRun` / no escape hatch

Per 11 §There is no `unsafeRun` / `runIO`, the runtime exposes
**no** primitive that lets a Ruby snippet's value escape into
pure Sapphire without going through `run`. The runtime
deliberately does not provide such a primitive; if one is ever
needed, it requires a spec amendment in 11 first.

## `RubyError` and exception catching

Per 10 §Exception model, every Ruby-side exception inside a
running `Ruby a` action is caught at the boundary and converted
to a Sapphire-side `RubyError` value. The runtime carries the
type:

```ruby
# Sapphire-side type (per 10 + 13 amendment §Interaction with
# earlier drafts, positional-only after 04 OQ 2's disposition):
#   data RubyError = RubyError String String (List String)
#                              -- class_name message backtrace

module Sapphire
  module Runtime
    module RubyError
      def self.from_exception(e)
        ADT.make(:RubyError, [
          e.class.name,
          e.message.to_s,
          (e.backtrace || []),
        ])
      end
    end
  end
end
```

The exception-catching point is the `execute_in_isolation`
boundary inside `Ruby.run` (above). Per 11 §Execution model item
5, the first raised exception short-circuits the action: any
sub-step whose continuation has not yet started is skipped, and
the `Result` returned by `run` is `Err`.

The catch is **broad**: every `StandardError` (and only
`StandardError`; system-level signals like `Interrupt` and
`SystemExit` propagate). Whether to catch `Exception` more
broadly is 03 OQ 5.

The captured `e.class.name`, `e.message`, and `e.backtrace`
populate the three `RubyError` fields per 10. `backtrace` may be
`nil` if Ruby did not assemble one (rare); the runtime
substitutes an empty list.

## Errors namespace (`Sapphire::Runtime::Errors`)

The runtime defines a small Ruby exception hierarchy for its
own use:

- `Sapphire::Runtime::Errors::Base` — root of all runtime errors.
- `Sapphire::Runtime::Errors::MarshalError` — raised by
  `Marshal.to_ruby` / `to_sapphire` when input shape disagrees
  with the declared type.
- `Sapphire::Runtime::Errors::BoundaryError` — raised by
  `ADT.match` (and similar) when a non-tagged value reaches a
  point that requires one.

These are **Ruby-side** exceptions, not Sapphire-side
`RubyError`s. They surface only when the runtime itself is
asked to do something the compiled-code contract should have
prevented. A user running properly compiled code should never
see them; a third-party Ruby caller violating the calling
convention will.

When such an error is raised *inside* a `Ruby a` action's
execution, the boundary catch (above) repackages it as a
`RubyError` like any other exception. Outside a `Ruby a`
action — i.e. when called from the host application's plain
Ruby code — they propagate normally.

## Loading and `require` order

The runtime gem's `lib/sapphire/runtime.rb` is the single entry
point. A typical generated file's first non-comment line is:

```ruby
require 'sapphire/runtime'
```

After that, the file may `require` other generated Sapphire
modules (per 02 §Cross-module requires). Order does not matter
beyond the runtime coming first; Ruby's load-once semantics
handle the rest, and the generated DAG is acyclic per 08
§Cyclic imports.

A host application's `Gemfile` and `$LOAD_PATH` setup is
documented in 05 §Embedding.

## Versioning and the calling convention

The runtime gem version pins the calling convention between
generated code and the runtime. Per 01 §Versioning and Ruby
target:

- A change to 10 §Data model (e.g. a new ground type, a change
  to the `:tag` / `:values` keys) is a **breaking** runtime
  change; the gem's major version bumps and existing generated
  code must be re-emitted.
- A change to 11 §Execution model (e.g. how `run` schedules)
  that the user can observe is also breaking.
- Internal refactors that do not change the
  generated-code-↔-runtime contract are non-breaking.

The runtime declares its version through the standard gem
mechanism (`Sapphire::Runtime::VERSION`); the compiler stamps the
generated-file header (per 02 §File-content shape) with the
runtime version it was built against. Whether the runtime should
*verify* at load time that every generated file was emitted
against a compatible version is 03 OQ 6.

## Interaction with other documents

- **Spec 10.** This document is the Ruby-side realisation of
  10's data model and exception model. The tagged-hash shape,
  the symbol-keyed records, the `Ordering` special case, and
  `RubyError`'s three-field record are all per 10. The runtime
  does not introduce new marshalling rules; it implements the
  ones 10 fixes.
- **Spec 11.** `Ruby.pure` / `Ruby.bind` / `Ruby.run` realise
  11's `primReturn` / `primBind` / `run`. The thread model is
  per 11 §Execution model.
- **Spec 12.** The example programs in 12 cite the runtime
  implicitly when they `run` a `Ruby` action; this document is
  what they actually call.
- **Build 02.** The output tree's per-file `require
  'sapphire/runtime'` (per 02 §File-content shape) is what
  loads this gem.
- **Build 04.** The CLI (04) does not invoke the runtime
  directly; the runtime is loaded by the *generated code* at
  Ruby execution time, not by the build-time pipeline.
- **Build 05.** Host-application integration (`Gemfile` entry,
  `$LOAD_PATH`) is in 05.

## Open questions

1. **Reservation of the `Sapphire::Runtime` namespace.** The
   compiler statically rejects a user module named `Runtime` so
   it cannot collide with `Sapphire::Runtime`. A stronger
   mechanism (e.g. the runtime gem freezes the namespace) is
   not currently planned. Draft: compiler-side check is
   sufficient. Deferred to implementation phase.

2. **Type-argument encoding for `Marshal`.** The `to_ruby` /
   `to_sapphire` helpers take a Sapphire type as input; how
   that type is represented at runtime (plain symbol tags,
   compiler-emitted constants, AST literals) is undecided.
   Draft: implementation chooses. Deferred.

3. **`:=` snippet body: literal `proc` vs `eval`'d string.**
   The compiler can emit a snippet's Ruby source either as a
   pre-compiled Ruby `proc` (carrying the source through
   `Proc.new { ... }`) or as a `String` that the runtime
   `eval`s on each call. Draft: prefer literal `proc` for both
   performance and predictability; revisit if `eval` becomes
   necessary for late-bound source. Deferred.

4. **Thread pooling vs per-`run` fresh thread.** Per 11
   §Execution model the runtime may pool threads if it
   guarantees per-step scope isolation. Draft: fresh thread per
   `run` in v0; pool only if measured cost demands it.
   Deferred.

5. **`StandardError` vs `Exception` catch breadth.** The
   runtime catches `StandardError` and lets `Interrupt` /
   `SystemExit` propagate. A stricter "catch every `Exception`"
   policy would surface user `Ctrl-C` as a `RubyError`, which
   is wrong; a looser policy would let `NoMemoryError` escape
   the boundary, which 10 forbids. Draft: `StandardError`.
   Deferred only as a sanity-check on the boundary; not a
   blocker.

6. **Runtime-version verification at load.** Every generated
   file's provenance comment names the runtime version it was
   built against (per 02 §File-content shape). The runtime
   could enforce compatibility by reading those headers (or by
   having the compiler emit a version-check call). Draft: no
   runtime-side enforcement in v0; rely on Bundler / gemspec
   constraints. Deferred.

7. **Public Ruby API for non-Sapphire callers.** A host app
   may want to construct a Sapphire-side ADT value directly
   (e.g. to pass `Just 3` into a Sapphire function) without
   going through the generated code. The `ADT.make` helper
   technically allows this, but a polished API would name
   constructors symbolically (`ADT.just(3)`). Draft:
   `ADT.make(:Just, [3])` is the supported form; sugar can
   come later. Deferred.
