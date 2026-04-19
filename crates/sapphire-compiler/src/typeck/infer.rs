//! Type inference — the W-algorithm with let-polymorphism,
//! constraint collection, and `do`-notation desugaring.
//!
//! Algorithm outline:
//!
//! 1. Walk every top-level declaration to register data-types, class
//!    declarations, type aliases, and constructor schemes into the
//!    type env. Signature-less value bindings get a fresh monotype
//!    placeholder; signatures are parsed into schemes up front.
//! 2. Process instance declarations: register each in the class env
//!    and check the instance body against the expected method schemes
//!    (with the instance's type substituted in).
//! 3. For each value binding, if it has a signature we use the
//!    signature's scheme as the expected type; otherwise we infer,
//!    generalise against the global env, and store the result.
//! 4. `do { ... }` is treated as surface sugar per spec 07: we
//!    desugar it on the fly during inference.
//!
//! The implementation stays close to textbook Algorithm W; we do not
//! build a full constraint graph. Residual constraints collected
//! during inference flow up through the `InferCtx` and are discharged
//! either by the enclosing scheme's assumptions or by instance
//! resolution at the next generalisation point.

use std::collections::{HashMap, HashSet};

use sapphire_core::ast::{
    ClassDecl, ClassItem, DataDecl, Decl, DoStmt, Expr, InstanceDecl, Literal, Module as AstModule,
    Pattern, Scheme as AstScheme, Type as AstType, TypeAlias, ValueClause,
};
use sapphire_core::span::Span;

use super::classes::simplify;
use super::env::{
    AliasInfo, ClassEnv, ClassInfo, CtorInfo, DataInfo, GlobalId, InstanceInfo, TypeEnv,
};
use super::error::{TypeError, TypeErrorKind};
use super::ty::{Constraint, Kind, Scheme, Subst, Ty, TyVar};
use super::unify::unify;

/// A per-module inference context.
pub struct InferCtx {
    pub module: String,
    pub type_env: TypeEnv,
    pub class_env: ClassEnv,
    fresh_counter: u32,
    /// Residual (unsolved) constraints accumulated during an
    /// inference; typically drained by `generalize`.
    pub wanted: Vec<(Constraint, Span)>,
    /// Deferred record-field-access goals. Each entry says "`record_ty`
    /// is expected to be a record containing field `field` of type
    /// `field_ty`; resolve after all other unification". Used to
    /// support field access on a parameter whose record type becomes
    /// known only after a later unification step.
    pub pending_fields: Vec<(Ty, String, Ty, Span)>,
    /// Current scheme-level assumptions (from the enclosing scheme's
    /// context).
    pub assumed: Vec<Constraint>,
    /// Inferred scheme for each top-level name in this module
    /// (populated during `check_module`).
    pub inferred: HashMap<String, Scheme>,
    /// Inferred top-level order (for dump stability).
    pub inferred_order: Vec<String>,
}

impl InferCtx {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            type_env: TypeEnv::new(),
            class_env: ClassEnv::new(),
            fresh_counter: 1,
            wanted: Vec::new(),
            pending_fields: Vec::new(),
            assumed: Vec::new(),
            inferred: HashMap::new(),
            inferred_order: Vec::new(),
        }
    }

    pub fn fresh(&mut self) -> TyVar {
        let id = self.fresh_counter;
        self.fresh_counter += 1;
        TyVar {
            id,
            name: String::new(),
        }
    }

    pub fn fresh_named(&mut self, hint: &str) -> TyVar {
        let id = self.fresh_counter;
        self.fresh_counter += 1;
        TyVar {
            id,
            name: hint.to_string(),
        }
    }

    pub fn fresh_ty(&mut self) -> Ty {
        Ty::Var(self.fresh())
    }

    /// Instantiate a scheme with fresh type variables, returning the
    /// instantiated body and the instantiated constraint list.
    pub fn instantiate(&mut self, s: &Scheme) -> (Ty, Vec<Constraint>) {
        let mut sub = Subst::new();
        for v in &s.vars {
            let nv = self.fresh_named(&v.name);
            sub.insert(v.id, Ty::Var(nv));
        }
        let ty = sub.apply(&s.body);
        let ctx = s.context.iter().map(|c| sub.apply_constraint(c)).collect();
        (ty, ctx)
    }

    /// Generalize a monotype w.r.t. the current global + enclosing
    /// scope's free variables. Returns a scheme with residual
    /// constraints promoted into the context (only those mentioning
    /// generalised vars; free-in-env constraints are left in
    /// `self.wanted`).
    ///
    /// The caller may pass `exclude_name` to suppress a specific local
    /// (or global) binding from the env-FV computation. This matters
    /// when the binding being generalised has a *provisional* entry in
    /// the env that points to the very type variable(s) we want to
    /// quantify over. Without that exclusion, e.g. `let f x = x in ...`
    /// pre-binds `f : mono(α)` into the locals frame, the lambda body
    /// resolves α to `β → β`, and generalize then sees α (now `β → β`)
    /// in env_fvs through the self-slot — so β leaks into env_fvs and
    /// `f` is monomorphised. Excluding the self-slot makes let-poly
    /// behave correctly without relying on later substitution
    /// coincidences.
    pub fn generalize_excluding(
        &mut self,
        sub: &Subst,
        ty: &Ty,
        exclude_name: Option<&str>,
    ) -> Scheme {
        let ty = sub.apply(ty);
        // Free vars of ty.
        let mut ty_fvs: Vec<u32> = Vec::new();
        ty.free_vars(&mut ty_fvs);

        // Free vars of the enclosing env (locals + globals) — minus
        // the entry named `exclude_name`, when one is requested and
        // present in the innermost local frame (for let-bindings) or
        // the current-module globals (for provisional top-level
        // slots).
        let mut env_fvs: HashSet<u32> = HashSet::new();
        for frame in &self.type_env.locals {
            for (n, s) in frame {
                if exclude_name.map(|x| x == n.as_str()).unwrap_or(false) {
                    continue;
                }
                collect_scheme_free(&sub.apply_scheme(s), &mut env_fvs);
            }
        }
        // Only the currently-building globals matter; already-finalized
        // globals are closed and free-of-tvars after generalisation.
        for (g, s) in &self.type_env.globals {
            if exclude_name
                .map(|x| x == g.name.as_str() && g.module == self.module)
                .unwrap_or(false)
            {
                continue;
            }
            collect_scheme_free(&sub.apply_scheme(s), &mut env_fvs);
        }

        let gen_vars: Vec<u32> = ty_fvs
            .into_iter()
            .filter(|v| !env_fvs.contains(v))
            .collect();

        // Pull out wanted constraints that mention only gen_vars (or
        // gen_vars ∪ assumed). Others remain in self.wanted to be
        // resolved by the enclosing context.
        let mut kept_wanted = Vec::new();
        let mut promoted: Vec<Constraint> = Vec::new();
        for (c, sp) in self.wanted.drain(..) {
            let c2 = sub.apply_constraint(&c);
            let mut fvs = Vec::new();
            c2.arg.free_vars(&mut fvs);
            let mentions_gen = fvs.iter().any(|v| gen_vars.contains(v));
            let only_gen_or_env = fvs
                .iter()
                .all(|v| gen_vars.contains(v) || env_fvs.contains(v));
            if mentions_gen && only_gen_or_env && !promoted.contains(&c2) {
                promoted.push(c2);
            } else {
                kept_wanted.push((c2, sp));
            }
        }
        self.wanted = kept_wanted;

        // Rewrite gen_vars to rigids keeping their hint names.
        let mut rigids = Vec::new();
        let mut rename = Subst::new();
        for id in &gen_vars {
            let hint = original_name_hint(&ty, *id).unwrap_or_else(|| format!("t{id}"));
            let nv = TyVar {
                id: *id,
                name: hint,
            };
            rigids.push(nv.clone());
            rename.insert(*id, Ty::Var(nv));
        }
        let body = rename.apply(&ty);
        let context: Vec<Constraint> = promoted
            .iter()
            .map(|c| rename.apply_constraint(c))
            .collect();
        Scheme {
            vars: rigids,
            context,
            body,
        }
    }

    /// Convenience wrapper: generalise without excluding any binding.
    ///
    /// Callers that are about to finalise a specific name's scheme
    /// should prefer [`generalize_excluding`] with `Some(name)` to
    /// avoid the self-slot pinning problem described on that method.
    pub fn generalize(&mut self, sub: &Subst, ty: &Ty) -> Scheme {
        self.generalize_excluding(sub, ty, None)
    }

    pub fn add_wanted(&mut self, c: Constraint, span: Span) {
        self.wanted.push((c, span));
    }
}

fn collect_scheme_free(s: &Scheme, out: &mut HashSet<u32>) {
    let bound: HashSet<u32> = s.vars.iter().map(|v| v.id).collect();
    let mut fv = Vec::new();
    s.body.free_vars(&mut fv);
    for c in &s.context {
        c.arg.free_vars(&mut fv);
    }
    for v in fv {
        if !bound.contains(&v) {
            out.insert(v);
        }
    }
}

fn original_name_hint(t: &Ty, id: u32) -> Option<String> {
    match t {
        Ty::Var(v) if v.id == id && !v.name.is_empty() => Some(v.name.clone()),
        Ty::App(a, b) | Ty::Fun(a, b) => {
            original_name_hint(a, id).or_else(|| original_name_hint(b, id))
        }
        Ty::Record(fs) => fs.iter().find_map(|(_, t)| original_name_hint(t, id)),
        _ => None,
    }
}

// =====================================================================
//  AST-type → Ty conversion
// =====================================================================

/// Convert a surface AST type to a core [`Ty`]. Type variables that
/// appear free are bound in the surrounding scheme context (`locals`).
pub fn ty_from_ast(
    ctx: &mut InferCtx,
    ast: &AstType,
    locals: &HashMap<String, TyVar>,
) -> Result<Ty, TypeError> {
    match ast {
        AstType::Var { name, .. } => {
            if let Some(v) = locals.get(name) {
                return Ok(Ty::Var(v.clone()));
            }
            // Free var not bound: treat as a fresh rigid. Callers who
            // want implicit quantification should pre-populate
            // `locals` with these names. If we get here the scheme
            // converter didn't collect it; return a fresh variable.
            Ok(Ty::Var(ctx.fresh_named(name)))
        }
        AstType::Con { name, span, .. } => {
            if ctx.type_env.aliases.contains_key(name) {
                // Nullary alias — expand with empty arg list.
                let ai = ctx.type_env.aliases[name].clone();
                if !ai.params.is_empty() {
                    return Err(TypeError::new(
                        TypeErrorKind::KindMismatch {
                            expected: format!("alias `{}` with {} params", name, ai.params.len()),
                            found: "saturated 0-arg".into(),
                        },
                        *span,
                    ));
                }
                return Ok(ai.body);
            }
            if ctx.type_env.datas.contains_key(name) || is_builtin_type(name) {
                let kind = builtin_kind_of(name).unwrap_or_else(|| {
                    let arity = ctx
                        .type_env
                        .datas
                        .get(name)
                        .map(|d| d.params.len())
                        .unwrap_or(0);
                    make_kind_arity(arity)
                });
                return Ok(Ty::con(name, kind));
            }
            // Class names also live in the type namespace; they are
            // not types themselves, but referencing `Eq` as a type is
            // an error at this layer.
            Err(TypeError::new(
                TypeErrorKind::UnknownType { name: name.clone() },
                *span,
            ))
        }
        AstType::App { func, arg, span } => {
            // If the head is an alias with the right number of args,
            // expand it. Otherwise build an App.
            if let Some((head_name, args)) = split_alias_head(func, arg) {
                if let Some(ai) = ctx.type_env.aliases.get(&head_name).cloned() {
                    if ai.params.len() == args.len() {
                        let mut arg_tys = Vec::new();
                        for a in &args {
                            arg_tys.push(ty_from_ast(ctx, a, locals)?);
                        }
                        // ai.body's params appear as `Ty::Var { id: 0,
                        // name }` — substitute by name.
                        let out = substitute_named_vars(&ai.body, &ai.params, &arg_tys);
                        return Ok(out);
                    }
                }
            }
            let f = ty_from_ast(ctx, func, locals)?;
            let a = ty_from_ast(ctx, arg, locals)?;
            let _ = span;
            Ok(Ty::app(f, a))
        }
        AstType::Fun { param, result, .. } => {
            let p = ty_from_ast(ctx, param, locals)?;
            let r = ty_from_ast(ctx, result, locals)?;
            Ok(Ty::fun(p, r))
        }
        AstType::Record { fields, .. } => {
            let mut fs = Vec::new();
            for (name, t) in fields {
                fs.push((name.clone(), ty_from_ast(ctx, t, locals)?));
            }
            Ok(Ty::record(fs))
        }
    }
}

