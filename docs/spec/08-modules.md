# 08. Modules

Status: **draft**. Subject to revision as the prelude (M6) and
Ruby-interop documents (M7 / M8) exercise module boundaries.

## Motivation

Every Sapphire source file up to this document has lived in a
single anonymous namespace. This document introduces **modules**:
named, file-level containers that bound visibility, share
qualified-name notation (02's `.`), and make cross-file composition
well-defined.

In scope:

- Module-declaration syntax, including export lists.
- `import` syntax for bringing names into scope, qualified and
  unqualified.
- Qualified-name resolution (`Mod.name`).
- Module hierarchy (`Data.List` style).
- Visibility rules and re-exports.
- Signature discipline at module boundaries (closing 01 OQ 2).
- The orphan-instance rule from 07 grounded in an actual module
  system.

Deliberately deferred:

- Cyclic module dependencies. Disallowed at this layer; reconsider
  only if M9 example programs demand.
- Package boundaries (multiple packages, dependency resolution,
  build artefacts).
- Hiding constructors while exposing the type (Haskell's
  `Type(..)` vs `Type`). See 08 OQ 2.
- `deriving` clauses interacting with cross-module visibility.
  That question is still 03 OQ 4 / M6.

## One module per file

A Sapphire source file **is** a module. The module's name mirrors
its path relative to a project root:

- `src/Main.sp` defines module `Main`.
- `src/Data/List.sp` defines module `Data.List`.
- `src/Data/List/Extra.sp` defines module `Data.List.Extra`.

The project-root ↔ module-path correspondence is a build-tool
convention and is not normative at this layer. What **is**
normative is:

- Exactly one `module` declaration per file. The `module`
  keyword, if present, must be the first non-comment,
  non-whitespace token in the file.
- The declared module name must match the file's dotted path
  relative to the compilation root (the build tool reports a mismatch).
- No two files may declare the same module name in the same
  compilation.

A single-file compilation without an explicit `module` declaration
is admitted as syntactic sugar for `module Main where ...`; this
is purely a convenience for small examples.

## Abstract syntax (BNF)

Extending 01 / 03 / 04 / 05 / 06 / 07:

```
program    ::= module_header? decl*                 -- (revised from 01)

module_header ::= 'module' mod_name export_list? 'where'

mod_name   ::= TCON ('.' TCON)*                     -- dotted UpperIdents

export_list ::= '(' export_item (',' export_item)* ')'
              | '(' ')'                             -- export nothing

export_item ::= IDENT                               -- export a value
              | TCON                                -- export a type,
                                                    -- no constructors
              | TCON '(' '..' ')'                   -- export a type with
                                                    -- all its constructors
              | TCON '(' TCON (',' TCON)* ')'       -- export a type with
                                                    -- selected constructors
              | 'class' TCON                        -- export a class,
                                                    -- no methods
              | 'class' TCON '(' '..' ')'           -- export a class with
                                                    -- all its methods
              | 'module' mod_name                   -- re-export a module

decl       ::= ...                                  -- (01, 03, 07)
             | import_decl

import_decl ::= 'import' mod_name import_tail
              | 'import' 'qualified' mod_name import_tail

import_tail ::= ε
              | 'as' TCON
              | 'as' TCON '(' import_item_list ')'
              | '(' import_item_list ')'
              | 'hiding' '(' import_item_list ')'

import_item_list ::= ε                                -- empty list OK
                   | import_item (',' import_item)*

import_item ::= IDENT
              | TCON
              | TCON '(' '..' ')'
              | TCON '(' TCON (',' TCON)* ')'
              | 'class' TCON
              | 'class' TCON '(' '..' ')'
```

Notes on the BNF:

- `mod_name` reuses 02's `upper_ident` and `.` reservation. A
  module name is a non-empty dotted sequence of `upper_ident`s.
- The explicit empty export list `module M () where ...` is
  admitted — it names a module that exposes nothing. Useful as a
  pure internal module imported for instance resolution: although
  no names are visible, the module's instances still travel with
  it (see §Instances and modules), so `import M` brings those
  instances into scope.
- An **omitted** export list is distinct from an empty one:
  `module M where ...` without parentheses exports every top-level
  non-import binding, every `data` constructor, and every `class`
  method. This matches Haskell's default. Prefer an explicit
  export list for library code.
- `import qualified M` requires every reference from the importing
  module to go through `M.name` (or `L.name` after `as L`);
  unqualified bare `name` does not resolve.
- `hiding (...)` is equivalent to an ordinary `import` minus the
  listed names; it cannot be combined with an explicit `(...)`
  import list.
- `as L` renames the module's qualified prefix to `L` locally. The
  original name remains available for qualified access unless the
  import uses `qualified`, in which case only the alias resolves.
  (This behaviour matches Haskell's semantics for the same
  combination of keywords.)
- The combined form `import M as L (foo, bar)` does both jobs at
  once: the qualified prefix becomes `L`, and the unqualified
  scope is restricted to `foo` and `bar` (plus whatever those
  item clauses select for types / classes). Qualified access via
  `L.otherName` is still allowed for unlisted exports.
- New keywords: `as`, `hiding`, `qualified`. All three are added
  to document 02's keyword set under its additive-growth clause.

## Visibility

A top-level declaration in module `M` is either:

- **Exported** by `M` — visible to modules that `import M`.
- **Private** to `M` — visible only to the rest of `M`.

With an explicit export list, the listed names are exported and
all others are private. Without an export list, every top-level
name is exported (see §BNF notes).

An exported `data` declaration exports the type constructor, but
not its value constructors, unless an `export_item` explicitly
lists them (`Maybe(..)` or `Maybe(Just)`). This matches Haskell's
abstract-data-type convention: a bare `Maybe` in the export list
exposes the type name for use in signatures but not its
representation.

An exported `class` declaration exports the class name, but not
its method names, unless `class C(..)` is listed. Exporting only
the class name — `class C` in the export list — makes `C` usable
in constraint position (`C a => ...`) from importing modules, but
does **not** make its methods available as identifiers in those
modules. In particular, external modules cannot write an
`instance C T where m = ...` because the method name `m` is not in
their scope. A class exported without `(..)` therefore behaves as
a closed abstraction whose instances are all defined in the
module that declares it. A class exported with `(..)` is the
ordinary open abstraction — external modules can reference its
methods and write instances for their own types (subject to the
orphan rule of §Instances and modules).

Class-method export granularity is **all-or-nothing** in this draft
(`class C` or `class C(..)` — no per-method selection). Finer
granularity is 08 OQ 1.

### Imported names and scope

An `import M` statement without qualification brings every
exported name of `M` into the importing module's **unqualified**
scope. It *also* brings them into scope qualified by `M` (or by
the alias given by `as L`).

An `import qualified M` brings every exported name only into
**qualified** scope. Unqualified references do not resolve.

An `import M (foo, Bar(Baz))` restricts the unqualified scope to
the listed names (here `foo` and the constructor `Baz` of type
`Bar`). The qualified form `M.otherName` still resolves to any
unimported export, because qualified access is always allowed for
imported modules.

When two unqualified imports expose the same name, the name is
**ambiguous** at each use site. Using the name unqualified is a
static error; the user must qualify the reference (`M1.foo` vs
`M2.foo`) or remove one import.

## Qualified-name resolution

Qualified references extend 01's `expr` and 03's `type` productions
with a qualified form:

```
qualified_name ::= mod_name '.' IDENT                -- qualified value
                 | mod_name '.' TCON                 -- qualified type /
                                                     -- constructor
```

(Where `mod_name` is the dotted `upper_ident` sequence defined in
§Abstract syntax.)

A qualified name `M.x` resolves by:

1. Treat `M` as the module prefix (possibly a dotted path like
   `Data.List`) and `x` as the identifier.
2. Look up the name in the set of exported bindings of the module
   named `M` (or of the module aliased to `M` via `as M`).
3. If no module named `M` is in scope, or `x` is not exported by
   it, the reference is a static error.

Qualified access never conflicts with 04's record-field selection.
Record selection `r.f` requires the left operand to be an
**expression** and the right operand to be a **`lower_ident`**
(04 §Abstract syntax); qualified access requires the left operand
to be a **`mod_name`** (a `TCON` or a dotted sequence thereof) and
the right operand to be either a `lower_ident` or a `TCON`. The
parser disambiguates by 02's `lower_ident` / `upper_ident` lexical
split: whenever the leftmost token of `X.y` is an `upper_ident`
(a `TCON`), the reference is qualified, not a record selection;
conversely, a record-type identifier is always `lower_ident`.
`Maybe.Just` is a qualified constructor; `p.x` with `p` a
value-level `lower_ident` is record selection.

## Top-level signatures

Document 01 left open (OQ 2) whether top-level type signatures are
required or optional. This document closes that question with a
**boundary rule**:

- Every **exported** top-level value binding must carry an
  explicit type signature (`x : scheme`) somewhere in the module.
  The signature may precede or follow the binding itself, but it
  must exist.
- Exported **class** method signatures come from the class
  declaration itself, not a separate top-level form.
- Exported **data** and **class** declarations do not need
  separate signatures; their kinds are inferred from the
  declaration.
- **Private** top-level value bindings may omit the signature;
  the type is inferred as usual.

Rationale. A signature at a module boundary fixes the type of an
exported binding against which downstream modules compile. Without
it, changes to an internal implementation can silently alter the
public type. Internal bindings have no such risk; imposing
signatures there would be pure paperwork.

This rule interacts cleanly with 07's constrained schemes: an
exported binding carries its full constrained scheme
(`foo : ∀ a. Eq a => a -> a -> Bool`), and the scheme is what
importers see.

## Cyclic imports

The module graph — the directed graph whose nodes are modules and
whose edges are `import` relationships — must be **acyclic**. A
module `M₁` that (transitively) imports a module `M₂` may not be
(transitively) imported by `M₂`. Circular imports are a static
error at this layer.

Rationale. A DAG of modules keeps type-inference order well-defined
and keeps instance-resolution closure computable by a single pass.
Relaxing this would require either lazy module evaluation (with
the attendant ordering pitfalls) or Haskell-style `.hs-boot`
signature files; neither is justified by currently-drafted
examples. See 08 OQ 4 for the escape question.

## Instances and modules (refining 07)

Document 07 stated that an instance `instance C T` must live in
the module defining either `C` or `T` (no orphans). This document
grounds that rule:

- An instance declaration is in scope in module `M` iff `M`
  transitively imports the module that hosts the instance.
- The no-orphans rule is a **static error**: a compiler (or
  checker) must reject an `instance C T` whose module defines
  neither `C` nor `T`'s outermost type constructor.
- Instances have no names and are **not** listed in export lists.
  They travel with their module: importing a module imports every
  instance it defines (or re-exports via `module M` in its
  export list).

This machinery is exactly what lets 07's "coherent resolution"
hold at cross-module scale: because every instance has a unique
home module, resolution does not depend on import order, only on
which modules are in the transitive import closure.

**Invariant:** given the orphan rule and the transitive-visibility
rule above, for any `(C, T)` class/type-head pair and any
importing module `M`, at most one instance `instance C T` is in
scope in `M`. This is the payoff of grounding 07's no-orphan rule
in a concrete module system: the no-overlap guarantee from 07
generalises from a single source file to the entire import
closure without further machinery.

## Re-exports

An export item `module Other.Module` re-exports every name that
`Other.Module` itself exports, under the same names. The
re-exported module must already be imported by the re-exporting
module — the `module M` export form refers to a name that must
first be brought into scope. Any form of `import` suffices
(`import M`, `import qualified M`, `import M as L`, etc.); the
precondition is scope, not unqualification.

```
module Data.Containers
  ( module Data.List
  , module Data.Map
  ) where

import Data.List
import Data.Map
```

Re-exports are **transitive**: if `Data.List` re-exports
`module Data.List.Internal`, then `module Data.Containers
( module Data.List )` also (transitively) exposes everything
`Data.List.Internal` exports. This is what "re-exports compose
monotonically" means.

**Name collisions across re-exports.** When two re-exports would
expose the same name, the umbrella's export list follows the
same rule as §Visibility's two-imports case:

- If the two re-exported names resolve to the **same** binding
  (i.e. the same original source declaration, traced through any
  chain of re-exports), the umbrella exports one copy and no
  ambiguity arises.
- If the two re-exported names resolve to **different** bindings,
  the umbrella's export list is a static error. The re-exporter
  must either rename one (via an `as`-imported module), or drop
  one from the export list, or export the selected names
  individually instead of using `module M`.

Re-exporting lets an umbrella module present a curated API across
several implementation modules. Whether the re-export form should
admit selective projection (e.g. `module Data.List (foo, bar)`-style)
is open; see 08 OQ 3.

## Design notes (non-normative)

- **One file = one module.** Hand-in-glove with path-based module
  names. Mirroring the filesystem keeps tooling simple and makes
  `Data.List` obvious on the page.

- **Unqualified import by default.** Matches Haskell's and Elm's
  common case. `import qualified` is one word longer and worth
  the explicitness when name conflicts loom.

- **Explicit export list for library code.** The spec admits both
  forms, but library authors should prefer the explicit form.
  Examples in M9 will use explicit lists.

- **Exported signatures.** The "exported bindings need signatures"
  rule was a compromise between the friendly-to-write optional
  regime and the friendly-to-read required-everywhere regime. It
  places the cost exactly at the place where the payoff lives: the
  module boundary.

- **No orphans enforced by the module system.** 07's no-orphans
  rule would be unenforceable without a concrete notion of "the
  module that defines C" — this document gives it one. The
  trade-off is stronger than Haskell's default (Haskell only
  warns on orphans) and matches modern Haskell style / PureScript /
  Idris practice.

