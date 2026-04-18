# 04. Records

Status: **draft**. Subject to revision as pattern matching (M3) and
modules (M4) land.

## Motivation

Records are Sapphire's **named-field product types**. Document 03
introduced ADTs, whose alternatives may already carry unnamed
positional arguments (`data Pair a b = Pair a b`), but positional
fields scale poorly past two or three elements and leak position
information into every use site. Records pair each component with a
name, decoupling construction and access from declaration order.

This document fixes:

- How record **types** are written and when two record types are
  equal.
- How record **values** are constructed.
- How individual fields are **selected** from a record.
- How a record is **updated** to produce a new record with some
  fields replaced.

The following programs anchor what this layer must accept:

```
-- A record type and literal
let origin = { x = 0, y = 0 } in origin

-- Field selection
let p = { x = 3, y = 4 } in p.x

-- Functional update (produces a new record)
let p = { x = 3, y = 4 } in
let p' = { p | x = 10 } in
p'

-- Records inside a data type
data Shape = Circle { cx : Int, cy : Int, r : Int }
           | Rect   { x1 : Int, y1 : Int, x2 : Int, y2 : Int }
```

The last example is **illustrative only**: whether record-shaped
constructor payloads are admitted at this layer is an open question;
see §Design notes and question 2. The rest of this document specifies
only bare records that appear as expression and type forms.

Deliberately deferred at this layer:

- **Destructuring / pattern matching on records.** M3.
- **Row polymorphism** (`{ f : Int | r }`-style extensible records).
  The draft is **closed structural records**, matching Elm 0.19;
  row polymorphism is question 1.
- **Record-punning shorthand** (`{ x, y }` for `{ x = x, y = y }`).
  Question 3.
- **Module-qualified field names.** Resolved together with
  qualified names in M4; the interaction of `.` with qualified
  names is discussed under §Design notes.

## Abstract syntax (BNF)

Extending documents 01 and 03:

```
expr       ::= ...                                (01, 03)
             | '{' field_bindings? '}'            -- record literal
             | '{' expr '|' field_bindings '}'    -- record update
             | expr '.' lower_ident               -- field selection

field_bindings ::= field_binding (',' field_binding)*
field_binding  ::= lower_ident '=' expr

type       ::= ...                                (01, 03)
             | '{' field_types? '}'               -- record type

field_types ::= field_type (',' field_type)*
field_type  ::= lower_ident ':' type
```

Field names are `lower_ident` (02 §Identifiers) — the same class as
ordinary term identifiers and type variables.

Static well-formedness of a record type or literal requires:

- Field names within a single record type (or literal) must be
  pairwise distinct. `{ x : Int, x : String }` is a static error.
- The empty record `{}` is admitted both as a type and as a value;
  see §Design notes.

§Ambiguity below discusses how the grammar is disambiguated in
practice.

### Ambiguity

The forms `{ e | ... }` (record update) and `{ f = e, ... }` (record
literal) start with the same opening brace, so a parser must inspect
more than one token of lookahead; identifying which arm is in play
is a parser concern, not a language-level ambiguity.

The `.` in `expr '.' lower_ident` is the same `.` that 02 reserves
for module-qualified names (resolved in M4). Resolution in this
document: at a parse site, `.` immediately after an expression and
before a `lower_ident` is field selection; `.` between an
`upper_ident` (module name) and any identifier is a qualified name,
which is not introduced until M4. The `forall TVAR* '.' type` use of
`.` (01) occurs only at scheme position and does not conflict with
expression-level field selection.

Field selection is **left-associative**: `p.x.y` parses as
`(p.x).y`, not `p.(x.y)`. This falls out of the BNF by treating
`.f` as an operator applied to a preceding expression.

Trailing commas are **not** admitted in `field_bindings` or
`field_types`: the BNF `field_binding (',' field_binding)*` requires
each `,` to be followed by a further binding. `{ x = 1, }` is a
syntax error.

## Types

A record type

    { f₁ : τ₁, ..., fₙ : τₙ }   with all fᵢ pairwise distinct

has kind `*`. Two record types are equal iff they have the same set
of field names and each shared field has an equal field type.
**Field order is not semantically significant** — `{ x : Int, y : Int }`
and `{ y : Int, x : Int }` are the same type.

A record type may contain type variables in its field types. Such a
record type quantifies nothing by itself; generalization is still
performed only at `let` (01) or top-level bindings (03), exactly as
for any other type.

Records are **structural**: there is no nominal tag on a record
type beyond its field set. Two distinct `data` declarations may wrap
"the same" record shape as different types; the record layer itself
does not distinguish them.

## Typing rules

### Record literal

```
Γ ⊢ e₁ : τ₁    ...    Γ ⊢ eₙ : τₙ     (all fᵢ pairwise distinct)
————————————————————————————————————————————————————————————————   (Rec)
 Γ ⊢ { f₁ = e₁, ..., fₙ = eₙ } : { f₁ : τ₁, ..., fₙ : τₙ }
```

The empty record rule is the `n = 0` instance of (Rec):

```
————————————————————     (RecEmpty)
 Γ ⊢ { } : { }
```

### Field selection

```
 Γ ⊢ e : { f₁ : τ₁, ..., fₙ : τₙ }    j ∈ {1, ..., n}
—————————————————————————————————————————————————————   (Sel)
              Γ ⊢ e.fⱼ : τⱼ
```

