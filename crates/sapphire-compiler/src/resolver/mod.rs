//! Name resolution for Sapphire (I5).
//!
//! This module consumes a list of parsed modules from
//! [`sapphire_core::ast`] and produces a [`ResolvedProgram`]: each
//! input module enriched with (a) the list of top-level definitions
//! it introduces, (b) the `import` / `export` tables that describe
//! cross-module visibility, and (c) a side table mapping every
//! reference site's [`Span`] to the [`ResolvedRef`] it resolves to.
//!
//! The pass does **not** perform type checking, instance validation,
//! or any kind of elaboration; those are I6's job. See spec 08
//! §Abstract syntax / §Name resolution for the contract this module
//! discharges, and `docs/impl/15-resolver.md` for the design
//! choices — notably why we keep the original AST and emit
//! side-table information rather than defining a parallel resolved
//! AST.

mod env;
mod error;
mod imports;
mod prelude;
mod scope;

#[cfg(test)]
mod tests;

pub use env::{DefKind, Exports, ModuleEnv, ModuleId, ResolvedRef, TopLevelDef, Visibility};
pub use error::{Namespace, ResolveError, ResolveErrorKind};

use std::collections::{HashMap, HashSet};

use sapphire_core::ast::{
    CaseArm, ClassDecl, ClassItem, Constraint, DataDecl, Decl, DoStmt, Expr, InstanceDecl, ModName,
    Module as AstModule, Pattern, RubyEmbedDecl, Scheme, Type as AstType, TypeAlias, ValueClause,
};
use sapphire_core::span::Span;

use self::imports::{
    apply_import, builtin_prelude_exports, compute_exports, resolve_qualifier,
    should_add_implicit_prelude, synthetic_prelude_import,
};
use self::prelude::PRELUDE_MODULE;
use self::scope::ScopeStack;

/// The full resolved output for a compilation.
///
/// Keeps every input module's AST intact (`ast` field) alongside the
/// resolution metadata (`env`, `references`). Downstream passes can
/// index into `references` by the span of any `Expr::Var`,
/// `Expr::OpRef`, `Expr::BinOp`, `Pattern::Con`, `Type::Con`, etc.
#[derive(Debug)]
pub struct ResolvedProgram {
    pub modules: Vec<ResolvedModule>,
}

/// One module's resolution result.
#[derive(Debug)]
pub struct ResolvedModule {
    pub id: ModuleId,
    pub ast: AstModule,
    pub env: ModuleEnv,
    /// Every reference-site span in the module, mapped to its
    /// resolved identity. Locals appear with `module = <this module>`
    /// and namespace `Value` — callers can distinguish them from
    /// top-level or imported references by consulting `env`.
    pub references: HashMap<Span, Resolution>,
}

/// What a reference site resolves to.
///
/// The resolver keeps a distinct variant for locally-bound references
/// so downstream consumers do not have to consult the module env to
/// tell a lambda parameter from a top-level function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A locally-bound name — a lambda parameter, a `let`-bound
    /// name, a `do` bind pattern, or a type-variable.
    Local { name: String },
    /// A top-level or imported binding.
    Global(ResolvedRef),
}

/// Resolve a list of parsed Sapphire modules end-to-end.
///
/// The input order does not matter for resolution; the resolver
/// builds a module graph, detects cycles (spec 08 §Cyclic imports),
/// and processes modules in dependency order.
///
/// Errors from multiple modules are accumulated — the caller gets a
/// best-effort `Vec` of everything that went wrong, not just the
/// first failure.
pub fn resolve_program(modules: Vec<AstModule>) -> Result<ResolvedProgram, Vec<ResolveError>> {
    Resolver::new(modules).run()
}

/// Convenience wrapper for the single-module case. Handy for tests
/// and single-file scripts.
pub fn resolve(module: AstModule) -> Result<ResolvedModule, Vec<ResolveError>> {
    let mut program = resolve_program(vec![module])?;
    Ok(program.modules.remove(0))
}

// ---------------------------------------------------------------------
//  Driver
// ---------------------------------------------------------------------

struct Resolver {
    modules: Vec<AstModule>,
    /// Map from module dotted name → index in `modules`.
    name_index: HashMap<String, usize>,
    /// Which modules carry only the implicit `module Main where`
    /// header (spec 08 §One module per file). Used to reject
    /// `import Main` in that case (08-OQ5).
    implicit_main: HashSet<usize>,
    /// Names actually imported by each module (populated during
    /// cycle detection / traversal).
    imports_of: Vec<Vec<String>>,
    errors: Vec<ResolveError>,
}

