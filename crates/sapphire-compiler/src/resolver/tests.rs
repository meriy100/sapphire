//! Unit tests for the I5 name resolver.
//!
//! Each test parses one or more Sapphire source strings end-to-end
//! (lexer → layout → parser) and asserts either a successful
//! resolution or a specific `ResolveErrorKind`. Tests cluster by
//! spec-08 area: local scoping, top-level registration, imports,
//! exports, the implicit prelude, multi-module programs, and the
//! assorted error paths (duplicates, undefined, ambiguous, private
//! leaks, Main-sugar imports).

use super::*;
use crate::parser;
use sapphire_core::ast::Module as AstModule;

fn parse(src: &str) -> AstModule {
    parser::parse(src).unwrap_or_else(|e| panic!("parse failed for source:\n{src}\n\n{e}"))
}

fn resolve_ok(src: &str) -> ResolvedModule {
    let m = parse(src);
    resolve(m).unwrap_or_else(|errs| {
        let joined: Vec<_> = errs.iter().map(|e| e.to_string()).collect();
        panic!("expected ok but got errors:\n{}", joined.join("\n"))
    })
}

fn resolve_err(src: &str) -> Vec<ResolveError> {
    let m = parse(src);
    match resolve(m) {
        Ok(_) => panic!("expected resolve errors but got ok"),
        Err(errs) => errs,
    }
}

fn has_kind<P: Fn(&ResolveErrorKind) -> bool>(errs: &[ResolveError], p: P) -> bool {
    errs.iter().any(|e| p(&e.kind))
}

// -----------------------------------------------------------------
//  Local scoping
// -----------------------------------------------------------------

#[test]
fn lambda_parameter_resolves_locally() {
    let r = resolve_ok(
        r#"module M (f) where
f : Int -> Int
f = \x -> x
"#,
    );
    // Every reference site for `x` should be Local.
    let local_count = r
        .references
        .values()
        .filter(|r| matches!(r, Resolution::Local { name } if name == "x"))
        .count();
    assert!(local_count >= 1, "expected a local reference for x");
}

#[test]
fn let_binding_is_recursive_and_local() {
    let r = resolve_ok(
        r#"module M (f) where
f : Int -> Int
f n = let y = n in y
"#,
    );
    // `y` should resolve as local in its body.
    assert!(
        r.references
            .values()
            .any(|res| matches!(res, Resolution::Local { name } if name == "y"))
    );
}

#[test]
fn case_arm_binds_pattern_variables() {
    let r = resolve_ok(
        r#"module M (f) where
f : Maybe Int -> Int
f m = case m of
  Just x -> x
  Nothing -> 0
"#,
    );
    assert!(
        r.references
            .values()
            .any(|res| matches!(res, Resolution::Local { name } if name == "x"))
    );
}

#[test]
fn do_bind_introduces_local() {
    let r = resolve_ok(
        r#"module M (f) where
f : Ruby Int
f = do
  x <- pure 1
  pure x
"#,
    );
    assert!(
        r.references
            .values()
            .any(|res| matches!(res, Resolution::Local { name } if name == "x"))
    );
}

