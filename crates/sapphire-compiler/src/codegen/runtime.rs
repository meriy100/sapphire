//! Mapping from Sapphire prelude / runtime names to the Ruby
//! expressions the generator should emit.
//!
//! Three kinds of names flow through here:
//!
//! 1. Prelude operators (`+`, `-`, `++`, `::`, …). BinOp sites inline
//!    them directly, but value positions (e.g. `foldr (+) 0 xs`)
//!    reference the curried lambda in `Sapphire::Prelude`.
//! 2. Prelude constructors (`Nothing`, `Just`, `Ok`, `Err`, `Nil`,
//!    `Cons`, `True`, `False`, `LT`, `EQ`, `GT`). Each maps to a
//!    Ruby-side constant or factory method that matches spec 10
//!    §Data model's representation decisions.
//! 3. Prelude value functions (`map`, `filter`, `foldr`, `show`,
//!    `readInt`, `print`, …). Each maps to a constant lambda defined
//!    in `prelude.rb.tpl`.
//!
//! None of this mapping is in the runtime gem; the prelude is
//! codegen-side surface. See `docs/impl/24-codegen-expr.md` §prelude
//! の取り扱い.

/// Map a prelude-level operator (spec 09 §Arithmetic et al.) to an
/// inline Ruby expression template. The returned `&str` is the
/// operator glyph the caller emits inside a Ruby `(left) OP (right)`
/// context. Returns `None` for operators that require more than a
/// simple infix substitution (`::`, `>>=`, `>>`).
pub fn binop_inline(op: &str) -> Option<&'static str> {
    match op {
        "+" => Some("+"),
        "-" => Some("-"),
        "*" => Some("*"),
        "/" => Some("/"),
        "%" => Some("%"),
        "==" => Some("=="),
        "/=" => Some("!="),
        "<" => Some("<"),
        "<=" => Some("<="),
        ">" => Some(">"),
        ">=" => Some(">="),
        "&&" => Some("&&"),
        "||" => Some("||"),
        "++" => Some("+"), // String concat; List concat is out of M9.
        _ => None,
    }
}