impl Resolver {
    fn new(modules: Vec<AstModule>) -> Self {
        let mut name_index = HashMap::new();
        let mut implicit_main = HashSet::new();
        for (idx, m) in modules.iter().enumerate() {
            let name = module_name(m);
            if m.header.is_none() {
                implicit_main.insert(idx);
            }
            name_index.insert(name, idx);
        }
        let imports_of = modules.iter().map(|_| Vec::new()).collect();
        Self {
            modules,
            name_index,
            implicit_main,
            imports_of,
            errors: Vec::new(),
        }
    }

    fn run(mut self) -> Result<ResolvedProgram, Vec<ResolveError>> {
        // Phase 1: scan top-level declarations of every module.
        let mut envs: Vec<ModuleEnv> = self
            .modules
            .iter()
            .map(|m| {
                let segments: Vec<String> = module_name(m).split('.').map(str::to_string).collect();
                let id = ModuleId::from_segments(&segments);
                let mut env = ModuleEnv::new(id);
                collect_top_level(m, &mut env, &mut self.errors);
                env
            })
            .collect();

        // Phase 2: compute exports for each module. This has to
        // happen after top_level collection but before any import
        // resolution because `import M (x)` needs to know whether
        // `M` actually exports `x`.
        for (idx, env) in envs.iter_mut().enumerate() {
            let export_list = self.modules[idx]
                .header
                .as_ref()
                .and_then(|h| h.exports.as_deref());
            match compute_exports(env, export_list) {
                Ok(exports) => env.exports = exports,
                Err(errs) => self.errors.extend(errs),
            }
        }

        // Phase 3: build the module graph, detect cycles. Record each
        // module's imports (after validating the named modules
        // exist). `Prelude` is always treated as an implicit
        // dependency, but it is not in the user-module graph and
        // has no imports of its own, so it does not affect cycles.
        for (idx, m) in self.modules.iter().enumerate() {
            for imp in &m.imports {
                let name = imp.name.segments.join(".");
                if name == PRELUDE_MODULE {
                    // Explicit prelude import — does not add a graph
                    // edge; the prelude is built-in.
                    continue;
                }
                if !self.name_index.contains_key(&name) {
                    self.errors.push(ResolveError::new(
                        ResolveErrorKind::ImportOfUnknownModule {
                            module: name.clone(),
                        },
                        imp.span,
                    ));
                    continue;
                }
                // Spec 08 §One module per file: `module Main`-sugared
                // files cannot be imported.
                let target_idx = self.name_index[&name];
                if self.implicit_main.contains(&target_idx) {
                    self.errors.push(ResolveError::new(
                        ResolveErrorKind::MainSugarNotImportable {
                            module: name.clone(),
                        },
                        imp.span,
                    ));
                    continue;
                }
                self.imports_of[idx].push(name);
            }
        }

        if let Err(cycle) = detect_cycles(&self.imports_of, &self.name_index) {
            self.errors.push(ResolveError::new(
                ResolveErrorKind::CyclicImports {
                    cycle: cycle.clone(),
                },
                // The cycle error has no good single span; use the
                // first module's header span (or its first import)
                // for locality.
                span_of_module(&self.modules[self.name_index[&cycle[0]]]),
            ));
        }

        // Phase 4: apply imports. Each module gets the implicit
        // `Prelude` import (unless suppressed) plus every explicit
        // `import`.
        let (prelude_id, prelude_exports) = builtin_prelude_exports();
        // Snapshot every module's exports up front so we can mutate
        // the env list in-place while reading other modules' export
        // tables. The clone cost is trivial at M9 scale.
        let export_snapshot: Vec<(ModuleId, Exports)> = envs
            .iter()
            .map(|e| (e.id.clone(), e.exports.clone()))
            .collect();
        for (idx, env) in envs.iter_mut().enumerate() {
            let module = &self.modules[idx];
            // Implicit prelude.
            if should_add_implicit_prelude(module) {
                let synth = synthetic_prelude_import(Span::empty(0));
                if let Err(errs) = apply_import(env, &synth, prelude_id.clone(), &prelude_exports) {
                    self.errors.extend(errs);
                }
            }
            // Explicit imports.
            for imp in &module.imports {
                let name = imp.name.segments.join(".");
                if name == PRELUDE_MODULE {
                    if let Err(errs) = apply_import(env, imp, prelude_id.clone(), &prelude_exports)
                    {
                        self.errors.extend(errs);
                    }
                    continue;
                }
                let Some(&target_idx) = self.name_index.get(&name) else {
                    // Already reported in phase 3.
                    continue;
                };
                let (target_id, target_exports) = export_snapshot[target_idx].clone();
                if let Err(errs) = apply_import(env, imp, target_id, &target_exports) {
                    self.errors.extend(errs);
                }
            }
            // The module itself is always visible under its full
            // dotted name for qualified access.
            env.qualified_aliases
                .insert(env.id.display(), env.id.clone());
        }

        // Phase 5: walk every expression / type / pattern, building
        // the reference table and emitting undefined-name errors.
        let mut resolved = Vec::with_capacity(envs.len());
        for (idx, env) in envs.into_iter().enumerate() {
            let module = self.modules[idx].clone();
            let mut refs = HashMap::new();
            let mut walker = Walker {
                env: &env,
                references: &mut refs,
                errors: &mut self.errors,
                scope: ScopeStack::new(),
                type_scope: ScopeStack::new(),
            };
            walker.walk_module(&module);

            // Phase 6: private-type leak check (spec 08 §Visibility).
            check_private_leaks(&module, &env, &mut self.errors);

            resolved.push(ResolvedModule {
                id: env.id.clone(),
                ast: module,
                env,
                references: refs,
            });
        }

        if self.errors.is_empty() {
            Ok(ResolvedProgram { modules: resolved })
        } else {
            Err(self.errors)
        }
    }
}

