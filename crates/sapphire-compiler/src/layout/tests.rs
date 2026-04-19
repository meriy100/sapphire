//! Unit tests for the layout pass.

use super::resolve_with_source;
use crate::lexer::{TokenKind, tokenize};

/// Collect the layout-resolved token kinds, dropping `Newline` /
/// `Indent` (layout should have removed them all) and `Eof`.
fn resolve_kinds(src: &str) -> Vec<TokenKind> {
    let tokens = tokenize(src).expect("lex ok");
    let resolved = resolve_with_source(tokens, src).expect("layout ok");
    resolved
        .into_iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Eof | TokenKind::Newline | TokenKind::Indent(_)
            )
        })
        .map(|t| t.kind)
        .collect()
}

#[test]
fn empty_input_produces_only_virtual_braces() {
    let kinds = resolve_kinds("");
    assert_eq!(kinds, vec![TokenKind::LBrace, TokenKind::RBrace]);
}

#[test]
fn no_newline_or_indent_tokens_in_output() {
    let src = "module Foo where\n  x = 1\n  y = 2\n";
    let tokens = tokenize(src).unwrap();
    let resolved = resolve_with_source(tokens, src).unwrap();
    for t in &resolved {
        assert!(
            !matches!(t.kind, TokenKind::Newline | TokenKind::Indent(_)),
            "unexpected layout marker in output: {:?}",
            t.kind
        );
    }
}

#[test]
fn top_level_wrapped_in_virtual_braces() {
    let kinds = resolve_kinds("x = 1\ny = 2\n");
    // {_ x = 1 ; y = 2 _}
    assert_eq!(kinds.first(), Some(&TokenKind::LBrace));
    assert_eq!(kinds.last(), Some(&TokenKind::RBrace));
    // A virtual `;` should appear between the two top-level bindings.
    let semis = kinds
        .iter()
        .filter(|k| matches!(k, TokenKind::Semicolon))
        .count();
    assert_eq!(semis, 1, "exactly one virtual `;` between two decls");
}

#[test]
fn where_block_opens_implicit_layout() {
    let src = "module Foo where\n  x = 1\n  y = 2\n";
    let kinds = resolve_kinds(src);
    // Three virtual `{` ... `}` pairs expected? Top-level +
    // `where`-block nesting = 2.
    let lbraces = kinds
        .iter()
        .filter(|k| matches!(k, TokenKind::LBrace))
        .count();
    let rbraces = kinds
        .iter()
        .filter(|k| matches!(k, TokenKind::RBrace))
        .count();
    assert_eq!(lbraces, rbraces);
    assert!(lbraces >= 2, "expected nested layout block for `where`");
}

#[test]
fn let_in_closes_implicit_block_on_in() {
    let src = "x = let a = 1\n        b = 2\n    in a\n";
    let kinds = resolve_kinds(src);
    // The `in` keyword should appear, and there should be a virtual
    // `}` immediately before it closing the let-block.
    let in_pos = kinds
        .iter()
        .position(|k| matches!(k, TokenKind::In))
        .expect("`in` in output");
    assert!(in_pos > 0);
    assert!(matches!(kinds[in_pos - 1], TokenKind::RBrace));
}

#[test]
fn of_block_closes_on_dedent() {
    let src = "f x = case x of\n  Just n -> n\n  Nothing -> 0\n";
    let kinds = resolve_kinds(src);
    let of_pos = kinds
        .iter()
        .position(|k| matches!(k, TokenKind::Of))
        .expect("`of`");
    assert!(matches!(kinds[of_pos + 1], TokenKind::LBrace));
    // Count the closing braces after `of`: should be at least one
    // implicit close for the `of` block itself.
    let closing_after_of = kinds[of_pos..]
        .iter()
        .filter(|k| matches!(k, TokenKind::RBrace))
        .count();
    assert!(closing_after_of >= 1);
}

#[test]
fn explicit_braces_disable_layout() {
    let src = "x = case e of { Just n -> n ; Nothing -> 0 }";
    let resolved = resolve_with_source(tokenize(src).unwrap(), src).unwrap();
    // All explicit braces + semis are from the programmer; no
    // virtual ones should be injected *inside* the explicit block.
    // The top-level still wraps the whole file.
    let kinds: Vec<_> = resolved
        .iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Eof | TokenKind::Newline | TokenKind::Indent(_)
            )
        })
        .map(|t| &t.kind)
        .collect();
    // Expect exactly two `LBrace` (top-level + explicit `{`) and two
    // `RBrace` to match.
    let l = kinds
        .iter()
        .filter(|k| matches!(k, TokenKind::LBrace))
        .count();
    let r = kinds
        .iter()
        .filter(|k| matches!(k, TokenKind::RBrace))
        .count();
    assert_eq!(l, 2);
    assert_eq!(r, 2);
}

#[test]
fn do_block_separates_statements_by_virtual_semi() {
    let src = "main = do\n  a\n  b\n  c\n";
    let kinds = resolve_kinds(src);
    // Find the `do` and count virtual semis between its LBrace and
    // RBrace.
    let do_pos = kinds
        .iter()
        .position(|k| matches!(k, TokenKind::Do))
        .unwrap();
    assert!(matches!(kinds[do_pos + 1], TokenKind::LBrace));
    // Three do-statements → two inner `;`s.
    let mut depth = 0usize;
    let mut semis = 0usize;
    for k in &kinds[do_pos + 1..] {
        match k {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => {
                if depth == 1 {
                    break;
                }
                depth -= 1;
            }
            TokenKind::Semicolon if depth == 1 => semis += 1,
            _ => {}
        }
    }
    assert_eq!(semis, 2);
}
