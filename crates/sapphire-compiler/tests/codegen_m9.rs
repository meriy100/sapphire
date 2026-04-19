//! End-to-end M9 example integration tests.
//!
//! These tests compile each of the four `examples/sources/` programs
//! to Ruby via the `sapphire-compiler` crate's codegen entry point,
//! then shell out to `ruby` to execute the result. The tests require
//! `ruby` (3.3) to be on `$PATH`; they are skipped with a print-only
//! warning otherwise so CI stages without Ruby can still run the
//! Rust-only suite.
//!
//! Example 4 (`fetch-summarise`) stubs out `Net::HTTP.get_response`
//! inline so the test does not make a real network call. `run_net_http`
//! preserves the exact shape spec 10 expects for the `get` snippet's
//! return.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use sapphire_compiler::codegen::generate;
use sapphire_compiler::parser::parse;
use sapphire_compiler::resolver::resolve_program;
use sapphire_compiler::typeck::check_program;
use sapphire_core::ast::Module as AstModule;

fn ruby_available() -> bool {
    Command::new("ruby")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn workspace_root() -> std::path::PathBuf {
    let mut here = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    // Walk up to where `runtime/lib` and `examples/` coexist.
    loop {
        if here.join("runtime").join("lib").is_dir() && here.join("examples").is_dir() {
            return here;
        }
        if !here.pop() {
            panic!(
                "cannot locate workspace root from {}",
                env!("CARGO_MANIFEST_DIR")
            );
        }
    }
}

fn build_example(source_paths: &[&Path], out_dir: &Path) {
    fs::create_dir_all(out_dir).unwrap();
    let mut modules: Vec<AstModule> = Vec::new();
    for p in source_paths {
        let src = fs::read_to_string(p).expect("read source");
        modules.push(parse(&src).unwrap_or_else(|e| panic!("parse {}: {e}", p.display())));
    }
    let resolved = resolve_program(modules).expect("resolve");
    let typed = check_program(&resolved).expect("typecheck");
    let program = generate(&resolved, &typed);
    for f in &program.files {
        let path = out_dir.join(&f.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, &f.content).unwrap();
    }
}

fn run_ruby(out_dir: &Path, runtime_lib: &Path, entry: &str) -> (i32, String, String) {
    let output = Command::new("ruby")
        .arg("-I")
        .arg(runtime_lib)
        .arg("-I")
        .arg(out_dir)
        .arg("-e")
        .arg(entry)
        .output()
        .expect("ruby exec");
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (code, stdout, stderr)
}

fn out_dir_for(name: &str) -> std::path::PathBuf {
    let dir = env::temp_dir().join(format!("sapphire-m9-{name}"));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn example_01_hello_ruby_runs_end_to_end() {
    if !ruby_available() {
        eprintln!("skipping: ruby not on PATH");
        return;
    }
    let root = workspace_root();
    let source = root.join("examples/sources/01-hello-ruby/Main.sp");
    let out = out_dir_for("01");
    build_example(&[&source], &out);

    let entry = "require 'sapphire/main'; exit Sapphire::Main.run_main";
    let (code, stdout, stderr) = run_ruby(&out, &root.join("runtime/lib"), entry);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "Hello, Sapphire!\nHello, world!\n");
}

#[test]
fn example_02_parse_numbers_runs_end_to_end() {
    if !ruby_available() {
        eprintln!("skipping: ruby not on PATH");
        return;
    }
    let root = workspace_root();
    let source = root.join("examples/sources/02-parse-numbers/NumberSum.sp");
    let out = out_dir_for("02");
    build_example(&[&source], &out);

    // Write numbers.txt where `File.readlines` will find it (cwd
    // is the out_dir for the ruby subprocess — we can force with
    // -C via --current-dir, but Ruby doesn't have that. Instead:
    // place numbers.txt inside out/ and chdir there for the run.
    let numbers = out.join("numbers.txt");
    fs::write(&numbers, "1\n2\n3\n").unwrap();

    let entry = "require 'sapphire/number_sum'; exit Sapphire::NumberSum.run_main";
    let output = Command::new("ruby")
        .arg("-I")
        .arg(root.join("runtime/lib"))
        .arg("-I")
        .arg(&out)
        .arg("-e")
        .arg(entry)
        .current_dir(&out)
        .output()
        .expect("ruby exec");
    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert_eq!(stdout.trim(), "6");
}

#[test]
fn example_03_students_records_topscorers_spot_check() {
    if !ruby_available() {
        eprintln!("skipping: ruby not on PATH");
        return;
    }
    let root = workspace_root();
    let source = root.join("examples/sources/03-students-records/Students.sp");
    let out = out_dir_for("03");
    build_example(&[&source], &out);

    let entry = r#"
require 'sapphire/students'
students = [
  { name: 'Alice', grade: 1, score: 90 },
  { name: 'Bob',   grade: 1, score: 80 },
  { name: 'Carol', grade: 2, score: 95 },
]
result = Sapphire::Students.topScorersByGrade.call(students)
expected = [
  { grade: 1, top: { name: 'Alice', grade: 1, score: 90 } },
  { grade: 2, top: { name: 'Carol', grade: 2, score: 95 } },
]
unless result == expected
  warn "mismatch: #{result.inspect}"
  exit 1
end
exit 0
"#;
    let (code, _stdout, stderr) = run_ruby(&out, &root.join("runtime/lib"), entry);
    assert_eq!(code, 0, "stderr: {stderr}");
}

#[test]
fn example_04_fetch_runs_with_network_stub() {
    if !ruby_available() {
        eprintln!("skipping: ruby not on PATH");
        return;
    }
    let root = workspace_root();
    let out = out_dir_for("04");
    build_example(
        &[
            &root.join("examples/sources/04-fetch-summarise/Fetch.sp"),
            &root.join("examples/sources/04-fetch-summarise/Http.sp"),
        ],
        &out,
    );

    let entry = r#"
require 'net/http'
module Net
  class HTTP
    def self.get_response(_uri)
      r = Object.new
      def r.is_a?(klass); klass == Net::HTTPSuccess; end
      def r.body; 'hello'; end
      r
    end
  end
end
require 'sapphire/fetch'
exit Sapphire::Fetch.run_main
"#;
    let (code, stdout, stderr) = run_ruby(&out, &root.join("runtime/lib"), entry);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("fetched 5 bytes"), "stdout: {stdout}");
}
