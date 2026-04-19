//! Minimal CLI that reads a Sapphire source file and dumps the AST
//! produced by the I4 parser (`lex → layout → parse`).
//!
//! Usage:
//!     cargo run -p sapphire-compiler --example parse_dump -- \
//!         examples/sources/01-hello-ruby/Main.sp
//!
//! Output is the `Debug`-pretty-printed AST, one node per line with
//! `{:#?}` indentation. Intended as a sanity-check binary and as a
//! source for the snapshots under `examples/parse-snapshot/`.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use sapphire_compiler::parser::parse;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: parse_dump <path.sp>");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {path}: {e}");
            return ExitCode::from(1);
        }
    };
    match parse(&source) {
        Ok(module) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            let _ = writeln!(out, "{module:#?}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("parse error: {err}");
            ExitCode::from(1)
        }
    }
}
