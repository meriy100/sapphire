# 02. Lexical syntax

Status: **draft**. Subject to revision as layout-sensitivity, string escapes,
and the operator table are exercised by later documents.

## Motivation and scope

Document 01 (core expressions) treated the tokens `IDENT`, `INT`, `STRING`,
`BOOL`, `TVAR`, and `TCON` as given. This document pins them down. It also
fixes the surface shape that later documents (ADTs, records, modules, Ruby
interop) will extend monotonically.

In scope:

- Character set and logical-line structure.
- Whitespace and comments.
- Identifiers (term-level and type-level).
- Keywords and reserved punctuation.
- Literals (integer, string; booleans are discussed under identifiers).
- Operator-character tokens.
- Layout: whether Sapphire is layout-sensitive, with an explicit-brace
  escape hatch.

Deferred to later documents:

- The full layout algorithm (the equivalent of the Haskell 2010 `L`
  function), which cannot be stated before `where` and `case` exist.
- Character, floating-point, and multi-line string literals.
- The operator precedence / associativity table (see question 3).
- Hexadecimal, octal, and binary integer literals.

## Source text

Sapphire source is a sequence of Unicode code points encoded as UTF-8.
Logical lines are terminated by `\n` (LF). `\r\n` (CRLF) is normalized to
`\n` on input; a bare `\r` is a lexical error. A UTF-8 BOM at the very
start of the file, if present, is ignored.

Line numbering starts at 1. Column numbering starts at 1 and counts
Unicode code points after any BOM has been stripped.

## Whitespace and comments

Whitespace characters are space (U+0020), horizontal tab (U+0009), and
line feed (U+000A). Tab width is **not** semantically significant; see
question 4 under *Layout*.

Comments come in two forms; both behave exactly like whitespace:

```
-- line comment, runs to end of the logical line
{- block comment, may span multiple lines,
   and {- may nest -} inside another block -}
```

Block comments nest. An unterminated block comment is a lexical error.
`--` inside a string literal is not a comment.

Comments and operator tokenization interact through the usual
maximal-munch rule: the lexer first takes the longest run of
`op_char`s it can, and only then asks whether the resulting run is
`--`. So `-->` lexes as a single operator token, never as `-` followed
by a line comment, while a line of source whose longest `op_char` run
happens to equal `--` begins a line comment.

## Identifiers

Two identifier classes are distinguished by the initial character:

```
lower_ident ::= [a-z_] [A-Za-z0-9_']*        -- term names, type variables
upper_ident ::= [A-Z]   [A-Za-z0-9_']*       -- type constructors,
                                             -- data constructors, module names
```

The prime character `'` may appear anywhere except as the first
character.

The token names used by document 01 map to the classes above as
follows:

- `IDENT` is `lower_ident` occurring in term position.
- `TVAR`  is `lower_ident` occurring in type position.
- `TCON`  is `upper_ident` occurring in type position.

A single underscore `_` is the reserved wildcard (used in patterns in
a later document) and is not a binding identifier.

## Keywords

The following lowercase words are reserved and cannot appear as
`lower_ident`:

```
let    in     if     then   else   case   of
module import export where  forall data
class  instance do
as     hiding qualified
```

`data` was added by document 03 under the additive-growth clause
below. `class`, `instance`, and `do` were added by document 07
(type classes and higher-kinded types) under the same clause.
`as`, `hiding`, and `qualified` were added by document 08
(modules) under the same clause.

`True` and `False` are **not** keywords. Their surface form is a
plain `upper_ident`. Whether the lexer nonetheless recognizes them
as a distinct token class, or leaves them as ordinary
`upper_ident`s that are resolved to the two values of `Bool` by a
prelude binding, is open question 1 below. §Boolean literals makes
the same point from the value side.

The keyword set will grow as later documents introduce data
declarations, pattern matching, and the Ruby-interop monad. Growth
during the spec phase is expected to be strictly additive.

## Literals

### Integer literals

```
int_lit ::= [0-9] [0-9_]*
```

Underscores are permitted as digit separators and are ignored in value
computation: `1_000_000` equals `1000000`. Leading zeros are permitted
but do not introduce octal interpretation. Hexadecimal, octal, and
binary forms are deferred.

