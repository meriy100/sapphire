//! Translating [`sapphire_compiler::error::CompileError`]s into LSP
//! [`Diagnostic`] values.
//!
//! Two concerns live here:
//!
//! 1. **Coordinate conversion.** `CompileError::span` is a byte-offset
//!    pair into the source buffer; the LSP client speaks line /
//!    character positions measured in **UTF-16 code units**
//!    (LSP 3.17 §Position — Sapphire does not negotiate
//!    `positionEncoding`, so we use the default). [`LineMap`]
//!    precomputes the byte offset of each line so we can go from
//!    byte offset to `Position` in `O(log n + line_length)`.
//! 2. **Diagnostic shaping.** We tag every diagnostic with a stable
//!    `source = "sapphire"` and an error-kind-specific `code`
//!    (`sapphire/lex-error`, `sapphire/layout-error`, or
//!    `sapphire/parse-error`) so future quick-fixes can pattern-match
//!    without parsing the message string. See
//!    `docs/impl/17-lsp-diagnostics.md` for the design notes.
//!
//! ## Why UTF-16
//!
//! VSCode's editor buffer is internally UTF-16, and LSP keeps the
//! default encoding there for backward compatibility. If Sapphire
//! later negotiates UTF-8 (`positionEncoding = "utf-8"`) the
//! conversion degenerates into a byte-offset arithmetic; extending
//! `LineMap` to emit UTF-8 or UTF-32 `character` values is a local
//! change. This is tracked as I-OQ53.

use sapphire_compiler::error::CompileError;
use sapphire_core::span::Span;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

/// Precomputed line-start byte offsets for a single source buffer.
///
/// `line_starts[i]` is the byte offset of the first character of
/// line `i` (0-indexed). The vector always has at least one entry
/// (the line-0 start at offset `0`), including for empty sources.
/// `position()` clamps byte offsets past `source.len()` so span
/// `end` values beyond the buffer still render sensibly.
#[derive(Debug, Clone)]
pub struct LineMap<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> LineMap<'a> {
    /// Scan `source` once and record the starting byte offset of
    /// every line. Handles `\n` and `\r\n` line terminators; a lone
    /// `\r` is *not* treated as a line terminator because the lexer
    /// rejects it (`LexErrorKind::BareCarriageReturn`) and we want
    /// `LineMap` to stay in sync with that coordinate system.
    pub fn new(source: &'a str) -> Self {
        let mut line_starts = Vec::with_capacity(source.len() / 40 + 1);
        line_starts.push(0);
        let bytes = source.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' {
                line_starts.push(i + 1);
            }
            i += 1;
        }
        Self {
            source,
            line_starts,
        }
    }

    /// Look up the LSP [`Position`] for `byte`, clamping to the end
    /// of the buffer if `byte` is past it. The `character` field is
    /// measured in UTF-16 code units per LSP defaults.
    pub fn position(&self, byte: usize) -> Position {
        let byte = byte.min(self.source.len());
        // Binary search: find the greatest line_starts[i] <= byte.
        let line_idx = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let line_start = self.line_starts[line_idx];
        // Count UTF-16 code units from line_start up to byte.
        let line_prefix = &self.source.as_bytes()[line_start..byte];
        let character = utf16_len(std::str::from_utf8(line_prefix).unwrap_or(""));
        Position {
            line: line_idx as u32,
            character: character as u32,
        }
    }

    /// Convert a `Span` into an LSP [`Range`]. Empty spans produce a
    /// zero-width range at the same position, which LSP clients
    /// render as a caret with no highlight.
    pub fn range(&self, span: Span) -> Range {
        Range {
            start: self.position(span.start),
            end: self.position(span.end),
        }
    }

    /// Raw line-start offsets, exposed for tests.
    #[cfg(test)]
    pub(crate) fn line_starts(&self) -> &[usize] {
        &self.line_starts
    }
}

