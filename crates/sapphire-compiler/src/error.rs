//! Unified compile-time error type.
//!
//! The lexer, layout pass, and parser each publish their own ADT
//! (`LexError`, `LayoutError`, `ParseError`) because each stage
//! reasons about a distinct view of the source. Downstream consumers
//! such as the CLI and the LSP server, however, want a single
//! enumeration to work with: "something went wrong before the AST
//! was available, and here is the span to point at."
//!
//! [`CompileError`] is that unifying type. It intentionally keeps the
//! original per-stage error inside — callers that want to branch on
//! the specific variant still can — and exposes a [`Span`] and a
//! stable diagnostic *code* suitable for tagging LSP diagnostics.
//!
//! The enum covers front-end passes only. Resolver / type-checker /
//! codegen errors land in their own types and will be folded in as
//! those passes join the pipeline (I5 and onwards).

use std::fmt;

use sapphire_core::span::Span;

use crate::layout::LayoutError;
use crate::lexer::LexError;
use crate::parser::ParseError;

/// A front-end compilation error paired with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub span: Span,
}

impl CompileError {
    /// Build a `CompileError` from a [`LexError`].
    pub fn from_lex(err: LexError) -> Self {
        let span = err.span;
        Self {
            kind: CompileErrorKind::Lex(err),
            span,
        }
    }

    /// Build a `CompileError` from a [`LayoutError`].
    pub fn from_layout(err: LayoutError) -> Self {
        let span = err.span;
        Self {
            kind: CompileErrorKind::Layout(err),
            span,
        }
    }

    /// Build a `CompileError` from a [`ParseError`].
    pub fn from_parse(err: ParseError) -> Self {
        let span = err.span;
        Self {
            kind: CompileErrorKind::Parse(err),
            span,
        }
    }

    /// A short, stable identifier for this error category.
    ///
    /// Callers such as the LSP server use this as the diagnostic
    /// `code` field so future quick-fixes can key off a stable
    /// string. The namespace (`sapphire/...`) is reserved for
    /// Sapphire-produced diagnostics.
    pub fn code(&self) -> &'static str {
        match &self.kind {
            CompileErrorKind::Lex(_) => "sapphire/lex-error",
            CompileErrorKind::Layout(_) => "sapphire/layout-error",
            CompileErrorKind::Parse(_) => "sapphire/parse-error",
        }
    }

    /// Human-readable error message, without the `bytes A..B` prefix
    /// the per-stage `Display` impls add. LSP diagnostics carry the
    /// span separately in their `range` field, so the prefix would be
    /// redundant.
    pub fn message(&self) -> String {
        match &self.kind {
            CompileErrorKind::Lex(e) => e.kind.to_string(),
            CompileErrorKind::Layout(e) => e.kind.to_string(),
            CompileErrorKind::Parse(e) => e.kind.to_string(),
        }
    }
}

/// The concrete per-stage error behind a [`CompileError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileErrorKind {
    Lex(LexError),
    Layout(LayoutError),
    Parse(ParseError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            CompileErrorKind::Lex(e) => write!(f, "{e}"),
            CompileErrorKind::Layout(e) => write!(f, "{e}"),
            CompileErrorKind::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CompileError {}

impl From<LexError> for CompileError {
    fn from(err: LexError) -> Self {
        Self::from_lex(err)
    }
}

impl From<LayoutError> for CompileError {
    fn from(err: LayoutError) -> Self {
        Self::from_layout(err)
    }
}

impl From<ParseError> for CompileError {
    fn from(err: ParseError) -> Self {
        Self::from_parse(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutErrorKind;
    use crate::lexer::{LexErrorKind, TokenKind};
    use crate::parser::ParseErrorKind;

    #[test]
    fn code_differs_per_variant() {
        let lex = CompileError::from_lex(LexError::new(
            LexErrorKind::UnexpectedChar('@'),
            Span::new(0, 1),
        ));
        let layout = CompileError::from_layout(LayoutError::new(
            LayoutErrorKind::UnclosedExplicitBlock,
            Span::new(0, 1),
        ));
        let parse = CompileError::from_parse(ParseError::new(
            ParseErrorKind::Unexpected(TokenKind::Eof),
            Span::new(0, 1),
        ));

        assert_eq!(lex.code(), "sapphire/lex-error");
        assert_eq!(layout.code(), "sapphire/layout-error");
        assert_eq!(parse.code(), "sapphire/parse-error");
    }

    #[test]
    fn message_does_not_repeat_span_prefix() {
        let err = CompileError::from_lex(LexError::new(
            LexErrorKind::UnexpectedChar('@'),
            Span::new(4, 5),
        ));
        // Per-stage Display adds a `lex error at bytes ...: ` prefix;
        // `message()` strips it so LSP diagnostics don't duplicate
        // span info they already carry in `range`.
        let msg = err.message();
        assert!(!msg.starts_with("lex error"));
        assert!(msg.contains("unexpected character"));
    }

    #[test]
    fn span_propagates_from_inner() {
        let s = Span::new(10, 20);
        let err = CompileError::from_parse(ParseError::new(
            ParseErrorKind::UnexpectedEof { expected: "=" },
            s,
        ));
        assert_eq!(err.span, s);
    }
}
