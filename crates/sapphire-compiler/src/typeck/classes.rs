//! Class and instance machinery.
//!
//! Constraint resolution follows spec 07 §Instance resolution: given a
//! constraint `C τ`, find the unique in-scope instance whose head
//! unifies with `τ`, apply the instance context under the matching
//! substitution, and recurse. Superclass entailment lets a `Functor m`
//! constraint be discharged by a `Monad m` assumption, per spec 07
//! OQ 9's draft intent.
//!
//! This module does not materialize dictionaries; I7 codegen does.
//! Resolution is purely type-level here.

use sapphire_core::span::Span;

use super::env::{ClassEnv, InstanceInfo};
use super::error::{TypeError, TypeErrorKind};
use super::ty::{Constraint, Subst, Ty, TyVar};
use super::unify::unify;

/// Does `assumed` entail `wanted` via superclass walk?
///
/// `Ord a => Eq a` (Ord is a subclass of Eq) so when `Ord a` is in
/// scope we can discharge `Eq a` without searching instances.
pub fn entails_by_super(env: &ClassEnv, assumed: &Constraint, wanted: &Constraint) -> bool {
    if assumed.arg != wanted.arg {
        return false;
    }
    let closure = env.super_closure(&assumed.class);
    closure.iter().any(|c| c == &wanted.class)
}

/// Try to resolve a single constraint against the in-scope instances
/// and the ambient constraint set `assumed`.
///
/// Returns `Ok(None)` when the constraint is discharged without
/// residual constraints, `Ok(Some(remaining))` when there are
/// unresolved constraints that should be propagated (i.e. the
/// constraint is "deferred" because its argument is still a free
/// variable), and `Err` on a definite failure.
pub fn resolve_constraint(
    env: &ClassEnv,
    assumed: &[Constraint],
    wanted: &Constraint,
    span: Span,
    fresh: &mut dyn FnMut() -> TyVar,
) -> Result<Vec<Constraint>, TypeError> {
    // If the argument is still a (plain) type variable, we can only
    // discharge via an assumption (including a superclass of an
    // assumption). Otherwise it becomes the caller's residual.
    if let Ty::Var(_) = &wanted.arg {
        for a in assumed {
            if entails_by_super(env, a, wanted) {
                return Ok(vec![]);
            }
        }
        // Not entailed by assumptions — defer to the caller, who
        // either generalizes it or reports an ambiguous/unresolved
        // constraint.
        return Ok(vec![wanted.clone()]);
    }

    // First: try each assumption (including superclass walks).
    for a in assumed {
        if entails_by_super(env, a, wanted) {
            return Ok(vec![]);
        }
    }

    // Then try instances of this class (and all subclasses are
    // checked implicitly through superclass lookup at use-site).
    //
    // Check the class exists.
    if !env.classes.contains_key(&wanted.class) {
        return Err(TypeError::new(
            TypeErrorKind::UnknownClass {
                name: wanted.class.clone(),
            },
            span,
        ));
    }

    let mut matches: Vec<(&InstanceInfo, Subst)> = Vec::new();
    for inst in env.instances_for(&wanted.class) {
        // Refresh inst head with fresh vars.
        let (fresh_head, fresh_ctx) = refresh_instance(inst, fresh);
        match unify(&fresh_head, &wanted.arg, span) {
            Ok(s) => {
                // All fresh_ctx entries are not yet residualised.
                let residual: Vec<Constraint> =
                    fresh_ctx.iter().map(|c| s.apply_constraint(c)).collect();
                matches.push((inst, s));
                let _ = residual; // stored below
            }
            Err(_) => continue,
        }
    }

    if matches.is_empty() {
        // Also check if the class has a superclass that might resolve
        // via a subclass instance (e.g. `Eq (Maybe a)` is entailed by
        // `Ord (Maybe a)` — but at ordinary position we demand an
        // `Eq` instance). Since we've registered superclass entailment
        // above via assumptions only, not via instances, this is a
        // "no instance" failure.
        return Err(TypeError::new(
            TypeErrorKind::UnresolvedConstraint {
                constraint: wanted.clone(),
            },
            span,
        ));
    }
    if matches.len() > 1 {
        return Err(TypeError::new(
            TypeErrorKind::OverlappingInstance {
                class: wanted.class.clone(),
                head: wanted.arg.clone(),
            },
            span,
        ));
    }

    // Recurse on the chosen instance's context.
    let (inst, sub) = &matches[0];
    let mut residual: Vec<Constraint> = Vec::new();
    // Re-derive the substituted context the same way as above.
    let (_, fresh_ctx) = refresh_instance_with_sub(inst, sub, fresh);
    for c in fresh_ctx {
        let r = resolve_constraint(env, assumed, &c, span, fresh)?;
        residual.extend(r);
    }
    Ok(residual)
}

/// Refresh an instance's head + context with fresh type variables.
fn refresh_instance(
    inst: &InstanceInfo,
    fresh: &mut dyn FnMut() -> TyVar,
) -> (Ty, Vec<Constraint>) {
    let mut sub = Subst::new();
    for v in &inst.vars {
        let nv = fresh();
        sub.insert(v.id, Ty::Var(nv));
    }
    let head = sub.apply(&inst.head);
    let ctx = inst
        .context
        .iter()
        .map(|c| sub.apply_constraint(c))
        .collect();
    (head, ctx)
}

/// Like `refresh_instance`, but the caller has already matched the
/// head via a unifier `s`. We re-run the refresh with the same
/// substitution composition applied.
fn refresh_instance_with_sub(
    inst: &InstanceInfo,
    s: &Subst,
    fresh: &mut dyn FnMut() -> TyVar,
) -> (Ty, Vec<Constraint>) {
    let (head, ctx) = refresh_instance(inst, fresh);
    let h2 = s.apply(&head);
    let c2 = ctx.iter().map(|c| s.apply_constraint(c)).collect();
    (h2, c2)
}

/// Entry point: resolve every wanted constraint, returning the
/// residual set (those that need to be propagated into the enclosing
/// scheme's context).
pub fn simplify(
    env: &ClassEnv,
    assumed: &[Constraint],
    wanted: &[Constraint],
    span: Span,
    fresh: &mut dyn FnMut() -> TyVar,
) -> Result<Vec<Constraint>, TypeError> {
    let mut residual: Vec<Constraint> = Vec::new();
    for w in wanted {
        let r = resolve_constraint(env, assumed, w, span, fresh)?;
        for c in r {
            if !residual.contains(&c) {
                residual.push(c);
            }
        }
    }
    Ok(residual)
}
