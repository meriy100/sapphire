# 10. Ruby interop — embedding and data model

Status: **draft**. Subject to revision as M8 (the Ruby-evaluation
monad) and M9 (example programs) exercise the boundary.

## Motivation

Sapphire is intended to compile to Ruby. That target implies two
distinct concerns:

1. **Calling Ruby from Sapphire.** A Sapphire program embeds Ruby
   code snippets whose results flow back into the Sapphire
   pipeline.
2. **Calling Sapphire from Ruby.** The generated Ruby module
   exposes Sapphire's exported bindings for use by an ambient
   Ruby application.

This document fixes the data-model half of both directions — how
Sapphire values correspond to Ruby values and how errors flow —
and introduces the surface syntax for embedding Ruby source. It
**does not** fix the evaluation semantics of the embedded Ruby
code; that is the concern of document 11 (the Ruby-evaluation
monad). This document treats the monad as an opaque type `Ruby`;
11 names and fully specifies it.

In scope:

- The embedding form: a definition whose body is a Ruby source
  snippet rather than a Sapphire expression (reusing 02's
  reserved `:=` punctuation).
- Triple-quoted string literals for multi-line Ruby source
  (the M7 amendment to 02's string-literal grammar).
- The marshalling rules for each Sapphire type (`Int`, `String`,
  `Bool`, records, ADTs, functions, `Maybe`, `Result`, `List`,
  `Ordering`, and the opaque `Ruby a` wrapper).
- The Ruby-side exception model — how a raised Ruby exception
  surfaces on the Sapphire side.
- The shape and naming of the generated Ruby module.

Out of scope (document 11):

- Evaluation order of chained Ruby actions.
- Threading / concurrency model for Ruby execution.
- `return` / `pure` / `>>=` for `Ruby`.
- The `run` function and the `Monad Ruby` instance.

## The embedding form

Extending documents 01, 07, 09:

```
decl ::= ...                                      -- (01, 07)
       | IDENT lower_ident* ':=' ruby_string      -- Ruby embedding

ruby_string ::= triple_string                     -- see §Triple-quoted
                                                  -- string literals
              | string_lit                        -- single-line Ruby,
                                                  -- from 02
```

A Ruby-embedded binding has the form

    name p₁ p₂ ... pₙ := ruby_source

and is the Ruby-interop counterpart of 07's `clause` form
`name p₁ ... pₙ = expr` — restricted to plain `lower_ident`
parameters (no wildcards, no destructuring patterns). Destructuring
must happen in a wrapping pure function if needed. The `:=`
punctuation, reserved by 02 for exactly this purpose, replaces the
pure `=` whenever the right side is a Ruby-source string rather
than a Sapphire expression.

The binding's type **must** be stated with an explicit signature
in scope (the `:` form preceding the binding). 08's boundary rule
already requires this for exported bindings; 10 additionally
requires it for **every** `:=` binding, including private ones,
because the type is what the marshalling boundary consults. The
signature's result type is required to be of the form `Ruby τ`
(possibly wrapped in further `-> τ'` arrows before hitting the
monad). A `:=` binding whose declared result is a pure type is a
static error.

Example:

```
rubyUpper : String -> Ruby String
rubyUpper s := "s.upcase"

rubyReadLines : String -> Ruby (List String)
rubyReadLines path := """
  File.readlines(path).map(&:chomp)
"""
```

In `rubyReadLines`, Ruby's `File.readlines(...).map(&:chomp)`
produces an `Array<String>` on the Ruby side, which the boundary
unmarshals back to `List String` per §Data model (§Lists).

The Ruby source sees each `lower_ident` parameter as a **Ruby local
variable** of the same name, pre-populated by the marshalling
rules of §Data model.

### Triple-quoted string literals

Document 02 §String literals deferred multi-line string forms.
This document activates them. The lexer rule is maximal-munch:

    `triple_string` ::= `"""` followed by the longest sequence of
                        code points and escapes that does **not**
                        contain an unescaped `"""`, followed by
                        the closing `"""`.

Inside a triple-quoted literal:

- Bare line feeds (`\n`) are admitted and preserved in the
  resulting string value.
- Escapes from 02 §String literals (`\n`, `\t`, `\r`, `\\`, `\"`,
  `\u{...}`) still work, and are post-processed into the final
  `String` value in the usual way.
- A single `"` or two consecutive `"`s in the body are admitted
  verbatim; only an unescaped `"""` terminates the literal.
- Writing `"""` inside the body requires escaping at least one
  `"` as `\"`: e.g. `""\""` or `"\""\"`.

**Ruby snippet content is the post-escape `String` value.**
Whichever string-literal form supplies the snippet (single-line
`string_lit` or `triple_string`), the Ruby interpreter sees the
resulting `String` value after all 02 / 10 escapes are decoded.
Authors who want a literal backslash-n in their Ruby source write
`\\n`, not `\n`. Single-line `string_lit` still follows 02's rule
that a raw `\n` in the source is a lexical error — multi-line
Ruby snippets therefore require the `triple_string` form.

Triple-quoted literals have type `String` just as ordinary string
literals do; they are a pure syntactic extension. Their primary
use is Ruby embedding, but they may appear anywhere `string_lit`
may.

## Data model

This section fixes the Ruby-side representation of each Sapphire
value as it crosses the boundary. The mapping is **total**: every
Sapphire value of a supported type has exactly one Ruby-side
representation, and every Ruby value produced by a Ruby snippet
is expected to conform.

### Ground types

| Sapphire | Ruby representation                   |
|----------|---------------------------------------|
| `Int`    | `Integer`                             |
| `String` | `String` (encoding UTF-8)             |
| `Bool`   | `true` / `false`                      |

### Records

A record `{ f₁ = v₁, ..., fₙ = vₙ }` marshals to a Ruby `Hash` with
**symbol keys**:

```
{ f1: ruby(v1), f2: ruby(v2), ..., fn: ruby(vn) }
```

(where `ruby(·)` is the recursive marshalling function). Field
order is not preserved on the Ruby side — `Hash` is insertion-
ordered in Ruby 1.9+ but Sapphire records are order-insensitive
at the type level (04), so any insertion order is admissible.

Symbol keys — not string keys — are the contract. A string-keyed
alternative (`{ "f1" => ruby(v1), ... }`) would round-trip JSON
more naturally, but Sapphire field names already share the
`lower_ident` shape of Ruby symbols and are a closed set known
statically, so the symbol-keyed form preserves Ruby's
conventional reading of hash-as-record without any loss of
expressiveness.

Ruby values returned to Sapphire where a record type is expected
must be a `Hash` whose key set is exactly the field set of the
record type; extra or missing keys are runtime errors at the
boundary (surfaced per §Exception model).

### ADTs (constructor form)

A value `K v₁ v₂ ... vₖ` of an ADT `T` marshals to a tagged
`Hash`:

```
{ tag: :K, values: [ruby(v1), ..., ruby(vk)] }
```

For a nullary constructor (`k = 0`), `values` is `[]`. The tag
is the constructor name as a Ruby symbol.

On the return path, the boundary expects a `Hash` with exactly
keys `:tag` and `:values`, where `:tag` names a constructor of the
expected type and `:values` is an `Array` of the right length.

The two most common ADTs — `Maybe` and `Result` — thus marshal as:

```
Nothing        → { tag: :Nothing, values: [] }
Just x         → { tag: :Just,    values: [ruby(x)] }
Err e          → { tag: :Err,     values: [ruby(e)] }
Ok  a          → { tag: :Ok,      values: [ruby(a)] }
```

(An optional surface convenience would allow the Ruby side to
return the payload directly for `Maybe a` with `nil` standing in
for `Nothing`. This is tempting but conflates `Just nil` with
`Nothing`; Sapphire **does not admit** the shortcut. A Ruby
snippet returning to a `Maybe a` context must produce the
tagged-hash envelope like any other ADT.)

A user record type `{ tag : String, values : List Int }` marshals
(per §Records) to a Ruby hash whose *shape* is indistinguishable
from an ADT envelope. There is no actual ambiguity: the boundary
picks the marshalling rule by the **expected Sapphire type**, not
by inspecting the Ruby hash. Hand-written Ruby code that crosses
both directions should still avoid field names that coincide with
the envelope keys (`tag`, `values`), since a future relaxation
could add heuristic disambiguation.

### Lists

A `List a` value marshals to a Ruby `Array`:

```
Nil                → []
Cons x xs          → [ruby(x), *ruby(xs)]
```

On return, any Ruby `Array` is admissible for `List a` provided
every element can be unmarshalled as `a`.

### `Ordering` (special-cased)

**Ordering is a deliberate exception to the general §ADTs rule.**
Its three nullary constructors `LT`, `EQ`, `GT` marshal as the
three Ruby symbols `:lt`, `:eq`, `:gt`, **not** as tagged hashes.

All other ADTs — including nullary-only ADTs — follow the tagged-
hash rule of §ADTs; the symbol shorthand is reserved for
`Ordering` because it interoperates cleanly with Ruby's `<=>`
comparison convention (which returns `-1 / 0 / 1`; the symbol
form preserves unambiguity on the Sapphire side while reading
naturally in Ruby).

### Functions

A Sapphire function `f : a -> b` marshals to a Ruby `Proc` (more
precisely, a `Lambda`). When Ruby calls the marshalled proc with
a Ruby-side argument, the boundary unmarshals it as `a`, runs the
Sapphire function, and marshals the result as `b`.

Crossing functions in the return direction (Ruby returning a
function to Sapphire) requires the Ruby side to supply a `Proc` /
`Lambda`. Currying semantics are preserved: a curried Sapphire
function `a -> b -> c` surfaces as a Ruby lambda that returns
another lambda when called with one argument (not a two-argument
lambda).

### `Ruby a`

Values of the opaque type `Ruby a` do not cross the boundary as
data — they **are** the boundary. A `Ruby a` on the Sapphire
side represents a suspended computation that, when run, produces
a Ruby-side side-effect and returns a Ruby value that unmarshals
as `a`. M8 defines the monadic operations and the `run`-shaped
function that drives a `Ruby a` to completion.

This document only states the **marshalling contract**: the Ruby
source code embedded in a `:=` binding's body produces a Ruby
value whose marshalled type is `a` (assuming the binding's
declared type is `Ruby a`); errors during evaluation surface per
§Exception model.

## Exception model

Ruby code is allowed to raise. When a `Ruby a` action is run
(per M8) and the underlying Ruby code raises an exception, the
action's result is **not** a successful `a` but an exception
surfaced to the Sapphire side.

The contract: every Ruby-side **user-level** exception
(`StandardError` and its descendants) is caught at the boundary
and converted to a Sapphire-side `RubyError` value. System-level
exceptions (`SystemExit`, `Interrupt`, `NoMemoryError`,
`SystemStackError`, and other non-`StandardError` `Exception`
subclasses) are **not** caught and propagate through the
boundary, ending the Ruby process in the usual way. This
matches standard Ruby practice, where `rescue => e` (i.e.
`rescue StandardError`) is the conventional catch breadth.

The `RubyError` type is defined here as:

```
data RubyError = RubyError String String (List String)
                         -- class_name   message    backtrace
```

(Positional-only per 04 OQ 2's decision. Field semantics are
class name, message, and backtrace, in that order.)

`RubyError` lives in a **prelude-adjacent module named `Ruby`**,
imported implicitly alongside `Prelude`. It is not added to
`Prelude` itself because its shape is Ruby-specific and unrelated
to the core prelude concerns of 09. Document 11 houses the
monad type `Ruby` and the `run` function in the same module,
avoiding orphan-instance concerns under 08.

<!-- 04 OQ 2 was closed 2026-04-18 as positional-only. The type
     is therefore defined positionally above, not as a named-field
     constructor. Older drafts may still show a named-field
     spelling for `RubyError`; update call sites to positional
     (`RubyError class msg bt` / `case e of RubyError c m b -> ...`). -->


How `RubyError` reaches user code is fixed by document 11: the
`Ruby a` type treats a caught user-level Ruby exception as
short-circuiting termination of the action, and document 11
exposes `run : Ruby a -> Result RubyError a` as the single
pure-side entry point that surfaces the outcome. This document
fixes only the `RubyError` type and the scoped catching rule
("user-level Ruby exceptions — `StandardError` and below — are
caught at the boundary; system-level exceptions propagate past
it"); document 11 fills in the user-facing interface.

## Generated Ruby module shape

A Sapphire module `M₁.M₂. ... .Mₙ` compiles to a Ruby namespace
under a top-level `Sapphire`. The naming rule is:

- Every ancestor segment `M₁, ..., Mₙ₋₁` becomes a Ruby `module`.
- The leaf segment `Mₙ` becomes a Ruby `class`.
- The full qualified name is `Sapphire::M₁::M₂:: ... ::Mₙ`.

For a single-segment Sapphire module (e.g. `Main`), only the
top-level `Sapphire` wrapper module is emitted, containing the
leaf class directly: `module Sapphire; class Main; ...; end; end`.

```
# generated from Sapphire module Data.List
module Sapphire
  module Data
    class List
      ...
    end
  end
end
```

Each exported Sapphire top-level binding `name` becomes a class
method of the generated class:

```
# exported: map : (a -> b) -> List a -> List b
module Sapphire
  module Data
    class List
      def self.map(f, xs)
        # generated implementation
      end
    end
  end
end
```

The generated method takes its Sapphire arguments in order,
unmarshals them per §Data model, runs the Sapphire-side logic,
and marshals the result back.

For `Ruby`-typed bindings:

- The generated method does **not** marshal the body's Ruby
  output back to Sapphire; it runs the Ruby source verbatim and
  returns the Ruby value directly (modulo exception catching).
- Exception catching wraps the body in a `begin ... rescue
  => e ... end` that repackages `e` as a `RubyError`-shaped
  `Hash` on the return path.

ADT constructors become factory class methods:

```
# data Maybe a = Nothing | Just a
module Sapphire
  class Prelude   # or wherever Maybe is defined
    def self.Nothing
      { tag: :Nothing, values: [] }
    end

    def self.Just(x)
      { tag: :Just, values: [x] }
    end
  end
end
```

Naming details:

- Constructor names are preserved literally as Ruby method names
  on the class. Ruby accepts uppercase method names (invoked as
  `Foo.Bar`), so no mangling is needed. **Caveat:** uppercase
  Ruby method names with arguments must use parentheses at the
  call site (`Foo.Just(1)`), because `Foo.Just 1` parses as
  constant access rather than a method call. Nullary calls
  (`Foo.Nothing`) work without parentheses.
- Sapphire operators like `(+)` or `(>>=)` have `upper_ident`-free
  representations in Ruby. They become methods with mangled
  names like `op_plus` / `op_bind`. The exact mangling scheme is
  10 OQ 1.

## Interaction with other documents

- **02.** Triple-quoted string literals extend 02's string-literal
  grammar, under its additive-growth clause. The existing
  single-line `string_lit` is unchanged.
- **07.** `:=` bindings produce `Ruby a` values, which must be
  `Monad` instances for `do` notation to work. M8 declares
  `instance Monad Ruby`; this document presupposes the instance.
- **08.** A Sapphire module's generated Ruby class is keyed by
  its module name (per 08 §One module per file). Imports across
  the Sapphire side do not affect Ruby-side class layout — each
  module's Ruby class is independent.
- **09.** The prelude's `print` stub is the canonical example of
  a forthcoming `Ruby`-typed binding. Its current
  `Result String {}` type will be replaced by something like
  `Show a => a -> Ruby {}` once M8 lands.

## Design notes (non-normative)

- **Why `:=` rather than a block form.** 02 pre-reserved `:=`
  for this use, and the declaration-level operator feels more
  natural for a Haskell-FFI-style boundary than a nested block.
  A block form would mean Ruby source could appear mid-expression,
  which invites parser complications and makes `do`-notation
  interactions harder.

- **Tagged-hash ADTs, not classes.** Representing ADTs as tagged
  hashes (rather than Ruby classes with subclasses per
  constructor) keeps the marshalling contract simple and avoids
  requiring the Ruby side to depend on generated class
  definitions. The Ruby author can pattern-match the hash or use
  `.dig(:tag)` directly.

- **No `nil`-for-`Nothing` shortcut.** Tempting but hazardous:
  `Just nil` / `Nothing` become indistinguishable. Explicit
  tagging trades a little Ruby-side verbosity for unambiguity.

- **Function values cross as lambdas.** Curried functions surface
  as returning-lambdas because that's the shape Sapphire already
  has at the type level. Ruby callers that prefer multi-argument
  lambdas can call `.curry(n)` to adapt.

- **Triple-quoted strings are a pure extension.** They are worth
  specifying here (rather than deferring to a generic
  "multi-line strings" OQ) because Ruby embedding creates
  immediate need for them. Other uses (JSON literals, doc
  strings) come along for free.

- **Boundary, not FFI.** Sapphire is not opening itself to
  arbitrary Ruby gem loading / monkey-patching at this layer.
  Each `:=` binding is a contained snippet; larger Ruby
  integrations go through normal Ruby code calling the generated
  Sapphire module, not the other way round.

## Open questions

1. **Operator-method mangling scheme.** What is the exact Ruby
   method-name mapping for operators like `(+)`, `(>>=)`, `(::)`?
   Options: `op_plus` / `op_bind` / `op_cons`, or
   `sapphire_op_7_6_plus` (include tier), or Unicode-like
   escape. Draft: unspecified; settle during implementation.

2. **Exception backtrace structure.** `RubyError.backtrace` is a
   `List String` today. A more structured form
   (`List { file : String, line : Int, method : String }`)
   would enable richer error surfacing. Draft: `List String`
   matches Ruby's `Exception#backtrace` directly.

3. **Importing Ruby source from files.** Beyond inline `:=`
   snippets, should Sapphire admit a `ruby_import "path.rb"`
   declaration that pulls an external Ruby file into the
   generated module? Draft: no; M9 may revisit if examples
   demand.

4. **Ruby version support beyond 3.x.** The data model pins
   Ruby 3.x (specifically Ruby 3.3 per
   `docs/project-status.md`). Earlier Ruby versions are out of
   scope — this is a decision, not an open question. The OQ kept
   here is whether **future** Ruby versions (4.x whenever it
   arrives) require amending the marshalling contract; draft says
   "monitor, no action needed yet".

5. **Higher-arity ADT constructors.** The tagged-hash
   representation scales to any constructor arity. No intrinsic
   upper bound; but for ergonomics the Ruby side might want a
   named-tuple-like wrapper (`OpenStruct`, `Struct`). Not
   planned now.