- **Cyclic imports forbidden.** §Cyclic imports makes this
  normative. Design-note amplification: M9 example programs may
  yet reveal cases that warrant an escape hatch (Haskell-style
  `.hs-boot` signatures), but until then the DAG constraint holds.

- **`import` is a kind of `decl`.** The BNF places `import_decl`
  as a top-level `decl`, but in practice imports cluster at the
  top of a file after the `module` header. This document does not
  force that placement syntactically; a tooling / style convention
  will. Real-world code puts imports first.

## Open questions

1. **`Maybe` vs `Maybe(..)` defaults.** Sapphire's default when a
   type appears in an export list without a constructor clause is
   "type only, no constructors." Haskell's is the same. An
   alternative would be `Maybe` ≡ `Maybe(..)` (export-everything
   by default). Draft: no.

2. **Diagnostic timing for leaked private types.** A private type
   used in an exported signature leaks an identifier that
   importers cannot name. Should the compiler reject such a
   signature at *definition time* (early, clear error in the
   defining module), or accept it silently and let importers fail
   at use sites (late, actionable error at the call site)? Draft
   is silent; both are defensible.

3. **Selective re-exports.** `module M (foo, bar)` style selective
   re-export? Draft admits only whole-module re-export. Selective
   forms (either `module M (foo)` or `M.foo`) are OQ.

4. **Mutual recursion across modules.** Strictly disallowed at
   this layer. Haskell admits it via `hs-boot` files; Sapphire may
   eventually need a similar escape. Default: no.

5. **Default `module Main` without a header.** Draft admits it as
   sugar for small examples. Whether library files must have an
   explicit `module` header, or whether any `.sp` file may omit
   it, is OQ. Leaning toward "library files require the header,
   single-file scripts may omit".

6. **Module-level fixity declarations.** 05 / 07 use a fixed
   operator table with tiers. If 05 OQ 3 (user-declarable fixity)
   ever flips yes, fixity declarations would be module-scoped and
   this document would need to specify export / import semantics
   for them — including whether `module Other.Module` re-exports
   also re-export fixity declarations. Not planned now; noted for
   completeness.

7. **Per-method class export.** The draft admits only
   `class C` or `class C(..)`. Haskell admits `class C(m1, m2)` to
   export selected methods. Whether to extend 08's
   `export_item` grammar with a similar form is open.
