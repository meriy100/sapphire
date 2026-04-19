//! Layout resolution — "off-side rule" pass.
//!
//! The lexer (`crate::lexer`) emits physical tokens plus two layout
//! markers: `Newline` at the end of every logical line and
//! `Indent(col)` in front of the first non-whitespace token of a
//! logical line. This pass consumes those markers and rewrites the
//! token stream into a shape the parser can consume directly:
//! virtual `{`, `;`, `}` tokens delimit blocks that the programmer
//! wrote with indentation rather than explicit braces. The input
//! and output token types are the same [`Token`] struct the lexer
//! already produces; virtual markers are materialised as
//! [`TokenKind::LBrace`] / [`TokenKind::Semicolon`] / [`TokenKind::RBrace`]
//! with empty spans anchored at the lexically-nearest real token.
//!
//! ## Why a dedicated pass
//!
//! Keeping layout separate from both the lexer and the parser:
//!
//! - **Keeps the lexer small.** The lexer's job is byte-level
//!   tokenisation (spec 02); off-side rule semantics belong to
//!   a layer that reasons about logical structure.
//! - **Keeps the parser uniform.** Once layout has resolved, the
//!   parser sees an unambiguous stream of `{`, `;`, `}` and no
//!   `Newline` / `Indent` trivia. Explicit-brace blocks and
//!   layout-driven blocks look identical at the parser level.
//! - **Simplifies diagnostics.** A "malformed indentation" error
//!   can be reported from this pass without having to disentangle
//!   a deep parser recovery path.
//!
//! See `docs/impl/13-parser.md` §Layout for the decision record.
//!
//! ## Algorithm (Haskell-98-ish)
//!
//! Maintain a stack of layout contexts. Each context is either
//! `Explicit` (inside a `{ ... }` the programmer wrote directly —
//! layout is inert) or `Implicit(col)` (opened by one of the
//! block-opening keywords `where`, `let`, `of`, `do` when the next
//! token is *not* an explicit `{`, recording the column `col` of
//! that next token).
//!
//! On each real token `t` at column `c` that is the first token
//! of a logical line:
//!
//! - While the top context is `Implicit(col)` and `c < col`: pop
//!   and emit a virtual `}`.
//! - If the top is `Implicit(col)` and `c == col`: emit a virtual
//!   `;` before `t`.
//! - Otherwise emit nothing before `t`.
//!
//! Tokens mid-line (no preceding `Newline` that wasn't swallowed by
//! the above rules) are emitted verbatim.
//!
//! Special cases:
//!
//! - If `t` is one of `where` / `let` / `of` / `do`, the *next*
//!   non-trivia token either is `{` (push `Explicit`) or isn't
//!   (emit a virtual `{` and push `Implicit(col-of-next-token)`).
//! - `in` pops the most recent `Implicit` context if that context
//!   was opened by `let`. The block close is implicit even when
//!   indentation has not yet dropped.
//! - On EOF, pop every remaining `Implicit` context, emitting
//!   virtual `}` for each. `Explicit` contexts left open on EOF
//!   are a parse error (emitted as-is; the parser reports the
//!   mismatched `{`).
//!
//! The algorithm is intentionally conservative — it does not try
//! to recover from inconsistent indentation; a file whose layout
//! is malformed surfaces as a `LayoutError` with a span into the
//! offending token.

mod error;

#[cfg(test)]
mod tests;

use crate::lexer::{Span, Token, TokenKind};

pub use error::{LayoutError, LayoutErrorKind};

/// Drive the layout resolver over a full token stream and return the
/// transformed vector. The input is expected to come from
/// [`crate::lexer::tokenize`] verbatim (including its final `Eof`).
///
/// `source` is the original source string the tokens came from. It
/// is used to compute the column of the first token inside a
/// same-line block opener (`of x -> e` etc.) so that the block's
/// reference column lines up with the first token instead of being
/// pinned at `usize::MAX`.
pub fn resolve(tokens: Vec<Token>) -> Result<Vec<Token>, LayoutError> {
    resolve_with_source(tokens, "")
}

/// Same as [`resolve`], but accepts the source buffer so that
/// same-line implicit blocks can derive an accurate reference
/// column for the off-side rule.
pub fn resolve_with_source(tokens: Vec<Token>, source: &str) -> Result<Vec<Token>, LayoutError> {
    let mut lay = Layout::new(tokens, source.as_bytes().to_vec());
    lay.run()
}

/// One entry on the layout context stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    /// `{` was written explicitly; layout is inert inside.
    Explicit,
    /// An implicit block opened at the given reference column,
    /// together with the keyword that opened it (so that `in` can
    /// pop a matching `let`). `fresh` is `true` until the block
    /// sees its first statement; subsequent same-column tokens
    /// emit a virtual `;` before the token, but the *first* such
    /// token is the block's initial statement and takes no
    /// separator.
    Implicit {
        col: usize,
        opener: Opener,
        fresh: bool,
    },
}

