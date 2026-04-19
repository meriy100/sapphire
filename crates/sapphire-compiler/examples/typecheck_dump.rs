//! CLI that reads one or more Sapphire source files, parses + resolves
//! + type-checks them, and dumps the inferred scheme of every
//!   top-level binding in a compact plain-text form.
//!
//! Usage:
//!
//! ```ignore
//! cargo run -p sapphire-compiler --example typecheck_dump -- \
//!     examples/sources/01-hello-ruby/Main.sp
//! ```
//!
//! Multiple `.sp` files — or a directory containing `.sp` files — can
//! be passed at once; the resolver + type checker will handle
//! inter-module dependencies.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sapphire_compiler::parser::parse;
use sapphire_compiler::resolver::resolve_program;
use sapphire_compiler::typeck::{TypedProgram, check_program};
use sapphire_core::ast::Module as AstModule;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: typecheck_dump <file-or-dir> [<file-or-dir> ...]");
        return ExitCode::from(2);
    }

    let mut sources: Vec<(PathBuf, String)> = Vec::new();
    for arg in &args[1..] {
        let p = Path::new(arg);
        if p.is_dir() {
            let mut entries: Vec<PathBuf> = match fs::read_dir(p) {
                Ok(rd) => rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "sp"))
                    .collect(),
                Err(e) => {
                    eprintln!("error: cannot read dir {}: {e}", p.display());
                    return ExitCode::from(1);
                }
            };
            entries.sort();
            for entry in entries {
                let Ok(src) = fs::read_to_string(&entry) else {
                    eprintln!("error: cannot read {}", entry.display());
                    return ExitCode::from(1);
                };
                sources.push((entry, src));
            }
        } else {
            let Ok(src) = fs::read_to_string(p) else {
                eprintln!("error: cannot read {}", p.display());
                return ExitCode::from(1);
            };
            sources.push((p.to_path_buf(), src));
        }
    }

    let mut modules: Vec<AstModule> = Vec::new();
    for (path, src) in &sources {
        match parse(src) {
            Ok(m) => modules.push(m),
            Err(e) => {
                eprintln!("parse error in {}: {e}", path.display());
                return ExitCode::from(1);
            }
        }
    }

    let program = match resolve_program(modules) {
        Ok(p) => p,
        Err(errs) => {
            for e in errs {
                eprintln!("{e}");
            }
            return ExitCode::from(1);
        }
    };

    match check_program(&program) {
        Ok(tp) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if let Err(e) = dump(&tp, &mut out) {
                eprintln!("write error: {e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(errs) => {
            for e in errs {
                eprintln!("{e}");
            }
            ExitCode::from(1)
        }
    }
}

fn dump<W: Write>(tp: &TypedProgram, out: &mut W) -> io::Result<()> {
    writeln!(out, "TypedProgram ({} modules)", tp.modules.len())?;
    for tm in &tp.modules {
        writeln!(out)?;
        writeln!(out, "== module {} ==", tm.id)?;
        for (name, scheme) in &tm.schemes {
            writeln!(out, "  {name} : {}", scheme.pretty())?;
        }
    }
    Ok(())
}
