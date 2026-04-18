# 02. Source and output layout

Status: **draft**. Pipeline-level companion to `docs/spec/08-modules.md`
(module rule) and `docs/spec/10-ruby-interop.md` §Generated Ruby
module shape (output naming).

## Scope

This document fixes:

- The on-disk shape of a Sapphire project's *source* tree.
- The mapping from Sapphire module names to source files (per 08
  §One module per file).
- The on-disk shape of the *generated Ruby* tree.
- The mapping from Sapphire module names to Ruby file names and
  class hierarchies (per 10 §Generated Ruby module shape).
- The handful of project-root files that the pipeline expects.

Out of scope:

- The CLI surface that drives compilation — see 04.
- The runtime gem that the generated code depends on — see 03.
- How a host Ruby application loads the generated tree — see 05.

## Project root

A Sapphire project is rooted at a directory containing at least:

- `sapphire.yml` — project configuration (schema in 04). Its
  presence marks the directory as a Sapphire project root and
  anchors all other paths.
- `src/` — the source tree (see §Source tree below).

Recommended additions, none of which the pipeline strictly
requires:

- `gen/` — default output tree (see §Output tree); created on the
  first build if absent.
- `Gemfile` — for managing the runtime-gem dependency from a host
  Ruby project (see 05).
- `README.md`, `LICENSE`, etc. — ordinary project hygiene.

```
my-project/
├── sapphire.yml
├── src/
│   ├── Main.sp
│   └── Data/
│       ├── List.sp
│       └── List/
│           └── Extra.sp
├── gen/                     # populated by `sapphire build`
│   └── sapphire/
│       ├── main.rb
│       └── data/
│           ├── list.rb
│           └── list/
│               └── extra.rb
├── Gemfile
└── README.md
```

The project-root location is what the user runs the compiler
from (or what they pass via `--project-root`; see 04). All paths
in this tree are written relative to that root.

## Source tree

Per `docs/spec/08-modules.md` §One module per file, every Sapphire
source file is exactly one module, and the module's dotted name
mirrors its path under the source tree.

The pipeline's source-tree contract:

- All Sapphire sources live under a single directory; the default
  is `src/` and the configuration may override it (see 04
  §Configuration schema).
- Source files use the `.sp` extension. Files with other
  extensions are ignored by the compiler (but may be picked up by
  unrelated tooling).
- A file at `src/A/B/C.sp` declares module `A.B.C` (per 08); the
  compiler reports a static error if the declared module name and
  the path-derived name disagree.
- A file at `src/Main.sp` declares module `Main`. A single-segment
  module's source file lives at the root of `src/`.
- Directory names that are intermediate path segments (here `A`
  and `B`) must be valid `upper_ident`s (per 02 §Identifiers); the
  compiler reports an error otherwise.
- Empty directories and directories containing no `.sp` files are
  ignored (no error, no output).

