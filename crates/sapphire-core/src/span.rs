//! Byte-offset spans into source text.
//!
//! `Span` is the single span representation shared by the lexer,
//! parser, AST, and downstream passes so that diagnostics pointing
//! at any phase's nodes speak the same coordinate system. It started
//! life in `sapphire-compiler::lexer` (I3) and was hoisted to
//! `sapphire-core` in I4 to let the AST (which `sapphire-lsp`
//! consumes) refer to positions without depending on the compiler
//! crate.
//!
//! Offsets are **byte** indices into the original UTF-8 source. This
//! keeps spans O(1) to compute during lexing and keeps the same
//! values meaningful if any downstream tool wants to slice
//! `&source[span.start..span.end]`.

use std::fmt;

/// Byte-offset span into the source text.
///
/// Both `start` and `end` are inclusive–exclusive byte indices into
/// the UTF-8 source buffer. `end >= start` always holds; an empty
/// span (`start == end`) is used for synthetic tokens such as `Eof`
/// and for zero-width markers inserted by later passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Construct a span from raw start / end byte offsets.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// An empty span at offset `at`. Useful for synthetic nodes
    /// inserted by later passes (e.g. virtual block markers).
    pub const fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }

    /// Smallest span that contains both `self` and `other`.
    ///
    /// Both spans must refer to the same source buffer; combining
    /// spans from different sources is a logic bug the caller is
    /// responsible for avoiding.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
