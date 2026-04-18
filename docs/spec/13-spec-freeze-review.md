# 13. Spec freeze review

Status: **review** — this document is the spec-first phase's own
closing act. It is non-normative (it does not introduce language
machinery) but it *does* produce a normative outcome: a decision
on whether Sapphire's spec is ready for the next phase, and on
how leftover open questions (OQs) are routed.

## Purpose

The spec-first phase of Sapphire (as defined by
`docs/project-status.md`) has produced twelve draft documents,
01–12. This document:

1. Summarises the status of each draft.
2. Consolidates every OQ across the drafts into a single audit
   table, with a proposed disposition for each.
3. Surfaces cross-document consistency checks.
4. Revisits `CLAUDE.md`'s phase-conditioned rules and proposes
   what changes when the implementation-language phase begins.
5. States the **freeze decision**: under what conditions the
   spec is considered stable enough to stop accreting and start
   implementing.

This document's outputs get applied back into the surrounding
repository after user sign-off — roadmap updates, draft-to-final
status bumps, `CLAUDE.md` rule revisions, and potentially the
deletion of OQs that are being deferred to implementation time
rather than resolved here.

## Document status summary

| Doc | Scope                                   | Status | Notes |
|-----|-----------------------------------------|--------|-------|
| 01  | Core expressions                        | draft  | All OQs closed or deferred by later docs. |
| 02  | Lexical syntax                          | draft  | All OQs closed by later docs except 02 OQ 4 (tabs) and OQ 5 (Unicode idents) — see §Consolidated OQs. |
| 03  | Data types (M1)                         | draft  | Higher-kinded OQ closed by 07. Others live. |
| 04  | Records (M2)                            | draft  | Row polymorphism (OQ 1) and related OQs remain open; none blocks implementation. |
| 05  | Operators and numbers (M5)              | draft  | OQ 2 closed by 07. Remaining: Float / `Num`, user-declarable fixity, sections, pipes, `^`. |
| 06  | Pattern matching (M3)                   | draft  | Guards, or-patterns, list-literal patterns (realised by 09), range patterns, let-patterns. |
| 07  | Type classes + HKT (MTC)                | draft  | MPTCs, flexible instance heads, overlap / orphans relaxation, `MonadFail`-like bind, kind annotations, `Num`, type families, `deriving`. |
| 08  | Modules (M4)                            | draft  | Selective re-export, leaked private types, mutual recursion escape, `module Main` sugar, fixity scoping, per-method class export. |
| 09  | Prelude (M6)                            | draft  | Tuples, type aliases, `Char`/`String`, `Num`, `IO` retype, infix composition, `<$>` / `=<<`-style operators. |
| 10  | Ruby interop data model (M7)            | draft  | `nil`-for-`Nothing`, operator mangling, symbol-vs-string keys, backtrace structure, `ruby_import`, Ruby 4.x. |
| 11  | Ruby evaluation monad (M8)              | draft  | Parallelism, timeouts, streaming, exception-class granularity, shared Ruby scope, nested `Ruby` join, thread ownership. |
| 12  | Example programs (M9)                   | draft  | Long-running Ruby, polymorphism-requiring example, Ruby-calls-Sapphire example, pure `main`, `type` alias syntax, `readInt` prelude dependency. |

All twelve are drafts. None has been promoted to "final". The
freeze decision below proposes a policy for when — and whether —
to do so.

## Consolidated open questions

Every OQ from 01–12, grouped by their likely disposition. The
"P" column marks the proposed disposition:

- **C** — "close here": the audit itself selects the answer and
  the draft is amended in this milestone. Typically chosen for
  tiny stylistic / syntactic questions that nothing downstream
  cares about.
- **K** — "keep open, route to implementation phase": the answer
  depends on writing the actual compiler / example corpus and
  seeing what the language feels like. The OQ stays in its doc
  but with a note that it is deferred rather than unresolved.
- **L** — "keep open, later language milestone": the answer
  requires more language-design thinking, likely beyond a first
  implementation.
- **D** — "decide now as part of M10": requires a user-visible
  position in this document because cross-doc consistency
  demands it.

