//! L5 goto-definition: translate a cursor position into the
//! definition site of the identifier it rests on.
//!
//! The handler receives a `(uri, position)` pair from the LSP client.
//! To answer it we need three ingredients:
//!
//! 1. A parsed [`sapphire_core::ast::Module`] so we can walk the
//!    syntactic tree to find the binding of a local name when
//!    necessary.
//! 2. A [`sapphire_compiler::resolver::ResolvedModule`] so we can
//!    consult the `references: HashMap<Span, Resolution>` side table
//!    the I5 pass populates. The table carries, for every reference
//!    site, whether the name resolves to a top-level definition in
//!    this module, a top-level import, or a local binding.
//! 3. A [`crate::diagnostics::LineMap`] so we can go back and forth
//!    between LSP's UTF-16 `Position` and the byte offsets Sapphire
//!    spans use.
//!
//! ## Scope (L5)
//!
//! - **Same-file only.** If the reference resolves to a definition
//!   living in another module we return `None`. Multi-file document
//!   management is a later milestone (tracked as I-OQ72).
//! - **Prelude references are not followed.** The prelude is baked
//!   into the resolver as a static table (`resolver/prelude.rs`) and
//!   has no source file the LSP could jump to. Flagged under I-OQ73.
//! - **Locals are handled via a second AST walk.** The I5 side table
//!   records `Resolution::Local { name }` without a binding span, so
//!   we re-walk the AST to find the innermost binder for the matching
//!   name whose scope encloses the reference site.
//! - **First-match wins on span keys.** `Span` collisions in the
//!   side table (I-OQ43) are possible in pathological inputs (e.g.
//!   degenerate `BinOp` trees); for goto we pick the span that
//!   *most tightly* contains the requested offset, which is as much
//!   as the current key shape allows.
//!
//! See `docs/impl/22-lsp-goto-definition.md` for the design notes.

use std::collections::HashMap;

use sapphire_compiler::resolver::{DefKind, ModuleEnv, Namespace, Resolution, ResolvedModule};
use sapphire_core::ast::{
    CaseArm, ClassDecl, ClassItem, DataDecl, Decl, DoStmt, Expr, InstanceDecl, Module as AstModule,
    Param, Pattern, RubyEmbedDecl, Scheme, TypeAlias, ValueClause,
};
use sapphire_core::span::Span;
use tower_lsp::lsp_types::Range;

use crate::diagnostics::LineMap;

/// Given a byte offset into the module's source, return the LSP
/// [`Range`] that points at the definition site of the identifier at
/// that offset, if one can be located within the same source file.
///
/// Returns `None` when any of the following hold:
///
/// - `byte_offset` does not fall inside any recorded reference span.
/// - The reference resolves to a definition in another module.
/// - The reference resolves to a built-in / prelude name without an
///   in-file definition.
/// - The reference is a local name whose binder cannot be located by
///   the AST walk (should not happen for well-typed input, but kept
///   defensive for partial resolver output).
pub fn find_definition(
    module: &AstModule,
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    line_map: &LineMap<'_>,
) -> Option<Range> {
    let span = find_reference_span(&resolved.references, byte_offset)?;
    let resolution = resolved.references.get(&span)?;
    let def_span = resolve_definition_span(module, &resolved.env, source, span, resolution)?;
    Some(line_map.range(def_span))
}

/// Find the reference-site span most tightly containing
/// `byte_offset` among the recorded reference keys.
///
/// "Tightly containing" is defined as: `start <= byte_offset < end`
/// (zero-width spans also qualify when `start == byte_offset`), and
/// ties are broken by choosing the span with the smallest
/// `(end - start)`. This matters for `Expr::BinOp`, whose span covers
/// `left.merge(right)`: the cursor at the operator glyph lands inside
/// both the `BinOp` and the enclosing `App` if any, and we want the
/// innermost — which is also what the I5 side table stores for the
/// operator itself.
fn find_reference_span(references: &HashMap<Span, Resolution>, byte_offset: usize) -> Option<Span> {
    let mut best: Option<Span> = None;
    for &span in references.keys() {
        if span_contains(span, byte_offset) {
            match best {
                None => best = Some(span),
                Some(b) if span_width(span) < span_width(b) => best = Some(span),
                _ => {}
            }
        }
    }
    best
}

fn span_contains(span: Span, byte: usize) -> bool {
    if span.start == span.end {
        span.start == byte
    } else {
        byte >= span.start && byte < span.end
    }
}

