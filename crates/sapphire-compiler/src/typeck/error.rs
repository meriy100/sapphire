//! Type-checker error ADT.

use std::fmt;

use sapphire_core::span::Span;

use super::ty::{Constraint, Ty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

impl TypeError {
    pub fn new(kind: TypeErrorKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeErrorKind {
    /// Types failed to unify.
    Mismatch { expected: Ty, found: Ty },
    /// Occurs check failed (infinite type).
    OccursCheck { var: String, ty: Ty },
    /// Unknown type constructor at a reference site.
    UnknownType { name: String },
    /// Unknown class name.
    UnknownClass { name: String },
    /// Constraint could not be solved (no matching instance).
    UnresolvedConstraint { constraint: Constraint },
    /// Ambiguous constraint at generalization (the constrained
    /// variable does not appear in the body).
    AmbiguousConstraint { constraint: Constraint },
    /// Kind mismatch while checking a type expression.
    KindMismatch { expected: String, found: String },
    /// Record does not have the requested field.
    MissingField { field: String, ty: Ty },
    /// `data T = C ...` expected n args but got m at a use site.
    CtorArity {
        ctor: String,
        expected: usize,
        found: usize,
    },
    /// Record-pattern field not present.
    RecordPatternField { field: String, ty: Ty },
    /// An orphan instance: neither the class nor the outer ctor of
    /// the head lives in this module.
    OrphanInstance { class: String },
    /// Two instances with overlapping heads.
    OverlappingInstance { class: String, head: Ty },
    /// An instance head outside the Haskell-98 admitted shape.
    InvalidInstanceHead { class: String, head: Ty },
    /// A `class Ctx => C a` superclass context that mentions a
    /// variable other than the class's own `a`. Spec 07
    /// §Class declarations requires the superclass to constrain the
    /// class's own type variable.
    InvalidSuperclassContext {
        class: String,
        expected: String,
        got: String,
    },
    /// A `do` block's final stmt is a bind or let (invalid per 07).
    InvalidDoFinalStmt,
    /// A `do` block is empty.
    EmptyDo,
    /// Catch-all for specific internal errors we surface to users.
    Other { msg: String },
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "type error at bytes {}..{}: {}",
            self.span.start, self.span.end, self.kind
        )
    }
}

impl fmt::Display for TypeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeErrorKind::Mismatch { expected, found } => {
                write!(f, "expected `{expected}`, found `{found}`")
            }
            TypeErrorKind::OccursCheck { var, ty } => {
                write!(f, "occurs check: `{var}` occurs in `{ty}`")
            }
            TypeErrorKind::UnknownType { name } => {
                write!(f, "unknown type `{name}`")
            }
            TypeErrorKind::UnknownClass { name } => {
                write!(f, "unknown class `{name}`")
            }
            TypeErrorKind::UnresolvedConstraint { constraint } => {
                write!(f, "no instance for `{constraint}`")
            }
            TypeErrorKind::AmbiguousConstraint { constraint } => {
                write!(f, "ambiguous constraint `{constraint}`")
            }
            TypeErrorKind::KindMismatch { expected, found } => {
                write!(f, "kind mismatch: expected `{expected}`, found `{found}`")
            }
            TypeErrorKind::MissingField { field, ty } => {
                write!(f, "record `{ty}` has no field `{field}`")
            }
            TypeErrorKind::CtorArity {
                ctor,
                expected,
                found,
            } => {
                write!(
                    f,
                    "constructor `{ctor}` expects {expected} arg(s) but was given {found}"
                )
            }
            TypeErrorKind::RecordPatternField { field, ty } => {
                write!(f, "record pattern field `{field}` not in `{ty}`")
            }
            TypeErrorKind::OrphanInstance { class } => {
                write!(f, "orphan instance `{class}` not admitted")
            }
            TypeErrorKind::OverlappingInstance { class, head } => {
                write!(f, "overlapping instance `{class} {head}`")
            }
            TypeErrorKind::InvalidInstanceHead { class, head } => {
                write!(f, "instance head `{class} {head}` outside Haskell-98 shape")
            }
            TypeErrorKind::InvalidSuperclassContext {
                class,
                expected,
                got,
            } => {
                write!(
                    f,
                    "superclass context of `{class}` must constrain its own type variable `{expected}`, got {got}"
                )
            }
            TypeErrorKind::InvalidDoFinalStmt => {
                f.write_str("final `do` statement must be an expression")
            }
            TypeErrorKind::EmptyDo => f.write_str("empty `do` block"),
            TypeErrorKind::Other { msg } => f.write_str(msg),
        }
    }
}

impl std::error::Error for TypeError {}