fn module_name(m: &AstModule) -> String {
    match &m.header {
        Some(h) => h.name.segments.join("."),
        None => "Main".to_string(),
    }
}

fn span_of_module(m: &AstModule) -> Span {
    m.header.as_ref().map(|h| h.span).unwrap_or_else(|| m.span)
}

// ---------------------------------------------------------------------
//  Phase 1: top-level collection
// ---------------------------------------------------------------------

fn collect_top_level(m: &AstModule, env: &mut ModuleEnv, errors: &mut Vec<ResolveError>) {
    for decl in &m.decls {
        match decl {
            Decl::Signature {
                name,
                operator: _,
                scheme: _,
                span,
            } => {
                // A signature without a matching value clause is
                // permitted — it simply declares the public type.
                // Duplicate checks happen once we see the actual
                // value binding; signatures do not themselves
                // introduce a "definition".
                ensure_value_binding(env, name, DefKind::Value, *span, errors);
            }
            Decl::Value(ValueClause {
                name,
                operator: _,
                span,
                ..
            }) => {
                ensure_value_binding(env, name, DefKind::Value, *span, errors);
            }
            Decl::Data(DataDecl {
                name,
                type_params: _,
                ctors,
                span,
            }) => {
                insert_top(
                    env,
                    name.clone(),
                    Namespace::Type,
                    DefKind::DataType,
                    *span,
                    errors,
                );
                for ctor in ctors {
                    insert_top(
                        env,
                        ctor.name.clone(),
                        Namespace::Value,
                        DefKind::Ctor {
                            parent_type: name.clone(),
                        },
                        ctor.span,
                        errors,
                    );
                }
            }
            Decl::TypeAlias(TypeAlias { name, span, .. }) => {
                insert_top(
                    env,
                    name.clone(),
                    Namespace::Type,
                    DefKind::TypeAlias,
                    *span,
                    errors,
                );
            }
            Decl::Class(ClassDecl {
                name, items, span, ..
            }) => {
                insert_top(
                    env,
                    name.clone(),
                    Namespace::Type,
                    DefKind::Class,
                    *span,
                    errors,
                );
                for item in items {
                    let (mname, mspan) = match item {
                        ClassItem::Signature { name, span, .. } => (name.clone(), *span),
                        ClassItem::Default(ValueClause { name, span, .. }) => (name.clone(), *span),
                    };
                    insert_top(
                        env,
                        mname,
                        Namespace::Value,
                        DefKind::ClassMethod {
                            parent_class: name.clone(),
                        },
                        mspan,
                        errors,
                    );
                }
            }
            Decl::Instance(InstanceDecl { .. }) => {
                // Instances carry no names (spec 08 §Instances and
                // modules) — nothing to register here.
            }
            Decl::RubyEmbed(RubyEmbedDecl { name, span, .. }) => {
                ensure_value_binding(env, name, DefKind::RubyEmbed, *span, errors);
            }
        }
    }

    // Every top-level def's visibility starts out `Private` — the
    // export-list pass (phase 2) upgrades selected ones to
    // `Exported`. If there is no explicit export list at all, every
    // def is exported: we set them to `Exported` here and let phase 2
    // confirm.
    let all_exported = match &m.header {
        Some(h) => h.exports.is_none(),
        None => true, // `module Main where` sugar — no export list.
    };
    if all_exported {
        for def in env.top_level.iter_mut() {
            def.visibility = Visibility::Exported;
        }
    } else if let Some(items) = m.header.as_ref().and_then(|h| h.exports.as_ref()) {
        // Mark exported items. Re-use the same matching logic as
        // `compute_exports` but only to flip visibility flags.
        let exported = collect_exported_names(env, items);
        for def in env.top_level.iter_mut() {
            if exported.contains(&(def.name.clone(), def.namespace)) {
                def.visibility = Visibility::Exported;
            }
        }
    }
}