fn span_width(span: Span) -> usize {
    span.end.saturating_sub(span.start)
}

/// Translate a [`Resolution`] at `ref_span` into the span of the
/// definition it points at, if that definition lives in this module.
fn resolve_definition_span(
    module: &AstModule,
    env: &ModuleEnv,
    source: &str,
    ref_span: Span,
    resolution: &Resolution,
) -> Option<Span> {
    match resolution {
        Resolution::Local { name } => find_local_binding(module, source, name, ref_span),
        Resolution::Global(r) => {
            if r.module != env.id {
                // Cross-module — out of scope for L5 (I-OQ72).
                return None;
            }
            let def = env.top_lookup(&r.name, r.namespace)?;
            Some(definition_name_span(
                module,
                def.namespace,
                &def.name,
                &def.kind,
                def.span,
            ))
        }
    }
}

/// For a top-level definition, the resolver records `span` as the
/// full header span (e.g. `name_span..scheme.span` for a signature).
/// LSP clients render the "go to definition" target by highlighting
/// the returned range, so a tighter span — ideally just the name —
/// gives a better experience. Walk the AST once to pick the name's
/// span out of the declaration header. Falls back to the recorded
/// header span when we can't narrow.
fn definition_name_span(
    module: &AstModule,
    ns: Namespace,
    name: &str,
    kind: &DefKind,
    header_span: Span,
) -> Span {
    // We search linearly because the module's top-level list is short
    // (tens of entries at M9 scale). Returning the header span is a
    // correct fallback if the walk misses.
    for decl in &module.decls {
        match decl {
            Decl::Signature {
                name: n,
                scheme,
                span,
                ..
            } => {
                if ns == Namespace::Value && n == name && *span == header_span {
                    // The recorded span is `name_span..scheme.span`;
                    // carve out just the name prefix.
                    let name_end = span.start + n.len();
                    return Span::new(span.start, name_end.min(scheme.span.start));
                }
            }
            Decl::Value(ValueClause {
                name: n,
                span,
                body,
                ..
            }) => {
                if ns == Namespace::Value && n == name && *span == header_span {
                    let name_end = span.start + n.len();
                    return Span::new(span.start, name_end.min(body.span().start));
                }
            }
            Decl::Data(DataDecl {
                name: n,
                ctors,
                span,
                ..
            }) => {
                if ns == Namespace::Type && n == name && *span == header_span {
                    // `span.start` is the `data` keyword position; the
                    // type name starts after `data ` (one keyword + a
                    // single space).
                    let data_kw = "data ";
                    let name_start = span.start + data_kw.len();
                    let name_end = name_start + n.len();
                    return Span::new(name_start, name_end);
                }
                if matches!(kind, DefKind::Ctor { .. }) && ns == Namespace::Value {
                    for c in ctors {
                        if c.name == name {
                            // A ctor's span is
                            // `name_span..last_arg.span`; slice out the
                            // name prefix.
                            let name_end = c.span.start + c.name.len();
                            return Span::new(c.span.start, name_end.min(c.span.end));
                        }
                    }
                }
            }
            Decl::TypeAlias(TypeAlias { name: n, span, .. }) => {
                if ns == Namespace::Type && n == name && *span == header_span {
                    let ty_kw = "type ";
                    let name_start = span.start + ty_kw.len();
                    let name_end = name_start + n.len();
                    return Span::new(name_start, name_end);
                }
            }
            Decl::Class(ClassDecl {
                name: n,
                items,
                span,
                ..
            }) => {
                if ns == Namespace::Type && n == name && *span == header_span {
                    // `class [Ctx =>] Name tvar where ...`. The class
                    // keyword and context width vary, so we return the
                    // header span as-is rather than guessing.
                    return *span;
                }
                if matches!(kind, DefKind::ClassMethod { .. }) && ns == Namespace::Value {
                    for it in items {
                        match it {
                            ClassItem::Signature {
                                name: mn,
                                span: mspan,
                                scheme,
                                ..
                            } => {
                                if mn == name {
                                    let name_end = mspan.start + mn.len();
                                    return Span::new(mspan.start, name_end.min(scheme.span.start));
                                }
                            }
                            ClassItem::Default(ValueClause {
                                name: mn,
                                span: mspan,
                                body,
                                ..
                            }) => {
                                if mn == name {
                                    let name_end = mspan.start + mn.len();
                                    return Span::new(mspan.start, name_end.min(body.span().start));
                                }
                            }
                        }
                    }
                }
            }
            Decl::RubyEmbed(RubyEmbedDecl { name: n, span, .. }) => {
                if ns == Namespace::Value && n == name && *span == header_span {
                    let name_end = span.start + n.len();
                    return Span::new(span.start, name_end.min(span.end));
                }
            }
            Decl::Instance(_) => {}
        }
    }
    header_span
}

