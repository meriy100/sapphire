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
        // Snap `byte` onto the nearest preceding char boundary so
        // slicing never fails silently. Lexer-produced spans always
        // land on char boundaries; this guard only matters for
        // synthetic spans or future span merges. Without it, a
        // mid-char byte would fall through `from_utf8(...).unwrap_or("")`
        // and silently report character=0 for the line.
        let mut byte = byte;
        while byte > 0 && !self.source.is_char_boundary(byte) {
            byte -= 1;
        }
        // Binary search: find the greatest line_starts[i] <= byte.
        let line_idx = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let line_start = self.line_starts[line_idx];
        // `byte` is on a char boundary and line_start is always on
        // one (it's either 0 or the byte after '\n'), so this slice
        // is guaranteed to be valid UTF-8.
        let line_prefix = &self.source[line_start..byte];
        let character = utf16_len(line_prefix);
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

    /// Convert an LSP [`Position`] back into a byte offset into the
    /// source buffer. Returns `None` if `pos.line` is past the last
    /// line (a bug-shaped input we refuse rather than silently
    /// absorbing).
    ///
    /// The LSP spec allows clients to send a `character` value that
    /// sits past the line's end; that case **clamps** to the line-
    /// end byte (just before the `\n` / `\r\n`, or the source's
    /// length for the last line). A position in the middle of a
    /// UTF-16 surrogate pair is **snapped** to the start byte of
    /// that codepoint; `byte_offset` never returns an offset that
    /// splits a codepoint.
    ///
    /// The returned offset is guaranteed to sit on a UTF-8 char
    /// boundary so slicing with it is always safe.
    pub fn byte_offset(&self, pos: Position) -> Option<usize> {
        let line_idx = pos.line as usize;
        if line_idx >= self.line_starts.len() {
            return None;
        }
        let line_start = self.line_starts[line_idx];
        // End of the line's content: the byte before the next
        // line-start (or end of source for the last line). Do not
        // include the `\n` (or the `\r\n` pair) in the walkable
        // region: `character` counts into the line, not across it.
        let line_end_excl = if line_idx + 1 < self.line_starts.len() {
            let next_start = self.line_starts[line_idx + 1];
            // next_start is 1 past the `\n`. Trim the `\n` and, if
            // the preceding byte is `\r`, trim that too.
            let mut e = next_start - 1;
            if e > line_start && self.source.as_bytes()[e - 1] == b'\r' {
                e -= 1;
            }
            e
        } else {
            self.source.len()
        };

        // Walk the line codepoint-by-codepoint accumulating UTF-16
        // units until we reach `pos.character` or the end of the
        // line. Clamp (rather than fail) when `character` points
        // past the line's content — LSP clients sometimes send
        // column == line_length for end-of-line positions.
        let target = pos.character as usize;
        let line = &self.source[line_start..line_end_excl];
        let mut utf16_used = 0usize;
        let mut byte_in_line = 0usize;
        for c in line.chars() {
            if utf16_used >= target {
                break;
            }
            let cu = c.len_utf16();
            if utf16_used + cu > target {
                // `target` fell in the middle of a surrogate pair.
                // Snap to the codepoint boundary (do not split it);
                // this matches how `position(byte_offset(pos))`
                // rounds trips back to the nearest representable
                // position.
                break;
            }
            utf16_used += cu;
            byte_in_line += c.len_utf8();
        }
        Some(line_start + byte_in_line)
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
///
/// Exposed publicly so sibling modules (incremental sync, future
/// tooling) can share the same counting rule as [`LineMap`]. See
/// `docs/impl/21-lsp-incremental-sync.md` for where this crosses the
/// module boundary.
pub fn utf16_len(s: &str) -> usize {
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

    #[test]
    fn byte_offset_ascii_round_trip() {
        // Byte → Position → Byte should be the identity on ASCII.
        let src = "hello\nworld";
        let map = build_line_map(src);
        for byte in 0..=src.len() {
            let pos = map.position(byte);
            let back = map.byte_offset(pos).expect("in-range");
            assert_eq!(back, byte, "round trip broke at {byte}");
        }
    }

    #[test]
    fn byte_offset_bmp_multibyte_round_trip() {
        // `é` = 2 UTF-8 bytes, 1 UTF-16 unit. Every char boundary
        // up to `src.len()` (inclusive) must round-trip.
        let src = "é=1\nxy";
        let map = build_line_map(src);
        for &byte in &[0usize, 2, 3, 4, 5, 6, 7] {
            let pos = map.position(byte);
            assert_eq!(map.byte_offset(pos), Some(byte), "byte={byte}");
        }
    }

    #[test]
    fn byte_offset_supplementary_round_trip() {
        // U+1F600 is 4 UTF-8 bytes, 2 UTF-16 units. Every whole
        // codepoint boundary must round-trip.
        let src = "\u{1F600}x\n\u{1F600}y";
        let map = build_line_map(src);
        let boundaries = [0usize, 4, 5, 6, 10, 11];
        for &byte in &boundaries {
            let pos = map.position(byte);
            assert_eq!(map.byte_offset(pos), Some(byte), "byte={byte}");
        }
    }

    #[test]
    fn byte_offset_inside_surrogate_snaps_down() {
        // Asking for the "middle" of a surrogate pair returns the
        // codepoint's start offset (we do not split codepoints).
        let src = "\u{1F600}x";
        let map = build_line_map(src);
        // Character = 1 lands inside the U+1F600 surrogate pair.
        assert_eq!(map.byte_offset(Position::new(0, 1)), Some(0));
    }

    #[test]
    fn byte_offset_past_line_end_clamps_to_eol() {
        let src = "abc\ndef";
        let map = build_line_map(src);
        // line 0 has 3 content chars; character=999 should land at
        // the byte just before the `\n` (byte 3).
        assert_eq!(map.byte_offset(Position::new(0, 999)), Some(3));
        // Last line: clamp to source end.
        assert_eq!(map.byte_offset(Position::new(1, 999)), Some(7));
    }

    #[test]
    fn byte_offset_line_past_eof_is_none() {
        let src = "abc\n";
        let map = build_line_map(src);
        // Two lines: [0, 4] → indices 0 and 1 valid; 2 is OOB.
        assert!(map.byte_offset(Position::new(2, 0)).is_none());
    }

    #[test]
    fn byte_offset_handles_crlf_line_end() {
        // `\r\n` counts as one line break. character past the line
        // end must clamp just before the `\r`.
        let src = "ab\r\ncd";
        let map = build_line_map(src);
        // Line 0 has "ab" (2 UTF-16 units). `\r` is not a character.
        assert_eq!(map.byte_offset(Position::new(0, 2)), Some(2));
        assert_eq!(map.byte_offset(Position::new(0, 999)), Some(2));
        // Line 1 starts after `\r\n` → byte 4.
        assert_eq!(map.byte_offset(Position::new(1, 0)), Some(4));
        assert_eq!(map.byte_offset(Position::new(1, 2)), Some(6));
    }

    #[test]
    fn byte_offset_empty_source() {
        let map = build_line_map("");
        assert_eq!(map.byte_offset(Position::new(0, 0)), Some(0));
        assert_eq!(map.byte_offset(Position::new(0, 5)), Some(0));
        assert!(map.byte_offset(Position::new(1, 0)).is_none());
    }

    #[test]
    fn utf16_len_counts_bmp_and_supplementary() {
        assert_eq!(utf16_len(""), 0);
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(utf16_len("é"), 1);
        assert_eq!(utf16_len("\u{1F600}"), 2);
        assert_eq!(utf16_len("a\u{1F600}b"), 4);
    }
}