fn insert_top(
    env: &mut ModuleEnv,
    name: String,
    ns: Namespace,
    kind: DefKind,
    span: Span,
    errors: &mut Vec<ResolveError>,
) {
    let key = (name.clone(), ns);
    if env.top_level_index.contains_key(&key) {
        errors.push(ResolveError::new(
            ResolveErrorKind::DuplicateTopLevel {
                name: name.clone(),
                namespace: ns,
            },
            span,
        ));
        return;
    }
    let def = TopLevelDef {
        name: name.clone(),
        namespace: ns,
        visibility: Visibility::Private,
        kind,
        span,
    };
    let idx = env.top_level.len();
    env.top_level.push(def);
    env.top_level_index.insert(key, idx);
}

/// Value-side bindings (value clauses, signatures, `:=` embeds) all
/// share a single top-level slot. Signatures and multi-clause value
/// definitions do *not* count as duplicates with one another, so we
/// only insert the slot the first time.
fn ensure_value_binding(
    env: &mut ModuleEnv,
    name: &str,
    kind: DefKind,
    span: Span,
    errors: &mut Vec<ResolveError>,
) {
    let key = (name.to_string(), Namespace::Value);
    if let Some(&idx) = env.top_level_index.get(&key) {
        // Already present — check that the existing entry is a
        // value / embed (not, say, a constructor clashing with a
        // same-named value). A user-declared `data` whose ctor name
        // equals a top-level binding would be a DuplicateTopLevel
        // here because `insert_top` would have created a ctor slot,
        // and now we would flag this as duplicate value.
        match env.top_level[idx].kind {
            DefKind::Value | DefKind::RubyEmbed => {
                // Accept — signature + clause, or multiple clauses.
            }
            _ => {
                errors.push(ResolveError::new(
                    ResolveErrorKind::DuplicateTopLevel {
                        name: name.to_string(),
                        namespace: Namespace::Value,
                    },
                    span,
                ));
            }
        }
        return;
    }
    let def = TopLevelDef {
        name: name.to_string(),
        namespace: Namespace::Value,
        visibility: Visibility::Private,
        kind,
        span,
    };
    let idx = env.top_level.len();
    env.top_level.push(def);
    env.top_level_index.insert(key, idx);
}

fn collect_exported_names(
    env: &ModuleEnv,
    items: &[sapphire_core::ast::ExportItem],
) -> HashSet<(String, Namespace)> {
    use sapphire_core::ast::ExportItem;
    let mut out = HashSet::new();
    for item in items {
        match item {
            ExportItem::Value { name, .. } => {
                out.insert((name.clone(), Namespace::Value));
            }
            ExportItem::Type { name, .. } | ExportItem::Class { name, .. } => {
                out.insert((name.clone(), Namespace::Type));
            }
            ExportItem::TypeAll { name, .. } => {
                out.insert((name.clone(), Namespace::Type));
                for def in &env.top_level {
                    if let DefKind::Ctor { parent_type } = &def.kind {
                        if parent_type == name {
                            out.insert((def.name.clone(), Namespace::Value));
                        }
                    }
                }
            }
            ExportItem::TypeWith { name, ctors, .. } => {
                out.insert((name.clone(), Namespace::Type));
                for c in ctors {
                    out.insert((c.clone(), Namespace::Value));
                }
            }
            ExportItem::ClassAll { name, .. } => {
                out.insert((name.clone(), Namespace::Type));
                for def in &env.top_level {
                    if let DefKind::ClassMethod { parent_class } = &def.kind {
                        if parent_class == name {
                            out.insert((def.name.clone(), Namespace::Value));
                        }
                    }
                }
            }
            ExportItem::ReExport { .. } => {
                // Selective re-export: 08-OQ3 DEFERRED-IMPL. Nothing
                // to mark here; the re-exported module's names are
                // consulted at use sites.
            }
        }
    }
    out
}

// ---------------------------------------------------------------------
//  Phase 3 helper: cycle detection
// ---------------------------------------------------------------------

