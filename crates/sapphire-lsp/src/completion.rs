//! L6 completion: translate a cursor position into a list of
//! in-scope identifier candidates (`textDocument/completion`).
//!
//! The handler reuses the L4 hover / L5 goto pipeline
//! (`analyze → resolve (→ typeck)`), then collects candidates from
//! three sources:
//!
//! 1. **Local binders** reachable from the cursor — a light AST walk
//!    mirrors the resolver's scope rules.
//! 2. **Top-level names** in the current module
//!    (`ModuleEnv::top_level`).
//! 3. **Imported / prelude names** surfaced through
//!    `ModuleEnv::unqualified`, plus the module qualifiers stored in
//!    `ModuleEnv::qualified_aliases`.
//!
//! A left-scan from the byte offset classifies the cursor as either
//! bare (`prefi|`) or module-qualified (`Foo.Ba|`). For the qualified
//! case we restrict the candidates to the target module's exports.
//!
//! The design note is `docs/impl/31-lsp-completion.md`. The item
//! kind / detail rules there are the single source of truth for
//! this module's rendering choices.

use sapphire_compiler::resolver::{
    DefKind, ModuleEnv, ModuleId, Namespace, ResolvedModule, ResolvedRef,
};
use sapphire_core::ast::{
    CaseArm, ClassDecl, ClassItem, Decl, DoStmt, Expr, InstanceDecl, Module as AstModule, Param,
    Pattern, RubyEmbedDecl, ValueClause,
};
use sapphire_core::span::Span;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

use crate::hover::HoverTypes;

/// Public entry point. Compute a list of [`CompletionItem`]s for the
/// identifier-ish cursor at `byte_offset`.
///
/// Arguments:
/// - `module` — the parsed AST (used for the local-binder walk).
/// - `resolved` — the resolver output, providing the module env.
/// - `typed` — optional typeck projection; when a scheme is known for
///   a top-level or imported name, we surface it as the completion
///   item's `detail`.
/// - `source` — the raw source buffer. The left-scan for prefix /
///   qualifier walks it directly.
/// - `byte_offset` — byte offset into `source` at the cursor position
///   (as produced by `LineMap::byte_offset`).
///
/// The returned list is in "insertion order": locals first (innermost
/// binding first), then top-level decls, then unqualified imports,
/// then module qualifiers. LSP clients re-sort by their own ranking
/// (VSCode applies fuzzy + MRU over this list), so the server-side
/// order only matters as a tie-breaker. An empty `Vec` is returned
/// when the cursor rests on whitespace or on a non-identifier location
/// and nothing reasonable can be proposed.
pub fn find_completion_items(
    module: &AstModule,
    resolved: &ResolvedModule,
    typed: &HoverTypes,
    source: &str,
    byte_offset: usize,
) -> Vec<CompletionItem> {
    let (qualifier, prefix) = scan_prefix(source, byte_offset);

    let mut items: Vec<CompletionItem> = Vec::new();

    if let Some(qualifier_str) = qualifier {
        // Module-qualified completion: restrict to the exports of the
        // module the qualifier resolves to. Consult the env's alias
        // table; the qualifier string may be the full dotted name or
        // an `as` alias.
        let env = &resolved.env;
        let Some(target_id) = env.qualified_aliases.get(&qualifier_str).cloned() else {
            return items;
        };
        collect_qualified_names(env, typed, &target_id, &prefix, &mut items);
        return items;
    }

    // Bare (unqualified) completion.
    collect_locals(module, &prefix, byte_offset, &mut items);
    collect_top_level(&resolved.env, typed, &prefix, &mut items);
    collect_unqualified(&resolved.env, typed, &prefix, &mut items);
    collect_module_qualifiers(&resolved.env, &prefix, &mut items);

    items
}

// ---------------------------------------------------------------------
//  Prefix / qualifier scan
// ---------------------------------------------------------------------

