//! Core type representations for I6.
//!
//! We keep types close to Hindley–Milner plus the small extensions spec
//! 01 / 03 / 04 / 07 require:
//!
//! - `Ty::Var` for unification variables and rigid quantified variables
//!   (distinguished by [`TyVar::rigid`] so that generalization and
//!   unification can treat them differently).
//! - `Ty::Con` for nullary type constructors (`Int`, `String`, `Bool`,
//!   user-defined `data T = ...`).
//! - `Ty::App` for saturated / partial type application, mirroring the
//!   surface AST shape without a separate "list / maybe / ..." case.
//! - `Ty::Fun` for arrow types; kept separate from `Ty::App` so the
//!   inferencer can walk arrows directly without hunting for the
//!   nominal `(->)` constructor.
//! - `Ty::Record` for spec-04 structural records. Records are
//!   compared *structurally* by field set (order-insensitive); same
//!   field set means same type.
//!
//! [`Scheme`] is a constrained polytype `∀ vs. ctx => body`; both
//! `vs` and `ctx` may be empty for a plain monotype. We keep the
//! surface `forall` variables as rigid `TyVar`s so that generalisation
//! and instantiation round-trip cleanly. `docs/impl/18-typecheck-hm.md`
//! records the rationale in more detail.

use std::collections::HashMap;
use std::fmt;

/// A unification / quantified type variable.
///
/// Every variable is globally unique (by `id`) so two variables with
/// the same user-written name are still distinct after renaming
/// passes. `name` is retained only for diagnostic printing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TyVar {
    pub id: u32,
    pub name: String,
}

impl fmt::Display for TyVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name.is_empty() {
            write!(f, "t{}", self.id)
        } else {
            write!(f, "{}#{}", self.name, self.id)
        }
    }
}

/// A kind (spec 07 §Kind system). Second-class: never appears at
/// value-type positions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    Star,
    Arrow(Box<Kind>, Box<Kind>),
    /// A kind variable used during inference. Replaced by `Star` at
    /// generalisation time when no usage forces a higher kind.
    Var(u32),
}

impl Kind {
    pub fn arr(a: Kind, b: Kind) -> Kind {
        Kind::Arrow(Box::new(a), Box::new(b))
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Star => f.write_str("*"),
            Kind::Arrow(a, b) => match a.as_ref() {
                Kind::Arrow(..) => write!(f, "({a}) -> {b}"),
                _ => write!(f, "{a} -> {b}"),
            },
            Kind::Var(i) => write!(f, "k{i}"),
        }
    }
}

/// A Sapphire type at the I6 layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Var(TyVar),
    /// A nominal type constructor. `kind` records the inferred kind
    /// for bookkeeping; unification treats two cons equal by name
    /// only (after module disambiguation at env construction time).
    Con {
        name: String,
        kind: Kind,
    },
    App(Box<Ty>, Box<Ty>),
    Fun(Box<Ty>, Box<Ty>),
    /// Structural record. Fields are stored sorted by name so that
    /// `==` on `Ty` respects structural equality.
    Record(Vec<(String, Ty)>),
}

impl Ty {
    pub fn con(name: impl Into<String>, kind: Kind) -> Ty {
        Ty::Con {
            name: name.into(),
            kind,
        }
    }

    pub fn star(name: impl Into<String>) -> Ty {
        Ty::con(name, Kind::Star)
    }

    pub fn app(f: Ty, x: Ty) -> Ty {
        Ty::App(Box::new(f), Box::new(x))
    }

    pub fn fun(a: Ty, b: Ty) -> Ty {
        Ty::Fun(Box::new(a), Box::new(b))
    }

    pub fn record(mut fields: Vec<(String, Ty)>) -> Ty {
        fields.sort_by(|a, b| a.0.cmp(&b.0));
        Ty::Record(fields)
    }

    /// Split a curried function `τ₁ -> τ₂ -> ... -> τₙ -> τ` into
    /// `(args, result)`.
    pub fn split_fun(&self) -> (Vec<&Ty>, &Ty) {
        let mut args = Vec::new();
        let mut t = self;
        while let Ty::Fun(a, b) = t {
            args.push(a.as_ref());
            t = b;
        }
        (args, t)
    }

    /// Collect free type variables (ids).
    pub fn free_vars(&self, out: &mut Vec<u32>) {
        match self {
            Ty::Var(v) => {
                if !out.contains(&v.id) {
                    out.push(v.id);
                }
            }
            Ty::Con { .. } => {}
            Ty::App(a, b) | Ty::Fun(a, b) => {
                a.free_vars(out);
                b.free_vars(out);
            }
            Ty::Record(fs) => {
                for (_, t) in fs {
                    t.free_vars(out);
                }
            }
        }
    }