/// If `App f x` ultimately has an alias name at the head, return
/// `(alias_name, args)`. Otherwise `None`.
fn split_alias_head(func: &AstType, arg: &AstType) -> Option<(String, Vec<AstType>)> {
    let mut args = vec![arg.clone()];
    let mut head = func;
    loop {
        match head {
            AstType::App { func, arg, .. } => {
                args.insert(0, (**arg).clone());
                head = func;
            }
            AstType::Con { name, .. } => return Some((name.clone(), args)),
            _ => return None,
        }
    }
}

/// Substitute `Ty::Var { name }` matches for each `param_name[i]`
/// with `args[i]`.
fn substitute_named_vars(body: &Ty, params: &[String], args: &[Ty]) -> Ty {
    match body {
        Ty::Var(v) => {
            for (i, p) in params.iter().enumerate() {
                if v.name == *p && v.id == 0 {
                    return args[i].clone();
                }
            }
            Ty::Var(v.clone())
        }
        Ty::Con { name, kind } => Ty::Con {
            name: name.clone(),
            kind: kind.clone(),
        },
        Ty::App(a, b) => Ty::app(
            substitute_named_vars(a, params, args),
            substitute_named_vars(b, params, args),
        ),
        Ty::Fun(a, b) => Ty::fun(
            substitute_named_vars(a, params, args),
            substitute_named_vars(b, params, args),
        ),
        Ty::Record(fs) => Ty::record(
            fs.iter()
                .map(|(n, t)| (n.clone(), substitute_named_vars(t, params, args)))
                .collect(),
        ),
    }
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "Int" | "String" | "Bool" | "Maybe" | "Result" | "List" | "Ordering" | "Ruby"
    )
}

fn builtin_kind_of(name: &str) -> Option<Kind> {
    match name {
        "Int" | "String" | "Bool" | "Ordering" => Some(Kind::Star),
        "Maybe" | "List" | "Ruby" => Some(Kind::arr(Kind::Star, Kind::Star)),
        "Result" => Some(Kind::arr(Kind::Star, Kind::arr(Kind::Star, Kind::Star))),
        _ => None,
    }
}

fn make_kind_arity(arity: usize) -> Kind {
    let mut k = Kind::Star;
    for _ in 0..arity {
        k = Kind::arr(Kind::Star, k);
    }
    k
}

/// Convert an AST scheme to a core scheme.
///
/// Free type variables (both in the explicit `forall` and implicit
/// ones mentioned in the body / context) become rigid `TyVar`s with
/// fresh ids and their written names as hints.
pub fn scheme_from_ast(ctx: &mut InferCtx, s: &AstScheme) -> Result<Scheme, TypeError> {
    // Collect every type-var name mentioned (explicit forall + body +
    // context), preserving source order.
    let mut names: Vec<String> = Vec::new();
    for n in &s.forall {
        if !names.contains(n) {
            names.push(n.clone());
        }
    }
    collect_names_in_ty(&s.body, &mut names);
    for c in &s.context {
        for a in &c.args {
            collect_names_in_ty(a, &mut names);
        }
    }

    let mut locals: HashMap<String, TyVar> = HashMap::new();
    let mut rigids: Vec<TyVar> = Vec::new();
    for n in &names {
        let nv = TyVar {
            id: {
                let id = ctx.fresh_counter;
                ctx.fresh_counter += 1;
                id
            },
            name: n.clone(),
        };
        locals.insert(n.clone(), nv.clone());
        rigids.push(nv);
    }

    let body = ty_from_ast(ctx, &s.body, &locals)?;
    let mut context = Vec::new();
    for c in &s.context {
        if c.args.len() != 1 {
            return Err(TypeError::new(
                TypeErrorKind::Other {
                    msg: format!(
                        "class `{}` constraint must have one argument (spec 07)",
                        c.class_name
                    ),
                },
                c.span,
            ));
        }
        let arg = ty_from_ast(ctx, &c.args[0], &locals)?;
        context.push(Constraint {
            class: c.class_name.clone(),
            arg,
        });
    }

    Ok(Scheme {
        vars: rigids,
        context,
        body,
    })
}

fn collect_names_in_ty(t: &AstType, out: &mut Vec<String>) {
    match t {
        AstType::Var { name, .. } => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        AstType::Con { .. } => {}
        AstType::App { func, arg, .. } => {
            collect_names_in_ty(func, out);
            collect_names_in_ty(arg, out);
        }
        AstType::Fun { param, result, .. } => {
            collect_names_in_ty(param, out);
            collect_names_in_ty(result, out);
        }
        AstType::Record { fields, .. } => {
            for (_, t) in fields {
                collect_names_in_ty(t, out);
            }
        }
    }
}

// =====================================================================
//  Top-level declaration processing
// =====================================================================