// ---------------------------------------------------------------------
//  Local-binding lookup
// ---------------------------------------------------------------------

/// Walk the AST and find the innermost binding of `name` whose scope
/// contains `ref_span`. Returns the span of the binding occurrence
/// (lambda parameter, let name, pattern variable, do-bind pattern, or
/// function clause parameter).
///
/// The traversal mirrors the scope rules the resolver applies in
/// `Walker::walk_expr` / `walk_value_clause`. We do not need full
/// resolver correctness here — only to locate a binding the resolver
/// already proved exists, so the outer resolver guarantees the walk
/// will find at least one match for a `Resolution::Local`.
fn find_local_binding(
    module: &AstModule,
    source: &str,
    name: &str,
    ref_span: Span,
) -> Option<Span> {
    let mut finder = LocalFinder {
        source,
        name,
        ref_span,
        best: None,
    };
    for decl in &module.decls {
        finder.visit_decl(decl);
    }
    finder.best
}

struct LocalFinder<'a> {
    source: &'a str,
    name: &'a str,
    ref_span: Span,
    /// Best match so far: the binding whose own span lies nearest to
    /// the reference (largest start offset that is still ≤ ref_span's
    /// start).
    best: Option<Span>,
}

impl LocalFinder<'_> {
    fn record(&mut self, binding_span: Span) {
        // A valid candidate binds before the reference uses it.
        if binding_span.start > self.ref_span.start {
            return;
        }
        match self.best {
            None => self.best = Some(binding_span),
            Some(prev) if binding_span.start > prev.start => self.best = Some(binding_span),
            _ => {}
        }
    }

    fn pattern_binds(&mut self, pat: &Pattern, scope_end: usize) {
        // Only bindings whose scope extends to the reference site count.
        // For a pattern in a `\x -> body`, the scope is the lambda body;
        // the caller is responsible for only invoking `pattern_binds`
        // when `ref_span.start` is inside that body's span. We re-check
        // here via `scope_end` so nested patterns with overlapping
        // scopes do the right thing.
        if self.ref_span.start >= scope_end {
            return;
        }
        match pat {
            Pattern::Wildcard(_) | Pattern::Lit(_, _) => {}
            Pattern::Var { name, span } => {
                if name == self.name {
                    self.record(*span);
                }
            }
            Pattern::As { name, inner, span } => {
                if name == self.name {
                    self.record(*span);
                }
                self.pattern_binds(inner, scope_end);
            }
            Pattern::Con { args, .. } => {
                for a in args {
                    self.pattern_binds(a, scope_end);
                }
            }
            Pattern::Cons { head, tail, .. } => {
                self.pattern_binds(head, scope_end);
                self.pattern_binds(tail, scope_end);
            }
            Pattern::List { items, .. } => {
                for p in items {
                    self.pattern_binds(p, scope_end);
                }
            }
            Pattern::Record { fields, .. } => {
                for (_, p) in fields {
                    self.pattern_binds(p, scope_end);
                }
            }
            Pattern::Annot { inner, .. } => self.pattern_binds(inner, scope_end),
        }
    }

    fn visit_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Signature { scheme, .. } => self.visit_scheme_type_vars(scheme),
            Decl::Value(clause) => self.visit_value_clause(clause),
            Decl::Data(_) | Decl::TypeAlias(_) => {}
            Decl::Class(ClassDecl { items, .. }) => {
                for it in items {
                    match it {
                        ClassItem::Signature { scheme, .. } => self.visit_scheme_type_vars(scheme),
                        ClassItem::Default(vc) => self.visit_value_clause(vc),
                    }
                }
            }
            Decl::Instance(InstanceDecl { items, .. }) => {
                for c in items {
                    self.visit_value_clause(c);
                }
            }
            Decl::RubyEmbed(RubyEmbedDecl { params, span, .. }) => {
                // Ruby-embed parameters are in scope only inside the
                // Ruby source string; we can't walk into that. Still,
                // record them so goto onto an embed parameter (if the
                // resolver ever exposes it) has a binder span.
                for p in params {
                    let Param { name, span: pspan } = p;
                    if name == self.name && span_contains(*span, self.ref_span.start) {
                        self.record(*pspan);
                    }
                }
            }
        }
    }

    fn visit_scheme_type_vars(&mut self, _scheme: &Scheme) {
        // Type variables flow through `Resolution::Local` too (see
        // `Walker::walk_type`). Locating a type variable's "binder" is
        // subtle — spec has both explicit `forall` quantifiers and
        // implicit binders. We punt on goto for type variables in L5;
        // the `Resolution::Local` path for them returns `None` via the
        // top-level `find_definition` fall-through.
    }

    fn visit_value_clause(&mut self, clause: &ValueClause) {
        let scope_end = clause.body.span().end;
        for p in &clause.params {
            self.pattern_binds(p, scope_end);
        }
        self.visit_expr(&clause.body);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Lit(_, _) | Expr::Var { .. } | Expr::OpRef { .. } => {}
            Expr::App { func, arg, .. } => {
                self.visit_expr(func);
                self.visit_expr(arg);
            }
            Expr::Lambda { params, body, span } => {
                let scope_end = span.end;
                for p in params {
                    self.pattern_binds(p, scope_end);
                }
                self.visit_expr(body);
            }
            Expr::Let {
                name,
                params,
                value,
                body,
                span,
                ..
            } => {
                // The let-bound name is in scope over both `value`
                // (recursive) and `body`.
                if name == self.name {
                    // Compute the name's span inside `span.start` —
                    // the parser records `span.start` as the byte of
                    // the `let` keyword itself, so we skip past it.
                    let name_span = locate_name_after_keyword(self.source, *span, "let", name);
                    if span_contains(*span, self.ref_span.start) {
                        self.record(name_span);
                    }
                }
                // Recurse with the full let span as the effective
                // scope for inner patterns.
                let scope_end = span.end;
                for p in params {
                    self.pattern_binds(p, scope_end);
                }
                self.visit_expr(value);
                self.visit_expr(body);
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.visit_expr(cond);
                self.visit_expr(then_branch);
                self.visit_expr(else_branch);
            }
            Expr::Case {
                scrutinee, arms, ..
            } => {
                self.visit_expr(scrutinee);
                for CaseArm { pattern, body, .. } in arms {
                    self.pattern_binds(pattern, body.span().end);
                    self.visit_expr(body);
                }
            }
            Expr::BinOp { left, right, .. } => {
                self.visit_expr(left);
                self.visit_expr(right);
            }
            Expr::Neg { value, .. } => self.visit_expr(value),
            Expr::RecordLit { fields, .. } => {
                for (_, e) in fields {
                    self.visit_expr(e);
                }
            }
            Expr::RecordUpdate { record, fields, .. } => {
                self.visit_expr(record);
                for (_, e) in fields {
                    self.visit_expr(e);
                }
            }
            Expr::FieldAccess { record, .. } => self.visit_expr(record),
            Expr::ListLit { items, .. } => {
                for e in items {
                    self.visit_expr(e);
                }
            }
            Expr::Do { stmts, span } => {
                let scope_end = span.end;
                for s in stmts {
                    match s {
                        DoStmt::Bind { pattern, expr, .. } => {
                            self.visit_expr(expr);
                            self.pattern_binds(pattern, scope_end);
                        }
                        DoStmt::Let {
                            name,
                            value,
                            span: lspan,
                            ..
                        } => {
                            if name == self.name {
                                // `do`-let statements carry the span
                                // of the whole `let name = expr`
                                // statement; there is no visible `let`
                                // keyword in the surface do-block
                                // binding shape, so we scan for the
                                // name literally.
                                let name_span = locate_name_literal(self.source, *lspan, name);
                                if span_contains(
                                    Span::new(lspan.start, scope_end),
                                    self.ref_span.start,
                                ) {
                                    self.record(name_span);
                                }
                            }
                            self.visit_expr(value);
                        }
                        DoStmt::Expr(e) => self.visit_expr(e),
                    }
                }
            }
        }
    }
}

