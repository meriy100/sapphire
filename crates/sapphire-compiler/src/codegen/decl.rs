//! Top-level declaration rendering.
//!
//! Emits a complete Ruby source file for one Sapphire module: the
//! `module Sapphire; class Leaf` namespace wrapping, the `require`
//! preamble, every value binding / data declaration translated to
//! Ruby, and — for modules that export a `main : Ruby {}` — a
//! `self.run_main` entry helper per
//! `docs/impl/26-codegen-effect-monad.md`.

use std::collections::HashMap;

use sapphire_core::ast::{
    DataCtor, DataDecl, Decl, Module as AstModule, Pattern, RubyEmbedDecl, Scheme as AstScheme,
    Type as AstType, ValueClause,
};

use crate::resolver::ResolvedModule;
use crate::typeck::{Scheme as TyScheme, Ty, TypedModule};

use super::emit::{Buf, escape_ruby_string, value_ident};
use super::expr::{ExprCtx, render_expr};
use super::{RUNTIME_VERSION_CONSTRAINT, SAPPHIRE_COMPILER_VERSION, to_snake_case};

/// Render one complete module file.
pub fn render_module(
    ast: &AstModule,
    resolved: &ResolvedModule,
    typed: Option<&TypedModule>,
) -> String {
    let mut buf = Buf::new();

    // Header banner — fixed by build 02 §File-content shape. The
    // provenance line must identify the source module, the compiler
    // version that produced the file, and the runtime-gem constraint
    // the generated code relies on, so that `require_version!` below
    // can fail loudly on a mismatched runtime.
    let source_label = match &ast.header {
        Some(h) => h.name.segments.join("."),
        None => "Main".into(),
    };
    buf.push_line("# frozen_string_literal: true");
    buf.push_line("#");
    buf.push_line(&format!(
        "# Generated from module {source_label} by sapphire-compiler \
         {SAPPHIRE_COMPILER_VERSION}."
    ));
    buf.push_line(&format!(
        "# Targets sapphire-runtime {RUNTIME_VERSION_CONSTRAINT}. Do not edit by hand."
    ));
    buf.push_line("# See docs/build/02-source-and-output-layout.md for the output contract.");
    buf.blank();

    // Requires, per build 02 §File-content shape:
    //
    // 1. The runtime gem's single-file entry. Generated code must
    //    start with `require 'sapphire/runtime'` so that even without
    //    the prelude (e.g. tests that poke at generated modules
    //    directly) the runtime surface is available.
    // 2. The generated prelude (spec 09), which itself `require`s the
    //    runtime but is emitted on every build, so both requires are
    //    safe together under Ruby's load-once semantics.
    // 3. Cross-module requires for every imported module.
    buf.push_line("require 'sapphire/runtime'");
    buf.push_line("require 'sapphire/prelude'");
    for imp in &ast.imports {
        if imp.name.segments.first().map(|s| s.as_str()) == Some("Prelude") {
            continue;
        }
        let path = imp
            .name
            .segments
            .iter()
            .map(|s| to_snake_case(s))
            .collect::<Vec<_>>()
            .join("/");
        buf.push_line(&format!("require 'sapphire/{path}'"));
    }
    buf.blank();

    // Runtime version guard, per build 02 §File-content shape and
    // `docs/impl/16-runtime-threaded-loading.md` §R6 loading 契約.
    // Raises `Sapphire::Runtime::Errors::RuntimeVersionMismatch` when
    // the loaded gem does not satisfy `RUNTIME_VERSION_CONSTRAINT`.
    buf.push_line(&format!(
        "Sapphire::Runtime.require_version!('{RUNTIME_VERSION_CONSTRAINT}')"
    ));
    buf.blank();

    // Namespace walk: module Sapphire; module Foo; class Bar; ...; end; end; end
    let segments: Vec<String> = match &ast.header {
        Some(h) => h.name.segments.clone(),
        None => vec!["Main".into()],
    };

    buf.push_line("module Sapphire");
    buf.indent();
    // All segments except the leaf become `module` wrappers; the leaf
    // becomes a `class`.
    let (leaf, wrappers) = segments.split_last().expect("non-empty module segments");
    for w in wrappers {
        buf.push_line(&format!("module {w}"));
        buf.indent();
    }
    buf.push_line(&format!("class {leaf}"));
    buf.indent();

    // Data declarations first (so value bindings can reference the
    // installed constructors).
    for decl in &ast.decls {
        if let Decl::Data(d) = decl {
            render_data_decl(d, &mut buf);
        }
    }

    // Group ValueClauses by name so multi-clause functions emit as a
    // single Ruby method.
    let grouped = group_value_clauses(&ast.decls);

    // Emit each signature-aware binding.
    for binding in &grouped {
        render_binding(binding, resolved, typed, &mut buf);
    }

    // Ruby-embedded (`:=`) bindings.
    for decl in &ast.decls {
        if let Decl::RubyEmbed(re) = decl {
            render_ruby_embed(re, &ast.decls, typed, &mut buf);
        }
    }

    // Emit `run_main` helper if this module exports a `main : Ruby τ`.
    if has_ruby_main(ast, typed) {
        buf.blank();
        render_run_main(&mut buf);
    }

    // Close class + module wrappers.
    buf.dedent();
    buf.push_line("end"); // class
    for _ in wrappers {
        buf.dedent();
        buf.push_line("end"); // module
    }
    buf.dedent();
    buf.push_line("end"); // Sapphire

    buf.into_string()
}

