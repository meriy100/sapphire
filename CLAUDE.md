# CLAUDE.md

Rules for Claude when operating in this repository. For background
(what Sapphire is, what phase the project is in, how the devcontainer
and GitHub access are set up), see `docs/project-status.md`.

## Phase-conditioned rules (spec-first phase)

These rules are in force while the project is in the spec-first phase
described in `docs/project-status.md`. When that phase ends, revisit
this section with the user.

- Stay neutral about the eventual host language. Do not bake "this is
  how we would write it in Rust / OCaml / Haskell / TypeScript" into
  spec documents. Prefer BNF, judgement rules, type rules, and
  pseudo-code when illustrating.
- Do not scaffold a compiler project, add a `Cargo.toml` /
  `package.json` / `dune-project` / `Gemfile`, or install a language
  toolchain until the implementation-language decision happens in a
  later phase.
- If the user asks for implementation or a prototype that is obviously
  tied to a specific host language, surface the tension first — they
  may want to revisit the ordering or deliberately prototype.
- Record spec decisions in a design doc inside the repo (under
  `docs/spec/`) rather than only in conversation.

## Git, GitHub, and out-of-scope systems

- Do not act on repositories or organizations other than
  `meriy100/sapphire`. The PAT is scoped such that the token *cannot*
  reach them anyway, but do not try.
- Do not run destructive or out-of-scope commands without being asked:
  no `git push --force`, no `rm -rf` outside this repo, no remote
  writes to systems not explicitly in scope. `bypassPermissions` mode
  removes the approval prompts, not the rule.
- Do not commit files listed in `.gitignore` (`/target`, `.sapphire/`,
  `.bundle/`, `vendor/bundle/`, `.devcontainer/.env`,
  `.claude/settings.local.json`).

## Writing conventions

- Instruction files for Claude (this `CLAUDE.md`, anything under
  `.claude/`, future agent / command / skill definitions) are written
  in **English**.
- User-facing documents (README, design notes under `docs/`, chat
  replies, commit messages) can stay in Japanese. Spec documents under
  `docs/spec/` are written in English.
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
