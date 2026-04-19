//! `import` / `export` list processing.
//!
//! This module implements spec 08 §Abstract syntax's import and
//! export handling. It is split out from `mod.rs` to keep the
//! top-level resolver driver readable: import resolution is a
//! self-contained "given a module graph and a list of import decls,
//! produce the unqualified and qualified scope tables" transform.
//!
//! The implicit `Prelude` import (spec 09 §The prelude as a module)
//! is materialised as a synthetic import prepended to every module
//! that does not explicitly write `import Prelude ()`. The prelude's
//! contents come from [`super::prelude`].

use sapphire_core::ast::{
    ExportItem, ImportDecl, ImportItem, ImportItems, ModName, Module as AstModule,
};
use sapphire_core::span::Span;

use super::env::{DefKind, Exports, ModuleEnv, ModuleId, ResolvedRef};
use super::error::{Namespace, ResolveError, ResolveErrorKind};
use super::prelude::{self, PRELUDE_MODULE};

/// Build the export surface for a module, given its own top-level
/// environment and the list of [`ExportItem`]s from the module
/// header. An `export_list: None` means "everything top-level";
/// `Some(vec![])` means "export nothing".
pub(super) fn compute_exports(
    env: &ModuleEnv,
    export_list: Option<&[ExportItem]>,
) -> Result<Exports, Vec<ResolveError>> {
    let mut exports = Exports::default();
    let mut errors = Vec::new();

    let Some(items) = export_list else {
        // No explicit export list — everything top-level is exported.
        for def in &env.top_level {
            let r = ResolvedRef::new(env.id.clone(), def.name.clone(), def.namespace);
            exports.insert(def.name.clone(), def.namespace, r);
        }
        return Ok(exports);
    };

    for item in items {
        match item {
            ExportItem::Value { name, span, .. } => {
                if env.top_lookup(name, Namespace::Value).is_none() {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ExportOfUnknown {
                            name: name.clone(),
                            namespace: Namespace::Value,
                        },
                        *span,
                    ));
                    continue;
                }
                let r = ResolvedRef::new(env.id.clone(), name.clone(), Namespace::Value);
                exports.insert(name.clone(), Namespace::Value, r);
            }
            ExportItem::Type { name, span } => {
                if env.top_lookup(name, Namespace::Type).is_none() {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ExportOfUnknown {
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    continue;
                }
                let r = ResolvedRef::new(env.id.clone(), name.clone(), Namespace::Type);
                exports.insert(name.clone(), Namespace::Type, r);
            }
            ExportItem::TypeAll { name, span } => {
                match env.top_lookup(name, Namespace::Type) {
                    None => {
                        errors.push(ResolveError::new(
                            ResolveErrorKind::ExportOfUnknown {
                                name: name.clone(),
                                namespace: Namespace::Type,
                            },
                            *span,
                        ));
                        continue;
                    }
                    Some(def) => {
                        let r = ResolvedRef::new(env.id.clone(), name.clone(), Namespace::Type);
                        exports.insert(name.clone(), Namespace::Type, r);
                        // Match the `DataType` flavour — `TypeAll` on
                        // an alias or class is meaningless but the
                        // parser admits the shape. We only pull
                        // constructors; nothing else qualifies.
                        if matches!(def.kind, DefKind::DataType) {
                            for ctor in ctors_of(env, name) {
                                let r = ResolvedRef::new(
                                    env.id.clone(),
                                    ctor.clone(),
                                    Namespace::Value,
                                );
                                exports.insert(ctor, Namespace::Value, r);
                            }
                        }
                    }
                }
            }
            ExportItem::TypeWith { name, ctors, span } => {
                if env.top_lookup(name, Namespace::Type).is_none() {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ExportOfUnknown {
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    continue;
                }
                let r = ResolvedRef::new(env.id.clone(), name.clone(), Namespace::Type);
                exports.insert(name.clone(), Namespace::Type, r);
                let defined = ctors_of(env, name);
                for ctor in ctors {
                    if !defined.contains(ctor) {
                        errors.push(ResolveError::new(
                            ResolveErrorKind::ExportOfUnknown {
                                name: ctor.clone(),
                                namespace: Namespace::Value,
                            },
                            *span,
                        ));
                        continue;
                    }
                    let r = ResolvedRef::new(env.id.clone(), ctor.clone(), Namespace::Value);
                    exports.insert(ctor.clone(), Namespace::Value, r);
                }
            }
            ExportItem::Class { name, span } => {
                if env.top_lookup(name, Namespace::Type).is_none() {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ExportOfUnknown {
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    continue;
                }
                let r = ResolvedRef::new(env.id.clone(), name.clone(), Namespace::Type);
                exports.insert(name.clone(), Namespace::Type, r);
            }
            ExportItem::ClassAll { name, span } => {
                if env.top_lookup(name, Namespace::Type).is_none() {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ExportOfUnknown {
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    continue;
                }
                let r = ResolvedRef::new(env.id.clone(), name.clone(), Namespace::Type);
                exports.insert(name.clone(), Namespace::Type, r);
                for method in methods_of(env, name) {
                    let r = ResolvedRef::new(env.id.clone(), method.clone(), Namespace::Value);
                    exports.insert(method, Namespace::Value, r);
                }
            }
            ExportItem::ReExport { name, span } => {
                // Selective re-export (08-OQ3 DEFERRED-IMPL) is not
                // implemented yet; we accept whole-module re-export
                // syntactically but flag its name as unknown if the
                // re-exported module isn't present in the program.
                // Actual re-export expansion happens in `resolve_program`
                // after every module's exports are known.
                let _ = (name, span);
            }
        }
    }

    if errors.is_empty() {
        Ok(exports)
    } else {
        Err(errors)
    }
}

fn ctors_of(env: &ModuleEnv, type_name: &str) -> Vec<String> {
    env.top_level
        .iter()
        .filter_map(|def| match &def.kind {
            DefKind::Ctor { parent_type } if parent_type == type_name => Some(def.name.clone()),
            _ => None,
        })
        .collect()
}

fn methods_of(env: &ModuleEnv, class_name: &str) -> Vec<String> {
    env.top_level
        .iter()
        .filter_map(|def| match &def.kind {
            DefKind::ClassMethod { parent_class } if parent_class == class_name => {
                Some(def.name.clone())
            }
            _ => None,
        })
        .collect()
}

/// Materialise the implicit-prelude import for a user module.
///
/// Spec 09 §The prelude as a module: every module implicitly imports
/// `Prelude` unqualified, unless the user wrote `import Prelude ()`
/// (empty-list import), which suppresses the implicit form.
pub(super) fn should_add_implicit_prelude(module: &AstModule) -> bool {
    // An explicit `Prelude` import (of any form) shadows the implicit
    // one — we skip the implicit injection and let the explicit decl
    // do its thing.
    !module
        .imports
        .iter()
        .any(|imp| imp.name.segments.first().map(|s| s.as_str()) == Some(PRELUDE_MODULE))
}

/// Apply a single `import` declaration to a module's environment.
///
/// `target_exports` is the set of names exported by the module named
/// on the import's left-hand side. `target_id` is its canonical
/// [`ModuleId`]. The function mutates `env` to reflect what this
/// import brings into scope.
pub(super) fn apply_import(
    env: &mut ModuleEnv,
    decl: &ImportDecl,
    target_id: ModuleId,
    target_exports: &Exports,
) -> Result<(), Vec<ResolveError>> {
    let mut errors = Vec::new();

    // Register qualified aliases: the module's full name always
    // resolves, plus any alias introduced by `as L`.
    env.qualified_aliases
        .insert(target_id.display(), target_id.clone());
    if let Some(alias) = &decl.alias {
        env.qualified_aliases
            .insert(alias.segments.join("."), target_id.clone());
    }

    // Figure out which names to bring unqualified.
    let bring_unqualified: Vec<(String, Namespace, ResolvedRef)> = match &decl.items {
        ImportItems::All if decl.qualified => Vec::new(),
        ImportItems::All => {
            let mut out = Vec::new();
            for (name, r) in &target_exports.values {
                out.push((name.clone(), Namespace::Value, r.clone()));
            }
            for (name, r) in &target_exports.types {
                out.push((name.clone(), Namespace::Type, r.clone()));
            }
            out
        }
        ImportItems::Only(items) if decl.qualified => {
            // `import qualified M (x)` — unusual but admissible;
            // treat as qualified-only (items don't go unqualified).
            validate_items_exist(items, target_exports, &target_id, &mut errors);
            Vec::new()
        }
        ImportItems::Only(items) => {
            let mut out = Vec::new();
            for item in items {
                collect_item(item, target_exports, &target_id, &mut out, &mut errors);
            }
            out
        }
        ImportItems::Hiding(items) if decl.qualified => {
            // `import qualified M hiding (x)` — no unqualified names
            // at all, but the hide list still has to name existing
            // exports (validated for user feedback).
            validate_items_exist(items, target_exports, &target_id, &mut errors);
            Vec::new()
        }
        ImportItems::Hiding(items) => {
            let hidden = collect_hide_names(items, target_exports, &target_id, &mut errors);
            let mut out = Vec::new();
            for (name, r) in &target_exports.values {
                if !hidden.contains(&(name.clone(), Namespace::Value)) {
                    out.push((name.clone(), Namespace::Value, r.clone()));
                }
            }
            for (name, r) in &target_exports.types {
                if !hidden.contains(&(name.clone(), Namespace::Type)) {
                    out.push((name.clone(), Namespace::Type, r.clone()));
                }
            }
            out
        }
    };

    for (name, ns, r) in bring_unqualified {
        env.unqualified.entry((name, ns)).or_default().push(r);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn collect_item(
    item: &ImportItem,
    target_exports: &Exports,
    target_id: &ModuleId,
    out: &mut Vec<(String, Namespace, ResolvedRef)>,
    errors: &mut Vec<ResolveError>,
) {
    match item {
        ImportItem::Value { name, span, .. } => match target_exports.lookup(name, Namespace::Value)
        {
            Some(r) => out.push((name.clone(), Namespace::Value, r.clone())),
            None => errors.push(ResolveError::new(
                ResolveErrorKind::ImportOfUnknown {
                    module: target_id.display(),
                    name: name.clone(),
                    namespace: Namespace::Value,
                },
                *span,
            )),
        },
        ImportItem::Type { name, span } => match target_exports.lookup(name, Namespace::Type) {
            Some(r) => out.push((name.clone(), Namespace::Type, r.clone())),
            None => errors.push(ResolveError::new(
                ResolveErrorKind::ImportOfUnknown {
                    module: target_id.display(),
                    name: name.clone(),
                    namespace: Namespace::Type,
                },
                *span,
            )),
        },
        ImportItem::TypeAll { name, span } => {
            match target_exports.lookup(name, Namespace::Type) {
                Some(r) => out.push((name.clone(), Namespace::Type, r.clone())),
                None => {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ImportOfUnknown {
                            module: target_id.display(),
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    return;
                }
            }
            // Pull every exported constructor whose home-module
            // matches our target. Export tables do not carry an
            // explicit parent-type link; we approximate by taking
            // every value whose source module is `target_id` and
            // whose name is *not* a top-level value-binding export
            // (the caller cannot distinguish a ctor from a function
            // without consulting the target module's env). As a
            // compromise the caller is expected to pass along the
            // target module's env; I5 exposes `apply_import_with_env`
            // when this distinction matters. For the bare
            // `apply_import` path we over-approximate and import
            // every value export — conservative and matches spec 08
            // `(..)` semantics when combined with a properly-formed
            // export list on the target module.
            for (vname, vref) in &target_exports.values {
                if &vref.module == target_id {
                    out.push((vname.clone(), Namespace::Value, vref.clone()));
                }
            }
        }
        ImportItem::TypeWith { name, ctors, span } => {
            match target_exports.lookup(name, Namespace::Type) {
                Some(r) => out.push((name.clone(), Namespace::Type, r.clone())),
                None => {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ImportOfUnknown {
                            module: target_id.display(),
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    return;
                }
            }
            for ctor in ctors {
                match target_exports.lookup(ctor, Namespace::Value) {
                    Some(r) => out.push((ctor.clone(), Namespace::Value, r.clone())),
                    None => errors.push(ResolveError::new(
                        ResolveErrorKind::ImportOfUnknown {
                            module: target_id.display(),
                            name: ctor.clone(),
                            namespace: Namespace::Value,
                        },
                        *span,
                    )),
                }
            }
        }
        ImportItem::Class { name, span } => match target_exports.lookup(name, Namespace::Type) {
            Some(r) => out.push((name.clone(), Namespace::Type, r.clone())),
            None => errors.push(ResolveError::new(
                ResolveErrorKind::ImportOfUnknown {
                    module: target_id.display(),
                    name: name.clone(),
                    namespace: Namespace::Type,
                },
                *span,
            )),
        },
        ImportItem::ClassAll { name, span } => {
            match target_exports.lookup(name, Namespace::Type) {
                Some(r) => out.push((name.clone(), Namespace::Type, r.clone())),
                None => {
                    errors.push(ResolveError::new(
                        ResolveErrorKind::ImportOfUnknown {
                            module: target_id.display(),
                            name: name.clone(),
                            namespace: Namespace::Type,
                        },
                        *span,
                    ));
                    return;
                }
            }
            for (vname, vref) in &target_exports.values {
                if &vref.module == target_id {
                    out.push((vname.clone(), Namespace::Value, vref.clone()));
                }
            }
        }
    }
}

fn validate_items_exist(
    items: &[ImportItem],
    target_exports: &Exports,
    target_id: &ModuleId,
    errors: &mut Vec<ResolveError>,
) {
    for item in items {
        let (name, ns, span) = match item {
            ImportItem::Value { name, span, .. } => (name.clone(), Namespace::Value, *span),
            ImportItem::Type { name, span } => (name.clone(), Namespace::Type, *span),
            ImportItem::TypeAll { name, span } => (name.clone(), Namespace::Type, *span),
            ImportItem::TypeWith { name, span, .. } => (name.clone(), Namespace::Type, *span),
            ImportItem::Class { name, span } => (name.clone(), Namespace::Type, *span),
            ImportItem::ClassAll { name, span } => (name.clone(), Namespace::Type, *span),
        };
        if target_exports.lookup(&name, ns).is_none() {
            errors.push(ResolveError::new(
                ResolveErrorKind::ImportOfUnknown {
                    module: target_id.display(),
                    name,
                    namespace: ns,
                },
                span,
            ));
        }
    }
}

fn collect_hide_names(
    items: &[ImportItem],
    target_exports: &Exports,
    target_id: &ModuleId,
    errors: &mut Vec<ResolveError>,
) -> std::collections::HashSet<(String, Namespace)> {
    let mut hidden = std::collections::HashSet::new();
    for item in items {
        match item {
            ImportItem::Value { name, span, .. } => {
                check_existence(
                    name,
                    Namespace::Value,
                    *span,
                    target_exports,
                    target_id,
                    errors,
                );
                hidden.insert((name.clone(), Namespace::Value));
            }
            ImportItem::Type { name, span } => {
                check_existence(
                    name,
                    Namespace::Type,
                    *span,
                    target_exports,
                    target_id,
                    errors,
                );
                hidden.insert((name.clone(), Namespace::Type));
            }
            ImportItem::TypeAll { name, span } => {
                check_existence(
                    name,
                    Namespace::Type,
                    *span,
                    target_exports,
                    target_id,
                    errors,
                );
                hidden.insert((name.clone(), Namespace::Type));
                for (vname, vref) in &target_exports.values {
                    if &vref.module == target_id {
                        hidden.insert((vname.clone(), Namespace::Value));
                    }
                }
            }
            ImportItem::TypeWith { name, ctors, span } => {
                check_existence(
                    name,
                    Namespace::Type,
                    *span,
                    target_exports,
                    target_id,
                    errors,
                );
                hidden.insert((name.clone(), Namespace::Type));
                for ctor in ctors {
                    hidden.insert((ctor.clone(), Namespace::Value));
                }
            }
            ImportItem::Class { name, span } => {
                check_existence(
                    name,
                    Namespace::Type,
                    *span,
                    target_exports,
                    target_id,
                    errors,
                );
                hidden.insert((name.clone(), Namespace::Type));
            }
            ImportItem::ClassAll { name, span } => {
                check_existence(
                    name,
                    Namespace::Type,
                    *span,
                    target_exports,
                    target_id,
                    errors,
                );
                hidden.insert((name.clone(), Namespace::Type));
                for (vname, vref) in &target_exports.values {
                    if &vref.module == target_id {
                        hidden.insert((vname.clone(), Namespace::Value));
                    }
                }
            }
        }
    }
    hidden
}

fn check_existence(
    name: &str,
    ns: Namespace,
    span: Span,
    target_exports: &Exports,
    target_id: &ModuleId,
    errors: &mut Vec<ResolveError>,
) {
    if target_exports.lookup(name, ns).is_none() {
        errors.push(ResolveError::new(
            ResolveErrorKind::ImportOfUnknown {
                module: target_id.display(),
                name: name.to_string(),
                namespace: ns,
            },
            span,
        ));
    }
}

/// Construct the synthetic `import Prelude` decl used when a module
/// does not explicitly opt out.
pub(super) fn synthetic_prelude_import(span: Span) -> ImportDecl {
    ImportDecl {
        name: ModName {
            segments: vec![PRELUDE_MODULE.to_string()],
            span,
        },
        qualified: false,
        alias: None,
        items: ImportItems::All,
        span,
    }
}

/// Populate an [`Exports`] table with everything in the prelude
/// static tables. Used when we have no user-authored `Prelude` module
/// in the compilation — the default case for M9 examples.
pub(super) fn builtin_prelude_exports() -> (ModuleId, Exports) {
    let id = ModuleId::from_segments(&[PRELUDE_MODULE.to_string()]);
    let mut exports = Exports::default();
    for (name, _is_ctor) in prelude::PRELUDE_VALUES {
        exports.insert(
            name.to_string(),
            Namespace::Value,
            ResolvedRef::new(id.clone(), name.to_string(), Namespace::Value),
        );
    }
    for (name, _is_class) in prelude::PRELUDE_TYPES {
        exports.insert(
            name.to_string(),
            Namespace::Type,
            ResolvedRef::new(id.clone(), name.to_string(), Namespace::Type),
        );
    }
    (id, exports)
}

/// Look up a module id for a qualifier. The qualifier may be the
/// module's full dotted path or an `as`-alias; both live in the same
/// [`ModuleEnv::qualified_aliases`] map.
pub(super) fn resolve_qualifier<'a>(
    env: &'a ModuleEnv,
    qualifier: &ModName,
) -> Option<&'a ModuleId> {
    let key = qualifier.segments.join(".");
    env.qualified_aliases.get(&key)
}