/// One grouped binding that the emitter treats as a single Ruby
/// method. Signatures are collected alongside so we can inspect the
/// return-type head for `pure` specialisation.
#[derive(Debug)]
struct Binding<'a> {
    name: String,
    signature: Option<&'a AstScheme>,
    clauses: Vec<&'a ValueClause>,
}

fn group_value_clauses(decls: &[Decl]) -> Vec<Binding<'_>> {
    let mut out: Vec<Binding> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for decl in decls {
        match decl {
            Decl::Signature { name, scheme, .. } => {
                if let Some(&idx) = index.get(name) {
                    out[idx].signature = Some(scheme);
                } else {
                    index.insert(name.clone(), out.len());
                    out.push(Binding {
                        name: name.clone(),
                        signature: Some(scheme),
                        clauses: Vec::new(),
                    });
                }
            }
            Decl::Value(clause) => {
                if let Some(&idx) = index.get(&clause.name) {
                    out[idx].clauses.push(clause);
                } else {
                    index.insert(clause.name.clone(), out.len());
                    out.push(Binding {
                        name: clause.name.clone(),
                        signature: None,
                        clauses: vec![clause],
                    });
                }
            }
            _ => {}
        }
    }
    // Discard bindings that have only a signature and no clauses
    // (nothing to emit).
    out.retain(|b| !b.clauses.is_empty());
    out
}

fn render_binding(
    binding: &Binding,
    resolved: &ResolvedModule,
    typed: Option<&TypedModule>,
    buf: &mut Buf,
) {
    let return_head = binding
        .signature
        .and_then(|s| head_con_of_scheme_ast(&s.body))
        .or_else(|| typed_return_head(binding.name.as_str(), typed).map(|s| s.to_string()));

    let ctx = ExprCtx::new(resolved, return_head.as_deref());
    // Expose no locals: top-level scope.
    let body = build_body_expr(&binding.clauses);
    // Compute parameter count from the first clause (all clauses must
    // have the same arity per Sapphire's semantics).
    let arity = binding.clauses[0].params.len();

    buf.blank();
    buf.push_line(&format!("def self.{}", value_ident(&binding.name)));
    buf.indent();
    if arity == 0 {
        let body_src = render_expr(&body, &ctx);
        buf.push_line(&body_src);
    } else {
        // Emit curried lambdas wrapping a case-matches body.
        let params: Vec<String> = (0..arity).map(|i| format!("_arg{i}")).collect();
        let mut new_locals: Vec<String> = Vec::new();
        for p in &params {
            new_locals.push(p.clone());
        }
        let inner_ctx = ctx.with_locals_slice(&new_locals);
        let body_src = render_multi_clause_body(&params, &binding.clauses, &inner_ctx);
        // Build `->(_arg0) { ->(_arg1) { body_src } }`.
        let mut lam = body_src;
        for p in params.iter().rev() {
            lam = format!("->({p}) {{ {lam} }}");
        }
        buf.push_line(&lam);
        // keep `ctx` alive for linting silence
        let _ = &ctx;
    }
    buf.dedent();
    buf.push_line("end");
}