Negative integer literals are **not** a lexical form: `-3` is an
application of the operator `-` (see question 2).

### String literals

```
string_lit   ::= '"' string_char* '"'
string_char  ::= <any Unicode code point except '"' and '\'>
               | '\' escape
escape       ::= 'n' | 't' | 'r' | '\\' | '"' | 'u' '{' hex_digit+ '}'
hex_digit    ::= [0-9a-fA-F]
```

`\u{…}` takes 1 to 6 hexadecimal digits naming a Unicode scalar
value; surrogate code points are a lexical error. An unknown
backslash escape is a lexical error.

At this layer a string literal does not span logical lines — a raw
`\n` inside the quotes is a lexical error. Multi-line string
literals are deferred.

### Boolean literals

The symbol `BOOL` used by document 01 ranges over the two surface
forms `True` and `False`. Whether these are recognized by the lexer
as a distinct token class, or are ordinary `upper_ident`s bound by
the prelude to the two values of `Bool`, is not decided in this
document — see open question 1 below. Both readings agree on the
surface syntax (the characters a programmer writes) but differ on
whether 01's (LitBool) rule is primitive or derivable from (Var)
plus a prelude binding.

## Operator tokens

Operator tokens are formed from the following symbol characters:

```
op_char ::= '+' | '-' | '*' | '/' | '%' |
            '<' | '>' | '=' | '!' | '&' | '|' |
            '^' | '?' | '~' | ':' | '.'
op      ::= op_char+     -- excluding the reserved forms listed below
```

An operator token is always the **maximal** run of `op_char`s, and
must not equal one of the reserved forms in the next section.

The parenthesis, bracket, and brace characters `(`, `)`, `[`, `]`,
`{`, `}` are each their own single-character token and are not
`op_char`s. Of these, `[` and `]` are not yet consumed by any
surface production — they are reserved ahead of time for a list
syntax introduced in a later document.

Comma `,` and semicolon `;` are each their own single-character
token.

The `op` nonterminal defined above is not referenced by any BNF
production at this layer. The tokenization it fixes (which runs of
`op_char`s form single tokens, and which are reserved) is what
matters; the grammar that consumes those tokens — including
precedence and associativity, see question 3 — is the concern of a
later document.

### Reserved punctuation

Certain operator-shaped sequences are reserved syntax rather than
user-level operators. They are excluded from the `op` nonterminal
above.

```
=     ->    <-    =>    :     ::    :=    |     .
\     @
```

- `=` is the definition form (used in `decl` and inside `let`).
- `->` is the function arrow (in types and in lambdas).
- `<-` is reserved for `do`-notation monadic bind (document 07).
- `=>` is the constraint / type-class arrow in schemes
  (document 07).
- `:` is the type-annotation separator (see document 01,
  `decl ::= IDENT ':' type`).
- `::` is reserved (candidates: list cons, pattern-level type
  annotation); the choice is made in a later document.
- `:=` is reserved for future assignment-like forms (e.g. Ruby
  interop bindings). It is not introduced in this layer.
- `|` is reserved for data-constructor alternation, case-pattern
  alternatives, and record-update syntax (`{ r | f = v }`, document
  04).
- `.` is reserved for qualified names (`Mod.name`), record field
  access, and the scheme-notation `forall TVAR* '.' type` from
  document 01; the overlap between the first two is resolved in the
  module and record documents.
