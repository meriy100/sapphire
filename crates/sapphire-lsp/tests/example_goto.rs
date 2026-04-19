//! Integration test: load `examples/lsp-goto/hello.sp` and exercise
//! [`find_definition`] end-to-end through the same pipeline
//! `SapphireLanguageServer::resolve_position_to_location` uses
//! internally.
//!
//! The L5 design note (`docs/impl/22-lsp-goto-definition.md`)
//! distinguishes "unit-testable helper" (`find_definition`) from "LSP
//! server handler" (`goto_definition`). This test sits in the middle:
//! it invokes the helper against a real source tree the VSCode
//! extension opens, which catches shape regressions that would only
//! show up when the example file evolves.

use sapphire_compiler::analyze::analyze;
use sapphire_compiler::resolver::resolve;
use sapphire_lsp::definition::find_definition;
use sapphire_lsp::diagnostics::build_line_map;

fn read_example() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lsp-goto/hello.sp");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

fn byte_of(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("needle `{needle}` not found in example source"))
}

fn line_of(src: &str, byte: usize) -> u32 {
    src[..byte].bytes().filter(|&b| b == b'\n').count() as u32
}

fn col_of(src: &str, byte: usize) -> u32 {
    let line_start = src[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    src[line_start..byte].chars().count() as u32
}

#[test]
fn example_hello_sp_parses_and_resolves_cleanly() {
    let src = read_example();
    let analysis = analyze(&src);
    assert!(analysis.is_ok(), "analyze errors: {:?}", analysis.errors);
    let module = analysis.module.expect("module present");
    resolve(module).unwrap_or_else(|errs| panic!("resolve errors: {errs:?}"));
}

#[test]
fn example_goto_greet_reference_lands_on_greet_signature() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let line_map = build_line_map(&src);

    // `main = do\n  greet "Sapphire"` — the `greet` reference.
    let use_off = byte_of(&src, "  greet \"Sapphire\"") + 2;
    let range =
        find_definition(&module, &resolved, &src, use_off, &line_map).expect("goto resolves");
    // Expected: `greet : String -> Ruby {}` — the signature line.
    // Skip past the doc-comment occurrence of "greet : String" by
    // anchoring on the line-leading form.
    let def_off = byte_of(&src, "\ngreet : String") + 1;
    assert_eq!(range.start.line, line_of(&src, def_off));
    assert_eq!(range.start.character, col_of(&src, def_off));
}

#[test]
fn example_goto_constructor_a_lands_on_ctor_site() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let line_map = build_line_map(&src);

    // `A -> 1` in the case arm.
    let use_off = byte_of(&src, "  A -> 1") + 2;
    let range =
        find_definition(&module, &resolved, &src, use_off, &line_map).expect("goto resolves");
    // Ctor `A` in `data T = A | B`.
    let def_off = byte_of(&src, "= A |") + 2;
    assert_eq!(range.start.line, line_of(&src, def_off));
    assert_eq!(range.start.character, col_of(&src, def_off));
}

#[test]
fn example_goto_type_name_t_in_signature_lands_on_data_t() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let line_map = build_line_map(&src);

    // `pick : T -> Int` — the `T` in the signature.
    let use_off = byte_of(&src, "pick : T") + "pick : ".len();
    let range =
        find_definition(&module, &resolved, &src, use_off, &line_map).expect("goto resolves");
    // Anchor to the line-leading `data T` to skip past the
    // doc-comment occurrence near the top of the file.
    let def_off = byte_of(&src, "\ndata T = A | B") + 1 + "data ".len();
    assert_eq!(range.start.line, line_of(&src, def_off));
    assert_eq!(range.start.character, col_of(&src, def_off));
}

#[test]
fn example_goto_let_body_reference_lands_on_let_binder() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let line_map = build_line_map(&src);

    // `let greeting = "Hello, "\n  in greeting ++ ...` — the
    // `greeting` reference in the `in` body.
    let use_off = byte_of(&src, "in greeting") + "in ".len();
    let range =
        find_definition(&module, &resolved, &src, use_off, &line_map).expect("goto resolves");
    let def_off = byte_of(&src, "let greeting") + "let ".len();
    assert_eq!(range.start.line, line_of(&src, def_off));
    assert_eq!(range.start.character, col_of(&src, def_off));
}

#[test]
fn example_goto_prelude_name_yields_none() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let line_map = build_line_map(&src);

    // `++` is a prelude operator; no in-file definition exists.
    let plus_off = byte_of(&src, "greeting ++ name") + "greeting ".len();
    assert!(
        find_definition(&module, &resolved, &src, plus_off, &line_map).is_none(),
        "prelude `++` should not resolve to an in-file def"
    );
}
