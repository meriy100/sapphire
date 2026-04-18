# 01. Build pipeline overview

Status: **draft**. The `docs/build/` tree is the implementation-side
companion to the spec tree (`docs/spec/`); see the parallel-track note
in `docs/roadmap.md` (B1). This tree describes how a Sapphire program
is turned into runnable Ruby and embedded into a host Ruby
application. It is not a language-spec change.

## Audience

Two readers are kept in mind throughout `docs/build/`:

1. The Sapphire owner, who will use these documents as the contract
   that drives implementation work.
2. The eventual implementer of a Sapphire compiler, who needs to know
   the build-time contract (what files exist, where output goes, what
   the runtime looks like, how the user invokes the toolchain) before
   writing code.

Both readers are presumed to be familiar with Ruby and with the spec
documents under `docs/spec/`. Especially relevant are documents 10
(Ruby interop data model) and 11 (Ruby evaluation monad), which fix
the *generated-code contract*. This tree extends that contract with
*pipeline-level* concerns; it does not redefine anything 10 or 11
have settled.

## What this tree covers

The five documents in `docs/build/` cover, in order:

- **01 (this document)** — the pipeline as a whole and what each
  subsequent document handles.
- **02 — source and output layout.** The tree shape on disk:
  `src/*.sp` source files (per 08 §One module per file) and the
  generated Ruby tree under (proposed) `gen/`. The mapping rule from
  Sapphire module names to Ruby class hierarchies (per 10 §Generated
  Ruby module shape).
- **03 — the Sapphire runtime gem.** The Ruby support library that
  every compiled Sapphire program depends on: ADT helpers (tagged
  hashes per 10 §ADTs), the `Ruby` monad evaluator (per 11
  §Execution model), exception catching that produces `RubyError`
  (per 10 §Exception model), marshalling helpers (`to_sapphire` /
  `to_ruby`).
- **04 — invocation and configuration.** The user-facing CLI
  (`sapphire build`, `sapphire run`, `sapphire check`), the project
  configuration file (proposed `sapphire.yml`), build ordering by
  the module DAG (per 08), notes on incremental compilation.
- **05 — testing and integration.** How to call generated Sapphire
  classes from Ruby test frameworks (RSpec / Minitest), how to embed
  the generated tree into a host Ruby project (Rails, Sinatra, plain
  Ruby), Bundler integration, and the (optional) path to publishing
  the generated module as a gem.

## One-paragraph pipeline sketch

A Sapphire project is a tree of `.sp` source files under `src/` plus
a project-root configuration file. The user invokes the Sapphire
compiler (`sapphire build` or equivalent) from the project root. The
compiler reads the configuration, discovers the source tree, computes
the module dependency DAG (per 08 §Cyclic imports), parses and
type-checks each module, and emits one Ruby file per Sapphire module
into the output tree (proposed `gen/`). The emitted Ruby code follows
10 §Generated Ruby module shape: a `Sapphire::M₁:: ... ::Mₙ` class
hierarchy with one class per leaf module. The emitted code depends
at runtime on the **Sapphire runtime gem** (proposed name
`sapphire-runtime`), which provides ADT marshalling, the `Ruby` monad
evaluator, the exception-to-`RubyError` boundary, and the
`to_sapphire` / `to_ruby` value helpers. The user's Ruby application
then `require`s the generated tree and calls into the
`Sapphire::*` classes as ordinary Ruby. The `sapphire run` CLI is a
convenience wrapper that builds (incrementally if possible) and
invokes a designated entry point in one step.

```
   .sp sources                           generated Ruby
   (project root)                        (output tree)
   ┌────────────────┐    ┌──────────┐    ┌──────────────────┐
   │ src/Main.sp    │    │ Sapphire │    │ gen/sapphire/    │
   │ src/Data/      │ -> │ compiler │ -> │   main.rb        │
   │   List.sp      │    │ (host    │    │   data/list.rb   │
   │ sapphire.yml   │    │  TBD)    │    │                  │
   └────────────────┘    └──────────┘    └──────────────────┘
                                             │
                                             │ require + call
                                             ▼
                                         ┌──────────────────┐
                                         │ Host Ruby app    │
                                         │ + sapphire-      │
                                         │   runtime gem    │
                                         └──────────────────┘
```

## Where decisions live

Pipeline decisions split across two trees:

- `docs/spec/` — *what the language and its boundary look like*.
  Normative for the surface language, the type system, and the Ruby
  data model (10) / monad semantics (11). Pipeline documents must
  not contradict spec documents; they extend the spec into the
  build-time and runtime layers.
