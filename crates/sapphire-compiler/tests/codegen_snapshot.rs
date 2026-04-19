//! Drift detector for `examples/codegen-snapshot/*.rb`.
//!
//! These snapshots are not a normative contract (see
//! `examples/codegen-snapshot/README.md`) — the normative targets are
//! spec 10 §Generated Ruby module shape and `docs/build/02-source-and-
//! output-layout.md`. They exist so a human reviewing a codegen change
//! can eyeball its effect on real M9 output. This test keeps them
//! honest: every time `cargo test` runs, the stored snapshot must byte-
//! match what the current generator produces from the same source.
//!
//! When a codegen change is intentional, regenerate the snapshots per
//! `examples/codegen-snapshot/README.md` §再生成 and check the diff in.
//! The failure message points at the exact file that drifted to make
//! that flow quick.

use std::fs;
use std::path::{Path, PathBuf};

use sapphire_compiler::codegen::generate;
use sapphire_compiler::parser::parse;
use sapphire_compiler::resolver::resolve_program;
use sapphire_compiler::typeck::check_program;
use sapphire_core::ast::Module as AstModule;

fn workspace_root() -> PathBuf {
    let mut here = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        if here.join("runtime").join("lib").is_dir() && here.join("examples").is_dir() {
            return here;
        }
        if !here.pop() {
            panic!("cannot locate workspace root");
        }
    }
}

fn generate_module_sources(source_paths: &[&Path]) -> Vec<(String, String)> {
    let mut modules: Vec<AstModule> = Vec::new();
    for p in source_paths {
        let src = fs::read_to_string(p).expect("read source");
        modules.push(parse(&src).unwrap_or_else(|e| panic!("parse {}: {e}", p.display())));
    }
    let resolved = resolve_program(modules).expect("resolve");
    let typed = check_program(&resolved).expect("typecheck");
    let program = generate(&resolved, &typed);
    program
        .files
        .into_iter()
        .map(|f| (f.path, f.content))
        .collect()
}

fn assert_snapshot_matches(snapshot_path: &Path, actual: &str) {
    let expected = fs::read_to_string(snapshot_path).unwrap_or_else(|e| {
        panic!(
            "cannot read snapshot {}: {e}. Regenerate per \
             examples/codegen-snapshot/README.md §再生成",
            snapshot_path.display()
        )
    });
    assert_eq!(
        expected,
        actual,
        "\n\nsnapshot drift: {}\n\n\
         The generated Ruby differs from the checked-in snapshot. If \
         the change is intentional, regenerate per \
         `examples/codegen-snapshot/README.md` §再生成. Otherwise, \
         investigate the codegen change that caused the drift.\n",
        snapshot_path.display()
    );
}

/// Find the (path, content) entry from a program output by its
/// generated relative path (e.g. `sapphire/main.rb`).
fn pick<'a>(files: &'a [(String, String)], rel: &str) -> &'a str {
    &files
        .iter()
        .find(|(p, _)| p == rel)
        .unwrap_or_else(|| panic!("no generated file at {rel}"))
        .1
}

#[test]
fn snapshot_example_01_hello_ruby_main_matches() {
    let root = workspace_root();
    let source = root.join("examples/sources/01-hello-ruby/Main.sp");
    let files = generate_module_sources(&[&source]);
    let actual = pick(&files, "sapphire/main.rb");
    let snapshot = root.join("examples/codegen-snapshot/01-hello-ruby-main.rb");
    assert_snapshot_matches(&snapshot, actual);
}

#[test]
fn snapshot_example_02_parse_numbers_number_sum_matches() {
    let root = workspace_root();
    let source = root.join("examples/sources/02-parse-numbers/NumberSum.sp");
    let files = generate_module_sources(&[&source]);
    let actual = pick(&files, "sapphire/number_sum.rb");
    let snapshot = root.join("examples/codegen-snapshot/02-parse-numbers-number_sum.rb");
    assert_snapshot_matches(&snapshot, actual);
}

#[test]
fn snapshot_example_03_students_records_students_matches() {
    let root = workspace_root();
    let source = root.join("examples/sources/03-students-records/Students.sp");
    let files = generate_module_sources(&[&source]);
    let actual = pick(&files, "sapphire/students.rb");
    let snapshot = root.join("examples/codegen-snapshot/03-students-records-students.rb");
    assert_snapshot_matches(&snapshot, actual);
}

#[test]
fn snapshot_example_04_fetch_summarise_matches() {
    let root = workspace_root();
    let files = generate_module_sources(&[
        &root.join("examples/sources/04-fetch-summarise/Fetch.sp"),
        &root.join("examples/sources/04-fetch-summarise/Http.sp"),
    ]);
    let snap_root = root.join("examples/codegen-snapshot");
    assert_snapshot_matches(
        &snap_root.join("04-fetch-summarise-fetch.rb"),
        pick(&files, "sapphire/fetch.rb"),
    );
    assert_snapshot_matches(
        &snap_root.join("04-fetch-summarise-http.rb"),
        pick(&files, "sapphire/http.rb"),
    );
}

#[test]
fn snapshot_prelude_matches() {
    // The emitted prelude is deterministic: any source program works.
    let root = workspace_root();
    let source = root.join("examples/sources/01-hello-ruby/Main.sp");
    let files = generate_module_sources(&[&source]);
    let actual = pick(&files, "sapphire/prelude.rb");
    let snapshot = root.join("examples/codegen-snapshot/prelude.rb");
    assert_snapshot_matches(&snapshot, actual);
}
