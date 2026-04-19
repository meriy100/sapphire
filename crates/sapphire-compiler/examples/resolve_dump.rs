//! Minimal CLI that reads one or more Sapphire source files, parses
//! them, runs the I5 name resolver, and dumps the resolution result
//! in a compact plain-text form.
//!
//! Usage:
//!
//! ```ignore
//! # Resolve a single module (single-file script / `module Main`):
//! cargo run -p sapphire-compiler --example resolve_dump -- \
//!     examples/sources/01-hello-ruby/Main.sp
//!
//! # Resolve a multi-module program by pointing at a directory or
//! # passing multiple `.sp` files explicitly:
//! cargo run -p sapphire-compiler --example resolve_dump -- \
//!     examples/sources/04-fetch-summarise/
//! ```
//!
//! The output lists every module in order with its top-level
//! definitions (marked `pub` / `priv`), the import table that resulted
//! from explicit and implicit-prelude imports, and every resolved
//! reference site (`loc -> origin`). This is the input from which
//! `examples/resolve-snapshot/` is generated.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sapphire_compiler::parser::parse;
use sapphire_compiler::resolver::{
    DefKind, Namespace, Resolution, ResolvedModule, ResolvedProgram, TopLevelDef, Visibility,
    resolve_program,
};
use sapphire_core::ast::Module as AstModule;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: resolve_dump <file-or-dir> [<file-or-dir> ...]");
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

    match resolve_program(modules) {
        Ok(program) => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if let Err(e) = dump(&program, &mut out) {
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

fn dump<W: Write>(program: &ResolvedProgram, out: &mut W) -> io::Result<()> {
    writeln!(out, "ResolvedProgram ({} modules)", program.modules.len())?;
    for rm in &program.modules {
        dump_module(rm, out)?;
    }
    Ok(())
}

fn dump_module<W: Write>(rm: &ResolvedModule, out: &mut W) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "== module {} ==", rm.id.display())?;

    writeln!(out, "  top-level:")?;
    let mut defs: Vec<&TopLevelDef> = rm.env.top_level.iter().collect();
    defs.sort_by(|a, b| a.name.cmp(&b.name).then(a.namespace.cmp(&b.namespace)));
    for def in defs {
        writeln!(
            out,
            "    {:<4} {:<6} {}  [{}]",
            vis_label(def.visibility),
            ns_label(def.namespace),
            def.name,
            kind_label(&def.kind),
        )?;
    }

    writeln!(out, "  exports:")?;
    let mut exp_v: Vec<_> = rm.env.exports.values.iter().collect();
    exp_v.sort_by(|a, b| a.0.cmp(b.0));
    for (name, r) in exp_v {
        writeln!(
            out,
            "    value {}  -> {}.{}",
            name,
            r.module.display(),
            r.name
        )?;
    }
    let mut exp_t: Vec<_> = rm.env.exports.types.iter().collect();
    exp_t.sort_by(|a, b| a.0.cmp(b.0));
    for (name, r) in exp_t {
        writeln!(
            out,
            "    type  {}  -> {}.{}",
            name,
            r.module.display(),
            r.name
        )?;
    }

    writeln!(out, "  unqualified-scope:")?;
    let mut unq: Vec<_> = rm.env.unqualified.iter().collect();
    unq.sort_by(|a, b| a.0.0.cmp(&b.0.0).then(a.0.1.cmp(&b.0.1)));
    for ((name, ns), rs) in unq {
        for r in rs {
            writeln!(
                out,
                "    {} {}  <- {}.{}",
                ns_label(*ns),
                name,
                r.module.display(),
                r.name
            )?;
        }
    }

    writeln!(out, "  qualifiers:")?;
    let mut q: Vec<_> = rm.env.qualified_aliases.iter().collect();
    q.sort_by(|a, b| a.0.cmp(b.0));
    for (alias, target) in q {
        writeln!(out, "    {}  -> {}", alias, target.display())?;
    }

    writeln!(out, "  references:")?;
    // Keyed by span for stable ordering across runs.
    let mut refs: BTreeMap<(usize, usize), &Resolution> = BTreeMap::new();
    for (span, res) in &rm.references {
        refs.insert((span.start, span.end), res);
    }
    for ((s, e), res) in &refs {
        match res {
            Resolution::Local { name } => {
                writeln!(out, "    {}..{}  local  {}", s, e, name)?;
            }
            Resolution::Global(r) => {
                writeln!(
                    out,
                    "    {}..{}  global {}.{} [{}]",
                    s,
                    e,
                    r.module.display(),
                    r.name,
                    ns_label(r.namespace),
                )?;
            }
        }
    }

    Ok(())
}

fn vis_label(v: Visibility) -> &'static str {
    match v {
        Visibility::Exported => "pub",
        Visibility::Private => "priv",
    }
}

fn ns_label(ns: Namespace) -> &'static str {
    match ns {
        Namespace::Value => "value",
        Namespace::Type => "type",
    }
}

fn kind_label(kind: &DefKind) -> &'static str {
    match kind {
        DefKind::Value => "value",
        DefKind::Ctor { .. } => "ctor",
        DefKind::ClassMethod { .. } => "method",
        DefKind::RubyEmbed => "ruby",
        DefKind::DataType => "data",
        DefKind::TypeAlias => "alias",
        DefKind::Class => "class",
    }
}