/// Install prelude bindings (Bool / Ordering / Maybe / Result / List /
/// standard classes / arithmetic + monad operators) into the context.
pub fn install_prelude(ctx: &mut InferCtx) {
    use Kind::*;
    // Data-types
    register_data(ctx, "Bool", &[], &[("False", vec![]), ("True", vec![])]);
    register_data(
        ctx,
        "Ordering",
        &[],
        &[("LT", vec![]), ("EQ", vec![]), ("GT", vec![])],
    );
    register_data(
        ctx,
        "Maybe",
        &["a"],
        &[("Nothing", vec![]), ("Just", vec![AstTypeKind::TVar("a")])],
    );
    register_data(
        ctx,
        "Result",
        &["e", "a"],
        &[
            ("Err", vec![AstTypeKind::TVar("e")]),
            ("Ok", vec![AstTypeKind::TVar("a")]),
        ],
    );
    register_data(
        ctx,
        "List",
        &["a"],
        &[
            ("Nil", vec![]),
            (
                "Cons",
                vec![
                    AstTypeKind::TVar("a"),
                    AstTypeKind::App(
                        Box::new(AstTypeKind::TCon("List")),
                        Box::new(AstTypeKind::TVar("a")),
                    ),
                ],
            ),
        ],
    );

    // Primitives (already handled via builtin kinds, but register in
    // type_kinds for arity lookup).
    for name in ["Int", "String", "Bool", "Ordering", "Maybe", "List", "Ruby"] {
        ctx.type_env.type_kinds.insert(name.into(), 0);
    }

    // Classes
    fn fresh_scheme(vars: Vec<&str>, ctx_cstrs: Vec<(&str, Ty)>, body: Ty) -> Scheme {
        let vvs: Vec<TyVar> = vars
            .iter()
            .enumerate()
            .map(|(i, n)| TyVar {
                id: (10_000 + i) as u32,
                name: (*n).to_string(),
            })
            .collect();
        // Substitute names -> ids in body/context via a rename.
        let mut subst_map: HashMap<&str, Ty> = HashMap::new();
        for v in &vvs {
            subst_map.insert(&v.name, Ty::Var(v.clone()));
        }
        let body = rename_named_vars(&body, &subst_map);
        let context = ctx_cstrs
            .into_iter()
            .map(|(c, a)| Constraint {
                class: c.to_string(),
                arg: rename_named_vars(&a, &subst_map),
            })
            .collect();
        Scheme {
            vars: vvs,
            context,
            body,
        }
    }

    let va = || {
        Ty::Var(TyVar {
            id: 0,
            name: "a".into(),
        })
    };
    let vb = || {
        Ty::Var(TyVar {
            id: 0,
            name: "b".into(),
        })
    };
    let vc = || {
        Ty::Var(TyVar {
            id: 0,
            name: "c".into(),
        })
    };
    let ve = || {
        Ty::Var(TyVar {
            id: 0,
            name: "e".into(),
        })
    };
    let vf = || {
        Ty::Var(TyVar {
            id: 0,
            name: "f".into(),
        })
    };
    let vm = || {
        Ty::Var(TyVar {
            id: 0,
            name: "m".into(),
        })
    };
    let int_t = || Ty::star("Int");
    let bool_t = || Ty::star("Bool");
    let string_t = || Ty::star("String");
    let ordering_t = || Ty::star("Ordering");
    let maybe = |t: Ty| Ty::app(Ty::con("Maybe", Kind::arr(Star, Star)), t);
    let list = |t: Ty| Ty::app(Ty::con("List", Kind::arr(Star, Star)), t);
    let result = |e: Ty, a: Ty| {
        Ty::app(
            Ty::app(Ty::con("Result", Kind::arr(Star, Kind::arr(Star, Star))), e),
            a,
        )
    };

    // Eq
    let eq = ClassInfo {
        name: "Eq".into(),
        type_var: "a".into(),
        superclasses: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert(
                "==".into(),
                fresh_scheme(vec!["a"], vec![], Ty::fun(va(), Ty::fun(va(), bool_t()))),
            );
            m.insert(
                "/=".into(),
                fresh_scheme(vec!["a"], vec![], Ty::fun(va(), Ty::fun(va(), bool_t()))),
            );
            m
        },
        defaults: vec!["/=".into()],
        home_module: "Prelude".into(),
    };
    ctx.class_env.register_class(eq);

    // Ord
    let ord = ClassInfo {
        name: "Ord".into(),
        type_var: "a".into(),
        superclasses: vec!["Eq".into()],
        methods: {
            let mut m = HashMap::new();
            m.insert(
                "compare".into(),
                fresh_scheme(
                    vec!["a"],
                    vec![],
                    Ty::fun(va(), Ty::fun(va(), ordering_t())),
                ),
            );
            for op in ["<", ">", "<=", ">="] {
                m.insert(
                    op.into(),
                    fresh_scheme(vec!["a"], vec![], Ty::fun(va(), Ty::fun(va(), bool_t()))),
                );
            }
            m
        },
        defaults: vec!["<".into(), ">".into(), "<=".into(), ">=".into()],
        home_module: "Prelude".into(),
    };
    ctx.class_env.register_class(ord);

    // Show
    let show = ClassInfo {
        name: "Show".into(),
        type_var: "a".into(),
        superclasses: vec![],
        methods: {
            let mut m = HashMap::new();
            m.insert(
                "show".into(),
                fresh_scheme(vec!["a"], vec![], Ty::fun(va(), string_t())),
            );
            m
        },
        defaults: vec![],
        home_module: "Prelude".into(),
    };
    ctx.class_env.register_class(show);

    // Functor f:: *->*
    let functor = ClassInfo {
        name: "Functor".into(),
        type_var: "f".into(),
        superclasses: vec![],
        methods: {
            let mut m = HashMap::new();
            // fmap : (a -> b) -> f a -> f b
            m.insert(
                "fmap".into(),
                fresh_scheme(
                    vec!["f", "a", "b"],
                    vec![],
                    Ty::fun(
                        Ty::fun(va(), vb()),
                        Ty::fun(Ty::app(vf(), va()), Ty::app(vf(), vb())),
                    ),
                ),
            );
            m
        },
        defaults: vec![],
        home_module: "Prelude".into(),
    };
    ctx.class_env.register_class(functor);

    // Applicative f
    let applicative = ClassInfo {
        name: "Applicative".into(),
        type_var: "f".into(),
        superclasses: vec!["Functor".into()],
        methods: {
            let mut m = HashMap::new();
            m.insert(
                "pure".into(),
                fresh_scheme(vec!["f", "a"], vec![], Ty::fun(va(), Ty::app(vf(), va()))),
            );
            // <*> : f (a -> b) -> f a -> f b
            m.insert(
                "<*>".into(),
                fresh_scheme(
                    vec!["f", "a", "b"],
                    vec![],
                    Ty::fun(
                        Ty::app(vf(), Ty::fun(va(), vb())),
                        Ty::fun(Ty::app(vf(), va()), Ty::app(vf(), vb())),
                    ),
                ),
            );
            m
        },
        defaults: vec![],
        home_module: "Prelude".into(),
    };
    ctx.class_env.register_class(applicative);

    // Monad m
    let monad = ClassInfo {
        name: "Monad".into(),
        type_var: "m".into(),
        superclasses: vec!["Applicative".into()],
        methods: {
            let mut m = HashMap::new();
            // >>= : m a -> (a -> m b) -> m b
            m.insert(
                ">>=".into(),
                fresh_scheme(
                    vec!["m", "a", "b"],
                    vec![],
                    Ty::fun(
                        Ty::app(vm(), va()),
                        Ty::fun(Ty::fun(va(), Ty::app(vm(), vb())), Ty::app(vm(), vb())),
                    ),
                ),
            );
            // >> : m a -> m b -> m b
            m.insert(
                ">>".into(),
                fresh_scheme(
                    vec!["m", "a", "b"],
                    vec![],
                    Ty::fun(
                        Ty::app(vm(), va()),
                        Ty::fun(Ty::app(vm(), vb()), Ty::app(vm(), vb())),
                    ),
                ),
            );
            // return : a -> m a
            m.insert(
                "return".into(),
                fresh_scheme(vec!["m", "a"], vec![], Ty::fun(va(), Ty::app(vm(), va()))),
            );
            m
        },
        defaults: vec!["return".into(), ">>".into()],
        home_module: "Prelude".into(),
    };
    ctx.class_env.register_class(monad);

    // Promote each class method into the globals with its constrained
    // scheme.
    let mut class_methods: Vec<(String, String, Scheme)> = Vec::new();
    for (cname, cinfo) in &ctx.class_env.classes {
        for (mname, mscheme) in &cinfo.methods {
            class_methods.push((mname.clone(), cname.clone(), mscheme.clone()));
        }
    }
    for (mname, cname, mscheme) in class_methods {
        // Add `ClassName type_var` constraint.
        let tv_name = &ctx.class_env.classes[&cname].type_var;
        let tv = mscheme
            .vars
            .iter()
            .find(|v| v.name == *tv_name)
            .cloned()
            .unwrap_or_else(|| TyVar {
                id: 20_000,
                name: tv_name.clone(),
            });
        let mut context = mscheme.context.clone();
        let c = Constraint {
            class: cname.clone(),
            arg: Ty::Var(tv),
        };
        if !context.contains(&c) {
            context.push(c);
        }
        let constrained = Scheme {
            vars: mscheme.vars,
            context,
            body: mscheme.body,
        };
        let gid = GlobalId::new("Prelude", &mname);
        ctx.type_env.globals.insert(gid, constrained);
    }

    // Arithmetic + comparison + etc.
    let mut add_prelude = |name: &str, sch: Scheme| {
        ctx.type_env
            .globals
            .insert(GlobalId::new("Prelude", name), sch);
    };

    let int_binop = Scheme::mono(Ty::fun(int_t(), Ty::fun(int_t(), int_t())));
    for op in ["+", "-", "*", "/", "%"] {
        add_prelude(op, int_binop.clone());
    }
    add_prelude("negate", Scheme::mono(Ty::fun(int_t(), int_t())));
    add_prelude(
        "&&",
        Scheme::mono(Ty::fun(bool_t(), Ty::fun(bool_t(), bool_t()))),
    );
    add_prelude(
        "||",
        Scheme::mono(Ty::fun(bool_t(), Ty::fun(bool_t(), bool_t()))),
    );
    add_prelude("not", Scheme::mono(Ty::fun(bool_t(), bool_t())));
    add_prelude(
        "++",
        Scheme::mono(Ty::fun(string_t(), Ty::fun(string_t(), string_t()))),
    );
    // cons :: a -> List a -> List a
    add_prelude(
        "::",
        fresh_scheme(
            vec!["a"],
            vec![],
            Ty::fun(va(), Ty::fun(list(va()), list(va()))),
        ),
    );

    // Utilities
    add_prelude("id", fresh_scheme(vec!["a"], vec![], Ty::fun(va(), va())));
    add_prelude(
        "const",
        fresh_scheme(vec!["a", "b"], vec![], Ty::fun(va(), Ty::fun(vb(), va()))),
    );
    add_prelude(
        "compose",
        fresh_scheme(
            vec!["a", "b", "c"],
            vec![],
            Ty::fun(
                Ty::fun(vb(), vc()),
                Ty::fun(Ty::fun(va(), vb()), Ty::fun(va(), vc())),
            ),
        ),
    );
    add_prelude(
        "flip",
        fresh_scheme(
            vec!["a", "b", "c"],
            vec![],
            Ty::fun(
                Ty::fun(va(), Ty::fun(vb(), vc())),
                Ty::fun(vb(), Ty::fun(va(), vc())),
            ),
        ),
    );
    // map/filter/foldr/foldl/concat/concatMap
    add_prelude(
        "map",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(Ty::fun(va(), vb()), Ty::fun(list(va()), list(vb()))),
        ),
    );
    add_prelude(
        "filter",
        fresh_scheme(
            vec!["a"],
            vec![],
            Ty::fun(Ty::fun(va(), bool_t()), Ty::fun(list(va()), list(va()))),
        ),
    );
    add_prelude(
        "foldr",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(
                Ty::fun(va(), Ty::fun(vb(), vb())),
                Ty::fun(vb(), Ty::fun(list(va()), vb())),
            ),
        ),
    );
    add_prelude(
        "foldl",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(
                Ty::fun(vb(), Ty::fun(va(), vb())),
                Ty::fun(vb(), Ty::fun(list(va()), vb())),
            ),
        ),
    );
    add_prelude(
        "concat",
        fresh_scheme(vec!["a"], vec![], Ty::fun(list(list(va())), list(va()))),
    );
    add_prelude(
        "concatMap",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(Ty::fun(va(), list(vb())), Ty::fun(list(va()), list(vb()))),
        ),
    );
    add_prelude(
        "length",
        fresh_scheme(vec!["a"], vec![], Ty::fun(list(va()), int_t())),
    );
    add_prelude(
        "head",
        fresh_scheme(vec!["a"], vec![], Ty::fun(list(va()), maybe(va()))),
    );
    add_prelude(
        "tail",
        fresh_scheme(vec!["a"], vec![], Ty::fun(list(va()), maybe(list(va())))),
    );
    add_prelude(
        "null",
        fresh_scheme(vec!["a"], vec![], Ty::fun(list(va()), bool_t())),
    );
    add_prelude(
        "fst",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(
                Ty::record(vec![("fst".into(), va()), ("snd".into(), vb())]),
                va(),
            ),
        ),
    );
    add_prelude(
        "snd",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(
                Ty::record(vec![("fst".into(), va()), ("snd".into(), vb())]),
                vb(),
            ),
        ),
    );
    add_prelude(
        "maybe",
        fresh_scheme(
            vec!["a", "b"],
            vec![],
            Ty::fun(
                vb(),
                Ty::fun(Ty::fun(va(), vb()), Ty::fun(maybe(va()), vb())),
            ),
        ),
    );
    add_prelude(
        "fromMaybe",
        fresh_scheme(vec!["a"], vec![], Ty::fun(va(), Ty::fun(maybe(va()), va()))),
    );
    add_prelude(
        "result",
        fresh_scheme(
            vec!["a", "b", "e"],
            vec![],
            Ty::fun(
                Ty::fun(ve(), vb()),
                Ty::fun(Ty::fun(va(), vb()), Ty::fun(result(ve(), va()), vb())),
            ),
        ),
    );
    add_prelude(
        "mapErr",
        fresh_scheme(vec!["e", "e2", "a"], vec![], {
            let ve2 = || {
                Ty::Var(TyVar {
                    id: 0,
                    name: "e2".into(),
                })
            };
            Ty::fun(
                Ty::fun(ve(), ve2()),
                Ty::fun(result(ve(), va()), result(ve2(), va())),
            )
        }),
    );
    add_prelude("readInt", Scheme::mono(Ty::fun(string_t(), maybe(int_t()))));
    // join : Monad m => m (m a) -> m a
    add_prelude(
        "join",
        fresh_scheme(
            vec!["m", "a"],
            vec![("Monad", vm())],
            Ty::fun(Ty::app(vm(), Ty::app(vm(), va())), Ty::app(vm(), va())),
        ),
    );
    // when : Applicative f => Bool -> f {} -> f {}
    let unit = || Ty::record(vec![]);
    add_prelude(
        "when",
        fresh_scheme(
            vec!["f"],
            vec![("Applicative", vf())],
            Ty::fun(
                bool_t(),
                Ty::fun(Ty::app(vf(), unit()), Ty::app(vf(), unit())),
            ),
        ),
    );
    add_prelude(
        "unless",
        fresh_scheme(
            vec!["f"],
            vec![("Applicative", vf())],
            Ty::fun(
                bool_t(),
                Ty::fun(Ty::app(vf(), unit()), Ty::app(vf(), unit())),
            ),
        ),
    );
    // compare is already a method of Ord; nothing to re-add.
    add_prelude(
        "print",
        fresh_scheme(
            vec!["a"],
            vec![("Show", va())],
            Ty::fun(va(), result(string_t(), unit())),
        ),
    );
    // == and /= come from Eq class; show from Show class.

    // Register prelude instances.
    install_prelude_instances(ctx);
}