/// Split the immediate left-of-cursor identifier chunk into an
/// optional module qualifier and a bare prefix.
///
/// Examples (with `|` as the cursor):
///
/// - `"foo bar|"` → `(None, "bar")`
/// - `"foo Bar.|"` → `(Some("Bar"), "")`
/// - `"Http.ma|"` → `(Some("Http"), "ma")`
/// - `"A.B.Ci|"` → `(Some("A.B"), "Ci")`
/// - `"  |"` → `(None, "")`
///
/// The scan consumes identifier bytes (`a-zA-Z0-9_'`) first, then
/// optionally a single `.` preceded by another identifier chunk that
/// starts with an uppercase letter. Multi-segment qualifiers repeat
/// that step left-ward. When the leftmost segment before a `.` is not
/// a capital-initial identifier we treat the whole thing as a bare
/// prefix — the dot is most likely belonging to record field access,
/// which L6 does not handle yet (I-OQ109).
fn scan_prefix(source: &str, byte_offset: usize) -> (Option<String>, String) {
    let upper = byte_offset.min(source.len());
    // Snap to a char boundary — the LSP client can land the cursor
    // inside a multi-byte codepoint during paste operations.
    let mut end = upper;
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    let bytes = source.as_bytes();

    // Step 1: walk left while on identifier bytes. This is the bare
    // prefix.
    let mut i = end;
    while i > 0 && is_ident_byte(bytes[i - 1]) {
        i -= 1;
    }
    let prefix = source[i..end].to_string();

    // Step 2: is the byte immediately to the left a `.`?  If not,
    // the prefix stands on its own.
    if i == 0 || bytes[i - 1] != b'.' {
        return (None, prefix);
    }

    // Step 3: walk left through the chain of `Segment.` groups. Every
    // segment must start with an uppercase ASCII letter and contain
    // only identifier bytes; otherwise we stop the qualifier scan at
    // the last successful segment.
    let mut end_q = i - 1; // position of the final `.`
    let mut segments: Vec<&str> = Vec::new();

    loop {
        // Walk left over a segment.
        let mut j = end_q;
        while j > 0 && is_ident_byte(bytes[j - 1]) {
            j -= 1;
        }
        if j == end_q {
            // No identifier preceding the `.` — not a qualifier chain.
            break;
        }
        let seg = &source[j..end_q];
        if !starts_with_upper(seg) {
            // A `foo.bar` chain is record field access, not module
            // qualification.
            break;
        }
        segments.push(seg);
        // Does another `.` precede?
        if j > 0 && bytes[j - 1] == b'.' {
            end_q = j - 1;
            continue;
        }
        break;
    }

    if segments.is_empty() {
        return (None, prefix);
    }
    segments.reverse();
    let qual = segments.join(".");
    (Some(qual), prefix)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

fn starts_with_upper(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

// ---------------------------------------------------------------------
//  Local binders (AST walk)
// ---------------------------------------------------------------------

/// Collect locals whose scope covers `cursor_offset`. The traversal
/// mirrors [`crate::definition::LocalFinder`] but emits every binder
/// rather than a single match; each binder's `Pattern::Var` (or
/// explicit name for `let` / `do`-let / lambda param) contributes a
/// candidate.
///
/// Only binders whose *own* span lies before the cursor and whose
/// enclosing scope contains the cursor are surfaced. That rule is
/// intentionally generous: a binder in a `let … in body` is reported
/// even when the cursor is still inside the `value` RHS, matching the
/// recursive `let` semantics of spec 03.
fn collect_locals(
    module: &AstModule,
    prefix: &str,
    cursor_offset: usize,
    out: &mut Vec<CompletionItem>,
) {
    let mut collector = LocalCollector {
        cursor: cursor_offset,
        seen: Vec::new(),
        prefix,
        items: out,
    };
    for decl in &module.decls {
        collector.visit_decl(decl);
    }
}

struct LocalCollector<'a> {
    cursor: usize,
    /// De-dup: shadowing means the same name may appear at multiple
    /// binder depths within one decl. We keep the innermost by using
    /// this as an "already emitted" filter.
    seen: Vec<String>,
    prefix: &'a str,
    items: &'a mut Vec<CompletionItem>,
}

impl LocalCollector<'_> {
    fn record(&mut self, name: &str) {
        if !matches_prefix(name, self.prefix) {
            return;
        }
        if self.seen.iter().any(|n| n == name) {
            return;
        }
        self.seen.push(name.to_string());
        self.items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("(local)".to_string()),
            ..CompletionItem::default()
        });
    }

    fn pattern_binds(&mut self, pat: &Pattern, scope_end: usize) {
        // `>` (not `>=`): cursor resting right at the end of a scope
        // (e.g. end-of-file after a function body) is still considered
        // inside the scope for completion. This matches LSP clients
        // that send `character == line_length` at end-of-line.
        if self.cursor > scope_end {
            return;
        }
        match pat {
            Pattern::Wildcard(_) | Pattern::Lit(_, _) => {}
            Pattern::Var { name, .. } => self.record(name),
            Pattern::As { name, inner, .. } => {
                self.record(name);
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
            Decl::Signature { .. } | Decl::Data(_) | Decl::TypeAlias(_) => {}
            Decl::Value(clause) => self.visit_value_clause(clause),
            Decl::Class(ClassDecl { items, .. }) => {
                for it in items {
                    if let ClassItem::Default(vc) = it {
                        self.visit_value_clause(vc);
                    }
                }
            }
            Decl::Instance(InstanceDecl { items, .. }) => {
                for c in items {
                    self.visit_value_clause(c);
                }
            }
            Decl::RubyEmbed(RubyEmbedDecl { params, span, .. }) => {
                // Ruby-embed parameters are only in scope while the
                // cursor sits inside the embed's span.
                if span_contains(*span, self.cursor) {
                    for p in params {
                        let Param { name, .. } = p;
                        self.record(name);
                    }
                }
            }
        }
    }

    fn visit_value_clause(&mut self, clause: &ValueClause) {
        let scope_end = clause.body.span().end;
        // Allow cursor at `== scope_end` so trailing-whitespace
        // positions at the end of a top-level body still see its
        // parameters.
        if self.cursor > scope_end {
            return;
        }
        for p in &clause.params {
            self.pattern_binds(p, scope_end);
        }
        self.visit_expr(&clause.body);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        let expr_span = expr.span();
        // Fast skip: the cursor is outside this subtree entirely.
        if !span_contains_inclusive(expr_span, self.cursor) {
            return;
        }
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
                // Spec 03: let is implicitly recursive — the name is
                // in scope over both `value` and `body`.
                if span_contains_inclusive(*span, self.cursor) {
                    self.record(name);
                }
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
                for CaseArm {
                    pattern,
                    body,
                    span: arm_span,
                } in arms
                {
                    // `case t of P1 -> b1 ; P2 -> b2` binds each
                    // arm's pattern only over its own body. The
                    // `pattern_binds` cursor guard is based on
                    // `body.span().end`, which would otherwise accept
                    // a sibling arm's binders when the cursor sits
                    // in an earlier arm's body (the sibling body ends
                    // *after* the cursor). Restrict to the arm whose
                    // span contains the cursor so siblings stay
                    // invisible.
                    if !span_contains_inclusive(*arm_span, self.cursor) {
                        continue;
                    }
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
                            // The binding is in scope from its own
                            // statement onwards, within the enclosing
                            // `do` block.
                            if span_contains_inclusive(
                                Span::new(lspan.start, scope_end),
                                self.cursor,
                            ) {
                                self.record(name);
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

fn span_contains(span: Span, offset: usize) -> bool {
    if span.start == span.end {
        span.start == offset
    } else {
        offset >= span.start && offset < span.end
    }
}

fn span_contains_inclusive(span: Span, offset: usize) -> bool {
    // Completion is interested in "where is my cursor?", which LSP
    // clients send as the position *after* the most recent key. That
    // position can equal `span.end` when the cursor sits at the very
    // end of an expression, so we include the right edge.
    offset >= span.start && offset <= span.end
}

// ---------------------------------------------------------------------
//  Top-level / imports / qualifiers
// ---------------------------------------------------------------------

fn collect_top_level(
    env: &ModuleEnv,
    typed: &HoverTypes,
    prefix: &str,
    out: &mut Vec<CompletionItem>,
) {
    let mut seen: Vec<(String, Namespace)> = Vec::new();
    for def in &env.top_level {
        if !is_identifier_label(&def.name) {
            continue;
        }
        if !matches_prefix(&def.name, prefix) {
            continue;
        }
        let key = (def.name.clone(), def.namespace);
        if seen.iter().any(|k| k == &key) {
            continue;
        }
        seen.push(key);
        let kind = kind_for_defkind(&def.kind);
        let detail = detail_for_def(&def.name, &def.kind, typed, &env.id);
        out.push(CompletionItem {
            label: def.name.clone(),
            kind: Some(kind),
            detail: Some(detail),
            ..CompletionItem::default()
        });
    }
}

fn collect_unqualified(
    env: &ModuleEnv,
    typed: &HoverTypes,
    prefix: &str,
    out: &mut Vec<CompletionItem>,
) {
    // `env.unqualified` lists every name brought into scope by an
    // `import` (including the implicit prelude import). A single name
    // can map to multiple refs when two imports collide; we emit one
    // completion item per (name, home-module) pair and de-dup by name
    // + module.
    let mut seen: Vec<(String, ModuleId)> = Vec::new();
    // Names already present in `env.top_level` take precedence — the
    // resolver itself shadows imports with same-named top-levels, so
    // we should not emit a duplicate entry here.
    let shadowed: Vec<(String, Namespace)> = env
        .top_level
        .iter()
        .map(|d| (d.name.clone(), d.namespace))
        .collect();

    // `env.unqualified` is a HashMap, so its iteration order is
    // effectively undefined and varies between runs. Sort the keys
    // by `(name, namespace)` so completion output is stable and
    // matches the "insertion order" contract of the design memo
    // (`docs/impl/31-lsp-completion.md` §候補源の優先順位) — clients
    // still re-sort by their own ranking, but deterministic server
    // output keeps snapshot-style tests reproducible.
    let mut keys: Vec<&(String, Namespace)> = env.unqualified.keys().collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0).then(namespace_cmp(a.1, b.1)));

    for (name, ns) in keys {
        if !is_identifier_label(name) {
            continue;
        }
        if !matches_prefix(name, prefix) {
            continue;
        }
        if shadowed.iter().any(|(n, s)| n == name && s == ns) {
            continue;
        }
        let refs = &env.unqualified[&(name.clone(), *ns)];
        for r in refs {
            let key = (name.clone(), r.module.clone());
            if seen.iter().any(|k| k == &key) {
                continue;
            }
            seen.push(key);
            let kind = kind_for_import_ref(r, *ns, typed);
            let detail = detail_for_import_ref(typed, r, *ns);
            out.push(CompletionItem {
                label: name.clone(),
                kind: Some(kind),
                detail: Some(detail),
                ..CompletionItem::default()
            });
        }
    }
}

/// Total order over [`Namespace`]. `Value` first, then `Type`, so
/// the stable-sort helper in [`collect_unqualified`] orders
/// same-name collisions deterministically.
fn namespace_cmp(a: Namespace, b: Namespace) -> std::cmp::Ordering {
    fn key(ns: Namespace) -> u8 {
        match ns {
            Namespace::Value => 0,
            Namespace::Type => 1,
        }
    }
    key(a).cmp(&key(b))
}

fn collect_module_qualifiers(env: &ModuleEnv, prefix: &str, out: &mut Vec<CompletionItem>) {
    // The module itself is entered into `qualified_aliases` under its
    // own dotted name; we still surface it (so `Foo.|` after a self-
    // qualified reference works uniformly). Aliases added via
    // `import M as L` also appear.
    //
    // `qualified_aliases` is a HashMap: sort keys so the emitted
    // order is stable across runs (see also the note on
    // [`collect_unqualified`]).
    let mut keys: Vec<&String> = env.qualified_aliases.keys().collect();
    keys.sort();
    for key in keys {
        if !matches_prefix(key, prefix) {
            continue;
        }
        out.push(CompletionItem {
            label: key.clone(),
            kind: Some(CompletionItemKind::MODULE),
            detail: Some("(module)".to_string()),
            ..CompletionItem::default()
        });
    }
}

fn collect_qualified_names(
    env: &ModuleEnv,
    typed: &HoverTypes,
    target: &ModuleId,
    prefix: &str,
    out: &mut Vec<CompletionItem>,
) {
    if target == &env.id {
        // Self-qualification: use the top-level table directly, which
        // covers every declaration the current module owns (private
        // and public alike; the export filter is handled by
        // `compute_exports` elsewhere and does not shrink what the
        // author sees from inside the module).
        for def in &env.top_level {
            if !is_identifier_label(&def.name) {
                continue;
            }
            if !matches_prefix(&def.name, prefix) {
                continue;
            }
            out.push(CompletionItem {
                label: def.name.clone(),
                kind: Some(kind_for_defkind(&def.kind)),
                detail: Some(detail_for_def(&def.name, &def.kind, typed, &env.id)),
                ..CompletionItem::default()
            });
        }
        return;
    }

    // For imported modules we rely on `env.unqualified` to enumerate
    // names whose home module is `target`. `apply_import` with a
    // qualified-only import (`import X qualified`) does not populate
    // `unqualified`, so for those we fall back to silence — a later
    // milestone (I-OQ108) pairs this with a workspace-wide module
    // export snapshot.
    let mut seen: Vec<(String, Namespace)> = Vec::new();
    // Stable iteration order — matches `collect_unqualified`.
    let mut keys: Vec<&(String, Namespace)> = env.unqualified.keys().collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0).then(namespace_cmp(a.1, b.1)));
    for (name, ns) in keys {
        if !is_identifier_label(name) {
            continue;
        }
        if !matches_prefix(name, prefix) {
            continue;
        }
        let refs = &env.unqualified[&(name.clone(), *ns)];
        for r in refs {
            if &r.module != target {
                continue;
            }
            let key = (name.clone(), *ns);
            if seen.iter().any(|k| k == &key) {
                continue;
            }
            seen.push(key);
            out.push(CompletionItem {
                label: name.clone(),
                kind: Some(kind_for_import_ref(r, *ns, typed)),
                detail: Some(detail_for_import_ref(typed, r, *ns)),
                ..CompletionItem::default()
            });
            break;
        }
    }
}