| Doc  | OQ | Summary                                         | P | Disposition note |
|------|----|-------------------------------------------------|---|-------------------|
| 01   | 1  | `let` recursion and generalization              | — | Closed by 03. |
| 01   | 2  | Top-level signatures required / optional        | — | Closed by 08. |
| 01   | 3  | `if` primitive vs sugar                         | — | Closed by 09. |
| 01   | 4  | Numeric tower (partial)                         | K | Deferred with 05 OQ 1 / 07 OQ 6. |
| 01   | 5  | Built-in operators                              | — | Closed by 05 + 07. |
| 02   | 1  | `True` / `False` as class vs constructors       | — | Closed by 09. |
| 02   | 2  | Unary minus                                     | — | Closed by 05. |
| 02   | 3  | Operator table fixed vs user-declared           | — | Closed by 05 (plus 05 OQ 3 for relaxation). |
| 02   | 4  | Tabs in layout positions                        | C | Keep strict-by-default. No implementation experience argues for tab-stop logic. Amend 02 to drop the OQ. |
| 02   | 5  | Identifier character set (ASCII vs Unicode)     | C | Restrict to ASCII for the first implementation. Unicode identifiers can be added later (pure monotonic extension); revisit during implementation if Ruby-side names force it. Amend 02. |
| 02   | 6  | `::` disambiguation                             | — | Closed by 05 / 06. |
| 03   | 1  | Implicit recursion vs `let rec`                 | K | Draft is implicit; nothing in 04–12 argues against. |
| 03   | 2  | Local multi-binding `let` / local mutual        | K | Orthogonal to implementation. |
| 03   | 3  | Strictness                                      | K | Implementation-phase question. |
| 03   | 4  | `deriving`                                      | K | Implementation convenience; not required for a minimum compiler. |
| 03   | 5  | Namespace and shadowing of constructor names    | — | Closed by 06. |
| 03   | 6  | Local `data`                                    | L | No obvious user need yet. |
| 04   | 1  | Row polymorphism                                | L | Large design increment; orthogonal. |
| 04   | 2  | Record-shaped constructor payloads              | D | **Decision:** positional-only at M10. Named-field constructor payloads not admitted in the first implementation; `data T = T { ... }` is sugar for `data T = T <anonymous-record-type>` if ever added. Amend 04. |
| 04   | 3  | Record punning                                  | C | Do not admit in the first implementation. Amend 04. |
| 04   | 4  | Symmetric (add/remove) update                   | K | Requires row polymorphism first; rides with 04 OQ 1. |
| 04   | 5  | Field name clashes across records               | K | Resolved by module-qualified reference (08) in practice. Leave as-is. |
| 05   | 1  | `Float` / numeric polymorphism                  | K | Tied to 07 OQ 6. |
| 05   | 2  | Polymorphic equality / ordering                 | — | Closed by 07 + 09. |
| 05   | 3  | User-declarable fixity                          | K | No pressing need. |
| 05   | 4  | Operator sections                               | K | Convenience only. |
| 05   | 5  | Pipe operators                                  | K | Can be added as pure extension. |
| 05   | 6  | Exponentiation `^`                              | C | Do not include in the first implementation. Amend 05. |
| 06   | 1  | Guards                                          | K | Ergonomic; not blocking. |
| 06   | 2  | Or-patterns                                     | K | Ergonomic; not blocking. |
| 06   | 3  | List-literal patterns                           | — | Closed by 09. |
| 06   | 4  | Exhaustiveness on `Int` / `String`              | K | Ranges are optional future work. |
| 06   | 5  | Pattern bindings in `let`                       | K | Ergonomic extension. |
| 06   | 6  | Named-field constructor patterns                | — | Closed here via 04 OQ 2 disposition. |
| 06   | 7  | Empty `case_alts`                               | K | Rare corner case. |
| 07   | 1  | Multi-parameter type classes                    | L | Big design area; defer. |
| 07   | 2  | Flexible instance heads                         | L | Same. |
| 07   | 3  | Overlap / orphans relaxation                    | L | Same. |
| 07   | 4  | Refutable `do` binds                            | K | `MonadFail` is optional. |
| 07   | 5  | Source-level kind annotations                   | K | Orthogonal. |
| 07   | 6  | `Num` class                                     | K | Tied to 05 OQ 1. |
| 07   | 7  | Type families                                   | L | Out of first-implementation scope. |
| 07   | 8  | `deriving`                                      | K | Duplicates 03 OQ 4. |
| 07   | 9  | Superclass chaining worked example              | K | Pedagogical; handle in a tutorial rather than the spec. |
| 08   | 1  | `Maybe(..)` vs `Maybe` export default           | C | Keep the current default ("type only, no constructors"). Amend 08. |
| 08   | 2  | Leaked private type diagnostic timing           | C | Reject at definition time (early error). Amend 08. |
| 08   | 3  | Selective re-export form                        | K | Ergonomic; not blocking. |
| 08   | 4  | Mutual recursion across modules                 | L | Escape-hatch design work; defer. |
| 08   | 5  | `module Main` sugar for library files           | C | Single-file scripts may omit the header; library files must carry one. Amend 08. |
| 08   | 6  | Module-level fixity declarations                | K | Tied to 05 OQ 3. |
| 08   | 7  | Per-method class export                         | K | Ergonomic. |
| 09   | 1  | Tuples                                          | K | Records cover the use case. |
| 09   | 2  | Type aliases                                    | D | **Decision:** admit `type T = τ` as a simple alias (not a new nominal type). Amend 09 and 12. See §Cross-doc consistency. |
| 09   | 3  | `String` as list of `Char`                      | L | Keep `String` opaque; `Char` is not planned for the first implementation. |
| 09   | 4  | `Num` vs `Int`-only                             | K | Duplicates 05 OQ 1 / 07 OQ 6. |
| 09   | 5  | `IO` / concrete Ruby monad retype               | — | Closed by 11. |
| 09   | 6  | `Char` primitive                                | L | Same as 09 OQ 3. |
| 09   | 7  | Implicit prelude import mechanism               | K | Implementation detail. |
| 09   | 8  | Infix composition operator                      | K | Convenience; not blocking. |
| 09   | 9  | `<$>` / `=<<` operator sugar                    | K | Convenience. |
| 10   | 1  | `nil`-for-`Nothing` shortcut                    | C | Do not admit. Amend 10. |
| 10   | 2  | Operator-method mangling scheme                 | K | Implementation detail. |
| 10   | 3  | Symbol vs string hash keys                      | C | Symbol keys, as drafted. Amend 10. |
| 10   | 4  | Exception backtrace structure                   | K | Ergonomic. |
| 10   | 5  | `ruby_import` external files                    | L | No user demand yet. |
| 10   | 6  | Ruby 4.x support                                | K | Monitor. |
| 10   | 7  | Higher-arity ADT ergonomics                     | K | Ergonomic. |
| 11   | 1  | Parallel composition                            | L | Concurrency design; defer. |
| 11   | 2  | Timeouts / cancellation                         | L | Same. |
| 11   | 3  | Streaming                                       | L | Same. |
| 11   | 4  | Exception-class granularity                     | K | Extension. |
| 11   | 5  | Escape hatch for shared Ruby-side state         | L | Needs user feedback. |
| 11   | 6  | `join` as prelude                               | C | `join = (>>= id)`; expose as a prelude utility. Amend 09 and 11. |
| 11   | 7  | Generated Ruby class threading semantics        | K | Implementation detail. |
| 12   | 1  | Long-running Ruby example                       | K | Add during implementation phase. |
| 12   | 2  | Polymorphism-requiring example                  | K | Same. |
| 12   | 3  | Ruby-calls-Sapphire example                     | K | Same. |
| 12   | 4  | `type` alias in example 3                       | — | Closed here via 09 OQ 2 disposition. |
| 12   | 5  | All-pure example                                | K | Implementation phase. |
| 12   | 6  | `readInt` prelude dependency                    | D | **Decision:** add `readInt : String -> Maybe Int` and `readFloat : String -> Maybe Float` (the latter contingent on 05 OQ 1 / 07 OQ 6) to 09's minimum utility set. Amend 09 and 12. |