#[test]
fn constructor_pattern_binds_fields() {
    let r = resolve_ok(
        r#"module M (f) where
f : List Int -> Int
f xs = case xs of
  Cons h t -> h
  Nil -> 0
"#,
    );
    let locals: Vec<&str> = r
        .references
        .values()
        .filter_map(|res| match res {
            Resolution::Local { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(locals.contains(&"h"));
}

// -----------------------------------------------------------------
//  Top-level registration
// -----------------------------------------------------------------

#[test]
fn top_level_value_registered() {
    let r = resolve_ok(
        r#"module M (f) where
f : Int
f = 1
"#,
    );
    assert!(r.env.top_lookup("f", Namespace::Value).is_some());
}

#[test]
fn data_declaration_registers_type_and_ctors() {
    let r = resolve_ok(
        r#"module M (Color(..)) where
data Color = Red | Green | Blue
"#,
    );
    assert!(r.env.top_lookup("Color", Namespace::Type).is_some());
    assert!(r.env.top_lookup("Red", Namespace::Value).is_some());
    assert!(r.env.top_lookup("Green", Namespace::Value).is_some());
    assert!(r.env.top_lookup("Blue", Namespace::Value).is_some());
}

#[test]
fn class_declaration_registers_methods() {
    let r = resolve_ok(
        r#"module M (class Foo(..)) where
class Foo a where
  foo : a -> a
"#,
    );
    assert!(r.env.top_lookup("Foo", Namespace::Type).is_some());
    assert!(r.env.top_lookup("foo", Namespace::Value).is_some());
}

#[test]
fn type_alias_registers_as_type() {
    let r = resolve_ok(
        r#"module M (Age) where
type Age = Int
"#,
    );
    assert!(r.env.top_lookup("Age", Namespace::Type).is_some());
}

#[test]
fn ruby_embed_registers_as_value() {
    let r = resolve_ok(
        r#"module M (puts) where
puts : String -> Ruby {}
puts s := """
  puts s
"""
"#,
    );
    assert!(r.env.top_lookup("puts", Namespace::Value).is_some());
}

// -----------------------------------------------------------------
//  Duplicates
// -----------------------------------------------------------------

#[test]
fn duplicate_top_level_value_errors() {
    // Two Ruby-embed bindings with the same name — also covered by
    // the ctor-vs-value case below, but this exercises the value/
    // value collision path through `ensure_value_binding`.
    let errs = resolve_err(
        r#"module M () where
data Color = Red
data Shape = Red
"#,
    );
    assert!(
        has_kind(&errs, |k| matches!(
            k,
            ResolveErrorKind::DuplicateTopLevel { namespace: Namespace::Value, name, .. }
                if name == "Red"
        )),
        "got {errs:?}"
    );
}

#[test]
fn duplicate_type_errors() {
    let errs = resolve_err(
        r#"module M () where
data T = A
data T = B
"#,
    );
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::DuplicateTopLevel {
            namespace: Namespace::Type,
            ..
        }
    )));
}

#[test]
fn duplicate_constructor_errors() {
    let errs = resolve_err(
        r#"module M () where
data A = X
data B = X
"#,
    );
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::DuplicateTopLevel {
            namespace: Namespace::Value,
            ..
        }
    )));
}

#[test]
fn multiple_value_clauses_not_duplicates() {
    // Matching on list constructors: the two clauses share a name
    // and are not duplicates (spec 07 §Abstract syntax).
    resolve_ok(
        r#"module M (f) where
f : List Int -> Int
f [] = 0
f (Cons x xs) = x
"#,
    );
}

#[test]
fn signature_plus_clause_not_duplicate() {
    resolve_ok(
        r#"module M (f) where
f : Int -> Int
f x = x
"#,
    );
}

// -----------------------------------------------------------------
//  Prelude
// -----------------------------------------------------------------

#[test]
fn prelude_values_visible_unqualified() {
    let r = resolve_ok(
        r#"module M (f) where
f : Int
f = length [1, 2, 3]
"#,
    );
    // `length` resolves to Prelude.
    let r_length = r
        .references
        .values()
        .find(|res| matches!(res, Resolution::Global(rr) if rr.name == "length"))
        .unwrap_or_else(|| panic!("no global reference to length"));
    match r_length {
        Resolution::Global(rr) => assert_eq!(rr.module.display(), "Prelude"),
        _ => unreachable!(),
    }
}

#[test]
fn prelude_types_visible_unqualified() {
    let r = resolve_ok(
        r#"module M (f) where
f : Maybe Int -> Int
f m = case m of
  Just x -> x
  Nothing -> 0
"#,
    );
    let found = r
        .references
        .values()
        .any(|res| matches!(res, Resolution::Global(rr) if rr.name == "Maybe" && rr.module.display() == "Prelude"));
    assert!(found);
}

#[test]
fn prelude_constructors_visible() {
    resolve_ok(
        r#"module M (xs) where
xs : List Int
xs = Cons 1 (Cons 2 Nil)
"#,
    );
}

// -----------------------------------------------------------------
//  Imports (single-module: prelude only, no user imports)
// -----------------------------------------------------------------

#[test]
fn undefined_top_level_value_errors() {
    let errs = resolve_err(
        r#"module M (f) where
f : Int
f = nonexistent
"#,
    );
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::UndefinedName { name, .. } if name == "nonexistent"
    )));
}

#[test]
fn unknown_qualifier_errors() {
    let errs = resolve_err(
        r#"module M (f) where
f : Int
f = Bogus.x
"#,
    );
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::QualifierNotInScope { qualifier } if qualifier == "Bogus"
    )));
}

// -----------------------------------------------------------------
//  Exports
// -----------------------------------------------------------------

#[test]
fn export_list_restricts_visibility() {
    let r = resolve_ok(
        r#"module M (f) where
f : Int
f = g
g : Int
g = 1
"#,
    );
    let f = r.env.top_lookup("f", Namespace::Value).unwrap();
    let g = r.env.top_lookup("g", Namespace::Value).unwrap();
    assert!(matches!(f.visibility, Visibility::Exported));
    assert!(matches!(g.visibility, Visibility::Private));
}