/// Map a prelude value name (spec 09) to the Ruby expression the
/// codegen should emit when the name is used in value position.
///
/// `None` means "not a prelude name at all"; the caller falls back to
/// ordinary module-qualified lookup.
pub fn prelude_value(name: &str) -> Option<String> {
    let lit = match name {
        // Bool / Ordering constants
        "True" => "Sapphire::Prelude::True",
        "False" => "Sapphire::Prelude::False",
        "LT" => "Sapphire::Prelude::LT",
        "EQ" => "Sapphire::Prelude::EQ",
        "GT" => "Sapphire::Prelude::GT",

        // Maybe / Result — expose the ADT.define-installed methods
        // wrapped as curried lambdas for value-position reference.
        "Nothing" => "Sapphire::Runtime::ADT.make(:Nothing, [])",
        "Just" => "->(v) { Sapphire::Runtime::ADT.make(:Just, [v]) }",
        "Ok" => "->(v) { Sapphire::Runtime::ADT.make(:Ok, [v]) }",
        "Err" => "->(v) { Sapphire::Runtime::ADT.make(:Err, [v]) }",

        // List
        "Nil" => "Sapphire::Prelude::SP_NIL",
        "Cons" => "Sapphire::Prelude::SP_CONS",

        // Arithmetic / logic operators as values (rare in M9 but
        // supported for completeness).
        "+" => "Sapphire::Prelude::OP_PLUS",
        "-" => "Sapphire::Prelude::OP_MINUS",
        "*" => "Sapphire::Prelude::OP_TIMES",
        "/" => "Sapphire::Prelude::OP_DIV",
        "%" => "Sapphire::Prelude::OP_MOD",
        "==" => "Sapphire::Prelude::OP_EQ",
        "/=" => "Sapphire::Prelude::OP_NEQ",
        "<" => "Sapphire::Prelude::OP_LT",
        "<=" => "Sapphire::Prelude::OP_LE",
        ">" => "Sapphire::Prelude::OP_GT",
        ">=" => "Sapphire::Prelude::OP_GE",
        "&&" => "Sapphire::Prelude::OP_AND",
        "||" => "Sapphire::Prelude::OP_OR",
        "++" => "Sapphire::Prelude::OP_CONCAT",
        "::" => "Sapphire::Prelude::SP_CONS",
        "negate" => "Sapphire::Prelude::NEGATE",
        "not" => "Sapphire::Prelude::NOT",

        // Utility functions
        "id" => "Sapphire::Prelude::ID",
        "const" => "Sapphire::Prelude::CONST",
        "compose" => "Sapphire::Prelude::COMPOSE",
        "flip" => "Sapphire::Prelude::FLIP",
        "map" => "Sapphire::Prelude::MAP",
        "filter" => "Sapphire::Prelude::FILTER",
        "foldr" => "Sapphire::Prelude::FOLDR",
        "foldl" => "Sapphire::Prelude::FOLDL",
        "concat" => "Sapphire::Prelude::CONCAT",
        "concatMap" => "Sapphire::Prelude::CONCAT_MAP",
        "length" => "Sapphire::Prelude::LENGTH",
        "head" => "Sapphire::Prelude::HEAD",
        "tail" => "Sapphire::Prelude::TAIL",
        "null" => "Sapphire::Prelude::NULL",
        "fst" => "Sapphire::Prelude::FST",
        "snd" => "Sapphire::Prelude::SND",
        "maybe" => "Sapphire::Prelude::MAYBE",
        "fromMaybe" => "Sapphire::Prelude::FROM_MAYBE",
        "result" => "Sapphire::Prelude::RESULT",
        "mapErr" => "Sapphire::Prelude::MAP_ERR",
        "readInt" => "Sapphire::Prelude::READ_INT",
        "compare" => "Sapphire::Prelude::COMPARE",
        "join" => "Sapphire::Prelude::JOIN",
        "when" => "Sapphire::Prelude::WHEN",
        "unless" => "Sapphire::Prelude::UNLESS",
        "show" => "Sapphire::Prelude::SHOW",
        "print" => "Sapphire::Prelude::PRINT",

        // Monad class methods that have a specialised dispatch
        // helper in the prelude file.
        //
        // The value-position `>>` wraps `n` in a zero-arg thunk so
        // that `monad_then` can defer its evaluation when `m` would
        // short-circuit. This matches the BinOp emission above (see
        // `render_binop`); keep them in sync.
        ">>=" => "->(m) { ->(k) { Sapphire::Prelude.monad_bind(m, k) } }",
        ">>" => "->(m) { ->(n) { Sapphire::Prelude.monad_then(m, -> { n }) } }",

        // `pure` / `return` in value position fall back to the
        // polymorphic stub; the generator specialises direct calls
        // via `specialised_pure_call` below when the enclosing
        // binding's type is known.
        "pure" => "->(x) { Sapphire::Prelude.pure_polymorphic(x) }",
        "return" => "->(x) { Sapphire::Prelude.pure_polymorphic(x) }",

        _ => return None,
    };
    Some(lit.to_string())
}

/// Decide which monad to specialise a `pure` / `return` call to, given
/// the head constructor of the enclosing binding's return type (as
/// reported by I6). The return value is a Ruby expression template:
/// substitute `{}` for the argument expression.
pub fn specialised_pure(head: Option<&str>) -> &'static str {
    match head {
        Some("Ruby") => "Sapphire::Runtime::Ruby.prim_return({})",
        Some("Result") => "Sapphire::Runtime::ADT.make(:Ok, [{}])",
        Some("Maybe") => "Sapphire::Runtime::ADT.make(:Just, [{}])",
        Some("List") => "[{}]",
        _ => "Sapphire::Prelude.pure_polymorphic({})",
    }
}

/// Classify a prelude constructor reference as always non-arity
/// (nullary). Used by codegen to decide whether a `Var` of an
/// upper-case name should emit directly or return a curried factory.
#[allow(dead_code)]
pub fn is_nullary_prelude_ctor(name: &str) -> bool {
    matches!(
        name,
        "Nothing" | "Nil" | "True" | "False" | "LT" | "EQ" | "GT"
    )
}
