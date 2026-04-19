//! Robinson unification with occurs check.
//!
//! The algorithm follows the textbook recipe: walk both types in
//! parallel, extend the substitution whenever one side is a variable,
//! and recurse into the structural cases. Records unify by exact
//! field-set match (spec 04's closed, structural-record rule).
//!
//! Kinds are not explicitly tracked during unification — the inferencer
//! is responsible for feeding unify with well-kinded inputs. Kind
//! mismatches show up as shape mismatches here (e.g. `Maybe` vs
//! `Int -> Int`) and surface as the same `Mismatch` diagnostic the
//! user would see for any other mismatch.

use sapphire_core::span::Span;

use super::error::{TypeError, TypeErrorKind};
use super::ty::{Subst, Ty, TyVar};

pub fn unify(a: &Ty, b: &Ty, span: Span) -> Result<Subst, TypeError> {
    match (a, b) {
        (Ty::Var(va), Ty::Var(vb)) if va.id == vb.id => Ok(Subst::new()),
        (Ty::Var(v), t) | (t, Ty::Var(v)) => bind(v, t, span),
        (
            Ty::Con {
                name: n1,
                kind: _k1,
            },
            Ty::Con {
                name: n2,
                kind: _k2,
            },
        ) if n1 == n2 => Ok(Subst::new()),
        (Ty::App(f1, x1), Ty::App(f2, x2)) => {
            let s1 = unify(f1, f2, span)?;
            let s2 = unify(&s1.apply(x1), &s1.apply(x2), span)?;
            Ok(s2.compose(&s1))
        }
        (Ty::Fun(a1, b1), Ty::Fun(a2, b2)) => {
            let s1 = unify(a1, a2, span)?;
            let s2 = unify(&s1.apply(b1), &s1.apply(b2), span)?;
            Ok(s2.compose(&s1))
        }
        (Ty::Record(fs1), Ty::Record(fs2)) => {
            if fs1.len() != fs2.len() {
                return Err(TypeError::new(
                    TypeErrorKind::Mismatch {
                        expected: a.clone(),
                        found: b.clone(),
                    },
                    span,
                ));
            }
            // Records are stored sorted; walk in parallel.
            let mut sub = Subst::new();
            for ((n1, t1), (n2, t2)) in fs1.iter().zip(fs2.iter()) {
                if n1 != n2 {
                    return Err(TypeError::new(
                        TypeErrorKind::Mismatch {
                            expected: a.clone(),
                            found: b.clone(),
                        },
                        span,
                    ));
                }
                let s = unify(&sub.apply(t1), &sub.apply(t2), span)?;
                sub = s.compose(&sub);
            }
            Ok(sub)
        }
        (x, y) => Err(TypeError::new(
            TypeErrorKind::Mismatch {
                expected: x.clone(),
                found: y.clone(),
            },
            span,
        )),
    }
}

fn bind(v: &TyVar, t: &Ty, span: Span) -> Result<Subst, TypeError> {
    if let Ty::Var(v2) = t {
        if v2.id == v.id {
            return Ok(Subst::new());
        }
    }
    if occurs_in(v.id, t) {
        return Err(TypeError::new(
            TypeErrorKind::OccursCheck {
                var: v.name.clone(),
                ty: t.clone(),
            },
            span,
        ));
    }
    Ok(Subst::single(v.id, t.clone()))
}

pub fn occurs_in(id: u32, t: &Ty) -> bool {
    match t {
        Ty::Var(v) => v.id == id,
        Ty::Con { .. } => false,
        Ty::App(a, b) | Ty::Fun(a, b) => occurs_in(id, a) || occurs_in(id, b),
        Ty::Record(fs) => fs.iter().any(|(_, t)| occurs_in(id, t)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::ty::Kind;
    use super::*;

    fn v(id: u32, name: &str) -> Ty {
        Ty::Var(TyVar {
            id,
            name: name.into(),
        })
    }

    #[test]
    fn unify_int_int() {
        let s = unify(&Ty::star("Int"), &Ty::star("Int"), Span::new(0, 0)).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn unify_var_int() {
        let a = v(1, "a");
        let s = unify(&a, &Ty::star("Int"), Span::new(0, 0)).unwrap();
        assert_eq!(s.apply(&a), Ty::star("Int"));
    }

    #[test]
    fn unify_fun() {
        let a = v(1, "a");
        let b = v(2, "b");
        let t1 = Ty::fun(a.clone(), b.clone());
        let t2 = Ty::fun(Ty::star("Int"), Ty::star("Bool"));
        let s = unify(&t1, &t2, Span::new(0, 0)).unwrap();
        assert_eq!(s.apply(&a), Ty::star("Int"));
        assert_eq!(s.apply(&b), Ty::star("Bool"));
    }

    #[test]
    fn unify_app() {
        let a = v(1, "a");
        let t1 = Ty::app(
            Ty::con("Maybe", Kind::arr(Kind::Star, Kind::Star)),
            a.clone(),
        );
        let t2 = Ty::app(
            Ty::con("Maybe", Kind::arr(Kind::Star, Kind::Star)),
            Ty::star("Int"),
        );
        let s = unify(&t1, &t2, Span::new(0, 0)).unwrap();
        assert_eq!(s.apply(&a), Ty::star("Int"));
    }

    #[test]
    fn occurs_check_fails() {
        let a = v(1, "a");
        let t = Ty::fun(a.clone(), Ty::star("Int"));
        let err = unify(&a, &t, Span::new(0, 0)).unwrap_err();
        assert!(matches!(err.kind, TypeErrorKind::OccursCheck { .. }));
    }

    #[test]
    fn mismatch_distinct_cons() {
        let err = unify(&Ty::star("Int"), &Ty::star("String"), Span::new(0, 0)).unwrap_err();
        assert!(matches!(err.kind, TypeErrorKind::Mismatch { .. }));
    }

    #[test]
    fn record_unify_same_fields() {
        let r1 = Ty::record(vec![("x".into(), v(1, "a")), ("y".into(), Ty::star("Int"))]);
        let r2 = Ty::record(vec![
            ("x".into(), Ty::star("String")),
            ("y".into(), Ty::star("Int")),
        ]);
        let s = unify(&r1, &r2, Span::new(0, 0)).unwrap();
        assert_eq!(s.apply(&v(1, "a")), Ty::star("String"));
    }

    #[test]
    fn record_mismatch_missing_field() {
        let r1 = Ty::record(vec![("x".into(), Ty::star("Int"))]);
        let r2 = Ty::record(vec![
            ("x".into(), Ty::star("Int")),
            ("y".into(), Ty::star("Int")),
        ]);
        let err = unify(&r1, &r2, Span::new(0, 0)).unwrap_err();
        assert!(matches!(err.kind, TypeErrorKind::Mismatch { .. }));
    }
}