- `docs/build/` (this tree) — *how the compiler is invoked, where
  files live, what the runtime gem provides, how a host Ruby
  application consumes the result*. Normative for the build
  pipeline contract.
- `docs/impl/` (proposed in 13 §Freeze decision; not yet created) —
  *what host language the compiler itself is written in*, plus the
  compiler-internal architecture (parser, type-checker, code
  emitter). The host-language decision is **not** made here; this
  tree stays neutral about whether the compiler is written in Rust,
  OCaml, Haskell, Ruby itself, or anything else.

When a build-pipeline question reduces to a host-language choice,
this tree flags the dependency on `docs/impl/` rather than picking
a side.

## Host-language neutrality

`CLAUDE.md` §Phase-conditioned rules requires the spec tree to stay
neutral about the eventual compiler-host language. The build tree
inherits that neutrality with one widening: **Ruby is named, because
Ruby is the target language**. The compiled output, the runtime gem,
and the host application are all Ruby (per `docs/project-status.md`,
Ruby 3.3). The compiler **itself** is host-language-agnostic in this
tree.

Concretely, that means:

- Documents in this tree describe Ruby file names, Ruby class
  hierarchies, Ruby gem packaging, and Ruby APIs freely.
- Documents in this tree do **not** say "the compiler reads its
  config with `serde_yaml`" or "the parser uses `parsec`"; those
  are forward-deferred to `docs/impl/`.
- Where a pipeline rule depends on host-language ergonomics (e.g.
  whether incremental builds use a content-addressed cache or a
  timestamp comparison), this tree states the *contract* (the
  observable input/output behaviour) without prescribing the
  *mechanism*.

## Versioning and Ruby target

The target Ruby version is fixed at **Ruby 3.3** by
`docs/project-status.md`. The runtime gem (03) and the generated
code (02) both target Ruby 3.3. Whether the runtime gem widens its
version constraint to admit Ruby 3.4+ or future 4.x is 01 OQ 1.

The compiler version and the runtime-gem version are conceptually
independent but pragmatically coupled: the runtime gem encodes the
generated-code calling convention (e.g. tagged-hash shape, `:tag` /
`:values` keys). A version bump on either side that changes the
calling convention is a breaking change for users with a mixed
generation: a regenerate-and-rebundle is required after any version
upgrade that touches 10 §Data model or 11 §Execution model. A
compatibility / version policy beyond this is 01 OQ 2.

## Open questions

These are pipeline-level OQs. They follow the spec-doc convention
(numbered, brief, with a draft position) but they all defer to the
implementation phase by default — none of them blocks documenting
the contract.

1. **Ruby version-pin policy for the runtime gem.** Should the
   runtime gem's `required_ruby_version` constrain to `~> 3.3` only,
   or also admit `>= 3.4`? Draft: pin to `~> 3.3` until a Ruby 3.4
   tested matrix exists; deferred to implementation phase.

2. **Compiler / runtime version-compatibility policy.** A formal
   "compiler emits code for runtime API version `N`; runtime gem
   declares supported API versions `[N, N+1, ...]`" scheme would
   let the two evolve independently. Draft: implicit lockstep
   (compiler version `X` requires runtime version `~> X`); revisit
   if a real divergence appears. Deferred to implementation phase.

3. **Source-map propagation to Ruby backtraces.** If the generated
   Ruby raises (or a `Ruby` snippet inside it raises), Ruby's
   backtrace will reference generated `.rb` lines, not the original
   `.sp` source. Should the pipeline emit source-map metadata that
   the runtime gem consults to rewrite backtrace entries to point
   at `.sp` lines? Draft: no source maps in v0; backtrace strings
   are Ruby-native. Deferred to implementation phase.

4. **Whether `sapphire run` invokes a fresh compile or a Rake
   task.** Two shapes exist: (a) the CLI is a one-shot tool that
   compiles in-process and then `require`s the output; (b) the
   pipeline is a Rake-task library and the CLI is a thin wrapper
   around `rake build && rake run`. Draft: (a), with (b)
   admissible as a future ergonomic addition. Deferred to
   implementation phase.

5. **Compiler self-hosting.** If the compiler is eventually written
   in Sapphire itself (a long-term option, not a short-term
   commitment), `sapphire build` would have to bootstrap from a
   pre-compiled snapshot. Draft: out of scope for v0; flag if and
   when the host-language decision in `docs/impl/` selects
   Sapphire.