#[test]
fn omitted_export_list_exports_all() {
    let r = resolve_ok(
        r#"module M where
f : Int
f = 1
g : Int
g = 2
"#,
    );
    let f = r.env.top_lookup("f", Namespace::Value).unwrap();
    let g = r.env.top_lookup("g", Namespace::Value).unwrap();
    assert!(matches!(f.visibility, Visibility::Exported));
    assert!(matches!(g.visibility, Visibility::Exported));
}

#[test]
fn empty_export_list_exports_nothing() {
    let r = resolve_ok(
        r#"module M () where
f : Int
f = 1
"#,
    );
    let f = r.env.top_lookup("f", Namespace::Value).unwrap();
    assert!(matches!(f.visibility, Visibility::Private));
}

#[test]
fn export_of_unknown_errors() {
    let errs = resolve_err(
        r#"module M (bogus) where
f : Int
f = 1
"#,
    );
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::ExportOfUnknown { name, .. } if name == "bogus"
    )));
}

#[test]
fn type_all_export_exposes_constructors() {
    let r = resolve_ok(
        r#"module M (Color(..)) where
data Color = Red | Green
"#,
    );
    assert!(r.env.exports.lookup("Red", Namespace::Value).is_some());
    assert!(r.env.exports.lookup("Green", Namespace::Value).is_some());
}

#[test]
fn bare_type_export_hides_constructors() {
    let r = resolve_ok(
        r#"module M (Color) where
data Color = Red | Green
"#,
    );
    assert!(r.env.exports.lookup("Color", Namespace::Type).is_some());
    assert!(r.env.exports.lookup("Red", Namespace::Value).is_none());
}

// -----------------------------------------------------------------
//  Main-sugar
// -----------------------------------------------------------------

#[test]
fn headerless_script_becomes_main() {
    let r = resolve_ok(
        r#"f : Int
f = 1
"#,
    );
    assert_eq!(r.id.display(), "Main");
}

#[test]
fn importing_headerless_main_rejected() {
    // Two modules: one header-less (sugar to Main), one that imports
    // Main. Spec 08 §One module per file forbids this.
    let a = parse("f : Int\nf = 1\n");
    let b = parse(
        r#"module B (g) where
import Main (f)
g : Int
g = f
"#,
    );
    match resolve_program(vec![a, b]) {
        Ok(_) => panic!("expected error"),
        Err(errs) => {
            assert!(has_kind(&errs, |k| matches!(
                k,
                ResolveErrorKind::MainSugarNotImportable { module } if module == "Main"
            )));
        }
    }
}

// -----------------------------------------------------------------
//  Multi-module programs
// -----------------------------------------------------------------

#[test]
fn simple_multi_module_import() {
    let http = parse(
        r#"module Http (get) where
get : String -> Int
get u = 0
"#,
    );
    let fetch = parse(
        r#"module Fetch (main) where
import Http (get)
main : Int
main = get "hello"
"#,
    );
    let program = resolve_program(vec![http, fetch]).expect("multi-module should resolve");
    assert_eq!(program.modules.len(), 2);
}

#[test]
fn import_of_unknown_module_errors() {
    let m = parse(
        r#"module M (f) where
import DoesNotExist (y)
f : Int
f = y
"#,
    );
    let errs = resolve_program(vec![m]).unwrap_err();
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::ImportOfUnknownModule { module } if module == "DoesNotExist"
    )));
}

#[test]
fn import_of_unknown_name_errors() {
    let http = parse(
        r#"module Http (get) where
get : Int
get = 1
"#,
    );
    let fetch = parse(
        r#"module Fetch (f) where
import Http (post)
f : Int
f = post
"#,
    );
    let errs = resolve_program(vec![http, fetch]).unwrap_err();
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::ImportOfUnknown { module, name, .. }
            if module == "Http" && name == "post"
    )));
}

#[test]
fn qualified_import_hides_unqualified() {
    let http = parse(
        r#"module Http (get) where
get : Int
get = 1
"#,
    );
    let fetch = parse(
        r#"module Fetch (f) where
import qualified Http
f : Int
f = get
"#,
    );
    let errs = resolve_program(vec![http, fetch]).unwrap_err();
    // Bare `get` should fail because only qualified access is
    // available.
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::UndefinedName { name, .. } if name == "get"
    )));
}

