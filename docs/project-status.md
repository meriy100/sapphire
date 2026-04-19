# Sapphire — project status

Living document describing *where the project currently is*. Rules that
Claude must follow while operating in the repository live in `CLAUDE.md`
at the repository root; this document is the background that explains
why those rules are shaped the way they are.

## What Sapphire is

Sapphire is a planned Haskell-parity functional language (type classes
+ higher-kinded types). Its compiler reads `.sp` sources and emits Ruby
modules that can be called from plain Ruby. A signature feature is an
**effect monad** (type `Ruby a`, see `docs/spec/11-ruby-monad.md`) that
runs embedded Ruby snippets on a separate thread and threads the result
back into the pure pipeline.

As of 2026-04-19, Waves 1–4 and part of Wave 5 of the implementation
phase have landed. The compiler pipeline is now **lexer → layout
resolution → parser (with AST in `sapphire-core`) → name resolution**,
with type checking (I6) and runtime thread/loading support (R5/R6)
in flight. The LSP serves `publishDiagnostics` for lex/layout/parse
errors over incremental text sync and is being extended with
`textDocument/definition`. The tutorial now covers ch1–ch7 including
a HKT/typeclasses bonus chapter. Distribution design is documented;
actual cross-compile CI is the next wave. Progress is tracked in
`docs/impl/06-implementation-roadmap.md`.

## Current phase: implementation (from 2026-04-19)

The spec-first phase concluded on 2026-04-19 with two milestones
landing:

- **M10** — spec-freeze review (`docs/spec/13-spec-freeze-review.md`),
  which audited open questions across 01–12 and consolidated them
  into `docs/open-questions.md`.
- **I1** — host-language selection (`docs/impl/05-decision.md`),
  which chose **Rust** as the language in which Sapphire's compiler
  will be written.

The project is now in the **implementation phase**. In scope during
this phase (tracks I / R / L / T / S / D, see
`docs/impl/06-implementation-roadmap.md`):

- Scaffolding the Rust compiler project (`Cargo.toml`, `src/`, CI) —
  done (Wave 1, I2).
- Implementing lexer, parser, AST, type checker, code generator in
  stages aligned with the spec documents.
- Building `sapphire-runtime` — the Ruby-side support gem that
  generated code depends on (per `docs/build/03-sapphire-runtime.md`) —
  scaffold done (Wave 1, R1).
- Building a Language Server (VSCode-only for the first iteration,
  track L) using `tower-lsp`.
- Tutorial maintenance (T2 pedagogy revision track) alongside
  implementation feedback.
- Distribution design (single `sapphire` gem vs split, platform
  native gems, track D).
- Closing open questions under `docs/open-questions.md` as
  implementation work reveals answers.

This phase ends when Sapphire has a first usable compiler + runtime
that can run the M9 example programs end-to-end. Subsequent phases
(self-host exploration, broader stdlib, ecosystem) will be scoped
separately.

### Historical record: the spec-first phase (concluded)

Between the project's start and 2026-04-19, the chosen order was
**spec first, implementation language second.** Twelve spec drafts
(01–12) plus the freeze review (13) were produced under that
discipline; the tutorial (T1) and build-strategy (B1) parallel
tracks filled in supporting material. Host-language neutrality was
maintained in `docs/spec/` throughout. The decision to end this
phase and select Rust is recorded in `docs/impl/05-decision.md`.

Progress tracking continues via `docs/roadmap.md` (living
milestones) and `docs/open-questions.md` (living OQ index).

## Runtime environment

Development happens inside the devcontainer defined in `.devcontainer/`,
shipping Ruby 3.3 (for running generated modules later) and the GitHub
CLI. The Node feature was intentionally removed; Claude Code in this
container is the native binary installed via
`curl -fsSL https://claude.ai/install.sh | bash`.

The container is driven by the `devcontainer` CLI (not VS Code). Typical
flow:

```
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
# then run `claude` inside the container
```

Claude Code runs with `permissions.defaultMode: "bypassPermissions"`.
Commands execute without approval prompts.

## GitHub access

`gh` and `git` authenticate via a fine-grained PAT scoped to **this
repository only** (`meriy100/sapphire`). The token is loaded from
`.devcontainer/.env`, which is gitignored. The token literally cannot
reach other repositories or organizations.