// ---------------------------------------------------------------------
//  Helpers: kind + detail rendering
// ---------------------------------------------------------------------

fn matches_prefix(name: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    name.starts_with(prefix)
}

/// Return `true` when `name` can be used in identifier position
/// without backtick / parenthesisation — i.e. it starts with a
/// letter or `_`. Operator symbols from the prelude (`+`, `++`,
/// `>>=`, `::`, `==`, …) live in the same value namespace as
/// ordinary identifiers, so they otherwise leak into the empty-
/// prefix candidate list. Completion clients usually cannot apply
/// them verbatim at a cursor mid-identifier, so we drop them here.
/// See I-OQ110: we may want to resurface them once the scan
/// distinguishes operator position (e.g. cursor after a space on
/// a binary-operator slot).
fn is_identifier_label(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c == '_' || c.is_alphabetic())
}

fn kind_for_defkind(kind: &DefKind) -> CompletionItemKind {
    match kind {
        DefKind::Value | DefKind::RubyEmbed => CompletionItemKind::FUNCTION,
        DefKind::Ctor { .. } => CompletionItemKind::CONSTRUCTOR,
        DefKind::ClassMethod { .. } => CompletionItemKind::METHOD,
        DefKind::DataType => CompletionItemKind::CLASS,
        DefKind::TypeAlias => CompletionItemKind::INTERFACE,
        DefKind::Class => CompletionItemKind::CLASS,
    }
}