    pub fn head_con(&self) -> Option<&str> {
        match self {
            Ty::Con { name, .. } => Some(name.as_str()),
            Ty::App(f, _) => f.head_con(),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_ty(f, self, 0)
    }
}

/// `ctx` is a parenthesisation level:
///   0 = top-level (no parens anywhere)
///   1 = left of arrow / argument of app (may still print App without parens)
///   2 = argument-of-app (always parenthesise App / Fun)
fn display_ty(f: &mut fmt::Formatter<'_>, t: &Ty, ctx: u8) -> fmt::Result {
    match t {
        Ty::Var(v) => write!(f, "{v}"),
        Ty::Con { name, .. } => f.write_str(name),
        Ty::App(g, x) => {
            if ctx >= 2 {
                f.write_str("(")?;
            }
            display_ty(f, g, 1)?;
            f.write_str(" ")?;
            display_ty(f, x, 2)?;
            if ctx >= 2 {
                f.write_str(")")?;
            }
            Ok(())
        }
        Ty::Fun(a, b) => {
            if ctx >= 1 {
                f.write_str("(")?;
            }
            display_ty(f, a, 1)?;
            f.write_str(" -> ")?;
            display_ty(f, b, 0)?;
            if ctx >= 1 {
                f.write_str(")")?;
            }
            Ok(())
        }
        Ty::Record(fs) => {
            f.write_str("{ ")?;
            for (i, (n, ty)) in fs.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{n} : ")?;
                display_ty(f, ty, 0)?;
            }
            f.write_str(" }")
        }
    }
}

/// A class constraint: `ClassName Ty`.
///
/// Single-parameter by spec 07 §Class declarations. Multi-parameter
/// support is 07-OQ1 and not in scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub class: String,
    pub arg: Ty,
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.class, self.arg)
    }
}

/// A constrained polytype. The `forall` variables are the rigids
/// bound by the scheme; the context is the class constraints that
/// accompany them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub vars: Vec<TyVar>,
    pub context: Vec<Constraint>,
    pub body: Ty,
}

impl Scheme {
    pub fn mono(body: Ty) -> Self {
        Self {
            vars: Vec::new(),
            context: Vec::new(),
            body,
        }
    }

    pub fn pretty(&self) -> String {
        let mut s = String::new();
        if !self.vars.is_empty() {
            s.push_str("forall");
            for v in &self.vars {
                s.push(' ');
                s.push_str(&v.name);
            }
            s.push('.');
            s.push(' ');
        }
        if !self.context.is_empty() {
            if self.context.len() == 1 {
                s.push_str(&format!("{}", self.context[0]));
            } else {
                s.push('(');
                for (i, c) in self.context.iter().enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(&format!("{c}"));
                }
                s.push(')');
            }
            s.push_str(" => ");
        }
        s.push_str(&format!("{}", self.body));
        s
    }
}

/// A substitution from type-variable ids to types.
#[derive(Debug, Clone, Default)]
pub struct Subst {
    map: HashMap<u32, Ty>,
}

impl Subst {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn single(v: u32, t: Ty) -> Self {
        let mut m = HashMap::new();
        m.insert(v, t);
        Self { map: m }
    }

    pub fn get(&self, id: u32) -> Option<&Ty> {
        self.map.get(&id)
    }

    pub fn insert(&mut self, id: u32, t: Ty) {
        self.map.insert(id, t);
    }

    /// Apply this substitution to a type, following chains to a fixed
    /// point. Guarded against cyclic substitutions (should not occur
    /// with a well-formed subst, but the occurs check in unify does
    /// not catch chains introduced by composition bugs).
    pub fn apply(&self, t: &Ty) -> Ty {
        self.apply_guarded(t, &mut Vec::new())
    }

    fn apply_guarded(&self, t: &Ty, seen: &mut Vec<u32>) -> Ty {
        match t {
            Ty::Var(v) => {
                if seen.contains(&v.id) {
                    // Cycle detected; return the last-known form to
                    // break the loop instead of stack-overflowing.
                    return Ty::Var(v.clone());
                }
                match self.map.get(&v.id) {
                    Some(t2) => {
                        seen.push(v.id);
                        let r = self.apply_guarded(t2, seen);
                        seen.pop();
                        r
                    }
                    None => Ty::Var(v.clone()),
                }
            }
            Ty::Con { name, kind } => Ty::Con {
                name: name.clone(),
                kind: kind.clone(),
            },
            Ty::App(a, b) => Ty::App(
                Box::new(self.apply_guarded(a, seen)),
                Box::new(self.apply_guarded(b, seen)),
            ),
            Ty::Fun(a, b) => Ty::Fun(
                Box::new(self.apply_guarded(a, seen)),
                Box::new(self.apply_guarded(b, seen)),
            ),
            Ty::Record(fs) => Ty::Record(
                fs.iter()
                    .map(|(n, t)| (n.clone(), self.apply_guarded(t, seen)))
                    .collect(),
            ),
        }
    }

    pub fn apply_constraint(&self, c: &Constraint) -> Constraint {
        Constraint {
            class: c.class.clone(),
            arg: self.apply(&c.arg),
        }
    }

    pub fn apply_scheme(&self, s: &Scheme) -> Scheme {
        // Do not substitute for bound vars; use a filtered subst.
        let bound: Vec<u32> = s.vars.iter().map(|v| v.id).collect();
        let filtered = Subst {
            map: self
                .map
                .iter()
                .filter(|(k, _)| !bound.contains(k))
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
        };
        Scheme {
            vars: s.vars.clone(),
            context: s
                .context
                .iter()
                .map(|c| filtered.apply_constraint(c))
                .collect(),
            body: filtered.apply(&s.body),
        }
    }

    /// `self ∘ other` — apply `other` first, then `self`.
    pub fn compose(&self, other: &Subst) -> Subst {
        let mut out = HashMap::new();
        for (k, v) in &other.map {
            out.insert(*k, self.apply(v));
        }
        for (k, v) in &self.map {
            out.entry(*k).or_insert_with(|| v.clone());
        }
        Subst { map: out }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