/// For single-clause bindings, the body is simply the clause body;
/// for multi-clause we synthesize a `case` over the parameters later.
fn build_body_expr(clauses: &[&ValueClause]) -> sapphire_core::ast::Expr {
    // Placeholder; only used when arity == 0 (no params).
    clauses[0].body.clone()
}

fn render_multi_clause_body(params: &[String], clauses: &[&ValueClause], ctx: &ExprCtx) -> String {
    if clauses.len() == 1 {
        // Single clause: introduce the params via case/in if any
        // clause uses non-trivial patterns, otherwise just rename.
        let c = clauses[0];
        return render_single_clause(params, c, ctx);
    }

    // Multi-clause: rebuild as a case on an array of params.
    // case [_arg0, _arg1, ...]
    //   in [pat00, pat01, ...]
    //     body0
    //   in [pat10, pat11, ...]
    //     body1
    let mut out = String::new();
    out.push_str("(case [");
    out.push_str(&params.join(", "));
    out.push(']');
    for c in clauses {
        let pats: Vec<String> = c
            .params
            .iter()
            .map(super::pattern::render_pattern)
            .collect();
        let binders = c
            .params
            .iter()
            .flat_map(collect_pattern_binders)
            .collect::<Vec<_>>();
        let inner_ctx = ctx.with_locals_slice(&binders);
        let body_src = render_expr(&c.body, &inner_ctx);
        out.push_str("; in [");
        out.push_str(&pats.join(", "));
        out.push_str("]; (");
        out.push_str(&body_src);
        out.push(')');
    }
    out.push_str("; else; raise 'non-exhaustive function clauses'");
    out.push_str("; end)");
    out
}

fn render_single_clause(params: &[String], clause: &ValueClause, ctx: &ExprCtx) -> String {
    // All params plain Var or Wildcard: rename by emitting a leading
    // `_arg0 => name` destructure or alias.
    let all_simple = clause
        .params
        .iter()
        .all(|p| matches!(p, Pattern::Var { .. } | Pattern::Wildcard(_)));
    if all_simple {
        let mut binders: Vec<String> = Vec::new();
        let mut alias_prelude = String::new();
        for (i, p) in clause.params.iter().enumerate() {
            if let Pattern::Var { name, .. } = p {
                alias_prelude.push_str(&format!("{name} = {}; ", params[i]));
                binders.push(name.clone());
            }
        }
        let inner_ctx = ctx.with_locals_slice(&binders);
        let body_src = render_expr(&clause.body, &inner_ctx);
        return format!("({alias_prelude}{body_src})");
    }

    // At least one non-trivial pattern: wrap with case/in.
    let pats: Vec<String> = clause
        .params
        .iter()
        .map(super::pattern::render_pattern)
        .collect();
    let binders: Vec<String> = clause
        .params
        .iter()
        .flat_map(collect_pattern_binders)
        .collect();
    let inner_ctx = ctx.with_locals_slice(&binders);
    let body_src = render_expr(&clause.body, &inner_ctx);
    format!(
        "(case [{}]; in [{}]; ({}); else; raise 'non-exhaustive function clause'; end)",
        params.join(", "),
        pats.join(", "),
        body_src
    )
}

fn collect_pattern_binders(p: &Pattern) -> Vec<String> {
    let mut out = Vec::new();
    collect_pattern_binders_rec(p, &mut out);
    out
}

