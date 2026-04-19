//! Minimal CLI that reads a Sapphire source file and dumps the
//! token stream produced by the I3 lexer.
//!
//! Usage:
//!     cargo run -p sapphire-compiler --example lex_dump -- \
//!         examples/lexer-snapshot/hello.sp
//!
//! Output format: one token per line,
//!
//!     <start>..<end>  <TokenKind>
//!
//! where offsets are byte-indices into the input. This is intended
//! as a sanity-check binary (matches `examples/lexer-snapshot/
//! hello.tokens.txt`), not a polished tool.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::ExitCode;

use sapphire_compiler::lexer::tokenize;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: lex_dump <path.sp>");
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
    match tokenize(&source) {
        Ok(tokens) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            for tok in &tokens {
                // Unwrap: writing to a lock never fails except on
                // closed stdout, which is an outer runtime concern.
                let _ = writeln!(
                    out,
                    "{:>4}..{:<4} {}",
                    tok.span.start, tok.span.end, tok.kind
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("lex error: {err}");
            ExitCode::from(1)
        }
    }
}
