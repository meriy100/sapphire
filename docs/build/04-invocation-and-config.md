# 04. Invocation and configuration

Status: **draft**. Pipeline-level companion to
`docs/spec/08-modules.md` (module DAG). This document fixes the
user-facing CLI of the Sapphire compiler, the project
configuration file's name and schema, the build ordering, and
forward-looking notes on incremental compilation.

## Scope

In scope:

- The CLI executable's name and its three primary subcommands
  (`build`, `run`, `check`).
- The `sapphire.yml` configuration file: location, name, schema,
  defaults.
- Build ordering: topological traversal of the module DAG (per
  08 §Cyclic imports).
- Notes on incremental compilation that v0 may take or defer.

Out of scope:

- The compiler's internal architecture (parser, type-checker,
  code emitter) — forward-deferred to `docs/impl/`.
- Source / output tree shape — see 02.
- Runtime gem behaviour — see 03.
- Test-runner integration — see 05.

## Executable name

The CLI executable is **`sapphire`**. It is installed by whatever
mechanism the host-language phase (`docs/impl/`) eventually
chooses; this document does not prescribe the install mechanism.

When a user types

```
$ sapphire <subcommand> [args] [options]
```

at a project root (or with `--project-root <dir>` to override),
the CLI dispatches to one of three subcommands.

A bare `sapphire` (no subcommand) prints a help summary and
exits with a non-zero status. A `sapphire --help` prints the
same summary and exits with zero.

## Subcommands

### `sapphire build`

Compiles every Sapphire module in the project and writes the
output tree (per 02 §Output tree).

```
$ sapphire build [--project-root DIR] [--config FILE]
                 [--clean] [--verbose]
```

Behaviour:

- Reads the configuration (defaults: `sapphire.yml` at the
  project root).
- Discovers source files under `src_dir:` (default `src/`).
- Computes the module DAG (per 08).
- Compiles every module; writes one `.rb` per module under
  `output_dir:` (default `gen/`).
- Exit status `0` on success, non-zero on any compile error.

Flags:

- `--project-root DIR` — anchor for relative paths in the
  configuration. Defaults to the current working directory.
- `--config FILE` — path to a configuration file other than
  `sapphire.yml`.
- `--clean` — remove the `output_dir:` tree first, then build.
  Equivalent to `rm -rf gen/ && sapphire build`. Provided as a
  CLI flag because the runtime may forbid the user from
  deleting `gen/` directly while a build is running.
- `--verbose` — emit per-module timing and dependency-resolution
  diagnostics. Off by default; default output is one line per
  module compiled (or an error summary).

### `sapphire run`

Builds the project (if needed) and invokes a designated entry
point.

```
$ sapphire run [<entry>] [--project-root DIR] [--config FILE]
               [--no-build] [--] [arg...]
```

Behaviour:

- If `--no-build` is not given, performs a `sapphire build`
  first.
- Resolves `<entry>` to a Sapphire module + binding pair. The
  default entry is `Main.run` — module `Main`, binding `run`.
  An explicit `<entry>` takes the same `Module.binding` form
  (e.g. `App.serve`). Module segments are `upper_ident`
  (PascalCase) per 02 §Identifiers; binding names are
  `lower_ident`.
- The resolved binding's type must unify with `Ruby a` for some
  `a` (per 11 §`run`). The pipeline invokes the
  `Sapphire::Runtime::Ruby.run(entry_action)` and inspects the
  resulting `Result`. On `Ok a`, `sapphire run` exits zero. On
  `Err e` (a `RubyError`), `sapphire run` prints the error
  (class name, message, backtrace) and exits non-zero.
- Arguments after `--` are forwarded to the entry binding's
  Ruby snippet via a runtime-supplied mechanism. The exact
  mechanism (a `Sapphire.argv : List String` global? a CLI-arg
  Sapphire binding?) is 04 OQ 1.

The `sapphire run` subcommand is the convenience wrapper that
spec 11's `Ruby a -> Result RubyError a` exit point implies the
need for. Whether it is implemented as in-process compile +
require + invoke, or as a Rake task wrapper, is 01 OQ 4.

### `sapphire check`

Type-checks every module without writing output.

```
$ sapphire check [--project-root DIR] [--config FILE]
                 [--verbose]
```

Behaviour:

- Performs the same parsing and type-checking that `build`
  does, but **does not** invoke the code emitter and does not
  touch the output tree.
- Exits zero on success, non-zero on any error.
- Intended use: editor / pre-commit hook / CI check that
  catches errors faster than a full build.

The `check` subcommand's existence is a usability convenience.
Whether it should run as a daemon for editor integration (in
the LSP spirit) is 04 OQ 2.

