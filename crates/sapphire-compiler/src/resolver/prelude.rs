//! The `Prelude` module's content, as seen by the name resolver.
//!
//! Spec 09 §Module export list fixes the names that every Sapphire
//! module can assume are in unqualified scope. Until the prelude is
//! implemented as an actual `.sp` file compiled alongside user code,
//! I5 bakes the names in as a static table. Whenever spec 09 grows
//! (new prelude utilities), this table grows in lock-step.
//!
//! The table is intentionally split into the two spec-08 namespaces
//! so that lookup at a reference site does not have to re-classify
//! each name. See `Namespace` in [`super::error`].

use super::error::Namespace;

/// The prelude module's fully-qualified name.
pub const PRELUDE_MODULE: &str = "Prelude";

/// A value or constructor exported by `Prelude`.
///
/// The second tuple element is `true` when the name is a value
/// constructor (e.g. `True`, `Nothing`, `Ok`). The distinction is
/// not used by I5 itself — both forms share the `Value` namespace
/// per spec 06 §Design notes — but is preserved so that later passes
/// (I6 `deriving`, I7 Ruby codegen) can consult the table.
pub const PRELUDE_VALUES: &[(&str, bool)] = &[
    // Bool constructors
    ("True", true),
    ("False", true),
    // Ordering constructors
    ("LT", true),
    ("EQ", true),
    ("GT", true),
    // Maybe constructors
    ("Nothing", true),
    ("Just", true),
    // Result constructors
    ("Err", true),
    ("Ok", true),
    // List constructors
    ("Nil", true),
    ("Cons", true),
    // Arithmetic & comparison operators (spec 09 §Arithmetic)
    ("+", false),
    ("-", false),
    ("*", false),
    ("/", false),
    ("%", false),
    ("negate", false),
    ("<", false),
    (">", false),
    ("<=", false),
    (">=", false),
    ("==", false),
    ("/=", false),
    ("compare", false),
    ("&&", false),
    ("||", false),
    ("not", false),
    ("++", false),
    // Cons operator — parser desugars `::` too, but the operator is
    // a prelude binding in spec 09.
    ("::", false),
    // Monad operators
    (">>=", false),
    (">>", false),
    ("pure", false),
    ("return", false),
    // Utility functions
    ("id", false),
    ("const", false),
    ("compose", false),
    ("flip", false),
    ("map", false),
    ("filter", false),
    ("foldr", false),
    ("foldl", false),
    ("concat", false),
    ("concatMap", false),
    ("length", false),
    ("head", false),
    ("tail", false),
    ("null", false),
    ("fst", false),
    ("snd", false),
    ("maybe", false),
    ("fromMaybe", false),
    ("result", false),
    ("mapErr", false),
    ("readInt", false),
    ("join", false),
    ("when", false),
    ("unless", false),
    ("show", false),
    ("print", false),
];

/// Types / type constructors / classes exported by `Prelude`.
///
/// The second element is `true` for class names, `false` for type
/// constructors / aliases. Same rationale as [`PRELUDE_VALUES`].
pub const PRELUDE_TYPES: &[(&str, bool)] = &[
    ("Bool", false),
    ("Ordering", false),
    ("Maybe", false),
    ("Result", false),
    ("List", false),
    // Primitive / built-in type names that I5 needs to know about so
    // that `Int` / `String` at type position resolve without
    // reporting "undefined". These are not declared in spec 09 as
    // `data` decls, but every Sapphire program can assume them
    // (spec 01 §Core types).
    ("Int", false),
    ("String", false),
    // The `Ruby` type constructor (spec 10 / 11). Not strictly part
    // of spec 09 prelude but is implicitly imported alongside
    // `Prelude` per spec 09 §The prelude as a module.
    ("Ruby", false),
    // Standard classes (spec 09 §Class instances).
    ("Eq", true),
    ("Ord", true),
    ("Show", true),
    ("Functor", true),
    ("Applicative", true),
    ("Monad", true),
];

/// Returns `true` if the given name belongs to the prelude in the
/// given namespace. Primarily used by tests that assert the static
/// table is up to date with spec 09.
#[allow(dead_code)]
pub fn is_prelude(name: &str, ns: Namespace) -> bool {
    match ns {
        Namespace::Value => PRELUDE_VALUES.iter().any(|(n, _)| *n == name),
        Namespace::Type => PRELUDE_TYPES.iter().any(|(n, _)| *n == name),
    }
}