/// Which block-opening keyword created an `Implicit` context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opener {
    Where,
    Let,
    Of,
    Do,
    /// Synthetic context opened for the whole file so top-level
    /// declarations share a layout block (column 0).
    TopLevel,
}

struct Layout {
    input: Vec<Token>,
    pos: usize,
    out: Vec<Token>,
    stack: Vec<Context>,
    /// Source bytes. Used to compute the column of a same-line
    /// block-opener follower (see [`column_of`]). Empty when the
    /// caller did not supply the source.
    source: Vec<u8>,
    /// Column of the first token on the current logical line, if the
    /// next real token is still line-start. `None` otherwise.
    ///
    /// The lexer emits `Indent(col)` directly before the first
    /// real token of each logical line; when we consume that
    /// `Indent` we stash its column here and it is consulted by the
    /// next real-token branch.
    pending_indent: Option<usize>,
    /// When set, the *next* real token is expected to open an
    /// implicit block with the given `Opener`. This is set when we
    /// see one of the four block-opening keywords and not yet seen
    /// the following token.
    pending_open: Option<Opener>,
}

impl Layout {
    fn new(input: Vec<Token>, source: Vec<u8>) -> Self {
        Self {
            input,
            pos: 0,
            out: Vec::new(),
            source,
            // The whole file lives inside an implicit column-0
            // block so that top-level declarations are separated by
            // virtual `;`s in the same way that `where`-body
            // declarations are.
            stack: vec![Context::Implicit {
                col: 0,
                opener: Opener::TopLevel,
                fresh: true,
            }],
            pending_indent: None,
            pending_open: None,
        }
    }

    /// Code-point column of the byte offset `at` within the source
    /// buffer. Counts columns from the last `\n` (or start of file),
    /// using Unicode code-point count (same convention as the
    /// lexer's `Indent(col)`). Returns `None` when the source
    /// buffer is empty (e.g. when the caller used `resolve` without
    /// supplying a source).
    fn column_of(&self, at: usize) -> Option<usize> {
        if self.source.is_empty() {
            return None;
        }
        let end = at.min(self.source.len());
        let line_start = self.source[..end]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        let slice = std::str::from_utf8(&self.source[line_start..end]).ok()?;
        Some(slice.chars().count())
    }

    fn run(&mut self) -> Result<Vec<Token>, LayoutError> {
        // Open the synthetic top-level block with a virtual `{`.
        self.out.push(virtual_tok(TokenKind::LBrace, 0));

        while self.pos < self.input.len() {
            let tok = self.input[self.pos].clone();
            match &tok.kind {
                TokenKind::Newline => {
                    // Skip; indentation for the next line will be
                    // supplied by the following `Indent`. Dropping
                    // `Newline` is safe because every logical-line
                    // boundary is also expressed through `Indent`.
                    self.pos += 1;
                }
                TokenKind::Indent(col) => {
                    self.pending_indent = Some(*col);
                    self.pos += 1;
                }
                TokenKind::Eof => {
                    // If a block-opening keyword was pending, emit
                    // an empty implicit block so the opener is
                    // always paired with a `{...}`.
                    if self.pending_open.take().is_some() {
                        self.out
                            .push(virtual_tok(TokenKind::LBrace, tok.span.start));
                        self.out
                            .push(virtual_tok(TokenKind::RBrace, tok.span.start));
                    }
                    // Close every remaining implicit block.
                    while let Some(ctx) = self.stack.last().copied() {
                        match ctx {
                            Context::Implicit { .. } => {
                                self.out
                                    .push(virtual_tok(TokenKind::RBrace, tok.span.start));
                                self.stack.pop();
                            }
                            Context::Explicit => {
                                return Err(LayoutError::new(
                                    LayoutErrorKind::UnclosedExplicitBlock,
                                    tok.span,
                                ));
                            }
                        }
                    }
                    self.out.push(tok);
                    self.pos += 1;
                    return Ok(std::mem::take(&mut self.out));
                }
                _ => {
                    self.handle_real_token(tok)?;
                    self.pos += 1;
                }
            }
        }
        // Should not be reached — the lexer appends `Eof`.
        Err(LayoutError::new(
            LayoutErrorKind::MissingEof,
            Span::empty(0),
        ))
    }

