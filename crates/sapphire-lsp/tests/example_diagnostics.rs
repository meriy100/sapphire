//! Integration test: analyze each `examples/lsp-diagnostics/*.sp`
//! and assert it produces the expected diagnostic category.
//!
//! The LSP samples are maintained alongside the L2 diagnostics work
//! so editors have something concrete to open. This test pins each
//! example to the front-end stage it is supposed to exercise, so a
//! later refactor (e.g. making the lexer accept some new character
//! class) cannot silently turn a `lex_error.sp` into a
//! `parse_error.sp` without someone noticing.

use sapphire_compiler::analyze::analyze;
use sapphire_compiler::error::CompileErrorKind;

fn read_example(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lsp-diagnostics")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

#[test]
fn good_example_has_no_diagnostics() {
    let src = read_example("good.sp");
    let result = analyze(&src);
    assert!(
        result.is_ok(),
        "expected good.sp to parse cleanly, got {:?}",
        result.errors
    );
}

#[test]
fn lex_error_example_triggers_lex_error() {
    let src = read_example("lex_error.sp");
    let result = analyze(&src);
    assert_eq!(result.errors.len(), 1);
    assert!(
        matches!(result.errors[0].kind, CompileErrorKind::Lex(_)),
        "expected Lex error, got {:?}",
        result.errors[0]
    );
}

#[test]
fn layout_error_example_triggers_layout_error() {
    let src = read_example("layout_error.sp");
    let result = analyze(&src);
    assert_eq!(result.errors.len(), 1);
    assert!(
        matches!(result.errors[0].kind, CompileErrorKind::Layout(_)),
        "expected Layout error, got {:?}",
        result.errors[0]
    );
}

#[test]
fn parse_error_example_triggers_parse_error() {
    let src = read_example("parse_error.sp");
    let result = analyze(&src);
    assert_eq!(result.errors.len(), 1);
    assert!(
        matches!(result.errors[0].kind, CompileErrorKind::Parse(_)),
        "expected Parse error, got {:?}",
        result.errors[0]
    );
}