/// Build a [`LineMap`] for `source`. A thin alias over
/// [`LineMap::new`] kept to match the naming used in the L2
/// design note (`docs/impl/17-lsp-diagnostics.md`).
pub fn build_line_map(source: &str) -> LineMap<'_> {
    LineMap::new(source)
}

/// Count UTF-16 code units in `s`. Mirrors `str::encode_utf16().count()`
/// but avoids allocating an iterator state struct per call.
fn utf16_len(s: &str) -> usize {
    let mut n = 0;
    for c in s.chars() {
        // Non-BMP code points take two UTF-16 code units.
        n += c.len_utf16();
    }
    n
}

/// Translate a [`CompileError`] into an LSP [`Diagnostic`].
///
/// The resulting diagnostic carries the span in LSP coordinates,
/// `Error` severity, `source = "sapphire"`, and an error-kind code
/// derived from [`CompileError::code`]. The message is the
/// per-stage kind description (no redundant `bytes A..B` prefix).
pub fn compile_error_to_diagnostic(err: &CompileError, map: &LineMap<'_>) -> Diagnostic {
    Diagnostic {
        range: map.range(err.span),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(err.code().to_owned())),
        code_description: None,
        source: Some("sapphire".to_owned()),
        message: err.message(),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapphire_compiler::error::{CompileError, CompileErrorKind};
    use sapphire_compiler::layout::{LayoutError, LayoutErrorKind};
    use sapphire_compiler::lexer::{LexError, LexErrorKind, TokenKind};
    use sapphire_compiler::parser::{ParseError, ParseErrorKind};

    #[test]
    fn line_map_records_every_line_start() {
        let src = "a\nbb\nccc\n";
        let map = build_line_map(src);
        // Four entries: the three `\n` plus the implicit line-0 start.
        assert_eq!(map.line_starts(), &[0, 2, 5, 9]);
    }

    #[test]
    fn line_map_handles_empty_source() {
        let map = build_line_map("");
        assert_eq!(map.line_starts(), &[0]);
        assert_eq!(map.position(0), Position::new(0, 0));
    }

    #[test]
    fn line_map_handles_no_trailing_newline() {
        let map = build_line_map("abc");
        assert_eq!(map.line_starts(), &[0]);
        assert_eq!(map.position(3), Position::new(0, 3));
    }

    #[test]
    fn line_map_handles_crlf() {
        // A `\r\n` line terminator must yield only the \n entry; the
        // `\r` does not get its own line break.
        let src = "a\r\nb";
        let map = build_line_map(src);
        assert_eq!(map.line_starts(), &[0, 3]);
        assert_eq!(map.position(3), Position::new(1, 0));
    }

    #[test]
    fn position_counts_ascii() {
        let map = build_line_map("hello\nworld");
        assert_eq!(map.position(0), Position::new(0, 0));
        assert_eq!(map.position(5), Position::new(0, 5));
        assert_eq!(map.position(6), Position::new(1, 0));
        assert_eq!(map.position(11), Position::new(1, 5));
    }

    #[test]
    fn position_utf16_counts_bmp_multibyte() {
        // `é` is 2 UTF-8 bytes but 1 UTF-16 code unit.
        let src = "é=1";
        let map = build_line_map(src);
        assert_eq!(map.position(0), Position::new(0, 0));
        // After the `é` (byte 2) the character column is 1.
        assert_eq!(map.position(2), Position::new(0, 1));
        assert_eq!(map.position(3), Position::new(0, 2));
    }

    #[test]
    fn position_utf16_counts_supplementary_as_two_units() {
        // U+1F600 "😀" is 4 UTF-8 bytes and 2 UTF-16 code units
        // (a surrogate pair).
        let src = "\u{1F600}x";
        let map = build_line_map(src);
        assert_eq!(map.position(0), Position::new(0, 0));
        assert_eq!(map.position(4), Position::new(0, 2));
        assert_eq!(map.position(5), Position::new(0, 3));
    }

    #[test]
    fn position_clamps_past_end() {
        let map = build_line_map("abc");
        // Out-of-bounds bytes clamp to the source length so callers
        // don't need to validate offsets before publishing
        // diagnostics.
        assert_eq!(map.position(999), Position::new(0, 3));
    }

    #[test]
    fn range_spans_multi_line() {
        let src = "ab\ncd\nef";
        let map = build_line_map(src);
        let r = map.range(Span::new(1, 7));
        assert_eq!(r.start, Position::new(0, 1));
        assert_eq!(r.end, Position::new(2, 1));
    }

    #[test]
    fn diagnostic_for_lex_error_has_lex_code() {
        let src = "@";
        let map = build_line_map(src);
        let err = CompileError::from_lex(LexError::new(
            LexErrorKind::UnexpectedChar('@'),
            Span::new(0, 1),
        ));
        let diag = compile_error_to_diagnostic(&err, &map);
        assert_eq!(
            diag.code,
            Some(NumberOrString::String("sapphire/lex-error".to_owned()))
        );
        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diag.source.as_deref(), Some("sapphire"));
        assert_eq!(diag.range.start, Position::new(0, 0));
        assert_eq!(diag.range.end, Position::new(0, 1));
    }

    #[test]
    fn diagnostic_for_layout_error_has_layout_code() {
        let src = "{ x = 1";
        let map = build_line_map(src);
        let err = CompileError::from_layout(LayoutError::new(
            LayoutErrorKind::UnclosedExplicitBlock,
            Span::new(0, 1),
        ));
        let diag = compile_error_to_diagnostic(&err, &map);
        assert_eq!(
            diag.code,
            Some(NumberOrString::String("sapphire/layout-error".to_owned()))
        );
        assert!(diag.message.contains("unclosed"));
    }

    #[test]
    fn diagnostic_for_parse_error_has_parse_code() {
        let src = "data T";
        let map = build_line_map(src);
        let err = CompileError::from_parse(ParseError::new(
            ParseErrorKind::UnexpectedEof { expected: "=" },
            Span::new(6, 6),
        ));
        let diag = compile_error_to_diagnostic(&err, &map);
        assert_eq!(
            diag.code,
            Some(NumberOrString::String("sapphire/parse-error".to_owned()))
        );
        assert!(diag.message.contains("unexpected"));
    }

    #[test]
    fn diagnostic_message_has_no_byte_prefix() {
        let src = "hello";
        let map = build_line_map(src);
        let err = CompileError::from_parse(ParseError::new(
            ParseErrorKind::Unexpected(TokenKind::Eof),
            Span::new(0, 5),
        ));
        let diag = compile_error_to_diagnostic(&err, &map);
        assert!(!diag.message.starts_with("parse error"));
    }

    #[test]
    fn diagnostic_kind_via_enum_variant() {
        // Round-trip: build a CompileError through each variant and
        // make sure the enum match behaves as expected. Guards
        // against someone accidentally collapsing variants when
        // refactoring `CompileErrorKind`.
        let lex = CompileError::from_lex(LexError::new(
            LexErrorKind::UnexpectedChar('@'),
            Span::new(0, 1),
        ));
        assert!(matches!(lex.kind, CompileErrorKind::Lex(_)));
        let layout = CompileError::from_layout(LayoutError::new(
            LayoutErrorKind::MissingEof,
            Span::new(0, 0),
        ));
        assert!(matches!(layout.kind, CompileErrorKind::Layout(_)));
        let parse = CompileError::from_parse(ParseError::new(
            ParseErrorKind::MalformedLayout,
            Span::new(0, 0),
        ));
        assert!(matches!(parse.kind, CompileErrorKind::Parse(_)));
    }

    #[test]
    fn range_for_empty_span_is_zero_width() {
        let src = "abc";
        let map = build_line_map(src);
        let r = map.range(Span::empty(2));
        assert_eq!(r.start, Position::new(0, 2));
        assert_eq!(r.end, Position::new(0, 2));
    }
}
