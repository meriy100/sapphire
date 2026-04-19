# CLAUDE.md

Rules for Claude when operating in this repository. For background
(what Sapphire is, what phase the project is in, how the devcontainer
and GitHub access are set up), see `docs/project-status.md`.

## Phase-conditioned rules (implementation phase, from 2026-04-19)

The spec-first phase concluded with M10 (`docs/spec/13-spec-freeze-
review.md`) and I1 (`docs/impl/05-decision.md`). Sapphire is now in
the **implementation phase** with Rust as the chosen host language.
This phase ends when Sapphire has a first usable compiler + runtime
that can run the M9 example programs end-to-end (see
`docs/project-status.md` §Current phase for the canonical
exit condition). At that point, revisit this section with the user
— subsequent phases (self-host exploration, broader stdlib,
ecosystem) will be scoped separately.

- **Spec tree (`docs/spec/`) stays host-language-neutral.** Do not
  bake "this is how Rust expresses it" into spec documents. Spec
  remains the target-contract for the implementation and any future
  self-host rewrite. Prefer BNF, judgement rules, type rules, and
  pseudo-code when illustrating.
- **Compiler scaffolding is unblocked.** `Cargo.toml`, `src/`,
  rustup / rustfmt / clippy config, CI workflows, and language-
  toolchain setup are now expected work. Do not add `package.json`
  / `dune-project` / `Gemfile` etc. — the host language decision
  (Rust) is settled per `docs/impl/05-decision.md`.
- **Record decisions in the repo, not only in conversation.** Spec
  decisions live in `docs/spec/`; implementation-side decisions
  live in `docs/impl/`; build-pipeline decisions live in
  `docs/build/`. Trivial code review comments can stay in chat,
  but anything shaping the code's structure goes in a doc.
- **Open-question tracking** continues via `docs/open-questions.md`
  as the living index. Any new OQ discovered during implementation
  goes there with an `I-OQk` ID (see that document's naming rules).
- **Rust-specific choices during implementation** — crate selection,
  MSRV pinning, error-handling patterns, parser strategy — are
  recorded under `docs/impl/` before the code change that assumes
  them, so the rationale survives.

## Git, GitHub, and out-of-scope systems

- Do not act on repositories or organizations other than
  `meriy100/sapphire`. The PAT is scoped such that the token *cannot*
  reach them anyway, but do not try.
- Do not run destructive or out-of-scope commands without being asked:
  no `git push --force`, no `rm -rf` outside this repo, no remote
  writes to systems not explicitly in scope. `bypassPermissions` mode
  removes the approval prompts, not the rule.
- Do not commit files listed in `.gitignore`. That file is the
  single source of truth for what is excluded from history — do not
  mirror its contents here.

## Parallel work with git worktrees

- Create git worktrees only under `.worktree/<name>` at the
  repository root. No temp directories, no sibling directories of
  the repo.
- Do not launch sub-agents in any mode that creates a worktree
  outside `.worktree/`. In particular, do not pass
  `isolation: "worktree"` to the Agent tool. If a sub-agent
  genuinely needs an isolated checkout, create it first with
  `git worktree add .worktree/<name> <ref>` and hand the agent
  that path explicitly in the prompt.
- Remove a worktree with `git worktree remove .worktree/<name>`,
  not `rm -rf`.

## Writing conventions

- Instruction files for Claude (this `CLAUDE.md`, anything under
  `.claude/`, future agent / command / skill definitions) are written
  in **English**.
- User-facing documents (README, design notes under `docs/`, commit
  messages) can stay in Japanese.
- Spec documents live in two parallel trees with matching filenames:
  the English version under `docs/spec/` and a Japanese translation
  under `docs/spec/ja/`. The English version is the normative source;
  the Japanese version is a translation kept in sync. When a spec
  document is added or its normative content changes, update both
  trees in the same change.
- Chat replies to the user should stay in Japanese unless the user
  switches.

## Working style

- Before making non-trivial changes (new files, schema proposals,
  design docs), state the intent briefly and proceed — do not wait for
  approval on every step, but do surface material direction changes.
- When spec decisions are made in conversation, persist them in
  `docs/spec/`; the chat is not the source of truth.

## Review flow

- After completing non-trivial work, the main Claude session invokes
  the `reviewer` agent as a subagent on the produced artifact, before
  treating the work as done. "Non-trivial" covers: any new file, any
  new or materially changed rule in an instruction file, any change
  to the normative content of a spec document, and any code change
  beyond renames or formatting. Typo fixes, heading renames, and
  formatting-only tweaks do not count and do not require review.
- For each item the reviewer raises, judge it: if it is a valid
  point, apply the fix; if it is not, note briefly why and move on.
- After applying fixes, re-invoke the `reviewer` agent on the updated
  artifact. Repeat up to a total of **three** review iterations
  (initial review + up to two follow-ups).
- Stop the loop earlier as soon as the reviewer returns
  `Verdict: approve` or `Verdict: approve with suggestions`. Three
  iterations is the ceiling, not a target.
- If must-fix items remain unresolved after three iterations, stop
  the loop and surface the remaining items to the user instead of
  continuing.
- When a commit is requested, let the review flow settle first;
  uncommitted review iterations should not be frozen into history.