fn kind_for_import_ref(r: &ResolvedRef, ns: Namespace, typed: &HoverTypes) -> CompletionItemKind {
    // We do not have the originating `DefKind` here — the unqualified
    // table stores only `ResolvedRef`. Distinguish via namespace plus
    // the `HoverTypes.ctors` table (ctors live in the Value namespace
    // but should render as `CONSTRUCTOR`). As a fallback, use the
    // upper-case-initial heuristic — Sapphire reserves those for
    // constructor / type names per spec 02 §Identifiers, so a
    // value-namespace import starting uppercase is a ctor even when
    // `typed.ctors` has not seen the name yet (e.g. typecheck raised
    // before registering it).
    match ns {
        Namespace::Value => {
            let is_ctor = typed.ctors.contains_key(&r.name) || starts_with_upper(&r.name);
            if is_ctor {
                CompletionItemKind::CONSTRUCTOR
            } else {
                CompletionItemKind::FUNCTION
            }
        }
        Namespace::Type => CompletionItemKind::CLASS,
    }
}

fn detail_for_def(name: &str, kind: &DefKind, typed: &HoverTypes, module_id: &ModuleId) -> String {
    match kind {
        DefKind::Value | DefKind::RubyEmbed | DefKind::ClassMethod { .. } => {
            if let Some(scheme) = typed.inferred.get(name) {
                return scheme.pretty();
            }
            if let Some(scheme) = typed.globals.get(&sapphire_compiler::typeck::GlobalId::new(
                module_id.display(),
                name,
            )) {
                return scheme.pretty();
            }
            kind_label(kind).to_string()
        }
        DefKind::Ctor { .. } => {
            if let Some(info) = typed.ctors.get(name) {
                return info.scheme.pretty();
            }
            kind_label(kind).to_string()
        }
        DefKind::DataType | DefKind::TypeAlias | DefKind::Class => kind_label(kind).to_string(),
    }
}

