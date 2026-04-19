//! Apply a single LSP [`TextDocumentContentChangeEvent`] to an
//! in-memory text buffer.
//!
//! L3 switches the server to `TextDocumentSyncKind::INCREMENTAL`,
//! which means the client may send range-scoped edits instead of
//! whole-document replacements. This module translates each change
//! into a mutation of the owning `String` buffer using a fresh
//! [`LineMap`] for the UTF-16 → byte coordinate conversion. A second
//! `LineMap` is built after each edit by the caller so the next
//! change in the same batch sees a consistent coordinate system.
//!
//! The decisions traded off here:
//!
//! - **`range_length` is ignored.** LSP deprecates it in favour of
//!   `range`, and implementations disagree on whether it counts
//!   UTF-16 units or bytes. The range alone is authoritative; a
//!   `range_length` mismatch does not invalidate the edit.
//! - **Errors are reported but not fatal.** An out-of-range edit
//!   returns `ApplyError` so the caller can log and decide what to
//!   do. The current `did_change` handler stops on the first error
//!   (to avoid compounding damage on a drifting buffer); future
//!   work may try to recover. See `docs/impl/21-lsp-incremental-
//!   sync.md` §Error handling for the rationale.

use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

use crate::diagnostics::LineMap;

/// A failure applying a single [`TextDocumentContentChangeEvent`].
///
/// None of these are structurally impossible — they can all be
/// reached by a misbehaving client — so they surface to the caller
/// as recoverable errors rather than panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// `range.start` resolved to a position past the end of the
    /// document (e.g. `line` larger than `line_count`).
    StartOutOfRange,
    /// `range.end` resolved to a position past the end of the
    /// document.
    EndOutOfRange,
    /// `range.start` was after `range.end` in the source buffer.
    /// LSP requires start ≤ end; this usually signals a client bug.
    InvertedRange,
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartOutOfRange => f.write_str("range.start past end of document"),
            Self::EndOutOfRange => f.write_str("range.end past end of document"),
            Self::InvertedRange => f.write_str("range.start > range.end"),
        }
    }
}

impl std::error::Error for ApplyError {}

