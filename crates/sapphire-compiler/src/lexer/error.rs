//! Lexer errors.
//!
//! See `docs/impl/09-lexer.md` §Design judgments for why the lexer
//! surfaces a dedicated ADT instead of going through `anyhow`.

use std::fmt;

use super::token::Span;

/// An error produced while lexing Sapphire source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

impl LexError {
    pub const fn new(kind: LexErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Categorisation of lexical errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    /// Bare `\r` appeared in the source. Spec 02 §Source text
    /// requires `\r\n` to be normalised; a lone `\r` is rejected.
    BareCarriageReturn,
    /// A horizontal tab appeared at a layout-anchoring position
    /// (before the first non-whitespace token of a logical line).
    /// Spec 02 §Layout / OQ4 DECIDED.
    TabInLayoutPosition,
    /// A non-ASCII character was found where only ASCII is admitted
    /// (identifier start in particular). Spec 02 §Identifiers /
    /// OQ5 DECIDED.
    NonAsciiIdentStart,
    /// A character did not match any lexical production.
    UnexpectedChar(char),
    /// A block comment `{-` was not closed by a matching `-}`.
    UnterminatedBlockComment,
    /// A string literal was not closed by a matching `"` before
    /// end-of-input or end-of-line.
    UnterminatedString,
    /// A physical newline appeared inside a single-line string
    /// literal. Multi-line strings are deferred.
    NewlineInString,
    /// An escape sequence `\X` had an unknown `X`.
    UnknownEscape(char),
    /// A `\u{...}` escape was malformed (missing braces, empty,
    /// too long, or non-hex).
    MalformedUnicodeEscape,
    /// A `\u{...}` escape named a value that is not a valid
    /// Unicode scalar (out of range or a surrogate code point).
    InvalidUnicodeScalar(u32),
    /// An integer literal overflowed `i64`.
    IntegerOverflow,
    /// An integer literal was empty (only underscores after the
    /// first digit was consumed, or `_` used as the first
    /// character — caught by the lowerIdent path, kept for safety).
    MalformedIntLiteral,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lex error at bytes {}..{}: {}",
            self.span.start, self.span.end, self.kind
        )
    }
}

impl fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexErrorKind::BareCarriageReturn => {
                write!(f, "bare carriage return (expected CRLF or LF)")
            }
            LexErrorKind::TabInLayoutPosition => {
                write!(f, "horizontal tab in layout-anchoring position")
            }
            LexErrorKind::NonAsciiIdentStart => {
                write!(f, "non-ASCII character cannot start an identifier")
            }
            LexErrorKind::UnexpectedChar(c) => {
                write!(f, "unexpected character {c:?}")
            }
            LexErrorKind::UnterminatedBlockComment => {
                write!(f, "unterminated block comment")
            }
            LexErrorKind::UnterminatedString => {
                write!(f, "unterminated string literal")
            }
            LexErrorKind::NewlineInString => {
                write!(f, "newline inside string literal")
            }
            LexErrorKind::UnknownEscape(c) => {
                write!(f, "unknown escape sequence \\{c}")
            }
            LexErrorKind::MalformedUnicodeEscape => {
                write!(f, "malformed \\u{{...}} escape")
            }
            LexErrorKind::InvalidUnicodeScalar(n) => {
                write!(f, "invalid Unicode scalar U+{n:X}")
            }
            LexErrorKind::IntegerOverflow => {
                write!(f, "integer literal overflows i64")
            }
            LexErrorKind::MalformedIntLiteral => {
                write!(f, "malformed integer literal")
            }
        }
    }
}

impl std::error::Error for LexError {}