The source tree is **flat in module-namespace terms**: there is
no notion of a "package" or "library" directory above `src/`.
Multi-package projects are out of scope at this layer (consistent
with 08 §Cyclic imports's "package boundaries" deferral).

### Naming rules

- File and directory names are case-sensitive on disk; the
  pipeline treats them as such regardless of host filesystem case-
  insensitivity. On a case-insensitive filesystem, two source
  files whose names differ only by case (`Foo.sp` and `foo.sp`)
  are an error at discovery time.
- The `.sp` extension is always lowercase.
- The single-file-script form (per 08 §One module per file: a
  source without an explicit `module` header is sugar for `module
  Main`) is admitted for `src/Main.sp` only. Other files must
  carry an explicit `module` header.

## Output tree

The output tree mirrors the source tree's shape, transformed by
two rules:

- The Sapphire module-name segments (which are `upper_ident`s in
  PascalCase per 02) become **lowercase** path segments on disk
  (Ruby idiom: file names are `snake_case`).
- The leaf segment becomes the file basename plus `.rb`; the
  ancestor segments become directories.
- Every output file lives under a single top-level `sapphire/`
  directory inside the output tree. This wraps the generated code
  in a single namespace directory regardless of how many
  top-level Sapphire modules there are, mirroring the
  `Sapphire::*` Ruby class hierarchy of 10.

| Sapphire module    | Source path             | Output path                       |
|--------------------|-------------------------|-----------------------------------|
| `Main`             | `src/Main.sp`           | `gen/sapphire/main.rb`            |
| `Data.List`        | `src/Data/List.sp`      | `gen/sapphire/data/list.rb`       |
| `Data.List.Extra`  | `src/Data/List/Extra.sp`| `gen/sapphire/data/list/extra.rb` |

The default output root is `gen/`. The configuration may
override it (`output_dir:` in `sapphire.yml`; see 04).

### Why a separate `gen/` tree (not `lib/`)

Two output-root candidates exist:

- `gen/` — emphasises "generated, do not edit by hand"; should
  be `.gitignore`d in most workflows.
- `lib/` — Ruby idiom for "what gets packaged in a gem".

The draft picks **`gen/`** as the default to keep "this is
machine output" obvious to a human reading the project tree, and
to avoid colliding with hand-written Ruby that a host application
might keep in its own `lib/`. Projects that publish the generated
tree as a gem (see 05 §Releasing as a gem) configure
`output_dir: lib/` in `sapphire.yml`. Whether the default itself
should flip to `lib/` is 02 OQ 1.

### Why a `sapphire/` wrapper directory

Putting all output under a `sapphire/` wrapper directory inside
`gen/` (so files land at `gen/sapphire/main.rb` rather than
`gen/main.rb`) means the host application's `$LOAD_PATH` setup
is symmetric: a single `$LOAD_PATH << 'gen'` makes
`require 'sapphire/main'` work, which lines up with the
`Sapphire::Main` class name (per Ruby's `require`-vs-namespace
convention). This also keeps the output tree from polluting the
top level of `gen/` if other tooling writes there.

## File-content shape

The shape of a single generated `.rb` file is fixed by 10
§Generated Ruby module shape. This document does not redefine
that contract. A pipeline-level summary, for orientation:

```ruby
# Generated from src/Data/List.sp; do not edit.
# sapphire-compiler vX.Y.Z   sapphire-runtime ~> X.Y

require 'sapphire/runtime'
# (other `require`s for imported modules go here)

module Sapphire
  module Data
    class List
      # class methods for each exported binding
      def self.map(f, xs)
        # generated implementation
      end

      # factory class methods for each exported constructor
      def self.Nil
        { tag: :Nil, values: [] }
      end

      def self.Cons(x, xs)
        { tag: :Cons, values: [x, xs] }
      end
    end
  end
end
```

Key pipeline-level requirements on the file's contents:

- The first line is a generation-provenance comment: source path,
  compiler version, runtime gem version constraint. The exact
  format is implementation-detail; the requirement is that any
  generated file be unambiguously identifiable as such by a
  human reader.
- The file `require`s `'sapphire/runtime'` (the runtime gem; see
  03) and any modules that the source `import`s (per 08).
- The body is exactly the `Sapphire::M₁:: ... ::Mₙ` namespace
  walk per 10, with the leaf as a `class`.

### Cross-module `require`s

When module `A.B.C` imports module `X.Y` (per 08 §Imported names
and scope), the generated `gen/sapphire/a/b/c.rb` emits a
`require 'sapphire/x/y'`. The require path mirrors the output
file path (with `.rb` stripped) so that a host application's
`$LOAD_PATH << 'gen'` setup makes the require resolve.

Re-exports (08 §Re-exports) do not affect `require` shape: a
module that re-exports another module still emits a single
`require` for the re-exported module, and Ruby's load-once
semantics handle the rest.

The generated `require` statements form the same DAG as the
Sapphire-level `import` graph (per 08 §Cyclic imports's acyclic
guarantee), so Ruby's `require` cannot diverge or loop.

## What does *not* go in the output tree

- The Sapphire runtime gem's source (it is a separate gem; see 03).
- Hand-written Ruby (the user's own host code) — that lives in
  the host application's tree, not in `gen/`.
- Test files for compiled Sapphire code — see 05 for where those
  go.

The output tree is **disposable**: deleting `gen/` and rebuilding
must produce a byte-identical (or at minimum semantically
identical) tree from the same source and the same compiler
version. Hand-edits to files in `gen/` are out of contract; the
pipeline is allowed to overwrite them.

## Interaction with other documents

- **Spec 08.** This document realises 08's "compilation root"
  notion as the concrete `src/` directory under the project
  root. The path-to-module-name correspondence that 08 leaves
  to the build tool is fixed here.
- **Spec 10.** The output naming ("Sapphire::M₁:: ... ::Mₙ" class
  hierarchy) is normative in 10 §Generated Ruby module shape; this
  document fixes the on-disk realisation of that hierarchy (one
  Ruby file per Sapphire module, snake_case path segments).
- **Build 03.** The runtime-gem `require` line at the top of
  every generated file is what couples the output tree to the
  runtime gem; 03 documents that gem.
- **Build 04.** The `src_dir:` and `output_dir:` config keys that
  let the user override the defaults live in 04 §Configuration
  schema.
- **Build 05.** How a host Ruby app's `$LOAD_PATH` and `require`
  lines pick up the output tree is documented in 05 §Embedding.

## Open questions

1. **Default output directory: `gen/` vs `lib/`.** Draft: `gen/`.
   `lib/` is more Ruby-idiomatic for code that ships as a gem
   but conflates with hand-written Ruby. Deferred to
   implementation phase.

2. **Embedded source-tree variants.** Some projects want
   `src/sapphire/` (under a sub-namespace) rather than `src/`
   directly. Draft: only the flat `src/` root is admitted in v0;
   the configuration may rename the root but not nest it.
   Deferred to implementation phase.

3. **Multi-source-root projects.** A workspace-style project with
   multiple independent source trees (e.g. `core/src/` and
   `app/src/`) would need either separate compiler invocations or
   a workspace-level `sapphire.yml`. Draft: out of scope for v0;
   one project = one source root. Deferred.

4. **Output tree as a gem.** A project that ships its compiled
   output as a Ruby gem will want `lib/sapphire/...` plus a
   `*.gemspec`. The pipeline could either gain a flag for "gem
   layout" or leave gem packaging to the user. Draft: leave to
   the user; document the steps in 05. Deferred.

5. **Filesystem case sensitivity on macOS / Windows.** On
   case-insensitive filesystems, the case-clash check at
   discovery time (above) protects against ambiguity, but the
   compiler still has to make file-system calls in a way that
   does not silently coerce. Draft: discovery treats names
   case-sensitively and reports a clash; how the host language
   exposes case-sensitive filesystem APIs is forward-deferred to
   `docs/impl/`.

6. **Generated-file header format.** The provenance comment at
   the top of each `.rb` file (source path, compiler version,
   runtime constraint) is mandatory but its exact text is
   currently underspecified. Draft: settle during implementation;
   the requirement is "human-readable, single line, includes
   compiler version".
