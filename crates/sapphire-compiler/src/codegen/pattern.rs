//! Sapphire pattern → Ruby `case/in` pattern translation.
//!
//! Ruby 3.0+ `case/in` patterns are a close enough match for
//! Sapphire's patterns that we can emit them almost literally:
//!
//! - Wildcards → `_`
//! - Var bindings → the same name (Ruby binds the match)
//! - Literals → the literal
//! - Constructor patterns → a tagged-hash pattern (or bare Array /
//!   Bool for the special-cased forms)
//! - List patterns → `[...]`
//! - Cons patterns → `[head, *tail]`
//! - Record patterns → `{ f: pat, ... }`
//!
//! The `as` pattern and type-annotated pattern are handled by
//! stripping the annotation and (for `as`) emitting a trailing `=>
//! name` binding in parentheses per Ruby's pattern-match syntax.

use sapphire_core::ast::{Literal, Pattern};

use super::emit::escape_ruby_string;

/// Render a Sapphire pattern as a Ruby `case/in` pattern string.
pub fn render_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Wildcard(_) => "_".into(),
        Pattern::Var { name, .. } => name.clone(),
        Pattern::As { name, inner, .. } => {
            format!("{} => {}", render_pattern(inner), name)
        }
        Pattern::Lit(lit, _) => render_literal(lit),
        Pattern::Con {
            name, args, module, ..
        } => render_ctor_pattern(module.as_ref(), name, args),
        Pattern::Cons { head, tail, .. } => {
            format!("[{}, *{}]", render_pattern(head), render_pattern(tail))
        }
        Pattern::List { items, .. } => {
            let inner: Vec<String> = items.iter().map(render_pattern).collect();
            format!("[{}]", inner.join(", "))
        }
        Pattern::Record { fields, .. } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(f, p)| format!("{f}: {}", render_pattern(p)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Pattern::Annot { inner, .. } => render_pattern(inner),
    }
}

fn render_literal(lit: &Literal) -> String {
    match lit {
        Literal::Int(i) => format!("{i}"),
        Literal::Str(s) => format!("\"{}\"", escape_ruby_string(s)),
    }
}

fn render_ctor_pattern(
    _module: Option<&sapphire_core::ast::ModName>,
    name: &str,
    args: &[Pattern],
) -> String {
    // Special cases for the representations spec 10 pins down.
    match name {
        "True" => return "true".into(),
        "False" => return "false".into(),
        "LT" => return ":lt".into(),
        "EQ" => return ":eq".into(),
        "GT" => return ":gt".into(),
        "Nil" => return "[]".into(),
        "Cons" if args.len() == 2 => {
            return format!(
                "[{}, *{}]",
                render_pattern(&args[0]),
                render_pattern(&args[1])
            );
        }
        _ => {}
    }
    // Tagged-hash pattern for every other ADT constructor.
    if args.is_empty() {
        format!("{{ tag: :{name}, values: [] }}")
    } else {
        let inner: Vec<String> = args.iter().map(render_pattern).collect();
        format!("{{ tag: :{name}, values: [{}] }}", inner.join(", "))
    }
}