fn collect_pattern_binders_rec(p: &Pattern, out: &mut Vec<String>) {
    match p {
        Pattern::Wildcard(_) | Pattern::Lit(_, _) => {}
        Pattern::Var { name, .. } => out.push(name.clone()),
        Pattern::As { name, inner, .. } => {
            out.push(name.clone());
            collect_pattern_binders_rec(inner, out);
        }
        Pattern::Con { args, .. } => {
            for a in args {
                collect_pattern_binders_rec(a, out);
            }
        }
        Pattern::Cons { head, tail, .. } => {
            collect_pattern_binders_rec(head, out);
            collect_pattern_binders_rec(tail, out);
        }
        Pattern::List { items, .. } => {
            for i in items {
                collect_pattern_binders_rec(i, out);
            }
        }
        Pattern::Record { fields, .. } => {
            for (_, pat) in fields {
                collect_pattern_binders_rec(pat, out);
            }
        }
        Pattern::Annot { inner, .. } => collect_pattern_binders_rec(inner, out),
    }
}

fn render_data_decl(d: &DataDecl, buf: &mut Buf) {
    // Skip Bool / Ordering / Maybe / Result / List — these are
    // provided by the generated prelude.
    if matches!(
        d.name.as_str(),
        "Bool" | "Ordering" | "Maybe" | "Result" | "List"
    ) {
        return;
    }
    buf.blank();
    let pairs: Vec<String> = d
        .ctors
        .iter()
        .map(|c: &DataCtor| format!("[:{}, {}]", c.name, c.args.len()))
        .collect();
    buf.push_line(&format!(
        "Sapphire::Runtime::ADT.define_variants(self, [{}])",
        pairs.join(", ")
    ));
}

fn render_ruby_embed(
    re: &RubyEmbedDecl,
    all_decls: &[Decl],
    typed: Option<&TypedModule>,
    buf: &mut Buf,
) {
    buf.blank();
    buf.push_line(&format!("def self.{}", value_ident(&re.name)));
    buf.indent();

    // Build curried lambda chain, innermost being the prim_embed.
    let param_names: Vec<String> = re.params.iter().map(|p| p.name.clone()).collect();

    // Detect `Ruby {}` return type so that the snippet's block
    // evaluates to a unit-compatible value (empty Hash per spec 10
    // §Records). Ruby's `puts` returns nil, which `Marshal.from_ruby`
    // rejects per 10-OQ1; a trailing `; {}` makes the common case work
    // without forcing the user to remember the marshalling contract.
    let returns_unit = ruby_embed_returns_unit(&re.name, all_decls, typed);

    let mut body = String::new();
    body.push_str("Sapphire::Runtime::Ruby.prim_embed do\n");
    // Emit snippet source indented once relative to block opener.
    let snippet = format_snippet(&re.source);
    for line in snippet.lines() {
        body.push_str("            ");
        body.push_str(line);
        body.push('\n');
    }
    if returns_unit {
        body.push_str("            {}\n");
    }
    body.push_str("          end");

    let mut lam = body;
    for p in param_names.iter().rev() {
        lam = format!("->({p}) {{\n          {lam}\n        }}");
    }
    buf.push_line(&lam);
    buf.dedent();
    buf.push_line("end");
}

/// Does the `:=` binding named `name` have an inferred return type of
/// `Ruby {}` (empty record)? Checks the explicit AST signature first,
/// then falls back to typed schemes.
fn ruby_embed_returns_unit(name: &str, all_decls: &[Decl], typed: Option<&TypedModule>) -> bool {
    for decl in all_decls {
        if let Decl::Signature {
            name: sig_name,
            scheme,
            ..
        } = decl
        {
            if sig_name == name {
                let tail = peel_fun_ast(&scheme.body);
                return is_ast_ruby_unit(tail);
            }
        }
    }
    if let Some(tm) = typed {
        if let Some((_, sch)) = tm.schemes.iter().find(|(n, _)| n == name) {
            let tail = peel_fun_ty(&sch.body);
            return is_ty_ruby_unit(tail);
        }
    }
    false
}

fn peel_fun_ast(t: &AstType) -> &AstType {
    let mut cur = t;
    while let AstType::Fun { result, .. } = cur {
        cur = result;
    }
    cur
}

