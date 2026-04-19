//! Type checker for Sapphire (I6).
//!
//! The type checker takes a [`ResolvedProgram`] from I5 and produces
//! an [`TypedProgram`]: the original AST plus inferred type schemes
//! for every top-level binding in every module. The inferencer
//! implements Hindley–Milner with let-polymorphism (spec 01), extended
//! to cover algebraic data types + value constructors (spec 03),
//! structural records (spec 04), pattern matching (spec 06),
//! single-parameter type classes and higher-kinded types (spec 07),
//! the spec-09 prelude, and the `Ruby` monad (specs 10 / 11).
//!
//! The design rationale is split across three docs under `docs/impl`:
//!
//! - `docs/impl/18-typecheck-hm.md` — HM core (I6a).
//! - `docs/impl/19-typecheck-adt.md` — ADT / record (I6b).
//! - `docs/impl/20-typecheck-classes.md` — classes + HKT (I6c).
//!
//! The public entry point is [`check_program`]. A single-module
//! convenience, [`check_module`], is also exposed for tests.

pub mod classes;
pub mod env;
pub mod error;
pub mod infer;
pub mod ty;
pub mod unify;

#[cfg(test)]
mod tests;

pub use env::{ClassEnv, ClassInfo, CtorInfo, DataInfo, GlobalId, InstanceInfo, TypeEnv};
pub use error::{TypeError, TypeErrorKind};
pub use ty::{Constraint, Kind, Scheme, Subst, Ty, TyVar};

use std::collections::HashMap;

use crate::resolver::ResolvedProgram;

/// Typed output: the modules, with inferred schemes attached.
#[derive(Debug)]
pub struct TypedProgram {
    pub modules: Vec<TypedModule>,
}

#[derive(Debug)]
pub struct TypedModule {
    pub id: String,
    /// The inferred scheme for each top-level binding, in declaration
    /// order for dump stability.
    pub schemes: Vec<(String, Scheme)>,
}

/// Run the type checker over a resolved program.
///
/// Returns a [`TypedProgram`] on success or all errors encountered
/// during inference otherwise. Processing proceeds module-by-module
/// in the order the resolver produced (which is a dependency order).
pub fn check_program(resolved: &ResolvedProgram) -> Result<TypedProgram, Vec<TypeError>> {
    let mut infer_ctx = infer::InferCtx::new("Prelude");
    infer::install_prelude(&mut infer_ctx);
    let mut all_errors: Vec<TypeError> = Vec::new();
    let mut modules: Vec<TypedModule> = Vec::new();

    // Sort modules so that each module's imports are processed first.
    // Resolver guarantees no cycles; we do a simple topological pass
    // keyed by the dotted module name.
    let order = topological_order(resolved);

    // The prelude lives in `infer_ctx` as module `Prelude`; user
    // modules share the same infer context so they see prelude
    // globals and existing registered datas/classes. But we update
    // `ctx.module` per-module to route inserts to the right key.
    for idx in order {
        let rm = &resolved.modules[idx];
        let name = rm.id.display();
        infer_ctx.module = name.clone();
        match infer::check_module(&mut infer_ctx, &rm.ast) {
            Ok(()) => {
                let mut schemes: Vec<(String, Scheme)> = Vec::new();
                for n in &infer_ctx.inferred_order {
                    if let Some(s) = infer_ctx.inferred.get(n) {
                        schemes.push((n.clone(), s.clone()));
                    }
                }
                modules.push(TypedModule { id: name, schemes });
                infer_ctx.inferred.clear();
                infer_ctx.inferred_order.clear();
            }
            Err(errs) => {
                all_errors.extend(errs);
            }
        }
    }

    if all_errors.is_empty() {
        Ok(TypedProgram { modules })
    } else {
        Err(all_errors)
    }
}

fn topological_order(resolved: &ResolvedProgram) -> Vec<usize> {
    use std::collections::HashMap;
    let n = resolved.modules.len();
    let mut name_to_idx: HashMap<String, usize> = HashMap::new();
    for (i, m) in resolved.modules.iter().enumerate() {
        name_to_idx.insert(m.id.display(), i);
    }
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, m) in resolved.modules.iter().enumerate() {
        for imp in &m.ast.imports {
            let name = imp.name.segments.join(".");
            if let Some(&j) = name_to_idx.get(&name) {
                deps[i].push(j);
            }
        }
    }
    // Kahn's algorithm by depth-first search with post-order.
    let mut visited = vec![false; n];
    let mut order: Vec<usize> = Vec::new();
    fn visit(u: usize, deps: &[Vec<usize>], visited: &mut Vec<bool>, order: &mut Vec<usize>) {
        if visited[u] {
            return;
        }
        visited[u] = true;
        for &v in &deps[u] {
            visit(v, deps, visited, order);
        }
        order.push(u);
    }
    for i in 0..n {
        visit(i, &deps, &mut visited, &mut order);
    }
    order
}

/// Convenience for tests: check a single AST module in a fresh
/// context with the prelude pre-loaded.
pub fn check_module_standalone(
    module_name: &str,
    module: &sapphire_core::ast::Module,
) -> Result<HashMap<String, Scheme>, Vec<TypeError>> {
    let mut ctx = infer::InferCtx::new(module_name);
    infer::install_prelude(&mut ctx);
    infer::check_module(&mut ctx, module)?;
    let mut out = HashMap::new();
    for (n, s) in ctx.inferred {
        out.insert(n, s);
    }
    Ok(out)
}
