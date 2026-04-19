//! The `sapphire` CLI — drives the compiler pipeline end-to-end.
//!
//! Three subcommands:
//!
//! - `sapphire check <path>` — run lex / layout / parse / resolve /
//!   typecheck over the given `.sp` file or directory. Exits zero on
//!   success, non-zero with diagnostics on failure. Intended for
//!   editor / pre-commit / CI checks (build 04 §`sapphire check`).
//!
//! - `sapphire build <path> [--out-dir <dir>]` — as above, then
//!   generate Ruby and write the output tree under `--out-dir`
//!   (default `gen/`). Matches the layout rules from build 02.
//!
//! - `sapphire run <path> [--out-dir <dir>]` — build, then spawn a
//!   `ruby` subprocess that loads the generated program and invokes
//!   `Sapphire::<Main>.run_main`. Exit code propagates from the
//!   subprocess.
//!
//! `--version` / `--help` (or `-h` / `-V`) are handled at the top
//! level. Argument parsing is hand-written by design; see
//! `docs/impl/27-cli.md` §引数パーサ.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use sapphire_compiler::codegen::{GeneratedProgram, generate};
use sapphire_compiler::parser::parse;
use sapphire_compiler::resolver::resolve_program;
use sapphire_compiler::typeck::check_program;
use sapphire_core::ast::Module as AstModule;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
sapphire — the Sapphire language compiler

USAGE:
    sapphire <SUBCOMMAND> [OPTIONS] [ARGS]

SUBCOMMANDS:
    check <path>                     Type-check only; no output written
    build <path> [--out-dir <dir>]   Compile to Ruby under <out-dir> (default: gen/)
    run   <path> [--out-dir <dir>]   build, then execute via `ruby`

OPTIONS:
    -h, --help      Print this message and exit
    -V, --version   Print compiler version and exit

See docs/build/04-invocation-and-config.md and docs/impl/27-cli.md for
the full contract.
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    match args[0].as_str() {
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "-V" | "--version" | "version" => {
            println!("sapphire {VERSION}");
            ExitCode::SUCCESS
        }
        "check" => run_check(&args[1..]),
        "build" => run_build(&args[1..]),
        "run" => run_run(&args[1..]),
        other => {
            eprintln!("sapphire: unknown subcommand '{other}'");
            eprintln!();
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

// ---------------------------------------------------------------------
//  Argument parsing
// ---------------------------------------------------------------------

struct CheckArgs {
    path: PathBuf,
}

struct BuildArgs {
    path: PathBuf,
    out_dir: PathBuf,
}

fn parse_check(args: &[String]) -> Result<CheckArgs, String> {
    let mut path: Option<PathBuf> = None;
    for a in args {
        if a == "-h" || a == "--help" {
            println!("usage: sapphire check <path>");
            std::process::exit(0);
        }
        if a.starts_with("--") {
            return Err(format!("check: unknown flag '{a}'"));
        }
        if path.is_some() {
            return Err("check: multiple positional arguments".into());
        }
        path = Some(PathBuf::from(a));
    }
    let path = path.ok_or_else(|| "check: missing <path>".to_string())?;
    Ok(CheckArgs { path })
}

fn parse_build(args: &[String]) -> Result<BuildArgs, String> {
    let mut path: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "-h" || a == "--help" {
            println!("usage: sapphire build <path> [--out-dir <dir>]");
            std::process::exit(0);
        }
        if a == "--out-dir" {
            i += 1;
            if i >= args.len() {
                return Err("build: --out-dir requires a directory argument".into());
            }
            out_dir = Some(PathBuf::from(&args[i]));
        } else if a.starts_with("--") {
            return Err(format!("build: unknown flag '{a}'"));
        } else if path.is_some() {
            return Err("build: multiple positional arguments".into());
        } else {
            path = Some(PathBuf::from(a));
        }
        i += 1;
    }
    let path = path.ok_or_else(|| "build: missing <path>".to_string())?;
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from("gen"));
    Ok(BuildArgs { path, out_dir })
}

// ---------------------------------------------------------------------
//  Subcommand bodies
// ---------------------------------------------------------------------