### M10-landed decisions

The **D** rows in the table above require user sign-off, since
each amends a previously-landed draft. Summarised:

1. **Record-shaped constructor payloads (04 OQ 2).** Do not
   admit in the first implementation. `data T = T { ... }` is
   reserved syntax but unresolved until a later spec milestone
   after implementation experience.
2. **Type aliases (09 OQ 2).** Admit `type T = τ` as a
   transparent alias (no nominal identity). Amend 09 with a
   small section defining the form, and update 12's example 3
   to cite the amendment rather than an open question.
3. **`readInt` / `readFloat` in prelude (12 OQ 6).** Add both
   to 09's utility set, with `readFloat` contingent on the
   numeric-tower decision. Update 09 and 12.

The **C** rows are typographic / non-blocking amendments that
can land alongside the M10 commit without further decision:
02 OQ 4 / 02 OQ 5; 04 OQ 3; 05 OQ 6; 08 OQ 1 / OQ 2 / OQ 5;
10 OQ 1 / OQ 3; 11 OQ 6.

## Cross-doc consistency checks

### Implicitly imported modules

09 (as amended by 10) says the implicit-import set is
`{ Prelude, Ruby }`. No other module joins it at draft time.
Any future implicit addition needs a new entry in both 09 and
whichever document introduces it.

### Keyword set (02)