Selection on a non-record type is a type error; selection of a field
name absent from the record's static type is also a type error. No
implicit structural subsumption is performed: a record of type
`{ x : Int, y : Int }` does not also have type `{ x : Int }` at this
layer (see question 1 for the alternative).

### Record update

```
Γ ⊢ e : { f₁ : τ₁, ..., fₙ : τₙ }
Γ ⊢ eᵢ₁ : τᵢ₁    ...    Γ ⊢ eᵢₖ : τᵢₖ       {i₁, ..., iₖ} ⊆ {1, ..., n}
all updated-field names pairwise distinct
——————————————————————————————————————————————————————————————————————   (Upd)
Γ ⊢ { e | fᵢ₁ = eᵢ₁, ..., fᵢₖ = eᵢₖ } : { f₁ : τ₁, ..., fₙ : τₙ }
```

Update preserves the input record's type: it neither introduces new
fields nor removes existing ones, and each replacement expression's
type must equal the declared type of the field being replaced.
Attempting to update a field absent from `e`'s static type is a type
error.

An update expression of zero updated fields `{ e | }` is not part of
the grammar; a minimum of one field binding is required by
`field_bindings`.

## Design notes (non-normative)

- **Closed structural records.** This layer admits only
  fully-specified record types. A function that takes a record
  argument must state exactly which fields it expects; it cannot
  abstract over "any record containing at least `x : Int`". The
  alternative — row polymorphism — is deferred (question 1). Elm
  chose the closed path (having experimented with extensible
  records in earlier versions and retreated), and Sapphire adopts
  that as its first pass.

- **Empty record.** `{}` has type `{}`. It is a unit-like value for
  the record layer and serves mostly as a typed placeholder for
  Ruby interop (M7) payloads that carry no data.

- **Records vs. `data` with named fields.** Document 03 explicitly
  deferred record-shaped constructor payloads to this document.
  Whether a `data` declaration's constructor may carry named
  fields (`data Shape = Circle { cx : Int, cy : Int, r : Int }`),
  and whether such a declaration desugars to a positional `data`
  wrapping an anonymous record, is question 2. The motivating
  example at the top of this document assumes the desugaring form
  for readability; the spec does not yet admit it.

- **Field names share the term-identifier namespace.** Field names
  are `lower_ident`s. A field name can coincide with a term-level
  binding in scope; `p.x` in
  `let x = 1 in let p = { x = 2 } in p.x` is `2`, not `1`, because
  `.f` is a dedicated selection form that does not look `f` up in
  the environment.

- **No row unification at this layer.** The absence of row
  polymorphism means record types are decided by structural
  equality on their field sets — no row variables, no row
  unification. Introducing row polymorphism later would require a
  row-unification procedure; deferring it keeps M2 decidable by
  the same machinery 01 and 03 already assume.

- **`this` and method dispatch are out of scope.** Sapphire is not
  an object-oriented language. Records are data; they have no
  implicit receiver, no inheritance, and no methods. Call sites use
  ordinary function application over projected fields
  (`(describe p).x` rather than `p.describe.x`).

## Open questions

1. **Row polymorphism.** Should Sapphire admit extensible record
   types (`{ f₁ : τ₁, ..., fₙ : τₙ | r }` with `r` a row variable)?
   A `yes` answer enables functions polymorphic over "any record
   with at least these fields" at the cost of row unification in
   the type checker, and interacts with 01 OQ 5's "type-class-like
   resolution" alternative. The draft answer is **no** (closed
   records, Elm 0.19 style).

2. **Record-shaped constructor payloads in `data`.** Two questions
   in sequence. First, should a `data` constructor be allowed to
   declare named fields at all (e.g.
   `data Shape = Circle { radius : Int }`)? Second, if so, is the
   surface form **primitive** — each field becoming a top-level
   selector in its own right, à la Haskell's record-syntax ADTs —
   or is it **sugar** for a positional constructor taking a single
   anonymous-record argument whose type is this document's record
   type? The two readings look identical at the declaration site
   but diverge on what selectors and patterns become available.
   This document takes no position; the design interacts with
   pattern matching (M3).
   *Closed 2026-04-18*: do not admit named-field constructor
   payloads in the first implementation. Constructor arguments
   are positional; a user who wants named access declares a
   record type and has the constructor take that record as a
   single argument (e.g.
   `type Circle = { radius : Int }; data Shape = Circle Circle`).
   Revisiting (b) "sugar form" remains available as a pure
   addition if demand emerges later.

3. **Punning shorthand.** Should `{ x, y }` be admitted as shorthand
   for `{ x = x, y = y }` in record literals (and, once M3 lands,
   for `{ x, y }` as a pattern)? A `yes` answer would match
   PureScript / Elm; a `no` answer is simpler to parse and teach.
   *Closed 2026-04-18*: no — not admitted. Users write the
   explicit form `{ x = x, y = y }`.

4. **Symmetric update (field addition / removal).** `{ r | f = v }`
   currently replaces an existing field's value without changing
   the record's type. An orthogonal extension would admit
   `{ r | f = v }` for fields *not* yet in `r`, producing a record
   with one extra field. This only makes sense in the presence of
   row polymorphism (question 1); noted here for completeness.

5. **Field name clashes across records.** `{ x = 1 }.x` and
   `{ x = "hi" }.x` are both well-typed at different types. Whether
   a later module / typeclass mechanism forbids unqualified uses of
   an ambiguous field name across imports (Haskell's `DuplicateRecordFields`
   debate) is beyond this document; M4 revisits.
