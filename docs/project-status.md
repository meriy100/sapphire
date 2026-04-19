# Sapphire — project status

Living document describing *where the project currently is*. Rules that
Claude must follow while operating in the repository live in `CLAUDE.md`
at the repository root; this document is the background that explains
why those rules are shaped the way they are.

## What Sapphire is

Sapphire is a planned Elm-inspired functional language. Its compiler is
intended to read `.sp` sources and emit a Ruby module that can be called
from plain Ruby. A signature feature is a `RubyEval`-style monad that
runs embedded Ruby snippets on a separate thread and threads the result
back into the pure pipeline.

At the time this document was first written there was **no substantive
source code in this repository**. That is still true as of the
phase-transition day (2026-04-19); scaffolding begins next.

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
this phase:

- Scaffolding the Rust compiler project (`Cargo.toml`, `src/`, CI).
- Implementing lexer, parser, AST, type checker, code generator in
  stages aligned with the spec documents.
- Building `sapphire-runtime` — the Ruby-side support gem that
  generated code depends on (per `docs/build/03-sapphire-runtime.md`).
- Tutorial maintenance (T2 pedagogy revision track) alongside
  implementation feedback.
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