fn install_prelude_instances(ctx: &mut InferCtx) {
    use Kind::*;
    // Int, String, Bool, Ordering -> Eq, Ord, Show
    for name in ["Int", "String", "Bool", "Ordering"] {
        ctx.class_env.register_instance(InstanceInfo {
            class: "Eq".into(),
            context: vec![],
            head: Ty::star(name),
            vars: vec![],
            methods: vec!["==".into(), "/=".into()],
        });
        ctx.class_env.register_instance(InstanceInfo {
            class: "Ord".into(),
            context: vec![],
            head: Ty::star(name),
            vars: vec![],
            methods: vec!["compare".into()],
        });
        ctx.class_env.register_instance(InstanceInfo {
            class: "Show".into(),
            context: vec![],
            head: Ty::star(name),
            vars: vec![],
            methods: vec!["show".into()],
        });
    }

    // Maybe: Eq a => Eq (Maybe a); Functor / Applicative / Monad
    // Result e a: similar
    // List a: similar
    let var = |name: &str, id: u32| TyVar {
        id,
        name: name.into(),
    };
    let maybe_ = |t: Ty| Ty::app(Ty::con("Maybe", Kind::arr(Star, Star)), t);
    let list_ = |t: Ty| Ty::app(Ty::con("List", Kind::arr(Star, Star)), t);
    let result_ = |e: Ty, a: Ty| {
        Ty::app(
            Ty::app(Ty::con("Result", Kind::arr(Star, Kind::arr(Star, Star))), e),
            a,
        )
    };

    // Functor / Applicative / Monad for Maybe, List, Ruby, Result e
    let add_inst = |ctx: &mut InferCtx,
                    class: &str,
                    context: Vec<Constraint>,
                    head: Ty,
                    vars: Vec<TyVar>,
                    methods: Vec<&str>| {
        ctx.class_env.register_instance(InstanceInfo {
            class: class.into(),
            context,
            head,
            vars,
            methods: methods.into_iter().map(String::from).collect(),
        });
    };

    // Eq (Maybe a) etc.
    {
        let a = var("a", 30_001);
        add_inst(
            ctx,
            "Eq",
            vec![Constraint {
                class: "Eq".into(),
                arg: Ty::Var(a.clone()),
            }],
            maybe_(Ty::Var(a.clone())),
            vec![a.clone()],
            vec!["==", "/="],
        );
    }
    {
        let a = var("a", 30_002);
        add_inst(
            ctx,
            "Ord",
            vec![Constraint {
                class: "Ord".into(),
                arg: Ty::Var(a.clone()),
            }],
            maybe_(Ty::Var(a.clone())),
            vec![a.clone()],
            vec!["compare"],
        );
    }
    {
        let a = var("a", 30_003);
        add_inst(
            ctx,
            "Show",
            vec![Constraint {
                class: "Show".into(),
                arg: Ty::Var(a.clone()),
            }],
            maybe_(Ty::Var(a.clone())),
            vec![a.clone()],
            vec!["show"],
        );
    }
    {
        let a = var("a", 30_010);
        add_inst(
            ctx,
            "Functor",
            vec![],
            Ty::con("Maybe", Kind::arr(Star, Star)),
            vec![],
            vec!["fmap"],
        );
        add_inst(
            ctx,
            "Applicative",
            vec![],
            Ty::con("Maybe", Kind::arr(Star, Star)),
            vec![],
            vec!["pure", "<*>"],
        );
        add_inst(
            ctx,
            "Monad",
            vec![],
            Ty::con("Maybe", Kind::arr(Star, Star)),
            vec![],
            vec![">>="],
        );
        let _ = a;
    }

    // Result e: classes at kind * -> *
    {
        let e = var("e", 30_020);
        // Functor (Result e)
        add_inst(
            ctx,
            "Functor",
            vec![],
            Ty::app(
                Ty::con("Result", Kind::arr(Star, Kind::arr(Star, Star))),
                Ty::Var(e.clone()),
            ),
            vec![e.clone()],
            vec!["fmap"],
        );
        let e = var("e", 30_021);
        add_inst(
            ctx,
            "Applicative",
            vec![],
            Ty::app(
                Ty::con("Result", Kind::arr(Star, Kind::arr(Star, Star))),
                Ty::Var(e.clone()),
            ),
            vec![e.clone()],
            vec!["pure", "<*>"],
        );
        let e = var("e", 30_022);
        add_inst(
            ctx,
            "Monad",
            vec![],
            Ty::app(
                Ty::con("Result", Kind::arr(Star, Kind::arr(Star, Star))),
                Ty::Var(e.clone()),
            ),
            vec![e.clone()],
            vec![">>="],
        );
    }
    {
        // Eq / Ord / Show (Result e a)
        for (class, methods, extra_inst) in [
            ("Eq", vec!["==", "/="], true),
            ("Ord", vec!["compare"], true),
            ("Show", vec!["show"], true),
        ] {
            let e = var("e", 30_030);
            let a = var("a", 30_031);
            let _ = extra_inst;
            add_inst(
                ctx,
                class,
                vec![
                    Constraint {
                        class: class.into(),
                        arg: Ty::Var(e.clone()),
                    },
                    Constraint {
                        class: class.into(),
                        arg: Ty::Var(a.clone()),
                    },
                ],
                result_(Ty::Var(e.clone()), Ty::Var(a.clone())),
                vec![e, a],
                methods,
            );
        }
    }

    // List
    {
        add_inst(
            ctx,
            "Functor",
            vec![],
            Ty::con("List", Kind::arr(Star, Star)),
            vec![],
            vec!["fmap"],
        );
        add_inst(
            ctx,
            "Applicative",
            vec![],
            Ty::con("List", Kind::arr(Star, Star)),
            vec![],
            vec!["pure", "<*>"],
        );
        add_inst(
            ctx,
            "Monad",
            vec![],
            Ty::con("List", Kind::arr(Star, Star)),
            vec![],
            vec![">>="],
        );
        let a = var("a", 30_040);
        add_inst(
            ctx,
            "Eq",
            vec![Constraint {
                class: "Eq".into(),
                arg: Ty::Var(a.clone()),
            }],
            list_(Ty::Var(a.clone())),
            vec![a.clone()],
            vec!["==", "/="],
        );
        let a = var("a", 30_041);
        add_inst(
            ctx,
            "Ord",
            vec![Constraint {
                class: "Ord".into(),
                arg: Ty::Var(a.clone()),
            }],
            list_(Ty::Var(a.clone())),
            vec![a.clone()],
            vec!["compare"],
        );
        let a = var("a", 30_042);
        add_inst(
            ctx,
            "Show",
            vec![Constraint {
                class: "Show".into(),
                arg: Ty::Var(a.clone()),
            }],
            list_(Ty::Var(a.clone())),
            vec![a.clone()],
            vec!["show"],
        );
    }

    // Ruby monad instance (spec 11).
    add_inst(
        ctx,
        "Functor",
        vec![],
        Ty::con("Ruby", Kind::arr(Star, Star)),
        vec![],
        vec!["fmap"],
    );
    add_inst(
        ctx,
        "Applicative",
        vec![],
        Ty::con("Ruby", Kind::arr(Star, Star)),
        vec![],
        vec!["pure", "<*>"],
    );
    add_inst(
        ctx,
        "Monad",
        vec![],
        Ty::con("Ruby", Kind::arr(Star, Star)),
        vec![],
        vec![">>="],
    );
}