## Common CLI conventions

- All subcommands accept `--project-root` and `--config`. They
  default to the current directory and `sapphire.yml`
  respectively.
- All subcommands write progress / error output to stderr and
  any "primary product" output to stdout. (For `build` and
  `check` the primary product is the file tree; stdout is
  empty on success. For `run` the primary product is the
  entry point's effects, which the runtime executes; stdout
  is whatever the entry point writes.)
- All subcommands support `--help` to print their own usage
  summary.
- Exit statuses are POSIX-conventional: `0` on success,
  non-zero on error. A more granular status taxonomy (e.g.
  `2` for parse error vs `3` for type error) is 04 OQ 3.

## Configuration: `sapphire.yml`

The project configuration file lives at the project root with
the default name `sapphire.yml`. YAML is chosen as the format
because it is widely understood by Ruby developers (the
audience), readable, and supported by Ruby's standard library.
JSON is admitted as an alternative format (`sapphire.json`)
without further configuration; whether to add other formats
(TOML, plain Ruby DSL) is 04 OQ 4.

### Schema (v0)

```yaml
# sapphire.yml — Sapphire project configuration

# Project name. Currently informational; future use for
# generated-gem packaging (see 05 §Releasing as a gem).
name: my-project

# Project version. Same status as `name`.
version: 0.1.0

# Source-tree root, relative to the project root.
# Default: src/
src_dir: src/

# Output-tree root, relative to the project root.
# Default: gen/
output_dir: gen/

# Default entry for `sapphire run`, in `Module.binding` form.
# Default: Main.run
entry: Main.run

# Runtime gem version constraint that the generated code
# embeds in its provenance header.
# Default: matches the compiler version's runtime dependency.
runtime: '~> 0.1'

# Ruby executable used by `sapphire run` to evaluate the
# generated code. Defaults to the host's `ruby` on $PATH.
# A path or a version manager spec ('asdf:3.3.0',
# 'rbenv:3.3.0') may be admitted later.
ruby: ruby

# (forward-looking) per-module compile flags; v0 admits an
# empty map only.
modules: {}
```

Every key has a default. A project that uses the defaults can
ship a near-empty `sapphire.yml` containing only `name:` and
`version:` (or, for one-off scripts, an empty file — the
pipeline still treats the directory as a Sapphire project root).

The schema **is versioned implicitly via the runtime / compiler
version**. A schema-change that is not backwards-compatible
bumps the compiler's major version. Whether to add an explicit
`schema_version:` key is 04 OQ 5.

### Validation

The pipeline validates the configuration at startup:

- Unknown keys are an error (not a warning), so that a typo
  like `sources_dir:` doesn't silently fall back to the
  default. The runtime / compiler version is included in the
  error message so the user knows whether a key is unknown
  because of a typo or because they're on an older compiler.
- `src_dir:` and `output_dir:` must be relative paths under
  the project root. Absolute paths or `..`-traversal are
  errors.
- `entry:` must be syntactically a `Module.binding` form
  (resolution against the actual module set is deferred until
  `sapphire run` time).

## Build ordering

Per 08 §Cyclic imports, the module graph (whose nodes are
modules and whose edges are `import` relationships) is acyclic.
The pipeline computes a topological order over this DAG and
compiles modules in that order.

Pipeline-level ordering contract:

- Modules are compiled bottom-up: leaves of the import DAG
  first, dependent modules last. A module's compile begins only
  after every module it imports has been fully compiled
  (parsed, type-checked, and emitted).
- Within a single topological "level" (modules with no
  unsatisfied dependency from each other), parallel compilation
  is admitted but not required. The v0 pipeline may compile
  serially; a parallelism flag is 04 OQ 6.
- A cycle in the import graph is a static error reported by
  the pipeline before any output is written.

The toposort is also what makes 08 §Instances and modules'
no-orphan invariant computable per build: instance visibility
follows transitive imports, so a bottom-up compile order lets
each module observe exactly the instances in its import closure.
The pipeline computes the toposort once per build invocation;
incremental builds may cache it (see §Incremental compilation
below).

## Incremental compilation

v0's commitment is **forward-looking only**: the pipeline must
behave **as if** every build were clean (i.e. produce the same
output tree as `sapphire build --clean`). Whether the pipeline
actually rebuilds every module or skips unchanged ones is an
implementation choice — both behaviours are admissible v0.

A v0+ incremental scheme would, sketched:

1. For each source file, hash its contents and the contents
   of every module it transitively imports (the *interface
   hash*, not just the source hash — a module's *type*
   signature changing should invalidate every dependent's
   cache).
