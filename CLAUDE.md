# Sapphire

Sapphire is a planned Elm-inspired functional language. Its compiler is
intended to read `.sp` sources and emit a Ruby module that can be called from
plain Ruby. A signature feature is a `RubyEval`-style monad that runs embedded
Ruby snippets on a separate thread and threads the result back into the pure
pipeline.

At the time of writing there is **no source code in this repository**. A
previous Rust-based lexer / parser / codegen existed but was wiped in the
"refresh" commit; every implementation decision is open.

## Current phase: language specification

The user's chosen order is **spec first, implementation language second.**
Work on the language spec until its shape is substantially settled, then
pick the implementation language based on what the spec actually requires.

Practical consequences while in this phase:

- Stay neutral about the eventual host language. Do not bake "this is how we
  would write it in Rust / OCaml / Haskell / TypeScript" into spec documents.
  Prefer BNF, judgement rules, type rules, and pseudo-code when illustrating.
- Do not scaffold a compiler project, add a `Cargo.toml` / `package.json` /
  `dune-project` / `Gemfile`, or install a language toolchain until the
  implementation-language decision happens in a later phase.
- If the user asks for implementation or a prototype that is obviously tied
  to a specific host language, surface the tension first — they may want to
  revisit the ordering or deliberately prototype.

Things that are legitimately in scope right now:

- Deciding lexical syntax (keywords, literals, operators, layout rules).
- Deciding the type system (type variables, records, ADTs, type classes or
  not, row polymorphism or not, etc.).
- Deciding the module / import / visibility story.
- Designing the Ruby interop model: how `.sp` programs express embedded
  Ruby, how results are modeled, the async/monad semantics, error shapes,
  how the generated Ruby module is shaped and named.
- Writing motivating example programs in the proposed syntax.
- Collecting open questions and tradeoffs for the user to decide.

When the spec has enough shape (roughly: core syntax, core type system, and
the Ruby interop / monad story are all answered), revisit phase ordering
with the user.

## Runtime environment

- Development happens inside the devcontainer defined in `.devcontainer/`,
  shipping Ruby 3.3 (for running generated modules later) and the GitHub CLI.
  The Node feature was intentionally removed; Claude Code here is the native
  binary installed via `curl -fsSL https://claude.ai/install.sh | bash`.
- The container is driven by the `devcontainer` CLI (not VS Code). Typical
  flow: `devcontainer up --workspace-folder .` then
  `devcontainer exec --workspace-folder . bash`, then run `claude` inside.
- Claude Code runs with `permissions.defaultMode: "bypassPermissions"`.
  Commands execute without approval prompts. Despite that, do not take
  destructive or out-of-scope actions without being asked: no
  `git push --force`, no `rm -rf` outside this repo, no remote writes to
  systems not explicitly in scope.

## GitHub access

- `gh` and `git` authenticate via a fine-grained PAT scoped to **this
  repository only** (`meriy100/sapphire`). The token is loaded from
  `.devcontainer/.env`, which is gitignored.
- The token literally cannot reach other repositories or organizations, but
  do not try to act on them anyway.

## Conventions

- Instruction files for Claude (this `CLAUDE.md`, anything under `.claude/`,
  future agent / command / skill definitions) are written in **English**.
  User-facing documents (README, design notes, chat replies, commit
  messages) can stay in Japanese.
- Chat replies to the user should stay in Japanese unless the user switches.
- Do not commit build artifacts or secrets listed in `.gitignore`
  (`/target`, `.sapphire/`, `vendor/bundle/`, `.devcontainer/.env`).
- Before making non-trivial changes (new files, schema proposals, design
  docs), state the intent briefly and proceed — do not wait for approval on
  every step, but do surface material direction changes.
- When spec decisions are made, record them in a design doc inside the repo
  (suggested path: `docs/spec/`) rather than only in conversation.