Current keywords:
```
let    in     if     then   else   case   of
module import export where  forall data
class  instance do
as     hiding qualified
```

Post-M10 additions (amendment to 02):
```
type                       -- added by 13, for 09 OQ 2's `type T = τ`
```

Cross-referenced:
- `let` / `in` / `if` / `then` / `else` / `case` / `of` /
  `module` / `import` / `where` / `forall` — 01 and 02 (core
  forms and scheme notation).
- `export` — reserved in 02 but not consumed by any published
  grammar. **Unused at M10.** M10 keeps it reserved for a future
  selective-re-export syntax (08 OQ 3); this is a reservation, not
  a commitment.
- `data` — 03.
- `class` / `instance` / `do` — 07.
- `as` / `hiding` / `qualified` — 08.
- `type` — to be added by the M10 amendment (see §Interaction
  with earlier drafts below) for 09's type-alias form.

### Operator table (05)

| Tier | Associativity | Operators                          |
|------|---------------|------------------------------------|
| app  | left          | function application               |
| 9    | prefix        | `-` (unary)                        |
| 7    | left          | `*` `/` `%`                        |
| 6    | left          | `+` `-` (binary)                   |
| 5    | right         | `++` `::`                          |
| 4    | none          | `==` `/=` `<` `>` `<=` `>=`        |
| 3    | right         | `&&`                               |
| 2    | right         | `\|\|`                             |
| 1    | left          | `>>=` `>>`                         |

Cross-referenced:
- `++` : `String -> String -> String` (05 + 09)
- `::` : `∀ a. a -> List a -> List a` (05 + 09)
- `>>=` / `>>` : `Monad m => ...` (07 + 09 + 11)
- `==` / `/=` / comparisons : `Eq a =>` / `Ord a =>` (07 + 09)
- Arithmetic : `Int -> Int -> Int` (05 + 09)

Consistent across references.

### Reserved punctuation (02)