2. Persist a per-build cache keyed by `(compiler version,
   runtime version, module name, interface hash)` mapping to
   the generated `.rb`.
3. On rebuild, recompute interface hashes and skip emission
   for any module whose `(version, version, name, hash)` key
   is unchanged.

Pipeline-level commitments around incremental builds:

- The cache is opaque to the user: its on-disk shape is not
  contract. The user-visible contract is "the output of
  `sapphire build` matches the output of `sapphire build
  --clean`".
- The cache lives under the project root (proposed
  `.sapphire-cache/`) and should be `.gitignore`d.
- `sapphire build --clean` always produces a clean output
  tree; the cache is invalidated as part of `--clean`.

The exact cache mechanism, hash algorithm, and on-disk format
are forward-deferred to `docs/impl/`. v0 may ship without
incremental builds; that is not a contract violation.

## Diagnostics

The CLI's primary user-visible failure mode is "the source
doesn't compile". Pipeline-level commitments:

- Diagnostics are written to stderr.
- Each diagnostic carries: source path, line number, column
  range, severity (`error` / `warning`), short summary, and
  optional explanatory paragraph.
- The set of diagnostics is *complete* before exit: the
  pipeline does not stop at the first error in `build`-time
  type checking. The exact "how many errors before bailing"
  policy (every error vs at most N) is 04 OQ 7.
- Diagnostic *text* is host-language-agnostic; the wording
  is a property of the compiler implementation, not of this
  pipeline contract.

A future LSP-style integration (04 OQ 2) would consume the
same diagnostic shape over a JSON-RPC channel; the
diagnostic-format machine encoding is therefore part of the
forward-looking work, not the v0 contract.

## Interaction with other documents

- **Spec 08.** §Build ordering realises 08's acyclic
  module-DAG guarantee as the topological sort the pipeline
  performs.
- **Spec 11.** §`sapphire run` invokes
  `Sapphire::Runtime::Ruby.run` per 11 §`run`; the entry
  binding's required `Ruby a` shape comes from 11's typing.
- **Build 02.** All `src_dir:` / `output_dir:` defaults match
  02's defaults; the configuration just makes them
  user-overridable.
- **Build 03.** The runtime gem version constraint that
  `runtime:` controls is what 02 §File-content shape stamps
  into the generated provenance header.
- **Build 05.** Test-tool integration calls `sapphire build`
  internally (see 05 §Bundler integration).

## Open questions

1. **CLI argument forwarding to `sapphire run`'s entry.**
   Arguments after `--` need to reach the Sapphire entry
   binding. Options: a runtime-side `Sapphire.argv : List
   String` constant; an extra parameter on the entry binding's
   signature; a Ruby-snippet read of `ARGV`. Draft: a
   runtime-side `Sapphire::Runtime::Ruby.argv` accessor that
   `:=`-bound snippets can read. Deferred to implementation
   phase.

2. **`sapphire check` daemon mode for editor integration.**
   Whether the pipeline should expose an LSP-shaped server
   (`sapphire check --lsp`) for in-editor diagnostics. Draft:
   no in v0; one-shot `sapphire check` is enough for pre-commit
   and CI. Deferred.

3. **Granular exit-status taxonomy.** A finer mapping of exit
   statuses (`2` parse error, `3` type error, `4` link error,
   etc.) would let CI distinguish failure classes
   automatically. Draft: binary success / failure in v0;
   richer exit codes deferred.

4. **Configuration formats beyond YAML / JSON.** TOML, a Ruby
   DSL (e.g. `Sapphirefile`), or a `package.json`-style nested
   key. Draft: YAML + JSON only in v0. Deferred.

5. **Explicit `schema_version:` key in `sapphire.yml`.** Would
   let the pipeline give a precise "you wrote a v1 config but
   this compiler reads v2" error. Draft: implicit via compiler
   version; revisit if real schema breaks happen. Deferred.

6. **Parallel compilation at the same DAG level.** A
   `--jobs N` flag for parallel toposort. Draft: serial in
   v0; the contract admits parallel emission. Deferred.

7. **Bail-out policy on errors.** "Report every error before
   exit" vs "stop after N errors" vs "stop at first error".
   Draft: report every error before exit (closer to typical
   compiler experience and makes batch fix workflows easier).
   Tunable later.

8. **Watch mode (`sapphire build --watch`).** A
   filesystem-watching long-running build that re-emits on
   change. Draft: not in v0; deferred to implementation phase.

9. **Whether `sapphire run` should `bundle exec` automatically
   when a Gemfile is present.** Draft: yes — if a `Gemfile`
   exists at the project root, the runtime invocation goes
   through `bundle exec ruby ...`. Deferred to implementation
   phase to confirm against host-language ergonomics.
