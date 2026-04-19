//! Smoke tests for the `sapphire` binary. Exercise the subcommand
//! surface (`--version`, `--help`, `check`, `build`) against the
//! bundled M9 examples to make sure the CLI wiring is intact.
//!
//! The tests deliberately avoid `sapphire run` here — that flavour is
//! covered by `tests/codegen_m9.rs` which goes through the library
//! entry points. Keeping the CLI smoke narrow avoids duplicating the
//! Ruby-subprocess setup.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn sapphire_bin() -> PathBuf {
    // Cargo sets `CARGO_BIN_EXE_<name>` for integration tests.
    PathBuf::from(env!("CARGO_BIN_EXE_sapphire"))
}

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

#[test]
fn version_flag_prints_version() {
    let out = Command::new(sapphire_bin())
        .arg("--version")
        .output()
        .expect("run sapphire");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("sapphire "));
}

#[test]
fn help_flag_prints_usage() {
    let out = Command::new(sapphire_bin())
        .arg("--help")
        .output()
        .expect("run sapphire");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE:"));
    assert!(stdout.contains("build"));
    assert!(stdout.contains("check"));
    assert!(stdout.contains("run"));
}

#[test]
fn no_args_prints_usage_and_exits_nonzero() {
    let out = Command::new(sapphire_bin()).output().expect("run sapphire");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("USAGE:"));
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    let out = Command::new(sapphire_bin())
        .arg("nope")
        .output()
        .expect("run sapphire");
    assert!(!out.status.success());
}

#[test]
fn check_passes_on_valid_m9_example() {
    let root = workspace_root();
    let source = root.join("examples/sources/01-hello-ruby/Main.sp");
    let out = Command::new(sapphire_bin())
        .arg("check")
        .arg(&source)
        .output()
        .expect("run sapphire");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn check_fails_on_parse_error() {
    let tmp = env::temp_dir().join("sapphire-cli-smoke-bad.sp");
    fs::write(&tmp, "module Broken where\nbadsyntax").unwrap();
    let out = Command::new(sapphire_bin())
        .arg("check")
        .arg(&tmp)
        .output()
        .expect("run sapphire");
    assert!(!out.status.success());
}

#[test]
fn build_writes_out_dir_contents() {
    let root = workspace_root();
    let source = root.join("examples/sources/01-hello-ruby/Main.sp");
    let out_dir = env::temp_dir().join("sapphire-cli-smoke-build-out");
    let _ = fs::remove_dir_all(&out_dir);

    let out = Command::new(sapphire_bin())
        .arg("build")
        .arg(&source)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run sapphire");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let prelude_path = out_dir.join("sapphire").join("prelude.rb");
    let main_path = out_dir.join("sapphire").join("main.rb");
    assert!(prelude_path.is_file(), "prelude.rb missing");
    assert!(main_path.is_file(), "main.rb missing");
}

#[test]
fn build_rejects_unknown_flag() {
    let root = workspace_root();
    let source = root.join("examples/sources/01-hello-ruby/Main.sp");
    let out = Command::new(sapphire_bin())
        .arg("build")
        .arg(&source)
        .arg("--this-flag-does-not-exist")
        .output()
        .expect("run sapphire");
    assert!(!out.status.success());
}

/// Regression test for must-fix 1: a multi-segment entry module (e.g.
/// `App.Main`) must resolve to the Ruby class path
/// `Sapphire::App::Main`. The previous `format!("...Sapphire::{entry}"
/// , ...)` left dots in place, so Ruby would parse
/// `Sapphire::App.Main.run_main` as a method chain and fail at load
/// time. We exercise this by running `sapphire run` against a
/// synthesised two-segment module and asserting the subprocess exits
/// successfully.
#[test]
fn run_resolves_multi_segment_module_entry_path() {
    // Ruby is required for `sapphire run` to do anything meaningful;
    // skip silently when missing so CI without Ruby still completes
    // the CLI smoke suite.
    if Command::new("ruby")
        .arg("--version")
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(true)
    {
        eprintln!("skipping: ruby not on PATH");
        return;
    }

    let root = workspace_root();
    let tmp_src = env::temp_dir().join("sapphire-cli-smoke-multi-seg");
    let _ = fs::remove_dir_all(&tmp_src);
    fs::create_dir_all(&tmp_src).unwrap();
    let src_file = tmp_src.join("AppMain.sp");
    fs::write(
        &src_file,
        "\
module App.Main where
rubyPuts : String -> Ruby {}
rubyPuts s := \"puts s\"
main : Ruby {}
main = rubyPuts \"ok\"
",
    )
    .unwrap();

    let out_dir = env::temp_dir().join("sapphire-cli-smoke-multi-seg-out");
    let _ = fs::remove_dir_all(&out_dir);

    // SAPPHIRE_RUNTIME_LIB is honoured by the CLI's runtime-path
    // resolver; point it at the in-tree runtime so the subprocess can
    // `require 'sapphire/runtime'`.
    let runtime_lib = root.join("runtime/lib");
    let out = Command::new(sapphire_bin())
        .env("SAPPHIRE_RUNTIME_LIB", &runtime_lib)
        .arg("run")
        .arg(&src_file)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run sapphire");
    assert!(
        out.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "ok",
        "stdout should contain the rubyPuts output"
    );
}
