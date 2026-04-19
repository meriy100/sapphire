//! Integration test: load `examples/lsp-hover/hello.sp` and exercise
//! [`find_hover_info`] end-to-end through the same pipeline
//! `SapphireLanguageServer::resolve_position_to_hover` uses
//! internally.
//!
//! The L4 design note (`docs/impl/28-lsp-hover.md`) distinguishes
//! "unit-testable helper" (`find_hover_info`) from "LSP server
//! handler" (`hover`). This test sits in the middle: it invokes the
//! helper against the real source tree the VSCode extension opens,
//! which catches shape regressions that would only show up when the
//! example file evolves. The assertions are deliberately coarse —
//! they check the *category* of the hover (tag + scheme presence),
//! not the exact layout of the Markdown, so cosmetic renderer
//! tweaks do not break this test.

use sapphire_compiler::analyze::analyze;
use sapphire_compiler::resolver::resolve;
use sapphire_lsp::diagnostics::build_line_map;
use sapphire_lsp::hover::{collect_hover_types, find_hover_info};
use tower_lsp::lsp_types::{HoverContents, MarkupKind};

fn read_example() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/lsp-hover/hello.sp");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

fn byte_of(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("needle `{needle}` not found in example source"))
}

fn markdown_body(h: &tower_lsp::lsp_types::Hover) -> &str {
    match &h.contents {
        HoverContents::Markup(m) => {
            assert_eq!(m.kind, MarkupKind::Markdown, "hover must be Markdown");
            &m.value
        }
        other => panic!("expected Markup hover, got {other:?}"),
    }
}

#[test]
fn example_hello_sp_parses_resolves_and_typechecks_cleanly() {
    let src = read_example();
    let analysis = analyze(&src);
    assert!(analysis.is_ok(), "analyze errors: {:?}", analysis.errors);
    let module = analysis.module.expect("module present");
    let resolved =
        resolve(module.clone()).unwrap_or_else(|errs| panic!("resolve errors: {errs:?}"));
    // typeck is allowed to partially fail; we just want inferred to
    // contain the top-level names we later assert on.
    let typed = collect_hover_types(&resolved.env.id.display(), &module);
    assert!(
        typed.inferred.contains_key("greet"),
        "expected `greet` scheme in inferred map; got keys {:?}",
        typed.inferred.keys().collect::<Vec<_>>(),
    );
    assert!(
        typed.inferred.contains_key("main"),
        "expected `main` scheme in inferred map",
    );
}

#[test]
fn example_hover_top_level_greet_reference_shows_scheme() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);
    let line_map = build_line_map(&src);

    // `main = do\n  greet "Sapphire"` — the `greet` reference.
    let use_off = byte_of(&src, "  greet \"Sapphire\"") + 2;
    let hover = find_hover_info(&module, &resolved, &typed, &src, use_off, &line_map)
        .expect("hover present");
    let md = markdown_body(&hover);
    assert!(md.contains("```sapphire"), "missing code fence: {md}");
    assert!(md.contains("greet"), "missing greet in hover: {md}");
    assert!(
        md.contains("(top-level value)"),
        "expected top-level tag: {md}",
    );
}

#[test]
fn example_hover_constructor_a_shows_constructor_tag() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);
    let line_map = build_line_map(&src);

    // The `A` on the case arm `A -> 1`.
    let use_off = byte_of(&src, "  A -> 1") + 2;
    let hover = find_hover_info(&module, &resolved, &typed, &src, use_off, &line_map)
        .expect("hover present");
    let md = markdown_body(&hover);
    assert!(md.contains("constructor of `T`"), "expected ctor tag: {md}",);
    // `A : T` — a nullary constructor of T.
    assert!(md.contains("A : "), "expected A scheme: {md}");
}

#[test]
fn example_hover_prelude_just_shows_prelude_tag() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);
    let line_map = build_line_map(&src);

    // `packHalf n = Just n`.
    let use_off = byte_of(&src, "= Just n") + 2;
    let hover = find_hover_info(&module, &resolved, &typed, &src, use_off, &line_map)
        .expect("hover present");
    let md = markdown_body(&hover);
    assert!(md.contains("Just : "), "missing Just scheme: {md}");
    assert!(md.contains("(prelude)"), "expected prelude tag: {md}");
}

#[test]
fn example_hover_prelude_append_operator_shows_scheme() {
    // Regression for reviewer must-fix #1: `++` lives in
    // `type_env.globals` under `GlobalId::new("Prelude", "++")` and
    // nowhere else. The hover must surface the scheme, not just the
    // `(prelude)` tag + "型情報未取得" fallback.
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);
    let line_map = build_line_map(&src);

    // `in greeting ++ name ++ "!"` — the first `++`.
    let use_off = byte_of(&src, "greeting ++ name") + "greeting ".len();
    let hover = find_hover_info(&module, &resolved, &typed, &src, use_off, &line_map)
        .expect("hover present");
    let md = markdown_body(&hover);
    assert!(md.contains("(prelude)"), "expected prelude tag: {md}");
    assert!(
        md.contains("++ : String -> String -> String"),
        "expected `++` scheme line: {md}",
    );
    assert!(
        !md.contains("_型情報未取得_"),
        "`++` scheme must be populated, got fallback note: {md}",
    );
}

#[test]
fn example_hover_local_let_binder_shows_local_tag() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);
    let line_map = build_line_map(&src);

    // `let greeting = "Hello, "\n  in greeting ++ ...` — hover on
    // the `greeting` reference in the `in` body.
    let use_off = byte_of(&src, "in greeting") + "in ".len();
    let hover = find_hover_info(&module, &resolved, &typed, &src, use_off, &line_map)
        .expect("hover present");
    let md = markdown_body(&hover);
    assert!(md.contains("(local)"), "expected local tag: {md}");
    assert!(md.contains("_型情報未取得_"), "expected fallback: {md}");
}
