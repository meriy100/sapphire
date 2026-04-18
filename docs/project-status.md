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

At the time of writing there is **no substantive source code in this
repository**. A previous Rust-based lexer / parser / codegen existed but
was wiped in the "refresh" commit; every implementation decision is
open.

## Current phase: language specification

The chosen order is **spec first, implementation language second.**
Work on the language spec until its shape is substantially settled,
then pick the implementation language based on what the spec actually
requires.

In scope during this phase:

- Lexical syntax (keywords, literals, operators, layout rules).
- Type system (type variables, records, ADTs, type classes or not, row
  polymorphism or not, etc.).
- Module / import / visibility story.
- Ruby interop model: how `.sp` programs express embedded Ruby, how
  results are modeled, the async / monad semantics, error shapes, how
  the generated Ruby module is shaped and named.
- Motivating example programs in the proposed syntax.
- Open questions and tradeoffs for the user to decide.

When the spec has enough shape (roughly: core syntax, core type system,
and the Ruby interop / monad story are all answered), phase ordering
will be revisited with the user.

Progress is tracked as numbered documents under `docs/spec/`
(`01-core-expressions.md` is the first of these).

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