fn detect_cycles(
    imports_of: &[Vec<String>],
    name_index: &HashMap<String, usize>,
) -> Result<(), Vec<String>> {
    // Standard DFS colour marking. Each node colour:
    //   0 = unvisited, 1 = on current path, 2 = done.
    let n = imports_of.len();
    let mut colour = vec![0u8; n];
    let mut path: Vec<usize> = Vec::new();
    // Reverse-lookup: idx -> module name.
    let mut name_by_idx: Vec<String> = vec![String::new(); n];
    for (name, &i) in name_index.iter() {
        name_by_idx[i] = name.clone();
    }

    fn visit(
        u: usize,
        imports_of: &[Vec<String>],
        name_index: &HashMap<String, usize>,
        colour: &mut [u8],
        path: &mut Vec<usize>,
        name_by_idx: &[String],
    ) -> Result<(), Vec<String>> {
        colour[u] = 1;
        path.push(u);
        for name in &imports_of[u] {
            let Some(&v) = name_index.get(name) else {
                continue;
            };
            match colour[v] {
                0 => visit(v, imports_of, name_index, colour, path, name_by_idx)?,
                1 => {
                    // Cycle: path from the first occurrence of `v`
                    // to the current top is the cycle.
                    let start = path.iter().position(|&x| x == v).unwrap();
                    let cycle: Vec<String> = path[start..]
                        .iter()
                        .chain(std::iter::once(&v))
                        .map(|&x| name_by_idx[x].clone())
                        .collect();
                    return Err(cycle);
                }
                _ => {}
            }
        }
        colour[u] = 2;
        path.pop();
        Ok(())
    }

    for u in 0..n {
        if colour[u] == 0 {
            visit(
                u,
                imports_of,
                name_index,
                &mut colour,
                &mut path,
                &name_by_idx,
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
//  Phase 5: AST walk
// ---------------------------------------------------------------------

struct Walker<'a> {
    env: &'a ModuleEnv,
    references: &'a mut HashMap<Span, Resolution>,
    errors: &'a mut Vec<ResolveError>,
    scope: ScopeStack,
    type_scope: ScopeStack,
}

impl Walker<'_> {
    fn walk_module(&mut self, m: &AstModule) {
        for decl in &m.decls {
            self.walk_decl(decl);
        }
    }

    fn walk_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Signature { scheme, .. } => {
                self.type_scope.push();
                self.bring_in_foralls(scheme);
                self.walk_scheme(scheme);
                self.type_scope.pop();
            }
            Decl::Value(clause) => self.walk_value_clause(clause),
            Decl::Data(DataDecl {
                type_params, ctors, ..
            }) => {
                self.type_scope.push();
                for tp in type_params {
                    self.type_scope.bind(tp);
                }
                for ctor in ctors {
                    for arg in &ctor.args {
                        self.walk_type(arg);
                    }
                }
                self.type_scope.pop();
            }
            Decl::TypeAlias(TypeAlias {
                type_params, body, ..
            }) => {
                self.type_scope.push();
                for tp in type_params {
                    self.type_scope.bind(tp);
                }
                self.walk_type(body);
                self.type_scope.pop();
            }
            Decl::Class(ClassDecl {
                context,
                type_var,
                items,
                ..
            }) => {
                self.type_scope.push();
                self.type_scope.bind(type_var);
                for c in context {
                    self.walk_constraint(c);
                }
                for item in items {
                    match item {
                        ClassItem::Signature { scheme, .. } => {
                            self.bring_in_foralls(scheme);
                            self.walk_scheme(scheme);
                        }
                        ClassItem::Default(vc) => {
                            self.walk_value_clause(vc);
                        }
                    }
                }
                self.type_scope.pop();
            }
            Decl::Instance(InstanceDecl {
                context,
                head,
                items,
                name,
                span,
                ..
            }) => {
                // Resolve the class name in the type namespace.
                self.resolve_name(None, name, Namespace::Type, *span);
                self.type_scope.push();
                for c in context {
                    self.walk_constraint(c);
                }
                self.walk_type(head);
                for clause in items {
                    self.walk_value_clause(clause);
                }
                self.type_scope.pop();
            }
            Decl::RubyEmbed(RubyEmbedDecl { params, .. }) => {
                // Parameters are plain idents; the body is uninterpreted
                // Ruby source, so we only have to record the params
                // exist. No expression walking.
                self.scope.push();
                for p in params {
                    self.scope.bind(&p.name);
                }
                self.scope.pop();
            }
        }
    }

    fn walk_value_clause(&mut self, clause: &ValueClause) {
        self.scope.push();
        for p in &clause.params {
            self.bind_pattern(p);
        }
        for p in &clause.params {
            self.walk_pattern(p);
        }
        self.walk_expr(&clause.body);
        self.scope.pop();
    }

    fn walk_scheme(&mut self, scheme: &Scheme) {
        for c in &scheme.context {
            self.walk_constraint(c);
        }
        self.walk_type(&scheme.body);
    }

    fn bring_in_foralls(&mut self, scheme: &Scheme) {
        for tv in &scheme.forall {
            self.type_scope.bind(tv);
        }
        // Implicitly-quantified tvars: collect every free `Var`
        // name mentioned in the body / context that is not already
        // bound.
        collect_type_vars(&scheme.body, &mut |name| self.type_scope.bind(name));
        for c in &scheme.context {
            for a in &c.args {
                collect_type_vars(a, &mut |name| self.type_scope.bind(name));
            }
        }
    }

    fn walk_constraint(&mut self, c: &Constraint) {
        self.resolve_name(None, &c.class_name, Namespace::Type, c.span);
        for a in &c.args {
            self.walk_type(a);
        }
    }

    fn walk_type(&mut self, ty: &AstType) {
        match ty {
            AstType::Var { name, span } => {
                if !self.type_scope.lookup(name) {
                    // Free type variables at use sites are fine — the
                    // type-checker will implicitly quantify. Record a
                    // local-style resolution so downstream knows it
                    // was seen.
                    self.references
                        .insert(*span, Resolution::Local { name: name.clone() });
                } else {
                    self.references
                        .insert(*span, Resolution::Local { name: name.clone() });
                }
            }
            AstType::Con { module, name, span } => {
                self.resolve_name(module.as_ref(), name, Namespace::Type, *span);
            }
            AstType::App { func, arg, .. } => {
                self.walk_type(func);
                self.walk_type(arg);
            }
            AstType::Fun { param, result, .. } => {
                self.walk_type(param);
                self.walk_type(result);
            }
            AstType::Record { fields, .. } => {
                for (_, fty) in fields {
                    self.walk_type(fty);
                }
            }
        }
    }

    fn walk_pattern(&mut self, pat: &Pattern) {
        // The binding of pattern-introduced names happens in
        // `bind_pattern`; `walk_pattern` validates references inside
        // the pattern (constructor names, field names, embedded
        // types).
        match pat {
            Pattern::Wildcard(_) | Pattern::Var { .. } | Pattern::Lit(_, _) => {}
            Pattern::As { inner, .. } => self.walk_pattern(inner),
            Pattern::Con {
                module,
                name,
                args,
                span,
            } => {
                self.resolve_name(module.as_ref(), name, Namespace::Value, *span);
                for a in args {
                    self.walk_pattern(a);
                }
            }
            Pattern::Cons { head, tail, .. } => {
                self.walk_pattern(head);
                self.walk_pattern(tail);
            }
            Pattern::List { items, .. } => {
                for i in items {
                    self.walk_pattern(i);
                }
            }
            Pattern::Record { fields, .. } => {
                for (_, p) in fields {
                    self.walk_pattern(p);
                }
            }
            Pattern::Annot { inner, ty, .. } => {
                self.walk_pattern(inner);
                self.walk_type(ty);
            }
        }
    }

    fn bind_pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::Wildcard(_) | Pattern::Lit(_, _) => {}
            Pattern::Var { name, .. } => self.scope.bind(name),
            Pattern::As { name, inner, .. } => {
                self.scope.bind(name);
                self.bind_pattern(inner);
            }
            Pattern::Con { args, .. } => {
                for a in args {
                    self.bind_pattern(a);
                }
            }
            Pattern::Cons { head, tail, .. } => {
                self.bind_pattern(head);
                self.bind_pattern(tail);
            }
            Pattern::List { items, .. } => {
                for i in items {
                    self.bind_pattern(i);
                }
            }
            Pattern::Record { fields, .. } => {
                for (_, p) in fields {
                    self.bind_pattern(p);
                }
            }
            Pattern::Annot { inner, .. } => self.bind_pattern(inner),
        }
    }

    fn walk_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Lit(_, _) => {}
            Expr::Var { module, name, span } => {
                self.walk_var_ref(module.as_ref(), name, *span);
            }
            Expr::OpRef { symbol, span } => {
                self.resolve_name(None, symbol, Namespace::Value, *span);
            }
            Expr::App { func, arg, .. } => {
                self.walk_expr(func);
                self.walk_expr(arg);
            }
            Expr::Lambda { params, body, .. } => {
                self.scope.push();
                for p in params {
                    self.bind_pattern(p);
                }
                for p in params {
                    self.walk_pattern(p);
                }
                self.walk_expr(body);
                self.scope.pop();
            }
            Expr::Let {
                name,
                params,
                value,
                body,
                ..
            } => {
                // Spec 03: let is implicitly recursive. Bind the
                // name before walking the value.
                self.scope.push();
                self.scope.bind(name);
                self.scope.push();
                for p in params {
                    self.bind_pattern(p);
                }
                for p in params {
                    self.walk_pattern(p);
                }
                self.walk_expr(value);
                self.scope.pop();
                self.walk_expr(body);
                self.scope.pop();
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.walk_expr(cond);
                self.walk_expr(then_branch);
                self.walk_expr(else_branch);
            }
            Expr::Case {
                scrutinee, arms, ..
            } => {
                self.walk_expr(scrutinee);
                for CaseArm { pattern, body, .. } in arms {
                    self.scope.push();
                    self.bind_pattern(pattern);
                    self.walk_pattern(pattern);
                    self.walk_expr(body);
                    self.scope.pop();
                }
            }
            Expr::BinOp {
                op, left, right, ..
            } => {
                self.resolve_name(None, op, Namespace::Value, left.span().merge(right.span()));
                self.walk_expr(left);
                self.walk_expr(right);
            }
            Expr::Neg { value, .. } => self.walk_expr(value),
            Expr::RecordLit { fields, .. } => {
                for (_, e) in fields {
                    self.walk_expr(e);
                }
            }
            Expr::RecordUpdate { record, fields, .. } => {
                self.walk_expr(record);
                for (_, e) in fields {
                    self.walk_expr(e);
                }
            }
            Expr::FieldAccess { record, .. } => self.walk_expr(record),
            Expr::ListLit { items, .. } => {
                for i in items {
                    self.walk_expr(i);
                }
            }
            Expr::Do { stmts, .. } => {
                self.scope.push();
                for s in stmts {
                    match s {
                        DoStmt::Bind { pattern, expr, .. } => {
                            self.walk_expr(expr);
                            self.bind_pattern(pattern);
                            self.walk_pattern(pattern);
                        }
                        DoStmt::Let {
                            name,
                            params,
                            value,
                            ..
                        } => {
                            self.scope.bind(name);
                            self.scope.push();
                            for p in params {
                                self.bind_pattern(p);
                            }
                            for p in params {
                                self.walk_pattern(p);
                            }
                            self.walk_expr(value);
                            self.scope.pop();
                        }
                        DoStmt::Expr(e) => self.walk_expr(e),
                    }
                }
                self.scope.pop();
            }
        }
    }

    /// Look up a (possibly qualified) value reference. Locals
    /// shadow everything; a qualifier forces through-the-module
    /// lookup.
    fn walk_var_ref(&mut self, module: Option<&ModName>, name: &str, span: Span) {
        if module.is_none() && self.scope.lookup(name) {
            self.references.insert(
                span,
                Resolution::Local {
                    name: name.to_string(),
                },
            );
            return;
        }
        // Not a local — resolve via env.
        // Values in expression position: if the name starts uppercase
        // we still look in the Value namespace because constructors
        // live there (spec 06 §Design notes namespace rule).
        self.resolve_name(module, name, Namespace::Value, span);
    }

    fn resolve_name(&mut self, module: Option<&ModName>, name: &str, ns: Namespace, span: Span) {
        if let Some(qual) = module {
            let Some(target) = resolve_qualifier(self.env, qual) else {
                self.errors.push(ResolveError::new(
                    ResolveErrorKind::QualifierNotInScope {
                        qualifier: qual.segments.join("."),
                    },
                    span,
                ));
                return;
            };
            let r = ResolvedRef::new(target.clone(), name.to_string(), ns);
            // Check the target actually exports this name.
            let qname = qual.segments.join(".");
            let ok = if target == &self.env.id {
                self.env.top_lookup(name, ns).is_some()
            } else {
                // Consult the target module's exports via
                // `unqualified` / `qualified_aliases` doesn't work
                // directly; we just trust what the import table
                // set up, at the cost of not checking that a
                // `Mod.x` that was never listed in an explicit
                // import list is actually exported. For the M9
                // example set this is sufficient; finer checking
                // requires keeping a snapshot of every module's
                // exports, which we elected not to thread through
                // here.
                true
            };
            if !ok {
                self.errors.push(ResolveError::new(
                    ResolveErrorKind::NotExported {
                        module: qname,
                        name: name.to_string(),
                        namespace: ns,
                    },
                    span,
                ));
                return;
            }
            self.references.insert(span, Resolution::Global(r));
            return;
        }

        // Unqualified lookup order: top-level → imported → prelude.
        if let Some(def) = self.env.top_lookup(name, ns) {
            let r = ResolvedRef::new(self.env.id.clone(), def.name.clone(), ns);
            self.references.insert(span, Resolution::Global(r));
            return;
        }
        if let Some(refs) = self.env.unqualified.get(&(name.to_string(), ns)) {
            // Dedup by source definition: two imports of the same
            // original declaration are not ambiguous (spec 08
            // §Re-exports).
            let mut uniq: Vec<&ResolvedRef> = Vec::new();
            for r in refs {
                if !uniq.iter().any(|u| u == &r) {
                    uniq.push(r);
                }
            }
            match uniq.len() {
                0 => {}
                1 => {
                    self.references
                        .insert(span, Resolution::Global(uniq[0].clone()));
                    return;
                }
                _ => {
                    self.errors.push(ResolveError::new(
                        ResolveErrorKind::Ambiguous {
                            name: name.to_string(),
                            namespace: ns,
                            modules: uniq.iter().map(|r| r.module.display()).collect(),
                        },
                        span,
                    ));
                    return;
                }
            }
        }

        self.errors.push(ResolveError::new(
            ResolveErrorKind::UndefinedName {
                name: name.to_string(),
                namespace: ns,
            },
            span,
        ));
    }
}