// A tiny helper AST for registering prelude data decls cheaply.
enum AstTypeKind {
    TVar(&'static str),
    TCon(&'static str),
    App(Box<AstTypeKind>, Box<AstTypeKind>),
}

fn register_data(
    ctx: &mut InferCtx,
    name: &str,
    params: &[&str],
    ctors: &[(&str, Vec<AstTypeKind>)],
) {
    let param_strings: Vec<String> = params.iter().map(|s| (*s).to_string()).collect();
    let ctor_names: Vec<String> = ctors.iter().map(|c| c.0.to_string()).collect();
    ctx.type_env.datas.insert(
        name.into(),
        DataInfo {
            name: name.into(),
            params: param_strings.clone(),
            ctor_names,
            home_module: "Prelude".into(),
        },
    );

    for (ctor_name, args) in ctors {
        // Build the ctor's scheme. Each type parameter becomes a fresh
        // TyVar; the ctor's result is `T p1 p2 ...`.
        let mut param_vars: HashMap<String, TyVar> = HashMap::new();
        let mut vars_ordered: Vec<TyVar> = Vec::new();
        for p in &param_strings {
            let tv = TyVar {
                id: {
                    let id = ctx.fresh_counter;
                    ctx.fresh_counter += 1;
                    id
                },
                name: p.clone(),
            };
            param_vars.insert(p.clone(), tv.clone());
            vars_ordered.push(tv);
        }
        // Build result type `Con name` applied to each param.
        let head_kind = make_kind_arity(param_strings.len());
        let mut res = Ty::con(name, head_kind);
        for v in &vars_ordered {
            res = Ty::app(res, Ty::Var(v.clone()));
        }
        // Build the ctor's function type: args... -> res.
        let mut ctor_ty = res.clone();
        for a in args.iter().rev() {
            let a_ty = prelude_ast_to_ty(a, &param_vars);
            ctor_ty = Ty::fun(a_ty, ctor_ty);
        }
        let scheme = Scheme {
            vars: vars_ordered,
            context: vec![],
            body: ctor_ty,
        };
        ctx.type_env.ctors.insert(
            ctor_name.to_string(),
            CtorInfo {
                type_name: name.into(),
                scheme: scheme.clone(),
                arity: args.len(),
            },
        );
        ctx.type_env
            .globals
            .insert(GlobalId::new("Prelude", *ctor_name), scheme);
    }
}

fn prelude_ast_to_ty(a: &AstTypeKind, params: &HashMap<String, TyVar>) -> Ty {
    match a {
        AstTypeKind::TVar(n) => Ty::Var(params.get(*n).cloned().unwrap_or_else(|| TyVar {
            id: 0,
            name: (*n).into(),
        })),
        AstTypeKind::TCon(n) => {
            let kind = builtin_kind_of(n).unwrap_or(Kind::Star);
            Ty::con(*n, kind)
        }
        AstTypeKind::App(f, x) => {
            Ty::app(prelude_ast_to_ty(f, params), prelude_ast_to_ty(x, params))
        }
    }
}

fn rename_named_vars(t: &Ty, subst: &HashMap<&str, Ty>) -> Ty {
    match t {
        Ty::Var(v) => {
            if let Some(t2) = subst.get(v.name.as_str()) {
                return t2.clone();
            }
            Ty::Var(v.clone())
        }
        Ty::Con { name, kind } => Ty::Con {
            name: name.clone(),
            kind: kind.clone(),
        },
        Ty::App(a, b) => Ty::app(rename_named_vars(a, subst), rename_named_vars(b, subst)),
        Ty::Fun(a, b) => Ty::fun(rename_named_vars(a, subst), rename_named_vars(b, subst)),
        Ty::Record(fs) => Ty::record(
            fs.iter()
                .map(|(n, t)| (n.clone(), rename_named_vars(t, subst)))
                .collect(),
        ),
    }
}

// =====================================================================
//  Module-level driver
// =====================================================================

/// Process a single AST module: register data / alias / class / ctor
/// decls first, then check each value binding's body. Returns the
/// complete inferred top-level schemes (keyed by binding name).
pub fn check_module(ctx: &mut InferCtx, module: &AstModule) -> Result<(), Vec<TypeError>> {
    let mut errors: Vec<TypeError> = Vec::new();

    // Phase A: data, alias, class, signatures.
    for decl in &module.decls {
        match decl {
            Decl::Data(d) => {
                if let Err(e) = register_ast_data(ctx, d) {
                    errors.push(e);
                }
            }
            Decl::TypeAlias(a) => {
                if let Err(e) = register_ast_alias(ctx, a) {
                    errors.push(e);
                }
            }
            Decl::Class(c) => {
                if let Err(e) = register_ast_class(ctx, c) {
                    errors.push(e);
                }
            }
            _ => {}
        }
    }

    // Phase B: signatures become the expected schemes.
    let mut sigs: HashMap<String, Scheme> = HashMap::new();
    for decl in &module.decls {
        if let Decl::Signature { name, scheme, .. } = decl {
            match scheme_from_ast(ctx, scheme) {
                Ok(s) => {
                    sigs.insert(name.clone(), s.clone());
                    ctx.type_env
                        .globals
                        .insert(GlobalId::new(&ctx.module, name), s);
                }
                Err(e) => errors.push(e),
            }
        }
    }

    // Phase C: add provisional fresh schemes for signatureless values.
    let mut provisional: HashMap<String, TyVar> = HashMap::new();
    for decl in &module.decls {
        if let Decl::Value(vc) = decl {
            if !sigs.contains_key(&vc.name) && !provisional.contains_key(&vc.name) {
                let tv = ctx.fresh_named(&vc.name);
                provisional.insert(vc.name.clone(), tv.clone());
                ctx.type_env.globals.insert(
                    GlobalId::new(&ctx.module, &vc.name),
                    Scheme::mono(Ty::Var(tv)),
                );
            }
        }
        if let Decl::RubyEmbed(r) = decl {
            if !sigs.contains_key(&r.name) {
                // RubyEmbed must have a signature (spec 10); if
                // missing, treat as error.
                errors.push(TypeError::new(
                    TypeErrorKind::Other {
                        msg: format!("`:=` binding `{}` requires a type signature", r.name),
                    },
                    r.span,
                ));
            }
        }
    }

    // Phase D: pre-register instance heads (no body check yet) so
    // value-binding inference can see every instance in scope.
    // We will re-check the instance bodies in phase F with full
    // context once value bindings are done.
    for decl in &module.decls {
        if let Decl::Instance(i) = decl {
            if let Err(e) = register_instance_head(ctx, i, &module.decls) {
                errors.push(e);
            }
        }
    }

    // Phase E: check each value binding.
    // Group clauses by name (multiple clauses = single def).
    let mut clauses_by_name: Vec<(String, Vec<ValueClause>)> = Vec::new();
    for decl in &module.decls {
        if let Decl::Value(vc) = decl {
            if let Some(existing) = clauses_by_name.iter_mut().find(|(n, _)| n == &vc.name) {
                existing.1.push(vc.clone());
            } else {
                clauses_by_name.push((vc.name.clone(), vec![vc.clone()]));
            }
        }
    }

    for (name, clauses) in &clauses_by_name {
        let sig = sigs.get(name).cloned();
        match check_value_binding(ctx, name, clauses, sig) {
            Ok(scheme) => {
                ctx.type_env
                    .globals
                    .insert(GlobalId::new(&ctx.module, name), scheme.clone());
                if !ctx.inferred.contains_key(name) {
                    ctx.inferred_order.push(name.clone());
                }
                ctx.inferred.insert(name.clone(), scheme);
            }
            Err(e) => errors.push(e),
        }
    }

    // Ruby-embed bindings use their signature verbatim; no body to
    // check at this layer.
    for decl in &module.decls {
        if let Decl::RubyEmbed(r) = decl {
            if let Some(sch) = sigs.get(&r.name) {
                if !ctx.inferred.contains_key(&r.name) {
                    ctx.inferred_order.push(r.name.clone());
                }
                ctx.inferred.insert(r.name.clone(), sch.clone());
            }
        }
    }

    // Phase F: instance bodies.
    for decl in &module.decls {
        if let Decl::Instance(i) = decl {
            if let Err(e) = check_instance_body(ctx, i) {
                errors.push(e);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn register_ast_data(ctx: &mut InferCtx, d: &DataDecl) -> Result<(), TypeError> {
    // Register the data name first so recursive references in ctor
    // args resolve. The ctor list is filled in below.
    ctx.type_env.datas.insert(
        d.name.clone(),
        DataInfo {
            name: d.name.clone(),
            params: d.type_params.clone(),
            ctor_names: Vec::new(),
            home_module: ctx.module.clone(),
        },
    );

    let mut vars_ordered: Vec<TyVar> = Vec::new();
    let mut locals: HashMap<String, TyVar> = HashMap::new();
    for p in &d.type_params {
        let tv = TyVar {
            id: {
                let id = ctx.fresh_counter;
                ctx.fresh_counter += 1;
                id
            },
            name: p.clone(),
        };
        locals.insert(p.clone(), tv.clone());
        vars_ordered.push(tv);
    }
    let head_kind = make_kind_arity(d.type_params.len());
    let mut res_ty = Ty::con(&d.name, head_kind);
    for v in &vars_ordered {
        res_ty = Ty::app(res_ty, Ty::Var(v.clone()));
    }

    let mut ctor_names: Vec<String> = Vec::new();
    for c in &d.ctors {
        ctor_names.push(c.name.clone());
        let mut ctor_ty = res_ty.clone();
        for a in c.args.iter().rev() {
            let arg_ty = ty_from_ast(ctx, a, &locals)?;
            ctor_ty = Ty::fun(arg_ty, ctor_ty);
        }
        let scheme = Scheme {
            vars: vars_ordered.clone(),
            context: vec![],
            body: ctor_ty,
        };
        ctx.type_env.ctors.insert(
            c.name.clone(),
            CtorInfo {
                type_name: d.name.clone(),
                scheme: scheme.clone(),
                arity: c.args.len(),
            },
        );
        ctx.type_env
            .globals
            .insert(GlobalId::new(&ctx.module, &c.name), scheme);
    }

    if let Some(info) = ctx.type_env.datas.get_mut(&d.name) {
        info.ctor_names = ctor_names;
    }
    Ok(())
}

fn register_ast_alias(ctx: &mut InferCtx, a: &TypeAlias) -> Result<(), TypeError> {
    let mut locals: HashMap<String, TyVar> = HashMap::new();
    for p in &a.type_params {
        let tv = TyVar {
            id: 0,
            name: p.clone(),
        };
        locals.insert(p.clone(), tv);
    }
    let body = ty_from_ast(ctx, &a.body, &locals)?;
    ctx.type_env.aliases.insert(
        a.name.clone(),
        AliasInfo {
            name: a.name.clone(),
            params: a.type_params.clone(),
            body,
        },
    );
    Ok(())
}

fn register_ast_class(ctx: &mut InferCtx, c: &ClassDecl) -> Result<(), TypeError> {
    let mut superclasses: Vec<String> = Vec::new();
    for ctx_c in &c.context {
        // Spec 07 §Class declarations: "Superclass constraints come
        // before the class head" and constrain the class's own type
        // variable. A superclass that mentions a different variable
        // (e.g. `class Foo b => Ord a where ...`) is ill-formed.
        if ctx_c.args.len() != 1 {
            return Err(TypeError::new(
                TypeErrorKind::InvalidSuperclassContext {
                    class: c.name.clone(),
                    expected: c.type_var.clone(),
                    got: format!(
                        "superclass `{}` with {} argument(s)",
                        ctx_c.class_name,
                        ctx_c.args.len()
                    ),
                },
                ctx_c.span,
            ));
        }
        match &ctx_c.args[0] {
            AstType::Var { name, .. } if name == &c.type_var => {
                // OK — constrains the class's own tvar.
            }
            other => {
                let got = match other {
                    AstType::Var { name, .. } => format!("type variable `{name}`"),
                    _ => format!(
                        "non-variable type argument in superclass `{}`",
                        ctx_c.class_name
                    ),
                };
                return Err(TypeError::new(
                    TypeErrorKind::InvalidSuperclassContext {
                        class: c.name.clone(),
                        expected: c.type_var.clone(),
                        got,
                    },
                    ctx_c.span,
                ));
            }
        }
        superclasses.push(ctx_c.class_name.clone());
    }
    let tv = TyVar {
        id: {
            let id = ctx.fresh_counter;
            ctx.fresh_counter += 1;
            id
        },
        name: c.type_var.clone(),
    };
    let mut methods: HashMap<String, Scheme> = HashMap::new();
    let mut defaults: Vec<String> = Vec::new();
    for item in &c.items {
        match item {
            ClassItem::Signature { name, scheme, .. } => {
                let mut locals: HashMap<String, TyVar> = HashMap::new();
                // Class tvar is implicit in the method scheme's env.
                locals.insert(c.type_var.clone(), tv.clone());
                // Collect free type-var names from body + context.
                let mut names: Vec<String> = Vec::new();
                for n in &scheme.forall {
                    if !names.contains(n) {
                        names.push(n.clone());
                    }
                }
                collect_names_in_ty(&scheme.body, &mut names);
                for cc in &scheme.context {
                    for ac in &cc.args {
                        collect_names_in_ty(ac, &mut names);
                    }
                }
                let mut rigids: Vec<TyVar> = vec![tv.clone()];
                for n in &names {
                    if n == &c.type_var {
                        continue;
                    }
                    let nv = TyVar {
                        id: {
                            let id = ctx.fresh_counter;
                            ctx.fresh_counter += 1;
                            id
                        },
                        name: n.clone(),
                    };
                    locals.insert(n.clone(), nv.clone());
                    rigids.push(nv);
                }
                let body = ty_from_ast(ctx, &scheme.body, &locals)?;
                let mut context = Vec::new();
                for cc in &scheme.context {
                    if cc.args.len() != 1 {
                        return Err(TypeError::new(
                            TypeErrorKind::Other {
                                msg: "class constraint must be single-param".into(),
                            },
                            cc.span,
                        ));
                    }
                    let arg = ty_from_ast(ctx, &cc.args[0], &locals)?;
                    context.push(Constraint {
                        class: cc.class_name.clone(),
                        arg,
                    });
                }
                let full_scheme = Scheme {
                    vars: rigids,
                    context,
                    body,
                };
                methods.insert(name.clone(), full_scheme.clone());
                // Register the method as a global value with an
                // additional `C a =>` constraint.
                let mut ctx_list = full_scheme.context.clone();
                let class_constraint = Constraint {
                    class: c.name.clone(),
                    arg: Ty::Var(tv.clone()),
                };
                if !ctx_list.contains(&class_constraint) {
                    ctx_list.push(class_constraint);
                }
                let visible = Scheme {
                    vars: full_scheme.vars.clone(),
                    context: ctx_list,
                    body: full_scheme.body.clone(),
                };
                ctx.type_env
                    .globals
                    .insert(GlobalId::new(&ctx.module, name), visible);
            }
            ClassItem::Default(clause) => {
                defaults.push(clause.name.clone());
            }
        }
    }
    ctx.class_env.register_class(ClassInfo {
        name: c.name.clone(),
        type_var: c.type_var.clone(),
        superclasses,
        methods,
        defaults,
        home_module: ctx.module.clone(),
    });
    Ok(())
}

fn register_instance_head(
    ctx: &mut InferCtx,
    inst: &InstanceDecl,
    _all_decls: &[Decl],
) -> Result<(), TypeError> {
    if !ctx.class_env.classes.contains_key(&inst.name) {
        return Err(TypeError::new(
            TypeErrorKind::UnknownClass {
                name: inst.name.clone(),
            },
            inst.span,
        ));
    }
    // Validate head shape: either a ground type or C applied to
    // distinct type variables.
    let head_names: Vec<String> = ast_head_vars(&inst.head);
    let distinct_vars = {
        let mut seen = HashSet::new();
        head_names.iter().all(|n| seen.insert(n.clone()))
    };
    if !distinct_vars {
        return Err(TypeError::new(
            TypeErrorKind::InvalidInstanceHead {
                class: inst.name.clone(),
                head: Ty::star("<invalid>"),
            },
            inst.span,
        ));
    }

    let mut locals: HashMap<String, TyVar> = HashMap::new();
    let mut vars_ordered: Vec<TyVar> = Vec::new();
    for n in &head_names {
        let tv = TyVar {
            id: {
                let id = ctx.fresh_counter;
                ctx.fresh_counter += 1;
                id
            },
            name: n.clone(),
        };
        locals.insert(n.clone(), tv.clone());
        vars_ordered.push(tv);
    }
    let head_ty = ty_from_ast(ctx, &inst.head, &locals)?;

    // Context.
    let mut context: Vec<Constraint> = Vec::new();
    for c in &inst.context {
        if c.args.len() != 1 {
            return Err(TypeError::new(
                TypeErrorKind::Other {
                    msg: "instance context constraints must be single-param".into(),
                },
                c.span,
            ));
        }
        let arg = ty_from_ast(ctx, &c.args[0], &locals)?;
        context.push(Constraint {
            class: c.class_name.clone(),
            arg,
        });
    }

    // Orphan check (spec 07 §Orphan instances, strict).
    //
    // An `instance C T` is an orphan iff neither `C` nor the outermost
    // type constructor of `T` is declared in the same module as this
    // instance. Built-in prelude types (e.g. `Int`, `String`) have
    // `home_module = "Prelude"`; built-in class `home_module` is also
    // `"Prelude"`. For the current module to admit an instance, one
    // of the following must hold:
    //   - the class was declared here (`class_home == ctx.module`), OR
    //   - the head's outer ctor was declared here, OR
    //   - the instance declares a new class/data in this module (both
    //     cases already covered by the two predicates above via
    //     `_all_decls`, for symmetry with the pre-home_module code).
    //
    // Records (structural types, `head_con = None`) have no outer
    // ctor; such instances must rely on the class being local.
    let class_home: Option<String> = ctx
        .class_env
        .classes
        .get(&inst.name)
        .map(|ci| ci.home_module.clone());
    let head_ctor = head_ty.head_con();
    let head_home: Option<String> = head_ctor.and_then(|name| {
        if let Some(di) = ctx.type_env.datas.get(name) {
            Some(di.home_module.clone())
        } else if is_builtin_type(name) {
            Some("Prelude".to_string())
        } else {
            None
        }
    });
    // Accept if either home matches the current module, or if the
    // class / data is being freshly declared in this module (the
    // two decl predicates keep working for module-internal
    // re-declarations that pre-date `home_module`).
    let class_local =
        class_home.as_deref() == Some(ctx.module.as_str()) || is_user_class(&inst.name, _all_decls);
    let head_local = head_home.as_deref() == Some(ctx.module.as_str())
        || head_ctor
            .map(|n| is_user_data(n, _all_decls))
            .unwrap_or(false);
    if !class_local && !head_local {
        return Err(TypeError::new(
            TypeErrorKind::OrphanInstance {
                class: inst.name.clone(),
            },
            inst.span,
        ));
    }

    // Collect already-registered instances for overlap check.
    let already: Vec<InstanceInfo> = ctx.class_env.instances_for(&inst.name).cloned().collect();
    // Refresh the head with fresh vars to check unification with
    // existing instances.
    {
        let mut sub = Subst::new();
        for v in &vars_ordered {
            let nv = ctx.fresh();
            sub.insert(v.id, Ty::Var(nv));
        }
        let our_head = sub.apply(&head_ty);
        for other in &already {
            let mut sub2 = Subst::new();
            for v in &other.vars {
                let nv = ctx.fresh();
                sub2.insert(v.id, Ty::Var(nv));
            }
            let their_head = sub2.apply(&other.head);
            if unify(&our_head, &their_head, inst.span).is_ok() {
                return Err(TypeError::new(
                    TypeErrorKind::OverlappingInstance {
                        class: inst.name.clone(),
                        head: head_ty.clone(),
                    },
                    inst.span,
                ));
            }
        }
    }

    // Register the instance.
    let methods_impl: Vec<String> = inst.items.iter().map(|c| c.name.clone()).collect();
    ctx.class_env.register_instance(InstanceInfo {
        class: inst.name.clone(),
        context: context.clone(),
        head: head_ty.clone(),
        vars: vars_ordered.clone(),
        methods: methods_impl.clone(),
    });
    Ok(())
}

fn check_instance_body(ctx: &mut InferCtx, inst: &InstanceDecl) -> Result<(), TypeError> {
    if !ctx.class_env.classes.contains_key(&inst.name) {
        return Ok(());
    }
    let head_names: Vec<String> = ast_head_vars(&inst.head);
    let mut locals: HashMap<String, TyVar> = HashMap::new();
    let mut vars_ordered: Vec<TyVar> = Vec::new();
    for n in &head_names {
        let tv = TyVar {
            id: {
                let id = ctx.fresh_counter;
                ctx.fresh_counter += 1;
                id
            },
            name: n.clone(),
        };
        locals.insert(n.clone(), tv.clone());
        vars_ordered.push(tv);
    }
    let head_ty = ty_from_ast(ctx, &inst.head, &locals)?;
    let mut context: Vec<Constraint> = Vec::new();
    for c in &inst.context {
        if c.args.len() != 1 {
            continue;
        }
        let arg = ty_from_ast(ctx, &c.args[0], &locals)?;
        context.push(Constraint {
            class: c.class_name.clone(),
            arg,
        });
    }

    // Check each method clause body against the class's method scheme
    // with `a -> head_ty` substitution.
    let class_info = ctx.class_env.classes[&inst.name].clone();
    for clause in &inst.items {
        let Some(msch) = class_info.methods.get(&clause.name) else {
            return Err(TypeError::new(
                TypeErrorKind::Other {
                    msg: format!(
                        "instance `{}` provides method `{}` not declared in class",
                        inst.name, clause.name
                    ),
                },
                clause.span,
            ));
        };
        let mut sub = Subst::new();
        // Substitute the class tvar with head_ty.
        let class_tv = msch
            .vars
            .iter()
            .find(|v| v.name == class_info.type_var)
            .cloned();
        if let Some(ctv) = class_tv {
            sub.insert(ctv.id, head_ty.clone());
        }
        // Fresh substitutions for the other scheme vars.
        for v in &msch.vars {
            if v.name == class_info.type_var {
                continue;
            }
            let nv = ctx.fresh_named(&v.name);
            sub.insert(v.id, Ty::Var(nv));
        }
        let expected_body = sub.apply(&msch.body);

        // Push the instance's context as assumptions during body check.
        let saved_assumed = ctx.assumed.clone();
        ctx.assumed.extend(context.clone());
        let r = check_clause_against(ctx, clause, &expected_body);
        ctx.assumed = saved_assumed;
        r?;
    }
    Ok(())
}

fn is_user_class(name: &str, decls: &[Decl]) -> bool {
    decls.iter().any(|d| {
        if let Decl::Class(c) = d {
            c.name == name
        } else {
            false
        }
    })
}

fn is_user_data(name: &str, decls: &[Decl]) -> bool {
    decls.iter().any(|d| {
        if let Decl::Data(c) = d {
            c.name == name
        } else {
            false
        }
    })
}

fn ast_head_vars(t: &AstType) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(t: &AstType, out: &mut Vec<String>) {
        if let AstType::App { arg, func, .. } = t {
            walk(func, out);
            if let AstType::Var { name, .. } = arg.as_ref() {
                out.push(name.clone());
            }
        }
    }
    walk(t, &mut out);
    out
}

fn check_clause_against(
    ctx: &mut InferCtx,
    clause: &ValueClause,
    expected: &Ty,
) -> Result<(), TypeError> {
    // Infer body with params, unify the resulting function type with `expected`.
    let (ty, sub) = infer_value_clause(ctx, clause)?;
    let sub2 = unify(&sub.apply(&ty), &sub.apply(expected), clause.span)?;
    let final_sub = sub2.compose(&sub);
    let _ = final_sub;
    Ok(())
}

fn check_value_binding(
    ctx: &mut InferCtx,
    name: &str,
    clauses: &[ValueClause],
    sig: Option<Scheme>,
) -> Result<Scheme, TypeError> {
    // We handle multiple clauses by constraining all to a common type
    // via unification.
    // Recursive calls see the global slot we installed earlier
    // (either signature or provisional fresh var).

    let mut global_sub = Subst::new();

    // Expected type (either from signature or fresh).
    let (expected_ty, assumed, sig_vars): (Ty, Vec<Constraint>, Vec<TyVar>) = match &sig {
        Some(s) => {
            // Rigid scheme vars: keep them rigid during checking.
            (s.body.clone(), s.context.clone(), s.vars.clone())
        }
        None => {
            let tv = ctx.fresh_named(name);
            (Ty::Var(tv.clone()), vec![], vec![])
        }
    };

    let saved_assumed = ctx.assumed.clone();
    ctx.assumed.extend(assumed.clone());
    let _ = sig_vars;

    for clause in clauses {
        // If we have a signature, pre-split its fun-chain so each
        // parameter gets the annotated type rather than a fresh var.
        let hint = if sig.is_some() {
            let applied = global_sub.apply(&expected_ty);
            let (args, _res) = applied.split_fun();
            let hints: Vec<Ty> = args.iter().map(|t| (*t).clone()).collect();
            if hints.len() >= clause.params.len() {
                Some(hints)
            } else {
                None
            }
        } else {
            None
        };
        let (ty, sub) = infer_value_clause_hinted(ctx, clause, hint.as_deref())?;
        global_sub = sub.compose(&global_sub);
        let sub2 = unify(
            &global_sub.apply(&ty),
            &global_sub.apply(&expected_ty),
            clause.span,
        )?;
        global_sub = sub2.compose(&global_sub);
    }

    // Resolve any pending field-access goals under the current subst.
    let mut pending = std::mem::take(&mut ctx.pending_fields);
    let mut progress = true;
    while progress {
        progress = false;
        let mut still_pending = Vec::new();
        for (rty, field, result_ty, span) in pending.drain(..) {
            let rty_a = global_sub.apply(&rty);
            match &rty_a {
                Ty::Record(fs) => {
                    let Some((_, fty)) = fs.iter().find(|(n, _)| n == &field) else {
                        return Err(TypeError::new(
                            TypeErrorKind::MissingField {
                                field,
                                ty: rty_a.clone(),
                            },
                            span,
                        ));
                    };
                    let sub = unify(&global_sub.apply(fty), &global_sub.apply(&result_ty), span)?;
                    global_sub = sub.compose(&global_sub);
                    progress = true;
                }
                _ => {
                    still_pending.push((rty, field, result_ty, span));
                }
            }
        }
        pending = still_pending;
    }
    if !pending.is_empty() {
        let (_rty, field, _res, span) = &pending[0];
        return Err(TypeError::new(
            TypeErrorKind::Other {
                msg: format!(
                    "cannot resolve field access `.{}` — add a type annotation",
                    field
                ),
            },
            *span,
        ));
    }

    // Resolve residual constraints. We allocate fresh TyVars through
    // a local counter that we sync back into `ctx.fresh_counter` at
    // the end, since simplify borrows the class env immutably.
    let mut local_counter = ctx.fresh_counter;
    let mut fresh_fn = || {
        let id = local_counter;
        local_counter += 1;
        TyVar {
            id,
            name: String::new(),
        }
    };
    let wanted_snapshot: Vec<Constraint> = ctx
        .wanted
        .iter()
        .map(|(c, _)| global_sub.apply_constraint(c))
        .collect();
    let wanted_span = ctx
        .wanted
        .first()
        .map(|(_, s)| *s)
        .unwrap_or(Span::empty(0));
    let residual = simplify(
        &ctx.class_env,
        &ctx.assumed,
        &wanted_snapshot,
        wanted_span,
        &mut fresh_fn,
    )?;
    ctx.fresh_counter = local_counter;
    ctx.wanted.clear();
    for c in residual {
        ctx.wanted.push((c, wanted_span));
    }

    ctx.assumed = saved_assumed;

    // If a signature was given, use it verbatim (check no extra
    // residual beyond it).
    let scheme = if let Some(s) = sig {
        // Verify residual constraints are entailed by the signature's
        // context.
        let mut leftover = Vec::new();
        for (c, sp) in ctx.wanted.drain(..) {
            let c2 = global_sub.apply_constraint(&c);
            let mut entailed = false;
            for a in &s.context {
                if super::classes::entails_by_super(&ctx.class_env, a, &c2) {
                    entailed = true;
                    break;
                }
            }
            if !entailed {
                leftover.push((c2, sp));
            }
        }
        if !leftover.is_empty() {
            let (c, sp) = &leftover[0];
            return Err(TypeError::new(
                TypeErrorKind::UnresolvedConstraint {
                    constraint: c.clone(),
                },
                *sp,
            ));
        }
        s
    } else {
        // Exclude the provisional self-slot inserted for this name in
        // phase C of `check_module`. Without the exclusion, the
        // provisional `Scheme::mono(Var(tv))` would pin this
        // binding's fresh type variables into env_fvs (via substitution
        // after inference), breaking generalisation. See
        // `InferCtx::generalize_excluding`'s rationale.
        ctx.generalize_excluding(&global_sub, &expected_ty, Some(name))
    };

    Ok(scheme)
}

// =====================================================================
//  Expression inference
// =====================================================================

fn infer_value_clause(ctx: &mut InferCtx, clause: &ValueClause) -> Result<(Ty, Subst), TypeError> {
    infer_value_clause_hinted(ctx, clause, None)
}

/// Like `infer_value_clause` but uses `hint_params` (the expected
/// parameter types from a signature) as each pattern's target type
/// where available. This threads signature information into the body
/// inference so that `p.x` style field access works for annotated
/// parameters.
fn infer_value_clause_hinted(
    ctx: &mut InferCtx,
    clause: &ValueClause,
    hint_params: Option<&[Ty]>,
) -> Result<(Ty, Subst), TypeError> {
    ctx.type_env.push_locals();
    let mut param_tys: Vec<Ty> = Vec::new();
    let mut global_sub = Subst::new();
    for (i, p) in clause.params.iter().enumerate() {
        let pty = match hint_params {
            Some(hs) if i < hs.len() => hs[i].clone(),
            _ => ctx.fresh_ty(),
        };
        let (s, ()) = bind_pattern(ctx, p, &pty)?;
        global_sub = s.compose(&global_sub);
        param_tys.push(pty);
    }
    let (body_ty, sub) = infer_expr(ctx, &clause.body)?;
    global_sub = sub.compose(&global_sub);
    ctx.type_env.pop_locals();

    let mut result = body_ty;
    for pty in param_tys.iter().rev() {
        result = Ty::fun(global_sub.apply(pty), result);
    }
    Ok((result, global_sub))
}

/// Infer the type of a pattern, binding its vars into the current
/// local scope against `expected_ty`.
fn bind_pattern(
    ctx: &mut InferCtx,
    pat: &Pattern,
    expected_ty: &Ty,
) -> Result<(Subst, ()), TypeError> {
    match pat {
        Pattern::Wildcard(_) => Ok((Subst::new(), ())),
        Pattern::Var { name, .. } => {
            ctx.type_env
                .bind_local(name, Scheme::mono(expected_ty.clone()));
            Ok((Subst::new(), ()))
        }
        Pattern::As { name, inner, .. } => {
            ctx.type_env
                .bind_local(name, Scheme::mono(expected_ty.clone()));
            bind_pattern(ctx, inner, expected_ty)
        }
        Pattern::Lit(lit, span) => {
            let ty = literal_type(lit);
            let s = unify(&ty, expected_ty, *span)?;
            Ok((s, ()))
        }
        Pattern::Con {
            name, args, span, ..
        } => {
            let Some(cinfo) = ctx.type_env.ctors.get(name).cloned() else {
                return Err(TypeError::new(
                    TypeErrorKind::Other {
                        msg: format!("unknown constructor `{name}`"),
                    },
                    *span,
                ));
            };
            if cinfo.arity != args.len() {
                return Err(TypeError::new(
                    TypeErrorKind::CtorArity {
                        ctor: name.clone(),
                        expected: cinfo.arity,
                        found: args.len(),
                    },
                    *span,
                ));
            }
            let (ctor_ty, _ctor_ctx) = ctx.instantiate(&cinfo.scheme);
            // ctor_ty is A -> B -> ... -> T params
            let (arg_tys_ref, result_ref) = ctor_ty.split_fun();
            let arg_tys: Vec<Ty> = arg_tys_ref.into_iter().cloned().collect();
            let result = result_ref.clone();
            let mut sub = unify(&result, expected_ty, *span)?;
            for (p, pt) in args.iter().zip(arg_tys.iter()) {
                let (s, ()) = bind_pattern(ctx, p, &sub.apply(pt))?;
                sub = s.compose(&sub);
            }
            Ok((sub, ()))
        }
        Pattern::Cons { head, tail, span } => {
            // List a expected.
            let elem = ctx.fresh_ty();
            let list_ty = Ty::app(
                Ty::con("List", Kind::arr(Kind::Star, Kind::Star)),
                elem.clone(),
            );
            let mut sub = unify(&list_ty, expected_ty, *span)?;
            let (s1, ()) = bind_pattern(ctx, head, &sub.apply(&elem))?;
            sub = s1.compose(&sub);
            let (s2, ()) = bind_pattern(ctx, tail, &sub.apply(&list_ty))?;
            sub = s2.compose(&sub);
            Ok((sub, ()))
        }
        Pattern::List { items, span } => {
            let elem = ctx.fresh_ty();
            let list_ty = Ty::app(
                Ty::con("List", Kind::arr(Kind::Star, Kind::Star)),
                elem.clone(),
            );
            let mut sub = unify(&list_ty, expected_ty, *span)?;
            for p in items {
                let (s, ()) = bind_pattern(ctx, p, &sub.apply(&elem))?;
                sub = s.compose(&sub);
            }
            Ok((sub, ()))
        }
        Pattern::Record { fields, span } => {
            // Build a record type with fresh vars for unspecified fields?
            // Spec 04: subset patterns are allowed.
            // The expected_ty may be a record of wider set; we generate
            // fresh field types and unify each with expected_ty's
            // corresponding field.
            // Simplification: require expected_ty to be a Record.
            let expected = expected_ty.clone();
            match &expected {
                Ty::Record(fs) => {
                    let mut sub = Subst::new();
                    for (fname, pat) in fields {
                        let Some((_, fty)) = fs.iter().find(|(n, _)| n == fname) else {
                            return Err(TypeError::new(
                                TypeErrorKind::RecordPatternField {
                                    field: fname.clone(),
                                    ty: expected.clone(),
                                },
                                *span,
                            ));
                        };
                        let (s, ()) = bind_pattern(ctx, pat, &sub.apply(fty))?;
                        sub = s.compose(&sub);
                    }
                    Ok((sub, ()))
                }
                Ty::Var(_) => {
                    // If the expected type is a fresh var, build a
                    // record type from the patterns' types. Note this
                    // requires all fields to be enumerated; subset
                    // matching against a free var is not expressible
                    // with closed records alone.
                    let mut fs = Vec::new();
                    let mut sub = Subst::new();
                    for (fname, pat) in fields {
                        let ft = ctx.fresh_ty();
                        let (s, ()) = bind_pattern(ctx, pat, &ft)?;
                        sub = s.compose(&sub);
                        fs.push((fname.clone(), sub.apply(&ft)));
                    }
                    let r = Ty::record(fs);
                    let s = unify(&r, &sub.apply(&expected), *span)?;
                    Ok((s.compose(&sub), ()))
                }
                _ => Err(TypeError::new(
                    TypeErrorKind::Mismatch {
                        expected: expected.clone(),
                        found: Ty::Record(vec![]),
                    },
                    *span,
                )),
            }
        }
        Pattern::Annot { inner, ty, span } => {
            // Convert the annotation; unify.
            let bound_names = {
                let mut names = Vec::new();
                collect_names_in_ty(ty, &mut names);
                names
            };
            let mut locals = HashMap::new();
            for n in &bound_names {
                // If already in type scope, reuse. Otherwise fresh.
                if let Some(v) = ctx.type_env.lookup_type_local(n) {
                    locals.insert(n.clone(), v.clone());
                } else {
                    locals.insert(n.clone(), ctx.fresh_named(n));
                }
            }
            let ann_ty = ty_from_ast(ctx, ty, &locals)?;
            let s = unify(&ann_ty, expected_ty, *span)?;
            let (s2, ()) = bind_pattern(ctx, inner, &s.apply(expected_ty))?;
            Ok((s2.compose(&s), ()))
        }
    }
}

fn literal_type(lit: &Literal) -> Ty {
    match lit {
        Literal::Int(_) => Ty::star("Int"),
        Literal::Str(_) => Ty::star("String"),
    }
}

/// Like `infer_expr` but propagates a type hint into the lambda /
/// let special cases. The hint is just a suggestion — it is unified
/// with the inferred type at the call site, not used instead of it.
fn infer_expr_with_hint(
    ctx: &mut InferCtx,
    expr: &Expr,
    hint: Option<&Ty>,
) -> Result<(Ty, Subst), TypeError> {
    if let (Expr::Lambda { params, body, .. }, Some(hint_ty)) = (expr, hint) {
        // Split the hint into the expected parameter types.
        let (hint_args, _hint_res) = hint_ty.split_fun();
        if hint_args.len() >= params.len() {
            ctx.type_env.push_locals();
            let mut sub = Subst::new();
            let mut param_tys = Vec::new();
            for (i, p) in params.iter().enumerate() {
                let pty = hint_args[i].clone();
                let (s, ()) = bind_pattern(ctx, p, &pty)?;
                sub = s.compose(&sub);
                param_tys.push(pty);
            }
            let (bty, s2) = infer_expr(ctx, body)?;
            sub = s2.compose(&sub);
            ctx.type_env.pop_locals();
            let mut res = bty;
            for pt in param_tys.iter().rev() {
                res = Ty::fun(sub.apply(pt), res);
            }
            return Ok((res, sub));
        }
    }
    infer_expr(ctx, expr)
}

/// Main expression inference.
fn infer_expr(ctx: &mut InferCtx, expr: &Expr) -> Result<(Ty, Subst), TypeError> {
    match expr {
        Expr::Lit(lit, _) => Ok((literal_type(lit), Subst::new())),
        Expr::Var { module, name, span } => {
            let gid_name = module
                .as_ref()
                .map(|m| m.segments.join("."))
                .unwrap_or_default();
            let scheme = lookup_var(ctx, &gid_name, name, *span)?;
            let (ty, ctx_cstrs) = ctx.instantiate(&scheme);
            for c in ctx_cstrs {
                ctx.add_wanted(c, *span);
            }
            Ok((ty, Subst::new()))
        }
        Expr::OpRef { symbol, span } => {
            let scheme = lookup_var(ctx, "", symbol, *span)?;
            let (ty, ctx_cstrs) = ctx.instantiate(&scheme);
            for c in ctx_cstrs {
                ctx.add_wanted(c, *span);
            }
            Ok((ty, Subst::new()))
        }
        Expr::App { func, arg, span } => {
            let (fty, s1) = infer_expr(ctx, func)?;
            // If `func` after s1 has a known arrow shape, we can
            // propagate the expected argument type into the arg
            // inference. This buys us local type propagation that
            // helps, e.g., `filter (\s -> s.grade == g)` where the
            // lambda would otherwise infer a completely free record.
            let fty_a = s1.apply(&fty);
            let hint = match &fty_a {
                Ty::Fun(a, _) => Some((**a).clone()),
                _ => None,
            };
            let (aty, s2) = infer_expr_with_hint(ctx, arg, hint.as_ref())?;
            let s12 = s2.compose(&s1);
            let res = ctx.fresh_ty();
            let s3 = unify(
                &s12.apply(&fty),
                &Ty::fun(s12.apply(&aty), res.clone()),
                *span,
            )?;
            let s = s3.compose(&s12);
            Ok((s.apply(&res), s))
        }
        Expr::Lambda {
            params,
            body,
            span: _,
        } => {
            ctx.type_env.push_locals();
            let mut sub = Subst::new();
            let mut param_tys = Vec::new();
            for p in params {
                let pty = ctx.fresh_ty();
                let (s, ()) = bind_pattern(ctx, p, &pty)?;
                sub = s.compose(&sub);
                param_tys.push(pty);
            }
            let (bty, s2) = infer_expr(ctx, body)?;
            sub = s2.compose(&sub);
            ctx.type_env.pop_locals();
            let mut res = bty;
            for pt in param_tys.iter().rev() {
                res = Ty::fun(sub.apply(pt), res);
            }
            Ok((res, sub))
        }
        Expr::Let {
            name,
            params,
            value,
            body,
            span: _,
            ..
        } => {
            // `let f p1 ... = value in body`
            // Spec 03: let is implicitly recursive. Give the name a
            // fresh monotype, infer the value body, generalize, bind,
            // then infer the in-body.
            ctx.type_env.push_locals();
            let fresh_tv = ctx.fresh_ty();
            ctx.type_env
                .bind_local(name, Scheme::mono(fresh_tv.clone()));
            // Build the synthetic lambda-ish expression:
            let synthesised = if params.is_empty() {
                (**value).clone()
            } else {
                Expr::Lambda {
                    params: params.clone(),
                    body: value.clone(),
                    span: value.span(),
                }
            };
            let (vty, s1) = infer_expr(ctx, &synthesised)?;
            let s2 = unify(&s1.apply(&vty), &s1.apply(&fresh_tv), value.span())?;
            let sub = s2.compose(&s1);
            // Exclude the just-bound self-slot (Scheme::mono(fresh_tv))
            // from env_fvs, otherwise the let-binding can never be
            // generalised: its mono scheme body shares tvars with the
            // very type we are trying to quantify over.
            let scheme = ctx.generalize_excluding(&sub, &fresh_tv, Some(name));
            // Replace the local binding with the generalized scheme.
            ctx.type_env.bind_local(name, scheme);
            let (bty, s3) = infer_expr(ctx, body)?;
            let sub = s3.compose(&sub);
            ctx.type_env.pop_locals();
            Ok((sub.apply(&bty), sub))
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => {
            let (ct, s1) = infer_expr(ctx, cond)?;
            let s2 = unify(&s1.apply(&ct), &Ty::star("Bool"), cond.span())?;
            let s12 = s2.compose(&s1);
            let (tt, s3) = infer_expr(ctx, then_branch)?;
            let s123 = s3.compose(&s12);
            let (et, s4) = infer_expr(ctx, else_branch)?;
            let s1234 = s4.compose(&s123);
            let s5 = unify(&s1234.apply(&tt), &s1234.apply(&et), *span)?;
            let sub = s5.compose(&s1234);
            Ok((sub.apply(&tt), sub))
        }
        Expr::Case {
            scrutinee,
            arms,
            span,
        } => {
            let (sty, s1) = infer_expr(ctx, scrutinee)?;
            let mut sub = s1;
            let res = ctx.fresh_ty();
            for arm in arms {
                ctx.type_env.push_locals();
                let (sp, ()) = bind_pattern(ctx, &arm.pattern, &sub.apply(&sty))?;
                sub = sp.compose(&sub);
                let (bt, sb) = infer_expr(ctx, &arm.body)?;
                sub = sb.compose(&sub);
                let su = unify(&sub.apply(&bt), &sub.apply(&res), arm.span)?;
                sub = su.compose(&sub);
                ctx.type_env.pop_locals();
            }
            let _ = span;
            Ok((sub.apply(&res), sub))
        }
        Expr::BinOp {
            op,
            left,
            right,
            span,
        } => {
            let scheme = lookup_var(ctx, "", op, *span)?;
            let (op_ty, ctx_cstrs) = ctx.instantiate(&scheme);
            for c in ctx_cstrs {
                ctx.add_wanted(c, *span);
            }
            let (lt, s1) = infer_expr(ctx, left)?;
            let (rt, s2) = infer_expr(ctx, right)?;
            let s12 = s2.compose(&s1);
            let res = ctx.fresh_ty();
            let expected = Ty::fun(s12.apply(&lt), Ty::fun(s12.apply(&rt), res.clone()));
            let s3 = unify(&s12.apply(&op_ty), &expected, *span)?;
            let sub = s3.compose(&s12);
            Ok((sub.apply(&res), sub))
        }
        Expr::Neg { value, span } => {
            let (t, s1) = infer_expr(ctx, value)?;
            let s2 = unify(&s1.apply(&t), &Ty::star("Int"), *span)?;
            let sub = s2.compose(&s1);
            Ok((Ty::star("Int"), sub))
        }
        Expr::RecordLit { fields, span: _ } => {
            let mut sub = Subst::new();
            let mut fs = Vec::new();
            for (name, e) in fields {
                let (t, s) = infer_expr(ctx, e)?;
                sub = s.compose(&sub);
                fs.push((name.clone(), sub.apply(&t)));
            }
            Ok((Ty::record(fs), sub))
        }
        Expr::RecordUpdate {
            record,
            fields,
            span,
        } => {
            let (rty, s1) = infer_expr(ctx, record)?;
            let mut sub = s1;
            let after = sub.apply(&rty);
            let Ty::Record(exist_fs) = &after else {
                return Err(TypeError::new(
                    TypeErrorKind::Mismatch {
                        expected: Ty::Record(vec![]),
                        found: after.clone(),
                    },
                    *span,
                ));
            };
            let mut fs = exist_fs.clone();
            for (fname, fexpr) in fields {
                let (ft, s) = infer_expr(ctx, fexpr)?;
                sub = s.compose(&sub);
                let Some(pos) = fs.iter().position(|(n, _)| n == fname) else {
                    return Err(TypeError::new(
                        TypeErrorKind::MissingField {
                            field: fname.clone(),
                            ty: after.clone(),
                        },
                        *span,
                    ));
                };
                // Unify existing field type with new expr type.
                let (_, existing_ty) = fs[pos].clone();
                let s_u = unify(&sub.apply(&existing_ty), &sub.apply(&ft), *span)?;
                sub = s_u.compose(&sub);
                fs[pos] = (fname.clone(), sub.apply(&ft));
            }
            let result = Ty::record(fs);
            Ok((sub.apply(&result), sub))
        }
        Expr::FieldAccess {
            record,
            field,
            span,
        } => {
            let (rty, s1) = infer_expr(ctx, record)?;
            let rty_a = s1.apply(&rty);
            match &rty_a {
                Ty::Record(fs) => match fs.iter().find(|(n, _)| n == field) {
                    Some((_, t)) => Ok((t.clone(), s1)),
                    None => Err(TypeError::new(
                        TypeErrorKind::MissingField {
                            field: field.clone(),
                            ty: rty_a.clone(),
                        },
                        *span,
                    )),
                },
                Ty::Var(_) => {
                    // Defer: record the goal that `rty_a` must be a
                    // record type containing `field` of some type
                    // `fresh_ty`. We resolve these pending goals after
                    // all other unification has settled on the binding.
                    let fresh_ty = ctx.fresh_ty();
                    ctx.pending_fields.push((
                        rty_a.clone(),
                        field.clone(),
                        fresh_ty.clone(),
                        *span,
                    ));
                    Ok((fresh_ty, s1))
                }
                _ => Err(TypeError::new(
                    TypeErrorKind::MissingField {
                        field: field.clone(),
                        ty: rty_a.clone(),
                    },
                    *span,
                )),
            }
        }
        Expr::ListLit { items, span: _ } => {
            let elem = ctx.fresh_ty();
            let mut sub = Subst::new();
            for it in items {
                let (t, s) = infer_expr(ctx, it)?;
                sub = s.compose(&sub);
                let su = unify(&sub.apply(&t), &sub.apply(&elem), it.span())?;
                sub = su.compose(&sub);
            }
            Ok((
                Ty::app(
                    Ty::con("List", Kind::arr(Kind::Star, Kind::Star)),
                    sub.apply(&elem),
                ),
                sub,
            ))
        }
        Expr::Do { stmts, span } => {
            let desugared = desugar_do(stmts, *span)?;
            infer_expr(ctx, &desugared)
        }
    }
}

fn lookup_var(
    ctx: &mut InferCtx,
    module_prefix: &str,
    name: &str,
    span: Span,
) -> Result<Scheme, TypeError> {
    if module_prefix.is_empty() {
        if let Some(s) = ctx.type_env.lookup_local(name) {
            return Ok(s.clone());
        }
        // Try current module.
        let gid = GlobalId::new(&ctx.module, name);
        if let Some(s) = ctx.type_env.globals.get(&gid) {
            return Ok(s.clone());
        }
        // Try prelude.
        let gid = GlobalId::new("Prelude", name);
        if let Some(s) = ctx.type_env.globals.get(&gid) {
            return Ok(s.clone());
        }
        // Search all modules (typeck runs modules in dependency order,
        // so imports are available through globals added per-module).
        for (g, s) in &ctx.type_env.globals {
            if g.name == name {
                return Ok(s.clone());
            }
        }
        Err(TypeError::new(
            TypeErrorKind::Other {
                msg: format!("unbound value `{name}`"),
            },
            span,
        ))
    } else {
        let gid = GlobalId::new(module_prefix, name);
        if let Some(s) = ctx.type_env.globals.get(&gid) {
            return Ok(s.clone());
        }
        // Module alias not tracked; fall back to current-module lookup.
        for (g, s) in &ctx.type_env.globals {
            if g.name == name {
                return Ok(s.clone());
            }
        }
        Err(TypeError::new(
            TypeErrorKind::Other {
                msg: format!("unbound {module_prefix}.{name}"),
            },
            span,
        ))
    }
}

// =====================================================================
//  do-notation desugaring
// =====================================================================

fn desugar_do(stmts: &[DoStmt], span: Span) -> Result<Expr, TypeError> {
    if stmts.is_empty() {
        return Err(TypeError::new(TypeErrorKind::EmptyDo, span));
    }
    let last = stmts.last().unwrap();
    // Last stmt must be an expression.
    let final_expr = match last {
        DoStmt::Expr(e) => e.clone(),
        _ => return Err(TypeError::new(TypeErrorKind::InvalidDoFinalStmt, span)),
    };
    // Process in reverse.
    let mut acc = final_expr;
    for s in stmts[..stmts.len() - 1].iter().rev() {
        match s {
            DoStmt::Bind {
                pattern,
                expr,
                span,
            } => {
                // expr >>= \pattern -> acc
                let lam = Expr::Lambda {
                    params: vec![pattern.clone()],
                    body: Box::new(acc),
                    span: *span,
                };
                acc = Expr::BinOp {
                    op: ">>=".into(),
                    left: Box::new(expr.clone()),
                    right: Box::new(lam),
                    span: *span,
                };
            }
            DoStmt::Let {
                name,
                operator,
                params,
                value,
                span,
            } => {
                acc = Expr::Let {
                    name: name.clone(),
                    operator: *operator,
                    params: params.clone(),
                    value: Box::new(value.clone()),
                    body: Box::new(acc),
                    span: *span,
                };
            }
            DoStmt::Expr(e) => {
                // e >>= \_ -> acc
                let lam = Expr::Lambda {
                    params: vec![Pattern::Wildcard(e.span())],
                    body: Box::new(acc),
                    span: e.span(),
                };
                acc = Expr::BinOp {
                    op: ">>=".into(),
                    left: Box::new(e.clone()),
                    right: Box::new(lam),
                    span: e.span(),
                };
            }
        }
    }
    Ok(acc)
}