/// Find `name` as a standalone identifier inside the source slice
/// bounded by `span`, after skipping a leading keyword. Used to turn
/// a `let`-expression span into the tighter span of the binder
/// identifier.
///
/// "Standalone" means preceded by something non-alphanumeric (a
/// space, tab, newline, or buffer boundary) and followed by the same.
/// If we can't find the identifier literally the function falls back
/// to a zero-width span at the end of the keyword, which renders as a
/// caret in most LSP clients.
fn locate_name_after_keyword(source: &str, span: Span, keyword: &str, name: &str) -> Span {
    let upper = span.end.min(source.len());
    let lower = span.start.min(upper);
    let haystack = &source[lower..upper];
    // Skip past the `let` keyword first.
    let after_kw = match haystack.find(keyword) {
        Some(i) => i + keyword.len(),
        None => return locate_name_literal(source, span, name),
    };
    let rest_slice_start = lower + after_kw;
    locate_name_literal(source, Span::new(rest_slice_start, upper), name)
}

/// Scan the slice `source[span.start..span.end]` for an occurrence of
/// `name` that lies on identifier boundaries. Returns the span of the
/// first hit, or a zero-width span at `span.start` when none matches.
fn locate_name_literal(source: &str, span: Span, name: &str) -> Span {
    let upper = span.end.min(source.len());
    let lower = span.start.min(upper);
    if name.is_empty() || lower >= upper {
        return Span::new(lower, lower);
    }
    let haystack = &source[lower..upper];
    let bytes = haystack.as_bytes();
    let name_bytes = name.as_bytes();
    let mut i = 0;
    while i + name_bytes.len() <= bytes.len() {
        if bytes[i..i + name_bytes.len()] == *name_bytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok =
                i + name_bytes.len() == bytes.len() || !is_ident_byte(bytes[i + name_bytes.len()]);
            if before_ok && after_ok {
                let start = lower + i;
                return Span::new(start, start + name.len());
            }
        }
        i += 1;
    }
    Span::new(lower, lower)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapphire_compiler::analyze::analyze;
    use sapphire_compiler::resolver::resolve;

    use crate::diagnostics::build_line_map;

    fn prepare(src: &str) -> (AstModule, ResolvedModule) {
        let analysis = analyze(src);
        assert!(analysis.is_ok(), "analyze failed: {:?}", analysis.errors);
        let module = analysis.module.expect("module present");
        let resolved = resolve(module.clone()).expect("resolve ok");
        (module, resolved)
    }

    fn byte_first(src: &str, needle: &str) -> usize {
        src.find(needle)
            .unwrap_or_else(|| panic!("needle `{needle}` not found in source"))
    }

    /// Return the 0-indexed char column (UTF-16 units, but tests
    /// only use ASCII) of `byte_off` within its line.
    fn char_col_of(src: &str, byte_off: usize) -> usize {
        let line_start = src[..byte_off].rfind('\n').map(|i| i + 1).unwrap_or(0);
        src[line_start..byte_off].chars().count()
    }

    /// Return the 0-indexed line of `byte_off`.
    fn line_of(src: &str, byte_off: usize) -> usize {
        src[..byte_off].bytes().filter(|&b| b == b'\n').count()
    }

    #[test]
    fn find_definition_for_top_level_value_reference() {
        // Jump from the `x` in `y = x` back to the `x` signature on
        // line 2 (the resolver's TopLevelDef span for a value is the
        // earliest declaration, and `x :` comes before `x =`).
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        // `y = x` — the reference is the last non-whitespace byte.
        let use_off = byte_first(src, "y = x") + 4;
        let range =
            find_definition(&module, &resolved, src, use_off, &map).expect("definition found");
        assert_eq!(range.start.line, 2);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 2);
        assert_eq!(range.end.character, 1);
    }

    #[test]
    fn find_definition_for_let_bound_reference() {
        // `a` is let-bound inside `f`'s body; reference it to the
        // right of `in`.
        let src = "\
module M where

f : Int
f = let a = 1 in a
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        // The `a` on the right of `in`.
        let use_off = byte_first(src, "in a") + 3;
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        // Binder is at `let a = 1`: the `a` after `let `.
        let def_off = byte_first(src, "let a = 1") + 4;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
        assert_eq!(range.end.character, range.start.character + 1);
    }

    #[test]
    fn find_definition_for_lambda_parameter() {
        let src = "\
module M where

f : Int -> Int
f = \\x -> x
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = src.rfind('x').unwrap();
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        // `\x` lands `x` at byte after `\`.
        let def_off = byte_first(src, "\\x") + 1;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_function_parameter() {
        let src = "\
module M where

id : Int -> Int
id x = x
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = src.rfind('x').unwrap();
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "id x") + 3;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_constructor_application() {
        let src = "\
module M where

data Pair = P Int Int

first : Pair
first = P 1 2
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        // `P` in `first = P 1 2`.
        let use_off = byte_first(src, "= P 1 2") + 2;
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "= P Int Int") + 2;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_data_type_reference_in_signature() {
        let src = "\
module M where

data Pair = P Int Int

mkp : Pair
mkp = P 1 2
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, ": Pair") + 2;
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "data Pair") + "data ".len();
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
        assert_eq!(range.end.character, range.start.character + 4);
    }

    #[test]
    fn find_definition_returns_none_outside_any_reference() {
        let src = "\
module M where

x : Int
x = 1
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        // Byte 0 — on the `module` keyword, not a reference site.
        assert!(find_definition(&module, &resolved, src, 0, &map).is_none());
    }

    #[test]
    fn find_definition_returns_none_for_prelude_reference() {
        let src = "\
module M where

two : Int
two = 1 + 1
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let plus_off = byte_first(src, "1 + 1") + 2;
        assert!(find_definition(&module, &resolved, src, plus_off, &map).is_none());
    }

    #[test]
    fn find_definition_for_case_arm_pattern_binder() {
        let src = "\
module M where

only : Int -> Int
only n = case n of
  k -> k
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        // Last `k` (the arm body).
        let use_off = src.rfind('k').unwrap();
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "  k -> k") + 2;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_do_bind_reference() {
        // A do-bind pattern is in scope for later stmts. Use a
        // Ruby-embedded primitive that is a value binding in the
        // module so the resolver is happy.
        let src = "\
module M where

prim : Ruby Int
prim := \"\"\"
  42
\"\"\"

run1 : Ruby Int
run1 = do
  x <- prim
  pure x
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        // The `x` in `pure x` is the use site.
        let use_off = byte_first(src, "pure x") + "pure ".len();
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "  x <- prim") + 2;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_signature_points_at_binding() {
        let src = "\
module M where

plus : Int -> Int -> Int
plus x y = x

inc : Int -> Int
inc = plus 1
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "= plus 1") + 2;
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        // TopLevelDef.span is the signature `plus : ...` header.
        let def_off = byte_first(src, "plus : Int");
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_type_alias() {
        let src = "\
module M where

type Age = Int

mkage : Age
mkage = 0
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, ": Age") + 2;
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "type Age") + "type ".len();
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
        assert_eq!(range.end.character, range.start.character + 3);
    }

    #[test]
    fn find_definition_past_source_end_is_none() {
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let past = src.len();
        assert!(find_definition(&module, &resolved, src, past, &map).is_none());
    }

    #[test]
    fn find_definition_prefers_innermost_span_on_overlap() {
        // `y = x + x` — clicking on the first `x` should land on
        // `x` (signature), not on the whole `BinOp` span.
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x + x
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let op_off = byte_first(src, "x + x");
        let range = find_definition(&module, &resolved, src, op_off, &map).expect("found");
        let def_off = byte_first(src, "x : Int");
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_on_reference_to_nested_let_binding() {
        let src = "\
module M where

f : Int
f =
  let a = 1
  in let b = a
  in b
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "let b = a") + "let b = ".len();
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "let a = 1") + "let ".len();
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_on_constructor_pattern_in_case() {
        let src = "\
module M where

data T = A | B

pick : T -> Int
pick t = case t of
  A -> 1
  B -> 2
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "  A -> 1") + 2;
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        let def_off = byte_first(src, "= A |") + 2;
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }

    #[test]
    fn find_definition_for_ruby_embed_binding() {
        let src = "\
module M where

greet : Ruby {}
greet := \"\"\"
  puts \"hello\"
\"\"\"

main : Ruby {}
main = greet
";
        let (module, resolved) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "main = greet") + "main = ".len();
        let range = find_definition(&module, &resolved, src, use_off, &map).expect("found");
        // Resolver stores the signature span for `greet` (first decl).
        let def_off = byte_first(src, "greet : Ruby");
        assert_eq!(range.start.line, line_of(src, def_off) as u32);
        assert_eq!(range.start.character, char_col_of(src, def_off) as u32);
    }
}
