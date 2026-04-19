//! Codegen CLI helper: read one or more `.sp` files, run the full
//! front-end, and dump the generated Ruby to stdout (or a directory).
//!
//! Invocation:
//!
//! ```ignore
//! cargo run -p sapphire-compiler --example codegen_dump -- <file-or-dir> [...]
//! cargo run -p sapphire-compiler --example codegen_dump -- <file-or-dir> --out-dir <dir>
//! ```

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sapphire_compiler::codegen::generate;
use sapphire_compiler::parser::parse;
use sapphire_compiler::resolver::resolve_program;
use sapphire_compiler::typeck::check_program;
use sapphire_core::ast::Module as AstModule;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: codegen_dump <file-or-dir> [--out-dir <dir>]");
        return ExitCode::from(2);
    }

    let mut paths: Vec<String> = Vec::new();
    let mut out_dir: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--out-dir" {
            i += 1;
            if i >= args.len() {
                eprintln!("--out-dir requires an argument");
                return ExitCode::from(2);
            }
            out_dir = Some(args[i].clone());
        } else {
            paths.push(a.clone());
        }
        i += 1;
    }

    let mut sources: Vec<(PathBuf, String)> = Vec::new();
    for arg in &paths {
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

    let resolved = match resolve_program(modules) {
        Ok(p) => p,
        Err(errs) => {
            for e in errs {
                eprintln!("{e}");
            }
            return ExitCode::from(1);
        }
    };

    let typed = match check_program(&resolved) {
        Ok(t) => t,
        Err(errs) => {
            for e in errs {
                eprintln!("{e}");
            }
            return ExitCode::from(1);
        }
    };

    let program = generate(&resolved, &typed);
    match out_dir {
        Some(dir) => {
            let root = Path::new(&dir);
            for f in &program.files {
                let path = root.join(&f.path);
                if let Some(parent) = path.parent() {
                    if let Err(e) = fs::create_dir_all(parent) {
                        eprintln!("error: mkdir {}: {e}", parent.display());
                        return ExitCode::from(1);
                    }
                }
                if let Err(e) = fs::write(&path, &f.content) {
                    eprintln!("error: write {}: {e}", path.display());
                    return ExitCode::from(1);
                }
            }
        }
        None => {
            for f in &program.files {
                println!("== {} ==", f.path);
                println!("{}", f.content);
            }
        }
    }
    ExitCode::SUCCESS
}