/// Apply one [`TextDocumentContentChangeEvent`] to `buf` in place.
///
/// - If `change.range` is `None` the change is treated as a full-
///   document replacement (LSP's fallback shape, used by the client
///   when the server advertises `Full` sync but also permitted
///   inside an `Incremental` stream).
/// - If `change.range` is `Some`, the substring identified by the
///   range (UTF-16 positions resolved through a freshly built
///   [`LineMap`]) is spliced out and `change.text` is inserted in
///   its place.
///
/// The function is deliberately pure: the only mutation is the
/// target buffer. The caller is responsible for rebuilding any
/// derived state (e.g. a `LineMap`) between calls, since several
/// changes in a single `didChange` batch must be applied as if the
/// previous one had already committed.
pub fn apply_change(
    buf: &mut String,
    change: &TextDocumentContentChangeEvent,
) -> Result<(), ApplyError> {
    let Some(range) = change.range else {
        buf.clear();
        buf.push_str(&change.text);
        return Ok(());
    };

    let map = LineMap::new(buf);
    let start = map
        .byte_offset(range.start)
        .ok_or(ApplyError::StartOutOfRange)?;
    let end = map
        .byte_offset(range.end)
        .ok_or(ApplyError::EndOutOfRange)?;
    if start > end {
        return Err(ApplyError::InvertedRange);
    }
    buf.replace_range(start..end, &change.text);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Position, Range};

    fn change(range: Option<Range>, text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range,
            range_length: None,
            text: text.to_owned(),
        }
    }

    fn rng(sl: u32, sc: u32, el: u32, ec: u32) -> Range {
        Range {
            start: Position::new(sl, sc),
            end: Position::new(el, ec),
        }
    }

    #[test]
    fn full_replacement_when_range_is_none() {
        let mut buf = String::from("old body");
        apply_change(&mut buf, &change(None, "new body")).unwrap();
        assert_eq!(buf, "new body");
    }

    #[test]
    fn insert_at_head_of_line() {
        let mut buf = String::from("bc");
        // Zero-width range at (0,0) → insertion.
        apply_change(&mut buf, &change(Some(rng(0, 0, 0, 0)), "a")).unwrap();
        assert_eq!(buf, "abc");
    }

    #[test]
    fn insert_in_middle_of_line() {
        let mut buf = String::from("ac");
        apply_change(&mut buf, &change(Some(rng(0, 1, 0, 1)), "b")).unwrap();
        assert_eq!(buf, "abc");
    }

    #[test]
    fn insert_at_end_of_line() {
        let mut buf = String::from("ab");
        apply_change(&mut buf, &change(Some(rng(0, 2, 0, 2)), "c")).unwrap();
        assert_eq!(buf, "abc");
    }

    #[test]
    fn delete_single_char() {
        let mut buf = String::from("abc");
        apply_change(&mut buf, &change(Some(rng(0, 1, 0, 2)), "")).unwrap();
        assert_eq!(buf, "ac");
    }

    #[test]
    fn replace_single_char() {
        let mut buf = String::from("abc");
        apply_change(&mut buf, &change(Some(rng(0, 1, 0, 2)), "X")).unwrap();
        assert_eq!(buf, "aXc");
    }

    #[test]
    fn replace_spanning_newline() {
        let mut buf = String::from("ab\ncd");
        // Replace "b\nc" with "B-C".
        apply_change(&mut buf, &change(Some(rng(0, 1, 1, 1)), "B-C")).unwrap();
        assert_eq!(buf, "aB-Cd");
    }

    #[test]
    fn multiple_sequential_changes_compose_left_to_right() {
        let mut buf = String::from("hello");
        // First: insert "!" after "hello" → "hello!".
        apply_change(&mut buf, &change(Some(rng(0, 5, 0, 5)), "!")).unwrap();
        // Second: replace "hello" with "HI".
        apply_change(&mut buf, &change(Some(rng(0, 0, 0, 5)), "HI")).unwrap();
        assert_eq!(buf, "HI!");
    }

    #[test]
    fn change_over_multi_byte_utf8_char_boundary() {
        // "é" is 2 UTF-8 bytes and 1 UTF-16 unit.
        let mut buf = String::from("éX");
        // Delete the "é" (character range 0..1).
        apply_change(&mut buf, &change(Some(rng(0, 0, 0, 1)), "")).unwrap();
        assert_eq!(buf, "X");
    }

    #[test]
    fn change_across_supplementary_codepoint() {
        // U+1F600 is 2 UTF-16 units, 4 UTF-8 bytes.
        let mut buf = String::from("a\u{1F600}b");
        // Replace the smiley with "Y": character range 1..3.
        apply_change(&mut buf, &change(Some(rng(0, 1, 0, 3)), "Y")).unwrap();
        assert_eq!(buf, "aYb");
    }

    #[test]
    fn crlf_is_preserved_when_edit_avoids_it() {
        let mut buf = String::from("ab\r\ncd");
        // Insert at start of line 1.
        apply_change(&mut buf, &change(Some(rng(1, 0, 1, 0)), "Z")).unwrap();
        assert_eq!(buf, "ab\r\nZcd");
    }

    #[test]
    fn start_out_of_range_is_err() {
        let mut buf = String::from("abc");
        // Line 5 does not exist.
        let err = apply_change(&mut buf, &change(Some(rng(5, 0, 5, 0)), "x")).unwrap_err();
        assert_eq!(err, ApplyError::StartOutOfRange);
        assert_eq!(buf, "abc", "buffer must be untouched on error");
    }

    #[test]
    fn end_out_of_range_is_err() {
        let mut buf = String::from("abc");
        // Start is in range, end is past the last line.
        let err = apply_change(&mut buf, &change(Some(rng(0, 0, 9, 0)), "x")).unwrap_err();
        assert_eq!(err, ApplyError::EndOutOfRange);
        assert_eq!(buf, "abc");
    }

    #[test]
    fn inverted_range_is_err() {
        let mut buf = String::from("abc");
        // start = (0,2), end = (0,1) — inverted.
        let err = apply_change(&mut buf, &change(Some(rng(0, 2, 0, 1)), "x")).unwrap_err();
        assert_eq!(err, ApplyError::InvertedRange);
        assert_eq!(buf, "abc");
    }

    #[test]
    fn range_length_is_ignored() {
        // range_length disagrees with the actual range width; the
        // range still wins.
        let mut buf = String::from("abc");
        let ch = TextDocumentContentChangeEvent {
            range: Some(rng(0, 1, 0, 2)),
            range_length: Some(999),
            text: "X".to_owned(),
        };
        apply_change(&mut buf, &ch).unwrap();
        assert_eq!(buf, "aXc");
    }

    #[test]
    fn delete_entire_line_keeps_following_line() {
        let mut buf = String::from("foo\nbar\nbaz");
        // Delete "bar\n": (1,0)..(2,0).
        apply_change(&mut buf, &change(Some(rng(1, 0, 2, 0)), "")).unwrap();
        assert_eq!(buf, "foo\nbaz");
    }

    #[test]
    fn insert_newline_splits_line() {
        let mut buf = String::from("hello world");
        // At column 6 insert a newline.
        apply_change(&mut buf, &change(Some(rng(0, 5, 0, 6)), "\n")).unwrap();
        assert_eq!(buf, "hello\nworld");
    }
}