#[test]
fn import_as_introduces_alias() {
    let http = parse(
        r#"module Http (get) where
get : Int
get = 1
"#,
    );
    let fetch = parse(
        r#"module Fetch (f) where
import Http as H
f : Int
f = H.get
"#,
    );
    resolve_program(vec![http, fetch]).expect("alias should resolve");
}

#[test]
fn import_hiding_removes_names() {
    let http = parse(
        r#"module Http (get, post) where
get : Int
get = 1
post : Int
post = 2
"#,
    );
    let fetch = parse(
        r#"module Fetch (f) where
import Http hiding (get)
f : Int
f = post
"#,
    );
    resolve_program(vec![http, fetch]).expect("hiding should still expose post");
}

#[test]
fn import_hiding_leaves_hidden_undefined() {
    let http = parse(
        r#"module Http (get, post) where
get : Int
get = 1
post : Int
post = 2
"#,
    );
    let fetch = parse(
        r#"module Fetch (f) where
import Http hiding (get)
f : Int
f = get
"#,
    );
    let errs = resolve_program(vec![http, fetch]).unwrap_err();
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::UndefinedName { name, .. } if name == "get"
    )));
}

#[test]
fn two_imports_with_same_name_are_ambiguous() {
    let a = parse(
        r#"module A (foo) where
foo : Int
foo = 1
"#,
    );
    let b = parse(
        r#"module B (foo) where
foo : Int
foo = 2
"#,
    );
    let c = parse(
        r#"module C (g) where
import A
import B
g : Int
g = foo
"#,
    );
    let errs = resolve_program(vec![a, b, c]).unwrap_err();
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::Ambiguous { name, .. } if name == "foo"
    )));
}

#[test]
fn qualified_access_disambiguates() {
    let a = parse(
        r#"module A (foo) where
foo : Int
foo = 1
"#,
    );
    let b = parse(
        r#"module B (foo) where
foo : Int
foo = 2
"#,
    );
    let c = parse(
        r#"module C (g) where
import qualified A
import qualified B
g : Int
g = A.foo
"#,
    );
    resolve_program(vec![a, b, c]).expect("qualified access should resolve");
}

// -----------------------------------------------------------------
//  Private-type leak (spec 08 §Visibility)
// -----------------------------------------------------------------

#[test]
fn private_type_leak_in_signature_rejected() {
    let errs = resolve_err(
        r#"module M (f) where
data Secret = S
f : Secret -> Int
f _ = 0
"#,
    );
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::PrivateLeak { leak, .. } if leak == "Secret"
    )));
}

#[test]
fn private_type_in_private_signature_ok() {
    resolve_ok(
        r#"module M (main) where
data Secret = S
helper : Secret -> Int
helper _ = 0
main : Int
main = 0
"#,
    );
}

// -----------------------------------------------------------------
//  Cyclic imports
// -----------------------------------------------------------------

#[test]
fn cyclic_imports_rejected() {
    let a = parse(
        r#"module A (f) where
import B (g)
f : Int
f = g
"#,
    );
    let b = parse(
        r#"module B (g) where
import A (f)
g : Int
g = f
"#,
    );
    let errs = resolve_program(vec![a, b]).unwrap_err();
    assert!(has_kind(&errs, |k| matches!(
        k,
        ResolveErrorKind::CyclicImports { .. }
    )));
}

// -----------------------------------------------------------------
//  Type-variable scoping
// -----------------------------------------------------------------

#[test]
fn type_variables_scope_locally_to_scheme() {
    resolve_ok(
        r#"module M (f, g) where
f : a -> a
f x = x
g : b -> b
g y = y
"#,
    );
}

// -----------------------------------------------------------------
//  Integration: the M9 example files (smoke tests)
// -----------------------------------------------------------------

#[test]
fn m9_hello_ruby_resolves() {
    let src = include_str!("../../../../examples/sources/01-hello-ruby/Main.sp");
    resolve_ok(src);
}

#[test]
fn m9_parse_numbers_resolves() {
    let src = include_str!("../../../../examples/sources/02-parse-numbers/NumberSum.sp");
    resolve_ok(src);
}

#[test]
fn m9_students_resolves() {
    let src = include_str!("../../../../examples/sources/03-students-records/Students.sp");
    resolve_ok(src);
}

#[test]
fn m9_fetch_summarise_resolves() {
    let http = parse(include_str!(
        "../../../../examples/sources/04-fetch-summarise/Http.sp"
    ));
    let fetch = parse(include_str!(
        "../../../../examples/sources/04-fetch-summarise/Fetch.sp"
    ));
    resolve_program(vec![http, fetch]).expect("M9 example 4 should resolve");
}