`=` `->` `<-` `=>` `:` `::` `:=` `|` `.` `\` `@`

All in use:
- `=` : definitions (01).
- `->` : arrows (01).
- `<-` : `do` bind (07).
- `=>` : constraints (07).
- `:` : signatures (01).
- `::` : cons (05 + 09).
- `:=` : Ruby embedding (10).
- `|` : ADT alternation (03), record update (04).
- `.` : qualified names (08), record selection (04).
- `\` : lambda (01).
- `@` : as-patterns (06).

All consumed. ✓

### `print`, `read*`, and the prelude

09's `print` is retyped by 11 to `Show a => a -> Ruby {}`. After
M10's prelude amendment (decision 2 + 3):

- `readInt : String -> Maybe Int` joins 09 as a pure utility.
- `readFloat : String -> Maybe Float` joins 09 once the
  numeric-tower decision is made.
- `join : Monad m => m (m a) -> m a` joins 09 (M10 decision from
  11 OQ 6 **C**).

These are additive changes; no existing references break.

## `CLAUDE.md` phase-conditioned rules

The current `CLAUDE.md` has a "Phase-conditioned rules
(spec-first phase)" section containing:

- Host-language neutrality (no "this is how Rust / OCaml / Haskell
  would do it" inside specs).
- No compiler scaffolding / no toolchain installs.
- Flag implementation-implying requests as tension.
- Record spec decisions inside `docs/spec/`.

Of these:

- **Host-language neutrality** should stay in force **through the
  implementation-language selection process**, then relax once a
  host is chosen. Once a language is picked, code and build
  scaffolding in that language become both admissible and
  expected.
- **No compiler scaffolding** should stay in force until the
  implementation-language decision lands, then invert —
  scaffolding becomes part of the work.
- The other two rules are phase-independent and stay regardless.

M10 does not amend `CLAUDE.md` itself; it produces a user-visible
recommendation for the next phase's revision. The recommended
next-phase `CLAUDE.md` section is:

> ## Phase-conditioned rules (implementation-language-selection phase)
>
> - The spec-first phase has concluded; 01–13 are drafts with
>   OQs dispositioned per 13. Do not re-open closed questions
>   without a fresh decision.
> - Implementation-language selection is now the primary
>   activity. Candidates, trade-off tables, prototype results,
>   and the selection decision itself live in `docs/impl/` (a
>   new tree).
> - Until a language is selected, continue host-language
>   neutrality in the spec tree (`docs/spec/`). Trade-off
>   discussion under `docs/impl/` is exempt.
> - Actual compiler scaffolding waits for the selection to land.
> - Continue to record spec decisions inside `docs/spec/` (rule
>   carried over from the spec-first phase); the chat is not the
>   source of truth. Implementation decisions are recorded
>   under `docs/impl/`.

This is an opt-in proposal; user decides.

## Freeze decision

Sapphire's spec meets the M10 freeze criteria:

1. **Core syntax** fixed: lexical structure (02) and operator
   layer (05) are stable after M10's amendments; the composite
   surface forms introduced by 03 (ADTs), 04 (records), 06
   (patterns), 07 (classes, `do`), 08 (modules), 09 (prelude)
   all consume 02's reservations without gaps.
2. **Core type system** fixed: 01 + 03 + 04 + 07 give a
   Hindley-Milner core with ADTs, records, and single-parameter
   type classes at kind `* -> *`.
3. **Ruby interop / monad story** fixed: 10 + 11 give a closed
   boundary with a named monad and a single `run` exit.
4. **Prelude** fixed: 09, after M10 amendments, carries the
   minimum set that the published examples assume.
5. **Remaining OQs** are either deferred to implementation time
   (K) or to a later language milestone (L); none blocks a
   minimum compiler.

**Recommendation:** declare the spec-first phase **substantially
complete** upon user sign-off on M10's D decisions and C
amendments. Bump 01–12 to status "draft (spec-first phase
complete)". Introduce `docs/impl/` for the next phase's work.

## Interaction with earlier drafts

This document **amends** these earlier drafts upon M10 landing
(each amendment is small and typographic):

- **02** — remove OQs 4 and 5, adding the resolved rules to
  §Layout / §Identifiers. **Also** add `type` to the keyword
  set (needed by 09's type-alias form).
- **04** — close OQ 2 with positional-only disposition; close
  OQ 3 with "no punning".
- **05** — close OQ 6 ("no `^`").
- **06** — close OQ 6 (named-field constructor patterns) as a
  side-effect of 04 OQ 2's positional-only disposition.
- **08** — close OQs 1, 2, and 5 per the decisions above.
- **09** — add `readInt` to the utility set unconditionally;
  add `readFloat` only when 05 OQ 1 / 07 OQ 6 lands (until
  then, leave it pending); add `join` to the utility set; close
  OQ 2 with a new §Type aliases section.
- **10** — close OQs 1 and 3 per the decisions above.
  **Also** respell `RubyError`'s constructor from named-field to
  positional (`data RubyError = RubyError String String (List
  String)`) per 04 OQ 2's positional-only disposition.
- **11** — close OQ 6 (`join = (>>= id)` exposed via 09).
- **12** — swap Example 3's `type Student = ...` reading from
  "OQ pending" to "closed by M10"; remove Example 2's
  caveats about `readInt` not being in 09's minimum set (it now
  is), and drop 12 OQ 6 accordingly.

The amendments themselves are not included in this document to
keep the review crisp; they land in a follow-up commit once the
user signs off.

## Design notes (non-normative)

- **Freeze vs freezing.** "Freeze" in 13 means "stop adding new
  layers" — not "no further editing ever". Drafts stay drafts;
  implementation-phase feedback will drive further revisions. A
  real "final" status only makes sense after at least one
  complete compiler implementation has stretched every corner.

- **Implementation-language selection comes next.** M10 does not
  pick a host language. That is the first activity of the next
  phase. Candidates, trade-off criteria, and prototype results
  belong under `docs/impl/`, not here.

- **Why not resolve more OQs here.** Most "K" and "L" questions
  need either implementation experience or deeper language-design
  thinking. Answering them in the audit document would either
  rush the design or pre-commit to positions that implementation
  would have to defend without evidence. Keeping them open is
  the disciplined path.

- **Cross-doc checks are where bugs hide.** The consistency
  tables in §Cross-doc consistency checks are the load-bearing
  part of this review. Every future spec edit should re-run
  them (keyword set, operator table, reserved punctuation,
  implicit imports, prelude signatures).

## Open questions

Deliberately small — M10 is the one milestone whose own OQs
resolve in the same document:

1. **Timing of the `docs/impl/` introduction.** Open it
   immediately on M10 landing, or only after the
   implementation-language selection begins? The former is
   low-cost; the latter keeps the tree uncluttered.

2. **Whether to promote any draft to "final".** Draft is the
   defensible status for every document until a compiler
   actually consumes the spec. "Final" is available as a
   future status but not used at M10 landing.

3. **Should `docs/roadmap.md` be closed off** (archived as
   `docs/archive/roadmap-spec-first.md` or similar) with a
   fresh `docs/impl/roadmap.md` for the next phase, or kept
   as a living document through the next phase? The former is
   cleaner; the latter keeps continuity.