    fn handle_real_token(&mut self, tok: Token) -> Result<(), LayoutError> {
        // Snapshot the line-start indent so both the block-open and
        // the indentation-close paths can consult it.
        let line_start_col = self.pending_indent.take();

        // 1. A pending open from a previous block-opening keyword
        //    decides what to do with `tok`. The outer context does
        //    *not* apply its off-side rule here — the `where` / `let`
        //    / `of` / `do` keyword that set `pending_open` is part of
        //    the outer block's current statement, and the new block
        //    opens "inside" that statement. Applying the outer
        //    off-side rule would insert a stray `;` between the
        //    opener keyword and its body.
        if let Some(opener) = self.pending_open.take() {
            match &tok.kind {
                TokenKind::LBrace => {
                    // Explicit brace: push Explicit context and emit
                    // the real `{`.
                    self.stack.push(Context::Explicit);
                    self.out.push(tok);
                    return Ok(());
                }
                _ => {
                    // Implicit block. Reference column is the
                    // `tok`'s line-start column if it starts a fresh
                    // logical line; otherwise compute it from the
                    // source buffer using `column_of` so that a
                    // same-line opener (`let a = 1` continuing as
                    // `b = 2` on the next line at the same column
                    // as `a`) still gets the right reference.
                    self.out
                        .push(virtual_tok(TokenKind::LBrace, tok.span.start));
                    let col = line_start_col
                        .or_else(|| self.column_of(tok.span.start))
                        .unwrap_or(usize::MAX);
                    self.stack.push(Context::Implicit {
                        col,
                        opener,
                        fresh: true,
                    });
                    // The new block is now "fresh", and this is its
                    // first token — pre-mark non-fresh so a
                    // *later* token at the same column emits `;`.
                    if let Some(Context::Implicit { fresh, .. }) = self.stack.last_mut() {
                        *fresh = false;
                    }
                    self.out.push(tok);
                    return Ok(());
                }
            }
        }

        // 2. Indentation-driven close / separator for line-start tokens
        //    outside any pending-open handling.
        if let Some(col) = line_start_col {
            self.apply_line_start(col, tok.span.start)?;
        }

        // 3. Special-case `in`: pops the innermost `Implicit(Let)`.
        //    The layout pass might have already dedented out of the
        //    let-block via `apply_line_start`; handle both cases by
        //    walking the stack up to the nearest `Implicit(Let)`
        //    and popping it.
        if matches!(tok.kind, TokenKind::In) {
            if let Some(idx) = self.stack.iter().rposition(|c| {
                matches!(
                    c,
                    Context::Implicit {
                        opener: Opener::Let,
                        ..
                    }
                )
            }) {
                // Close every implicit block above the let, then
                // the let-block itself.
                while self.stack.len() > idx + 1 {
                    self.out
                        .push(virtual_tok(TokenKind::RBrace, tok.span.start));
                    self.stack.pop();
                }
                self.out
                    .push(virtual_tok(TokenKind::RBrace, tok.span.start));
                self.stack.pop();
            }
            self.out.push(tok);
            return Ok(());
        }

        // 4. Track explicit `{` and `}` that are not block openers.
        match &tok.kind {
            TokenKind::LBrace => {
                self.stack.push(Context::Explicit);
                self.out.push(tok);
                return Ok(());
            }
            TokenKind::RBrace => {
                // Close every implicit context above the nearest
                // explicit one, then pop the explicit context.
                while let Some(ctx) = self.stack.last().copied() {
                    match ctx {
                        Context::Implicit { .. } => {
                            self.out
                                .push(virtual_tok(TokenKind::RBrace, tok.span.start));
                            self.stack.pop();
                        }
                        Context::Explicit => {
                            self.stack.pop();
                            break;
                        }
                    }
                }
                self.out.push(tok);
                return Ok(());
            }
            _ => {}
        }

        // 5. Block-opening keywords: set `pending_open` so the
        //    *next* real token supplies the block's reference column.
        match tok.kind {
            TokenKind::Where => self.pending_open = Some(Opener::Where),
            TokenKind::Let => self.pending_open = Some(Opener::Let),
            TokenKind::Of => self.pending_open = Some(Opener::Of),
            TokenKind::Do => self.pending_open = Some(Opener::Do),
            _ => {}
        }

        self.out.push(tok);
        Ok(())
    }

    /// Given the column of a line-start token, close / separate
    /// implicit blocks as needed.
    fn apply_line_start(&mut self, col: usize, anchor: usize) -> Result<(), LayoutError> {
        while let Some(Context::Implicit {
            col: ref_col,
            fresh,
            ..
        }) = self.stack.last().copied()
        {
            if col < ref_col {
                self.out.push(virtual_tok(TokenKind::RBrace, anchor));
                self.stack.pop();
                continue;
            }
            if col == ref_col {
                if fresh {
                    // First token of this block — no `;`, just mark
                    // the block as no longer fresh.
                    if let Some(Context::Implicit { fresh, .. }) = self.stack.last_mut() {
                        *fresh = false;
                    }
                } else {
                    self.out.push(virtual_tok(TokenKind::Semicolon, anchor));
                }
            }
            break;
        }
        Ok(())
    }
}

fn virtual_tok(kind: TokenKind, anchor: usize) -> Token {
    Token::new(kind, Span::empty(anchor))
}