- `\` is the lambda binder.
- `@` is reserved for as-patterns.

Because the `op` nonterminal takes the maximal run of `op_char`s and
excludes the reserved forms above, a bare `:` is always the
type-annotation token, never a user operator. Longer sequences such
as `:>` or `:|` are ordinary operators (they are neither `:` alone
nor any other reserved form).

## Layout

Sapphire is **layout-sensitive** by default, following the Haskell /
Elm tradition, with explicit braces available as an escape hatch.

The rules below are written against block-opening keywords —
including `let` — that will gain multi-binding forms in a later
document. Document 01's single-binding `let IDENT = expr in expr`
does not exercise the block-opening behavior: the keyword `in`
serves as the explicit terminator of the single-item block, so
column comparisons never have a chance to fire.

Informal rule, to be made fully precise in a later document once
`where` and `case` exist:

- A *block-opening keyword* is `let`, `where`, `of`, or `do`.
  `do` was added by document 07 (type classes / `do` notation)
  under the additive-growth clause of §Keywords above.
- If the first non-whitespace token after a block-opening keyword is
  not `{`, it opens a new block; the column of that first token is
  the block's **reference column** `c`.
- A subsequent token whose column equals `c` begins a new item of
  the block.
- A subsequent token whose column is greater than `c` continues the
  current item.
- A subsequent token whose column is less than `c` closes the block.

Explicit brace / semicolon form is always available and disables
layout within the braces:

```
let { x = 1 ; y = 2 } in x + y
```

Tab characters are **not** used as layout anchors. A horizontal tab
that appears before the first non-whitespace token of a logical line
(i.e. in a position that would otherwise set or test layout
indentation) is a lexical error. Inside a line, after the first
non-whitespace token, a tab is ordinary whitespace. This is
strict-by-default; see question 4.

## Lexical summary for document 01

For convenience, the tokens referenced by document 01 resolve as:

- `IDENT`  — `lower_ident` in term position.
- `TVAR`   — `lower_ident` in type position.
- `TCON`   — `upper_ident` in type position.
- `INT`    — `int_lit`.
- `STRING` — `string_lit`.
- `BOOL`   — the surface forms `True` and `False`; whether this is a
  distinct lexical class or a pair of prelude-bound `upper_ident`s
  is open question 1.

## Open questions

1. **`True` / `False` as a distinct lexical class or as prelude
   constructors.** §Keywords already rules out making them
   keywords. Two positions remain. (a) They are a distinct lexical
   class recognized by the lexer — matches 01's `BOOL` token and
   keeps (LitBool) as a primitive rule. (b) They are ordinary
   `upper_ident`s bound by the prelude — keeps `Bool` uniform with
   future user-defined ADTs at the price of making 01's (LitBool)
   derivable from (Var) rather than primitive. (b) additionally
   raises the question of whether shadowing of prelude constructors
   should be forbidden at the language level or handled as a lint.
   *Closed by document 09*: (b). `Bool` is the prelude ADT
   `data Bool = False | True`; `True` and `False` are ordinary
   `upper_ident` constructors. 01's (LitBool) is derivable from
   (Var) applied to these constructor schemes. Rebinding a
   constructor name to a value is forbidden by document 06
   (§Design notes, namespace discipline); shadowing a prelude
   constructor by a user `data` declaration is governed by
   document 08's import-conflict rules (two in-scope definitions
   of the same name produce an ambiguity at use sites).
2. **Unary minus.** Is `-` at the start of an expression a unary
   operator with a fixed precedence, or must negation be written
   `negate x` / `(0 - x)`? The former is friendlier but forces `-` to
   be both a unary and a binary operator with a specified
   interaction; the latter is simpler to specify.
   *Closed by document 05*: unary `-` is admitted as surface sugar
   for `negate` in expression-start position.
3. **Operator table.** Fixed precedence / associativity levels baked
   into the language (Elm-style), or user-declarable fixity
   (Haskell-style `infixl N`)? This interacts with how ad-hoc
   overloading lands in a later document.
   *Closed by document 05*: fixed Elm-style table. User-declarable
   fixity is re-posed as 05 OQ 3.
4. **Tabs in layout positions.** Treating tabs in leading
   indentation as a lexical error is strict but unambiguous; the
   alternative is to fix a tab stop. Strict-by-default is the
   current draft.
5. **Identifier character set.** Restrict to ASCII, or allow Unicode
   letters in `lower_ident` / `upper_ident`? Ruby interop may push
   toward Unicode (Ruby method names are ASCII anyway, but user code
   may not be), but Unicode identifiers complicate tooling and
   review.
6. **`::` disambiguation.** Which of list cons and pattern-level
   type annotation wins `::`? If both are wanted, the other needs a
   different spelling.
   *Partially closed by document 05*: `::` is list cons. The
   spelling of pattern-level type annotation, if introduced at all,
   remains open and is to be fixed in M3.