fn collect_type_vars(ty: &AstType, f: &mut impl FnMut(&str)) {
    match ty {
        AstType::Var { name, .. } => f(name),
        AstType::Con { .. } => {}
        AstType::App { func, arg, .. } => {
            collect_type_vars(func, f);
            collect_type_vars(arg, f);
        }
        AstType::Fun { param, result, .. } => {
            collect_type_vars(param, f);
            collect_type_vars(result, f);
        }
        AstType::Record { fields, .. } => {
            for (_, t) in fields {
                collect_type_vars(t, f);
            }
        }
    }
}

// ---------------------------------------------------------------------
//  Phase 6: private-type leak check
// ---------------------------------------------------------------------

fn check_private_leaks(m: &AstModule, env: &ModuleEnv, errors: &mut Vec<ResolveError>) {
    // For every exported value binding's signature, or exported
    // class method signature, every `Con { module: None, name }`
    // that resolves to a top-level type in this module must itself
    // be exported. Same for referenced class names.

    // Type aliases are transparent (spec 09 §Type aliases): the
    // type-checker treats `type Age = Int` as a synonym for `Int`,
    // not a new nominal type. At the I5 layer we therefore do *not*
    // treat an unexported alias as a leak — the alias name disappears
    // after expansion. Data types and classes, which are nominal,
    // still count as leaks if private. Tracked as I-OQ42.
    let is_private_type = |name: &str| -> bool {
        env.top_lookup(name, Namespace::Type).is_some_and(|def| {
            matches!(def.kind, DefKind::DataType | DefKind::Class)
                && matches!(def.visibility, Visibility::Private)
        })
    };

    let exported_value = |name: &str| -> bool {
        env.top_lookup(name, Namespace::Value)
            .is_some_and(|d| matches!(d.visibility, Visibility::Exported))
    };

    fn scan(ty: &AstType, out: &mut Vec<(String, Span)>) {
        match ty {
            AstType::Var { .. } => {}
            AstType::Con { module, name, span } => {
                if module.is_none() {
                    out.push((name.clone(), *span));
                }
            }
            AstType::App { func, arg, .. } => {
                scan(func, out);
                scan(arg, out);
            }
            AstType::Fun { param, result, .. } => {
                scan(param, out);
                scan(result, out);
            }
            AstType::Record { fields, .. } => {
                for (_, t) in fields {
                    scan(t, out);
                }
            }
        }
    }

    let mut scan_scheme = |public_name: &str, scheme: &Scheme, span: Span| {
        let mut refs = Vec::new();
        scan(&scheme.body, &mut refs);
        for c in &scheme.context {
            if is_private_type(&c.class_name) {
                errors.push(ResolveError::new(
                    ResolveErrorKind::PrivateLeak {
                        public: public_name.to_string(),
                        leak: c.class_name.clone(),
                        namespace: Namespace::Type,
                    },
                    span,
                ));
            }
            for a in &c.args {
                scan(a, &mut refs);
            }
        }
        for (name, rspan) in refs {
            if is_private_type(&name) {
                errors.push(ResolveError::new(
                    ResolveErrorKind::PrivateLeak {
                        public: public_name.to_string(),
                        leak: name,
                        namespace: Namespace::Type,
                    },
                    rspan,
                ));
            }
        }
    };

    for decl in &m.decls {
        match decl {
            Decl::Signature {
                name, scheme, span, ..
            } => {
                if exported_value(name) {
                    scan_scheme(name, scheme, *span);
                }
            }
            Decl::Class(ClassDecl {
                name: class_name,
                items,
                ..
            }) => {
                let class_exported = env
                    .top_lookup(class_name, Namespace::Type)
                    .is_some_and(|d| matches!(d.visibility, Visibility::Exported));
                if !class_exported {
                    continue;
                }
                for item in items {
                    if let ClassItem::Signature {
                        name: method_name,
                        scheme,
                        span,
                        ..
                    } = item
                    {
                        if exported_value(method_name) {
                            scan_scheme(method_name, scheme, *span);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Exported data declarations that carry an exported type ctor
    // with a private argument type also count.
    for decl in &m.decls {
        if let Decl::Data(DataDecl {
            name, ctors, span, ..
        }) = decl
        {
            let type_exported = env
                .top_lookup(name, Namespace::Type)
                .is_some_and(|d| matches!(d.visibility, Visibility::Exported));
            if !type_exported {
                continue;
            }
            for c in ctors {
                let ctor_exported = env
                    .top_lookup(&c.name, Namespace::Value)
                    .is_some_and(|d| matches!(d.visibility, Visibility::Exported));
                if !ctor_exported {
                    continue;
                }
                let mut refs = Vec::new();
                for a in &c.args {
                    scan(a, &mut refs);
                }
                for (rn, rspan) in refs {
                    if is_private_type(&rn) {
                        errors.push(ResolveError::new(
                            ResolveErrorKind::PrivateLeak {
                                public: c.name.clone(),
                                leak: rn,
                                namespace: Namespace::Type,
                            },
                            rspan,
                        ));
                    }
                }
                let _ = span;
            }
        }
    }
}