fn run_check(args: &[String]) -> ExitCode {
    let parsed = match parse_check(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sapphire: {e}");
            return ExitCode::from(2);
        }
    };
    match full_pipeline(&parsed.path) {
        Ok(_) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run_build(args: &[String]) -> ExitCode {
    let parsed = match parse_build(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sapphire: {e}");
            return ExitCode::from(2);
        }
    };
    let (resolved, typed) = match full_pipeline(&parsed.path) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let program = generate(&resolved, &typed);
    match write_program(&program, &parsed.out_dir) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sapphire: write error: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_run(args: &[String]) -> ExitCode {
    // run accepts the same flags as build plus an optional entry
    // module hint (defaulting to the first module that exports
    // `main : Ruby τ`).
    let parsed = match parse_build(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sapphire: {e}");
            return ExitCode::from(2);
        }
    };
    let (resolved, typed) = match full_pipeline(&parsed.path) {
        Ok(x) => x,
        Err(code) => return code,
    };
    // Pick the first module that defines a `main` binding.
    let entry_module = resolved
        .modules
        .iter()
        .find(|m| {
            m.env
                .top_lookup("main", sapphire_compiler::resolver::Namespace::Value)
                .is_some()
        })
        .map(|m| m.id.display());
    let Some(entry_module) = entry_module else {
        eprintln!("sapphire: run: no module in the input exports `main`");
        return ExitCode::from(1);
    };

    let program = generate(&resolved, &typed);
    if let Err(e) = write_program(&program, &parsed.out_dir) {
        eprintln!("sapphire: write error: {e}");
        return ExitCode::from(1);
    }

    // The runtime lives at <repo_root>/runtime/lib. Resolve it
    // relative to the binary location if SAPPHIRE_RUNTIME_LIB is not
    // set; otherwise trust the env var.
    let runtime_lib = runtime_lib_path();
    let rb_entry_path = module_rb_path(&entry_module);
    let ruby_code = format!("require '{rb_entry_path}'; exit Sapphire::{entry_module}.run_main");

    let status = Command::new("ruby")
        .arg("-I")
        .arg(&runtime_lib)
        .arg("-I")
        .arg(&parsed.out_dir)
        .arg("-e")
        .arg(&ruby_code)
        .status();
    match status {
        Ok(s) => match s.code() {
            Some(0) => ExitCode::SUCCESS,
            Some(code) => ExitCode::from(code.clamp(1, 125) as u8),
            None => ExitCode::from(1),
        },
        Err(e) => {
            eprintln!("sapphire: could not exec ruby: {e}");
            ExitCode::from(1)
        }
    }
}

// ---------------------------------------------------------------------
//  Front-end driver
// ---------------------------------------------------------------------

type PipelineOk = (
    sapphire_compiler::resolver::ResolvedProgram,
    sapphire_compiler::typeck::TypedProgram,
);

fn full_pipeline(path: &Path) -> Result<PipelineOk, ExitCode> {
    let sources = match collect_sources(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sapphire: {e}");
            return Err(ExitCode::from(1));
        }
    };
    let mut modules: Vec<AstModule> = Vec::new();
    for (p, src) in &sources {
        match parse(src) {
            Ok(m) => modules.push(m),
            Err(e) => {
                eprintln!("{}:{}: {e}", p.display(), 1);
                return Err(ExitCode::from(1));
            }
        }
    }
    let resolved = match resolve_program(modules) {
        Ok(r) => r,
        Err(errs) => {
            for e in errs {
                eprintln!("{e}");
            }
            return Err(ExitCode::from(1));
        }
    };
    let typed = match check_program(&resolved) {
        Ok(t) => t,
        Err(errs) => {
            for e in errs {
                eprintln!("{e}");
            }
            return Err(ExitCode::from(1));
        }
    };
    Ok((resolved, typed))
}

fn collect_sources(path: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = Vec::new();
        collect_dir(path, &mut entries)
            .map_err(|e| format!("cannot read dir {}: {e}", path.display()))?;
        entries.sort();
        for entry in entries {
            let src = fs::read_to_string(&entry)
                .map_err(|e| format!("cannot read {}: {e}", entry.display()))?;
            out.push((entry, src));
        }
    } else {
        let src =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        out.push((path.to_path_buf(), src));
    }
    if out.is_empty() {
        return Err(format!("no .sp files found under {}", path.display()));
    }
    Ok(out)
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect_dir(&p, out)?;
        } else if p.extension().is_some_and(|e| e == "sp") {
            out.push(p);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
//  Output layout helpers
// ---------------------------------------------------------------------

fn write_program(program: &GeneratedProgram, out_dir: &Path) -> std::io::Result<()> {
    for f in &program.files {
        let path = out_dir.join(&f.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &f.content)?;
    }
    Ok(())
}

fn module_rb_path(dotted: &str) -> String {
    let segs: Vec<String> = dotted.split('.').map(to_snake).collect();
    format!("sapphire/{}", segs.join("/"))
}

fn to_snake(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn runtime_lib_path() -> PathBuf {
    if let Ok(p) = env::var("SAPPHIRE_RUNTIME_LIB") {
        return PathBuf::from(p);
    }
    // Fall back to runtime/lib relative to the current working
    // directory. This keeps the CLI useful inside the worktree
    // without requiring an install step. Packaging (D1) replaces this
    // with a gem-relative path.
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // Try `runtime/lib` walking upwards until we find it or hit root.
    let mut dir: Option<&Path> = Some(cwd.as_path());
    while let Some(d) = dir {
        let candidate = d.join("runtime").join("lib");
        if candidate.is_dir() {
            return candidate;
        }
        dir = d.parent();
    }
    PathBuf::from("runtime/lib")
}
