//! Integration test: load `examples/lsp-completion/hello.sp` and
//! exercise [`find_completion_items`] end-to-end through the same
//! pipeline `SapphireLanguageServer::resolve_completion_at` uses
//! internally.
//!
//! The L6 design note (`docs/impl/31-lsp-completion.md`) distinguishes
//! the pure helper (`find_completion_items`) from the LSP server
//! handler (`completion`). This test sits in the middle: it invokes
//! the helper against the real source tree the VSCode extension
//! opens. The assertions are deliberately coarse — they check the
//! *presence* of the expected candidates (and, where applicable,
//! their `kind`) rather than exact ordering, so cosmetic changes to
//! the example file or the insertion order do not break this test.
//!
//! Source positions are located by searching for unique needles in
//! the example text. Every cursor sits *inside* an existing
//! identifier (e.g. `gr` in `main = greet`), so `analyze` and
//! `resolve` remain clean — which matches how VSCode feeds positions
//! during live typing (the rest of the ident is already there to
//! the right of the cursor).

use sapphire_compiler::analyze::analyze;
use sapphire_compiler::resolver::resolve;
use sapphire_lsp::completion::find_completion_items;
use sapphire_lsp::hover::collect_hover_types;
use tower_lsp::lsp_types::CompletionItemKind;

fn read_example() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lsp-completion/hello.sp");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

#[test]
fn example_hello_sp_parses_and_resolves_cleanly() {
    let src = read_example();
    let analysis = analyze(&src);
    assert!(analysis.is_ok(), "analyze errors: {:?}", analysis.errors);
    let module = analysis.module.expect("module present");
    let _resolved =
        resolve(module.clone()).unwrap_or_else(|errs| panic!("resolve errors: {errs:?}"));
}

#[test]
fn example_completion_top_level_greet_prefix() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);

    // Cursor on `gr` inside the first `greet "Sapphire"` call
    // (`  greet "Sapphire"`). 2 chars into `greet`, so prefix `gr`.
    let ref_site = src
        .find("  greet \"Sapphire\"")
        .expect("greet needle present");
    let cur = ref_site + "  gr".len();
    let items = find_completion_items(&module, &resolved, &typed, &src, cur);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // Top-level value `greet` must appear.
    assert!(
        labels.contains(&"greet"),
        "expected `greet` in completion labels: {labels:?}",
    );
    // Top-level value `greeting` also matches the `gr` prefix.
    assert!(
        labels.contains(&"greeting"),
        "expected `greeting` in completion labels: {labels:?}",
    );
    // An unrelated top-level like `packHalf` must NOT match the `gr`
    // prefix.
    assert!(
        !labels.contains(&"packHalf"),
        "`packHalf` must not match `gr` prefix: {labels:?}",
    );

    // `greet` should have FUNCTION kind + a non-empty scheme detail.
    let greet_item = items
        .iter()
        .find(|i| i.label == "greet")
        .expect("greet item");
    assert_eq!(greet_item.kind, Some(CompletionItemKind::FUNCTION));
    let detail = greet_item.detail.as_deref().unwrap_or("");
    assert!(
        detail.contains("->") || detail.contains("Ruby"),
        "expected a scheme-shaped detail for greet, got: {detail:?}",
    );
}

#[test]
fn example_completion_prelude_constructor_just_in_packhalf() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);

    // `packHalf n = Just n` — cursor 2 chars into `Just`, prefix `Ju`.
    let site = src.find("= Just n").expect("Just reference needle present");
    let cur = site + "= Ju".len();
    let items = find_completion_items(&module, &resolved, &typed, &src, cur);
    let just_item = items
        .iter()
        .find(|i| i.label == "Just")
        .expect("Just candidate");
    assert_eq!(just_item.kind, Some(CompletionItemKind::CONSTRUCTOR));
}

#[test]
fn example_completion_local_binding_shadows_top_level_in_body() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);

    // `in greeting ++ name ++ "!"` — the local `greeting` is bound
    // in the enclosing `let` of `makeMessage`. Cursor 4 chars into
    // `greeting`, prefix `gree`.
    let site = src.find("in greeting").expect("in greeting needle present");
    let cur = site + "in gree".len();
    let items = find_completion_items(&module, &resolved, &typed, &src, cur);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // The local `greeting` is emitted with VARIABLE kind. The
    // top-level `greeting` is also surfaced (so the user can
    // explicitly refer to it by `Main.greeting`), but must not
    // displace the local one. Assert the local is present and
    // that the first `greeting` entry we emit — locals come before
    // top-level names in insertion order — is the VARIABLE one.
    let greetings: Vec<&tower_lsp::lsp_types::CompletionItem> =
        items.iter().filter(|i| i.label == "greeting").collect();
    assert!(
        !greetings.is_empty(),
        "expected at least one `greeting` completion: none found",
    );
    assert_eq!(
        greetings[0].kind,
        Some(CompletionItemKind::VARIABLE),
        "local `greeting` must come first with VARIABLE kind: {:?}",
        greetings
            .iter()
            .map(|c| (c.kind, &c.detail))
            .collect::<Vec<_>>(),
    );
    // The top-level `greet` still shows up (prefix `gree` matches).
    assert!(
        labels.contains(&"greet"),
        "expected top-level `greet` still in list: {labels:?}",
    );
}

#[test]
fn example_completion_module_qualifier_main_lists_top_levels() {
    let src = read_example();
    let analysis = analyze(&src);
    let module = analysis.module.expect("module present");
    let resolved = resolve(module.clone()).expect("resolve ok");
    let typed = collect_hover_types(&resolved.env.id.display(), &module);

    // Synthetic cursor: the real example file does not use
    // `Main.foo` qualifications, so we feed a separate buffer shaped
    // like an editor mid-edit. `find_completion_items` is pure over
    // the `(source, offset)` pair, so this is legal.
    let synthetic = "Main.gr";
    let items = find_completion_items(&module, &resolved, &typed, synthetic, synthetic.len());
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // `greet` / `greeting` are top-level in Main and match the `gr`
    // prefix; they must appear under the `Main.` qualifier.
    assert!(
        labels.contains(&"greet"),
        "expected `greet` under Main.gr: {labels:?}",
    );
    assert!(
        labels.contains(&"greeting"),
        "expected `greeting` under Main.gr: {labels:?}",
    );
    // `Alpha` (an Upper-case top-level ctor) does not start with
    // `gr`, so it must be absent.
    assert!(
        !labels.contains(&"Alpha"),
        "`Alpha` must not match `gr` prefix: {labels:?}",
    );
}
