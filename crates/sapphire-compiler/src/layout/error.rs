//! Layout-pass errors.
//!
//! The layout pass only raises errors for structural inconsistencies
//! it can diagnose without guessing: an unclosed explicit `{` at end
//! of input, and a missing final `Eof`. Every other layout
//! inconsistency (mismatched indentation) is surfaced through the
//! downstream parser, which gets a more useful context than this
//! pass has.

use crate::lexer::Span;
use std::fmt;

/// An error produced by the layout pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutError {
    pub kind: LayoutErrorKind,
    pub span: Span,
}

impl LayoutError {
    pub const fn new(kind: LayoutErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutErrorKind {
    /// An explicit `{` was still open when the file ended.
    UnclosedExplicitBlock,
    /// The input token stream did not end with an `Eof` sentinel.
    /// This is a contract violation on the lexer's side and should
    /// not happen in practice.
    MissingEof,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "layout error at bytes {}..{}: {}",
            self.span.start, self.span.end, self.kind
        )
    }
}

impl fmt::Display for LayoutErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutErrorKind::UnclosedExplicitBlock => {
                write!(f, "unclosed explicit `{{` at end of input")
            }
            LayoutErrorKind::MissingEof => {
                write!(f, "token stream missing trailing Eof")
            }
        }
    }
}

impl std::error::Error for LayoutError {}