fn detail_for_import_ref(typed: &HoverTypes, r: &ResolvedRef, ns: Namespace) -> String {
    match ns {
        Namespace::Value => {
            // Prelude operators / class methods land only under
            // qualified GlobalId keys, so prefer that path first.
            let gid = sapphire_compiler::typeck::GlobalId::new(r.module.display(), &r.name);
            if let Some(scheme) = typed.globals.get(&gid) {
                return scheme.pretty();
            }
            if let Some(info) = typed.ctors.get(&r.name) {
                return info.scheme.pretty();
            }
            if let Some(scheme) = typed.inferred.get(&r.name) {
                return scheme.pretty();
            }
            format!("(from {})", r.module.display())
        }
        Namespace::Type => format!("(type from {})", r.module.display()),
    }
}

fn kind_label(kind: &DefKind) -> &'static str {
    match kind {
        DefKind::Value => "(value)",
        DefKind::RubyEmbed => "(:=-binding)",
        DefKind::Ctor { .. } => "(constructor)",
        DefKind::ClassMethod { .. } => "(class method)",
        DefKind::DataType => "(data type)",
        DefKind::TypeAlias => "(type alias)",
        DefKind::Class => "(class)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapphire_compiler::analyze::analyze;
    use sapphire_compiler::resolver::resolve;

    use crate::hover::collect_hover_types;

    fn prepare(src: &str) -> (AstModule, ResolvedModule, HoverTypes) {
        let analysis = analyze(src);
        assert!(analysis.is_ok(), "analyze failed: {:?}", analysis.errors);
        let module = analysis.module.expect("module present");
        let resolved = resolve(module.clone()).expect("resolve ok");
        let typed = collect_hover_types(&resolved.env.id.display(), &module);
        (module, resolved, typed)
    }

    fn labels(items: &[CompletionItem]) -> Vec<String> {
        items.iter().map(|i| i.label.clone()).collect()
    }

    fn find_label<'a>(items: &'a [CompletionItem], label: &str) -> Option<&'a CompletionItem> {
        items.iter().find(|i| i.label == label)
    }

    // ----- scan_prefix -------------------------------------------------

    #[test]
    fn scan_prefix_bare_identifier() {
        let (q, p) = scan_prefix("hello wor", 9);
        assert_eq!(q, None);
        assert_eq!(p, "wor");
    }

    #[test]
    fn scan_prefix_whitespace_cursor_gives_empty_prefix() {
        let (q, p) = scan_prefix("foo bar  ", 9);
        assert_eq!(q, None);
        assert_eq!(p, "");
    }

    #[test]
    fn scan_prefix_module_qualifier_with_partial() {
        let (q, p) = scan_prefix("xs = Http.ma", 12);
        assert_eq!(q.as_deref(), Some("Http"));
        assert_eq!(p, "ma");
    }

    #[test]
    fn scan_prefix_module_qualifier_bare_dot() {
        let (q, p) = scan_prefix("xs = Http.", 10);
        assert_eq!(q.as_deref(), Some("Http"));
        assert_eq!(p, "");
    }

    #[test]
    fn scan_prefix_multi_segment_qualifier() {
        let (q, p) = scan_prefix("xs = A.B.Ci", 11);
        assert_eq!(q.as_deref(), Some("A.B"));
        assert_eq!(p, "Ci");
    }

    #[test]
    fn scan_prefix_lowercase_owner_is_not_qualifier() {
        // `foo.bar` is record field access, not module qualifier —
        // we must NOT treat it as `(Some("foo"), "bar")`.
        let (q, p) = scan_prefix("let x = foo.bar", 15);
        assert_eq!(q, None);
        assert_eq!(p, "bar");
    }

    // ----- completion pipeline ----------------------------------------
    //
    // Strategy: every source here parses and resolves cleanly, and
    // the cursor is placed *inside* an existing identifier so that
    // the prefix scan reads e.g. `hel` when the identifier is
    // `helper` (cursor = offset(helper) + 3). That matches how VSCode
    // feeds positions during live typing: the client is always
    // mid-identifier with the rest of the ident already typed to the
    // right of the cursor.

    /// Return the byte offset `n` chars into the first occurrence of
    /// `needle` in `src`. Panics if `needle` is absent.
    fn pos_inside(src: &str, needle: &str, offset_into: usize) -> usize {
        let i = src
            .find(needle)
            .unwrap_or_else(|| panic!("needle `{needle}` not in source"));
        i + offset_into
    }

    #[test]
    fn completion_top_level_prefix_matches_matching_names() {
        let src = "\
module M where

helper : Int
helper = 1

helperTwo : Int
helperTwo = 2

unrelated : Int
unrelated = 3

main : Int
main = helper
";
        // Cursor inside `= helper` — 3 chars into the `helper` ident,
        // so the scan sees prefix `hel`.
        let cur = pos_inside(src, "= helper\n", "= hel".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"helper".to_string()), "labels = {lbls:?}");
        assert!(lbls.contains(&"helperTwo".to_string()), "labels = {lbls:?}");
        assert!(
            !lbls.contains(&"unrelated".to_string()),
            "labels = {lbls:?}",
        );
    }

    #[test]
    fn completion_empty_prefix_returns_all_in_scope_names() {
        // Empty prefix = cursor on whitespace. Put the cursor at the
        // whitespace *after* a valid expression body so resolve still
        // succeeds.
        let src = "\
module M where

aaa : Int
aaa = 1

bbb : Int
bbb = 2

main : Int
main = aaa
";
        // Cursor between `main =` and `aaa` — 7 bytes into "main = aaa"
        // lands on the space before `aaa`.
        let cur = pos_inside(src, "main = aaa", "main = ".len() - 1);
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"aaa".to_string()), "labels = {lbls:?}");
        assert!(lbls.contains(&"bbb".to_string()), "labels = {lbls:?}");
        // Should also include prelude: e.g. `map` lives in the
        // unqualified import table.
        assert!(lbls.contains(&"map".to_string()), "labels = {lbls:?}");
    }

    #[test]
    fn completion_local_let_binding_appears_in_body() {
        let src = "\
module M where

f : Int
f = let myLocal = 1 in myLocal
";
        // Cursor inside the `in myLocal` reference — 2 chars into
        // `myLocal`, so the prefix is `my`.
        let cur = pos_inside(src, "in myLocal", "in my".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(
            lbls.contains(&"myLocal".to_string()),
            "labels = {lbls:?}; cursor expected to see `myLocal`",
        );
        let item = find_label(&items, "myLocal").unwrap();
        assert_eq!(item.kind, Some(CompletionItemKind::VARIABLE));
    }

    #[test]
    fn completion_lambda_parameter_visible_in_body() {
        let src = "\
module M where

f : Int -> Int
f = \\myParam -> myParam
";
        // Cursor inside the `-> myParam` reference, prefix `myP`.
        let cur = pos_inside(src, "-> myParam", "-> myP".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"myParam".to_string()), "labels = {lbls:?}");
    }

    #[test]
    fn completion_local_does_not_leak_across_let_scope() {
        // `inner` is bound only inside `f`'s body. When we complete
        // inside the *body* of a sibling top-level `g` the candidate
        // list must NOT expose `inner`.
        let src = "\
module M where

f : Int
f =
  let inner = 1 in inner

g : Int
g = 0
";
        // Cursor between `g =` and `0` (on the space before `0`).
        let cur = pos_inside(src, "g = 0", "g =".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(!lbls.contains(&"inner".to_string()), "labels = {lbls:?}");
    }

    #[test]
    fn completion_prelude_map_visible_with_prefix() {
        // `map` is registered by the implicit prelude import. Cursor
        // inside a valid expression that happens to reference `map`.
        let src = "\
module M where

plusOne : Int -> Int
plusOne n = n

doit : List Int -> List Int
doit xs = map plusOne xs
";
        // Cursor inside `map` — 2 chars into the ident, prefix `ma`.
        let cur = pos_inside(src, "= map plusOne", "= ma".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"map".to_string()), "labels = {lbls:?}");
        let item = find_label(&items, "map").expect("map present");
        assert_eq!(item.kind, Some(CompletionItemKind::FUNCTION));
        // Prelude `map` has a scheme registered as a global; the
        // detail should surface it (or at least a module-qualified
        // fallback). Accept either a `->` in the scheme or the
        // fallback string.
        let detail = item.detail.as_deref().unwrap_or("");
        assert!(
            detail.contains("->") || detail.contains("Prelude"),
            "unexpected detail: {detail:?}",
        );
    }

    #[test]
    fn completion_prelude_ctor_uses_constructor_kind() {
        let src = "\
module M where

packOne : Int -> Maybe Int
packOne n = Just n
";
        // Cursor 2 chars into `Just`, prefix `Ju`.
        let cur = pos_inside(src, "Just n\n", "Ju".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let item = find_label(&items, "Just").expect("Just present");
        assert_eq!(item.kind, Some(CompletionItemKind::CONSTRUCTOR));
    }

    #[test]
    fn completion_top_level_ctor_uses_constructor_kind() {
        let src = "\
module M where

data Pair = Pear Int Int

first : Pair
first = Pear 1 2
";
        // Cursor 2 chars into the `Pear` use, prefix `Pe`.
        let cur = pos_inside(src, "= Pear 1", "= Pe".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let item = find_label(&items, "Pear").expect("Pear present");
        assert_eq!(item.kind, Some(CompletionItemKind::CONSTRUCTOR));
        let detail = item.detail.as_deref().unwrap_or("");
        assert!(
            detail.contains("Pair"),
            "expected Pair in Pear detail, got: {detail:?}",
        );
    }

    #[test]
    fn completion_detail_carries_scheme_for_top_level_value() {
        let src = "\
module M where

greet : Int
greet = 1

main : Int
main = greet
";
        // Cursor 2 chars into the `greet` reference in main.
        let cur = pos_inside(src, "main = greet", "main = gr".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let item = find_label(&items, "greet").expect("greet present");
        let detail = item.detail.as_deref().unwrap_or("");
        assert!(detail.contains("Int"), "expected Int in detail: {detail:?}");
    }

    #[test]
    fn completion_module_qualifier_self_returns_top_level_names() {
        let src = "\
module M where

alpha : Int
alpha = 1

beta : Int
beta = 2

main : Int
main = M.alpha
";
        // Cursor 2 chars into `M.alpha` -> qualifier `M`, prefix `al`.
        let cur = pos_inside(src, "M.alpha", "M.al".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        // Qualifier `M`, prefix `al`: `alpha` matches, `beta` doesn't.
        assert!(lbls.contains(&"alpha".to_string()), "labels = {lbls:?}");
        assert!(!lbls.contains(&"beta".to_string()), "labels = {lbls:?}");
    }

    #[test]
    fn completion_module_qualifier_surfaces_module_kind_in_bare_scan() {
        // Module qualifier names themselves are proposed in the bare
        // scan, with MODULE kind.
        let src = "\
module M where

main : Int
main = 0
";
        // Cursor on the `M` of the `module M where` header — the
        // ident starts at the byte `M`. Put the cursor at the end of
        // `main : Int` signature's `M`... actually just put the cursor
        // on a whitespace and assert `M` appears in the full set.
        let cur = pos_inside(src, "main = 0", "main = ".len() - 1);
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let item = find_label(&items, "M").expect("`M` module qualifier present");
        assert_eq!(item.kind, Some(CompletionItemKind::MODULE));
    }

    #[test]
    fn completion_does_not_duplicate_shadowed_import() {
        // `map` is brought in by the implicit prelude import. If the
        // user defines their own `map` at top-level, the completion
        // list must contain a single entry for `map` (the local one),
        // not two.
        let src = "\
module M where

map : Int
map = 1

main : Int
main = map
";
        let cur = pos_inside(src, "main = map", "main = ma".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let mapped: Vec<&CompletionItem> = items.iter().filter(|i| i.label == "map").collect();
        assert_eq!(
            mapped.len(),
            1,
            "expected a single `map` entry, got: {:?}",
            mapped.iter().map(|c| &c.detail).collect::<Vec<_>>(),
        );
        // And it should be the top-level one (FUNCTION kind, scheme
        // detail), not the prelude one.
        let det = mapped[0].detail.as_deref().unwrap_or("");
        assert!(
            det.contains("Int") || det.contains("(value)"),
            "unexpected shadowed detail: {det:?}",
        );
    }

    #[test]
    fn completion_past_source_end_returns_top_level_names() {
        let src = "\
module M where

x : Int
x = 1
";
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, src.len() + 50);
        // We should still happily return candidates — `past end` is
        // valid for "end of file" completion. The list must not
        // crash; assert it at least contains some top-levels.
        let lbls = labels(&items);
        assert!(lbls.contains(&"x".to_string()), "labels = {lbls:?}");
    }

    #[test]
    fn completion_unknown_module_qualifier_returns_empty() {
        // An unknown qualifier (no alias in the env) must return an
        // empty list — we refuse to speculate.
        let src = "\
module M where

main : Int
main = 0
";
        // Hand-construct a qualifier string the source does not use.
        // The function is pure over source + offset, so we can feed
        // any offset against a synthetic buffer to exercise the
        // qualifier-not-in-env branch.
        let synthetic_src = "Unknown.fo";
        let cur = synthetic_src.len();
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, synthetic_src, cur);
        assert!(items.is_empty(), "expected empty for unknown qualifier");
    }

    #[test]
    fn completion_case_arm_binder_visible_in_arm_body() {
        let src = "\
module M where

f : Int -> Int
f n = case n of
  kappa -> kappa
";
        // Cursor 3 chars into the `kappa` use, prefix `kap`.
        let cur = pos_inside(src, "-> kappa", "-> kap".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"kappa".to_string()), "labels = {lbls:?}");
    }

    #[test]
    fn completion_case_arm_binder_does_not_leak_into_earlier_arm() {
        // Spec 06: each `case` arm binds its pattern only over its
        // own body. From inside arm 1 (the `Just xxx ->` body), the
        // binder `yyy` from arm 2 must NOT be visible. (The bug
        // reviewer-I6 caught: the cursor check in `pattern_binds`
        // only compared against each arm's `body.span().end`, so the
        // later arm's body-end landed *past* the cursor and leaked
        // its binders.)
        let src = "\
module M where

f : Maybe Int -> Int
f m = case m of
  Just xxx -> xxx
  Just yyy -> yyy
";
        // Cursor 2 chars into the RHS `xxx`, prefix `xx`. The second
        // arm's `yyy` must not appear.
        let cur = pos_inside(src, "-> xxx\n", "-> xx".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"xxx".to_string()), "labels = {lbls:?}");
        assert!(
            !lbls.contains(&"yyy".to_string()),
            "sibling arm binder leaked: labels = {lbls:?}",
        );
    }

    #[test]
    fn completion_case_arm_binder_does_not_leak_from_earlier_arm() {
        // Symmetric to the above: from arm 2, arm 1's binder must
        // not be visible either.
        let src = "\
module M where

f : Maybe Int -> Int
f m = case m of
  Just xxx -> xxx
  Just yyy -> yyy
";
        // Cursor 2 chars into the second arm's `yyy` RHS, prefix
        // `yy`.
        let cur = pos_inside(src, "-> yyy\n", "-> yy".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(lbls.contains(&"yyy".to_string()), "labels = {lbls:?}");
        assert!(
            !lbls.contains(&"xxx".to_string()),
            "earlier arm binder leaked: labels = {lbls:?}",
        );
    }

    #[test]
    fn completion_no_operator_symbols_in_empty_prefix() {
        // Operator-symbol imports (`+`, `++`, `>>=`, `::`, …) live
        // in the same value namespace as ordinary identifiers but are
        // not usable at an identifier cursor position. The empty-
        // prefix completion must filter them out. See suggestion-
        // reviewer (2026-04-19) and I-OQ110.
        let src = "\
module M where

main : Int
main = 0
";
        let cur = pos_inside(src, "main = 0", "main = ".len() - 1);
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        for op in ["+", "-", "*", "++", ">>=", "::", "==", "<", ">"] {
            assert!(
                !lbls.iter().any(|l| l == op),
                "operator label `{op}` leaked into completion: {lbls:?}",
            );
        }
    }

    #[test]
    fn completion_ruby_embed_params_only_visible_inside_embed_span() {
        // A Ruby-embed's parameters are in scope inside the embed
        // span, not outside. At a cursor inside a sibling top-level
        // we must NOT see those params.
        let src = "\
module M where

greet : String -> Ruby {}
greet nameParam := \"\"\"
  puts nameParam
\"\"\"

main : Ruby {}
main = greet \"hello\"
";
        // Cursor inside `= greet` in main — 2 chars into `greet`.
        let cur = pos_inside(src, "main = greet", "main = gr".len());
        let (module, resolved, typed) = prepare(src);
        let items = find_completion_items(&module, &resolved, &typed, src, cur);
        let lbls = labels(&items);
        assert!(
            !lbls.contains(&"nameParam".to_string()),
            "nameParam must not leak outside embed: {lbls:?}",
        );
        assert!(lbls.contains(&"greet".to_string()), "labels = {lbls:?}");
    }
}
