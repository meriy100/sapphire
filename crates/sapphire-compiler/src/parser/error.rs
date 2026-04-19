//! Parser errors.
//!
//! The parser surfaces a dedicated ADT (analogous to the lexer's
//! `LexError`) rather than stringly-typed errors. Downstream
//! diagnostic code (L2) can pattern-match on [`ParseErrorKind`] and
//! render per-case messages; this module only fixes the taxonomy and
//! `Display` output.

use std::fmt;

use crate::lexer::{Span, TokenKind};

/// An error produced while parsing a Sapphire source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
}

impl ParseError {
    pub const fn new(kind: ParseErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// A specific token was expected; the parser saw something else.
    /// `expected` is the human-readable description of the expected
    /// form (e.g. `"`=`"`, `"identifier"`, `"expression"`).
    Expected {
        expected: &'static str,
        found: TokenKind,
    },
    /// The parser encountered a token that is not valid in any
    /// position it was trying.
    Unexpected(TokenKind),
    /// End-of-input arrived while the parser still needed more
    /// tokens.
    UnexpectedEof { expected: &'static str },
    /// Two non-associative comparison operators (tier 4) appeared
    /// without parentheses, e.g. `a < b < c`. Spec 05 §Operator
    /// table makes this a parse-time error.
    NonAssociativeChain,
    /// An operator run that is legal as a lexer token but not at the
    /// expression layer (e.g. the fallback `Op` kind) appeared
    /// where a spec-05 operator was required.
    InvalidOperator(String),
    /// A feature was recognised lexically but is explicitly out of
    /// scope for the first implementation (operator sections,
    /// pipe operators, etc.).
    UnsupportedFeature(&'static str),
    /// Layout produced something the parser can't make sense of,
    /// e.g. a virtual `;` at the very start of a block.
    MalformedLayout,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse error at bytes {}..{}: {}",
            self.span.start, self.span.end, self.kind
        )
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::Expected { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            ParseErrorKind::Unexpected(t) => write!(f, "unexpected token {t}"),
            ParseErrorKind::UnexpectedEof { expected } => {
                write!(f, "unexpected end of input, expected {expected}")
            }
            ParseErrorKind::NonAssociativeChain => write!(
                f,
                "comparison operators are non-associative; parenthesise to group"
            ),
            ParseErrorKind::InvalidOperator(op) => {
                write!(f, "operator `{op}` is not defined at the expression layer")
            }
            ParseErrorKind::UnsupportedFeature(desc) => {
                write!(f, "{desc} is not supported in the first implementation")
            }
            ParseErrorKind::MalformedLayout => {
                write!(f, "malformed layout block")
            }
        }
    }
}

impl std::error::Error for ParseError {}
