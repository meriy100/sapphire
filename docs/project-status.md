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

As of 2026-04-19, Waves 1–6 and the I9 audit of Wave 7 have landed.
The compiler pipeline runs end-to-end **lexer → layout resolution →
parser → name resolution → type checker (HM + ADT/Record + type
classes) → codegen (expr / ADT / `Ruby` monad) → CLI**, and the
`sapphire-runtime` gem (R1–R6) executes the generated Ruby with
thread-based `Ruby` monad and version-checked loading. The LSP
serves `publishDiagnostics` / `hover` / `definition` over
incremental text sync, and the VSCode extension wraps the stack.
The M9 example suite (4 programs, 30–80 lines each) runs 4/4
under `sapphire build` + `ruby -I runtime/lib`. `docs/impl/30-
first-release-audit.md` records the evidence. Wave 7 residuals
(L6 completion, D3 release-prep) and the next-phase scoping
conversation with user are the outstanding work items.

## Current phase: implementation exit reached (2026-04-19)

The spec-first phase concluded on 2026-04-19 with two milestones
landing:

- **M10** — spec-freeze review (`docs/spec/13-spec-freeze-review.md`),
  which audited open questions across 01–12 and consolidated them
  into `docs/open-questions.md`.
- **I1** — host-language selection (`docs/impl/05-decision.md`),
  which chose **Rust** as the language in which Sapphire's compiler
  will be written.

The implementation phase (tracks I / R / L / T / S / D, see
`docs/impl/06-implementation-roadmap.md`) opened the same day. By
the evening of 2026-04-19 its exit condition was reached: the five
"first release" criteria in `docs/impl/06-implementation-roadmap.md`
§完成の定義 are all met, per the I9 audit
(`docs/impl/30-first-release-audit.md`):

1. `.sp` → `.rb` compilation is wired through `sapphire-compiler`.
2. `sapphire-runtime` executes the generated modules (142 rspec
   examples pass).
3. All four M9 example programs run end-to-end under
   `sapphire build` + `ruby -I runtime/lib`.
4. CLI `build` / `run` / `check` are implemented in a single
   `sapphire` binary.
5. The VSCode LSP returns `publishDiagnostics`, `hover`, and
   `definition` for `.sp` files.

Residual items from the implementation-phase roadmap:

- **L6 completion** and **D3 release-prep** (CHANGELOG / tag /
  gem-push plumbing) continue in their own worktrees.
- Publishing decisions (gem push, GitHub Release, VSCode
  marketplace, license dual-licensing) remain user judgement calls
  (`docs/open-questions.md` I-OQ11, I-OQ29–33, I-OQ78).
- A handful of known soundness / ergonomics holes (I-OQ60, I-OQ63,
  I-OQ82, I-OQ96, I-OQ97, etc.) are documented and deliberately
  deferred. The audit lists 27 such items.

### Next phase: to be scoped with user

`CLAUDE.md` §Phase-conditioned rules was written for the
implementation phase and references its exit condition. Now that
the exit is met, the next phase needs to be **scoped with user
before further rules work**:

- Candidate scopes include self-host exploration, stdlib
  expansion, packaging ecosystem (gem / marketplace publish),
  and closing the soundness OQs. None of these are pre-selected.
- The audit document flags the specific CLAUDE.md lines that will
  need to be revised once scope is agreed
  (`docs/impl/30-first-release-audit.md` §5).
- Until user signs off, the existing rules remain in force:
  spec-tree host-language neutrality, `docs/impl/` decision
  records, `docs/open-questions.md` as the OQ index.

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