fn is_ast_ruby_unit(t: &AstType) -> bool {
    if let AstType::App { func, arg, .. } = t {
        if let AstType::Con { name, .. } = &**func {
            if name == "Ruby" {
                return matches!(&**arg, AstType::Record { fields, .. } if fields.is_empty());
            }
        }
    }
    false
}

fn peel_fun_ty(t: &Ty) -> &Ty {
    let mut cur = t;
    while let Ty::Fun(_, r) = cur {
        cur = r;
    }
    cur
}

fn is_ty_ruby_unit(t: &Ty) -> bool {
    if let Ty::App(func, arg) = t {
        if let Ty::Con { name, .. } = &**func {
            if name == "Ruby" {
                return matches!(&**arg, Ty::Record(fs) if fs.is_empty());
            }
        }
    }
    false
}

fn format_snippet(src: &str) -> String {
    // Trim surrounding blank lines; leave inner indentation alone
    // (Ruby handles any indentation fine inside the block body).
    let trimmed = src.trim_matches(|c: char| c == '\n' || c == '\r');
    // Find the minimum non-blank indentation and strip it.
    let min_indent = trimmed
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out = String::new();
    for (i, line) in trimmed.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.len() >= min_indent {
            out.push_str(&line[min_indent..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

fn has_ruby_main(ast: &AstModule, typed: Option<&TypedModule>) -> bool {
    // Look for a `main` binding whose signature's return type has a
    // `Ruby` head. Use the AST signature first (authoritative), then
    // fall back to typed schemes.
    for decl in &ast.decls {
        if let Decl::Signature { name, scheme, .. } = decl {
            if name == "main" {
                return head_con_of_scheme_ast(&scheme.body).as_deref() == Some("Ruby");
            }
        }
    }
    if let Some(tm) = typed {
        if let Some((_, sch)) = tm.schemes.iter().find(|(n, _)| n == "main") {
            return head_con_of_ty(&sch.body) == Some("Ruby");
        }
    }
    false
}

fn render_run_main(buf: &mut Buf) {
    buf.push_line("# Entry helper — `sapphire run` dispatches here.");
    buf.push_line("def self.run_main");
    buf.indent();
    buf.push_line("result = Sapphire::Prelude.run_action(main)");
    buf.push_line("case result[:tag]");
    buf.push_line("when :Ok");
    buf.indent();
    buf.push_line("0");
    buf.dedent();
    buf.push_line("when :Err");
    buf.indent();
    buf.push_line("err = result[:values][0]");
    buf.push_line("klass = err[:values][0]");
    buf.push_line("msg   = err[:values][1]");
    buf.push_line("bt    = err[:values][2]");
    buf.push_line("warn \"[sapphire run] #{klass}: #{msg}\"");
    buf.push_line("bt.each { |line| warn \"  #{line}\" }");
    buf.push_line("1");
    buf.dedent();
    buf.push_line("end");
    buf.dedent();
    buf.push_line("end");
}

fn head_con_of_scheme_ast(ty: &AstType) -> Option<String> {
    // Walk a curried `τ₁ -> τ₂ -> ... -> τ`, peel off arrows, then
    // extract the head of the final τ.
    let mut cur = ty;
    while let AstType::Fun { result, .. } = cur {
        cur = result;
    }
    head_con_of_ast_type(cur)
}

fn head_con_of_ast_type(ty: &AstType) -> Option<String> {
    match ty {
        AstType::Con { name, .. } => Some(name.clone()),
        AstType::App { func, .. } => head_con_of_ast_type(func),
        _ => None,
    }
}

fn head_con_of_ty(ty: &Ty) -> Option<&str> {
    ty.head_con()
}

fn typed_return_head<'a>(name: &str, typed: Option<&'a TypedModule>) -> Option<&'a str> {
    let tm = typed?;
    let sch = tm.schemes.iter().find(|(n, _)| n == name)?;
    let TyScheme { body, .. } = &sch.1;
    let mut cur = body;
    while let Ty::Fun(_, r) = cur {
        cur = r;
    }
    cur.head_con()
}

// Keep the unused-escape warning quiet when not used.
#[allow(dead_code)]
fn _tmp(s: &str) -> String {
    escape_ruby_string(s)
}
