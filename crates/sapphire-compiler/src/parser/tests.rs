//! Unit tests for the parser.
//!
//! These tests exercise the parser directly via [`super::parse`],
//! which runs the full lex → layout → parse pipeline. The goal is
//! to cover every grammatical form described in spec 01, 03, 04, 05,
//! 06, 08, 09, and the parts of 07 / 10 that the first
//! implementation parses (type classes as declarations; Ruby
//! embedding via `:=`).

use sapphire_core::ast::*;

use super::parse;

fn parse_expect(src: &str) -> Module {
    parse(src).unwrap_or_else(|e| panic!("parse failed: {e}\nsource:\n{src}"))
}

fn parse_err(src: &str) -> super::ParseError {
    parse(src).expect_err("expected parse error")
}

// -------------------------------------------------------------------
//  Module headers / imports / exports
// -------------------------------------------------------------------

#[test]
fn empty_source_parses_as_empty_module() {
    let m = parse_expect("");
    assert!(m.header.is_none());
    assert!(m.imports.is_empty());
    assert!(m.decls.is_empty());
}

#[test]
fn bare_definition_without_module_header() {
    let m = parse_expect("x = 1\n");
    assert!(m.header.is_none());
    assert_eq!(m.decls.len(), 1);
}

#[test]
fn module_header_with_no_export_list() {
    let m = parse_expect("module Foo where\n");
    let h = m.header.unwrap();
    assert_eq!(h.name.segments, vec!["Foo".to_string()]);
    assert!(h.exports.is_none());
}

#[test]
fn module_header_dotted_name() {
    let m = parse_expect("module Data.List where\n");
    let h = m.header.unwrap();
    assert_eq!(
        h.name.segments,
        vec!["Data".to_string(), "List".to_string()]
    );
}

#[test]
fn module_header_with_empty_exports() {
    let m = parse_expect("module Foo () where\n");
    let h = m.header.unwrap();
    assert_eq!(h.exports, Some(vec![]));
}

#[test]
fn module_header_with_export_list() {
    let m = parse_expect("module Foo (x, Maybe(..), class Eq) where\n");
    let h = m.header.unwrap();
    let exports = h.exports.unwrap();
    assert_eq!(exports.len(), 3);
    assert!(matches!(exports[0], ExportItem::Value { .. }));
    assert!(matches!(exports[1], ExportItem::TypeAll { .. }));
    assert!(matches!(exports[2], ExportItem::Class { .. }));
}

#[test]
fn module_header_re_export() {
    let m = parse_expect("module F ( module Data.List ) where\n");
    let h = m.header.unwrap();
    let e = h.exports.unwrap();
    assert!(matches!(e[0], ExportItem::ReExport { .. }));
}

#[test]
fn import_plain() {
    let m = parse_expect("module F where\nimport Foo\n");
    assert_eq!(m.imports.len(), 1);
    assert!(!m.imports[0].qualified);
    assert!(matches!(m.imports[0].items, ImportItems::All));
}

#[test]
fn import_qualified_as_alias() {
    let m = parse_expect("module F where\nimport qualified Data.Map as M\n");
    let i = &m.imports[0];
    assert!(i.qualified);
    assert!(i.alias.is_some());
}

#[test]
fn import_with_item_list() {
    let m = parse_expect("module F where\nimport Foo (x, Bar(..))\n");
    let i = &m.imports[0];
    match &i.items {
        ImportItems::Only(items) => {
            assert_eq!(items.len(), 2);
        }
        _ => panic!("expected explicit item list"),
    }
}

#[test]
fn import_hiding() {
    let m = parse_expect("module F where\nimport Prelude hiding (foo)\n");
    assert!(matches!(m.imports[0].items, ImportItems::Hiding(_)));
}

#[test]
fn import_operator_in_list() {
    let m = parse_expect("module F where\nimport Prelude ((+), (>>=))\n");
    match &m.imports[0].items {
        ImportItems::Only(items) => {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], ImportItem::Value { operator: true, .. }));
        }
        _ => panic!(),
    }
}

// -------------------------------------------------------------------
//  Signatures, value bindings, patterns on LHS
// -------------------------------------------------------------------

#[test]
fn simple_signature() {
    let m = parse_expect("x : Int\n");
    match &m.decls[0] {
        Decl::Signature { name, operator, .. } => {
            assert_eq!(name, "x");
            assert!(!operator);
        }
        _ => panic!(),
    }
}

#[test]
fn signature_with_arrow_and_context() {
    let m = parse_expect("foo : Eq a => a -> a -> Bool\n");
    match &m.decls[0] {
        Decl::Signature { scheme, .. } => {
            assert_eq!(scheme.context.len(), 1);
            assert!(matches!(scheme.body, Type::Fun { .. }));
        }
        _ => panic!(),
    }
}

#[test]
fn signature_with_forall() {
    let m = parse_expect("foo : forall a. a -> a\n");
    match &m.decls[0] {
        Decl::Signature { scheme, .. } => {
            assert_eq!(scheme.forall, vec!["a".to_string()]);
        }
        _ => panic!(),
    }
}

#[test]
fn operator_signature() {
    let m = parse_expect("(+) : Int -> Int -> Int\n");
    match &m.decls[0] {
        Decl::Signature { name, operator, .. } => {
            assert_eq!(name, "+");
            assert!(*operator);
        }
        _ => panic!(),
    }
}

#[test]
fn value_def_zero_args() {
    let m = parse_expect("x = 42\n");
    match &m.decls[0] {
        Decl::Value(v) => {
            assert_eq!(v.name, "x");
            assert!(v.params.is_empty());
        }
        _ => panic!(),
    }
}

#[test]
fn value_def_with_args() {
    let m = parse_expect("greet name = name\n");
    match &m.decls[0] {
        Decl::Value(v) => {
            assert_eq!(v.name, "greet");
            assert_eq!(v.params.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn value_def_with_cons_pattern_lhs() {
    let m = parse_expect("parseAll (s::ss) = s\n");
    match &m.decls[0] {
        Decl::Value(v) => {
            assert_eq!(v.params.len(), 1);
            assert!(matches!(v.params[0], Pattern::Cons { .. }));
        }
        _ => panic!(),
    }
}

#[test]
fn value_def_with_empty_list_pattern_lhs() {
    let m = parse_expect("parseAll [] = 0\n");
    match &m.decls[0] {
        Decl::Value(v) => {
            assert!(matches!(v.params[0], Pattern::List { .. }));
        }
        _ => panic!(),
    }
}

#[test]
fn sectioned_call_with_app() {
    let m = parse_expect("sumOf = foldl (+) 0\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::App { .. } => (),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

// -------------------------------------------------------------------
//  Data / type / class / instance
// -------------------------------------------------------------------

#[test]
fn data_enum() {
    let m = parse_expect("data Bool = False | True\n");
    match &m.decls[0] {
        Decl::Data(d) => {
            assert_eq!(d.name, "Bool");
            assert_eq!(d.ctors.len(), 2);
        }
        _ => panic!(),
    }
}

#[test]
fn data_parametric() {
    let m = parse_expect("data Maybe a = Nothing | Just a\n");
    match &m.decls[0] {
        Decl::Data(d) => {
            assert_eq!(d.type_params, vec!["a".to_string()]);
            assert_eq!(d.ctors[1].args.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn data_recursive() {
    let m = parse_expect("data List a = Nil | Cons a (List a)\n");
    match &m.decls[0] {
        Decl::Data(d) => {
            assert_eq!(d.ctors[1].args.len(), 2);
        }
        _ => panic!(),
    }
}

#[test]
fn data_multi_line() {
    let m = parse_expect(
        "data HttpError\n  = NetworkError String\n  | StatusError Int String\n  | DecodeError String\n",
    );
    match &m.decls[0] {
        Decl::Data(d) => assert_eq!(d.ctors.len(), 3),
        _ => panic!(),
    }
}

#[test]
fn type_alias_simple() {
    let m = parse_expect("type Age = Int\n");
    match &m.decls[0] {
        Decl::TypeAlias(a) => assert_eq!(a.name, "Age"),
        _ => panic!(),
    }
}

#[test]
fn type_alias_record() {
    let m = parse_expect("type Student = { name : String, grade : Int }\n");
    match &m.decls[0] {
        Decl::TypeAlias(a) => {
            assert!(matches!(a.body, Type::Record { .. }));
        }
        _ => panic!(),
    }
}

#[test]
fn class_decl_simple() {
    let m = parse_expect("class Eq a where\n  (==) : a -> a -> Bool\n  (/=) : a -> a -> Bool\n");
    match &m.decls[0] {
        Decl::Class(c) => {
            assert_eq!(c.name, "Eq");
            assert_eq!(c.items.len(), 2);
        }
        _ => panic!(),
    }
}

#[test]
fn class_decl_with_default() {
    let m = parse_expect("class Eq a where\n  (==) : a -> a -> Bool\n  x /= y = not (x == y)\n");
    match &m.decls[0] {
        Decl::Class(c) => {
            assert_eq!(c.items.len(), 2);
            assert!(matches!(c.items[1], ClassItem::Default(_)));
        }
        _ => panic!(),
    }
}

#[test]
fn class_decl_superclass() {
    let m = parse_expect("class Eq a => Ord a where\n  compare : a -> a -> Int\n");
    match &m.decls[0] {
        Decl::Class(c) => {
            assert_eq!(c.context.len(), 1);
        }
        _ => panic!(),
    }
}

#[test]
fn instance_decl_simple() {
    let m = parse_expect("instance Eq Int where\n  x == y = True\n");
    match &m.decls[0] {
        Decl::Instance(i) => {
            assert_eq!(i.name, "Eq");
            assert_eq!(i.items.len(), 1);
        }
        _ => panic!(),
    }
}

// -------------------------------------------------------------------
//  Expressions
// -------------------------------------------------------------------

#[test]
fn let_in_expr() {
    let m = parse_expect("x = let a = 1 in a\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.body, Expr::Let { .. })),
        _ => panic!(),
    }
}

#[test]
fn if_then_else_expr() {
    let m = parse_expect("x = if True then 1 else 2\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.body, Expr::If { .. })),
        _ => panic!(),
    }
}

#[test]
fn lambda_single_param() {
    let m = parse_expect("f = \\x -> x\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Lambda { params, .. } => assert_eq!(params.len(), 1),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn lambda_multi_param() {
    let m = parse_expect("f = \\x y -> x\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Lambda { params, .. } => assert_eq!(params.len(), 2),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn function_application() {
    let m = parse_expect("x = f a b c\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.body, Expr::App { .. })),
        _ => panic!(),
    }
}

#[test]
fn arithmetic_precedence() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3).
    let m = parse_expect("x = 1 + 2 * 3\n");
    let body = match &m.decls[0] {
        Decl::Value(v) => v.body.clone(),
        _ => panic!(),
    };
    match body {
        Expr::BinOp { op, right, .. } => {
            assert_eq!(op, "+");
            assert!(matches!(&*right, Expr::BinOp { op, .. } if op == "*"));
        }
        _ => panic!(),
    }
}

#[test]
fn comparison_non_associative_rejected() {
    // a < b < c is a syntax error per spec 05.
    let e = parse_err("x = a < b < c\n");
    assert!(matches!(e.kind, super::ParseErrorKind::NonAssociativeChain));
}

#[test]
fn data_decl_requires_equals_and_ctor() {
    // spec 03 §Abstract syntax: `data T = C₁ | … | Cₘ` requires
    // `=` and at least one constructor. Bare `data T` is rejected.
    let e = parse_err("data T\n");
    match e.kind {
        super::ParseErrorKind::Expected { expected, .. }
        | super::ParseErrorKind::UnexpectedEof { expected } => {
            assert_eq!(expected, "`=`");
        }
        other => panic!("expected Expected/UnexpectedEof for `=`, got {other:?}"),
    }
}

#[test]
fn unary_minus_forms_negate() {
    let m = parse_expect("x = -3\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.body, Expr::Neg { .. })),
        _ => panic!(),
    }
}

#[test]
fn unary_minus_has_tighter_prec_than_binary() {
    // -a - b should parse as (negate a) - b, not negate (a - b).
    let m = parse_expect("x = -a - b\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::BinOp { op, left, .. } => {
                assert_eq!(op, "-");
                assert!(matches!(**left, Expr::Neg { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn operator_ref_as_value() {
    let m = parse_expect("x = (+)\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.body, Expr::OpRef { .. })),
        _ => panic!(),
    }
}

#[test]
fn cons_is_right_associative() {
    let m = parse_expect("x = 1 :: 2 :: Nil\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::BinOp { op, right, .. } => {
                assert_eq!(op, "::");
                assert!(matches!(&**right, Expr::BinOp { op, .. } if op == "::"));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn string_concat_right_associative() {
    let m = parse_expect("x = \"a\" ++ \"b\" ++ \"c\"\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::BinOp { op, right, .. } => {
                assert_eq!(op, "++");
                assert!(matches!(**right, Expr::BinOp { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn field_access_chain() {
    let m = parse_expect("x = p.a.b\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::FieldAccess { record, field, .. } => {
                assert_eq!(field, "b");
                assert!(matches!(**record, Expr::FieldAccess { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn record_literal() {
    let m = parse_expect("x = { a = 1, b = 2 }\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::RecordLit { fields, .. } => assert_eq!(fields.len(), 2),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn record_update() {
    let m = parse_expect("x = { p | a = 10 }\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.body, Expr::RecordUpdate { .. })),
        _ => panic!(),
    }
}

#[test]
fn empty_record() {
    let m = parse_expect("x = {}\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::RecordLit { fields, .. } => assert!(fields.is_empty()),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn list_literal() {
    let m = parse_expect("x = [1, 2, 3]\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::ListLit { items, .. } => assert_eq!(items.len(), 3),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn empty_list() {
    let m = parse_expect("x = []\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::ListLit { items, .. } => assert!(items.is_empty()),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn case_expr_basic() {
    let m = parse_expect("f x = case x of\n  Just n -> n\n  Nothing -> 0\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Case { arms, .. } => assert_eq!(arms.len(), 2),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn case_expr_brace_form() {
    let m = parse_expect("f x = case x of { Just n -> n ; Nothing -> 0 }\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Case { arms, .. } => assert_eq!(arms.len(), 2),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn do_block() {
    let m = parse_expect("main = do\n  greet \"a\"\n  greet \"b\"\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Do { stmts, .. } => assert_eq!(stmts.len(), 2),
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn do_block_with_bind() {
    let m = parse_expect("main = do\n  x <- get\n  pure x\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Do { stmts, .. } => {
                assert_eq!(stmts.len(), 2);
                assert!(matches!(stmts[0], DoStmt::Bind { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

// -------------------------------------------------------------------
//  Patterns
// -------------------------------------------------------------------

#[test]
fn wildcard_pattern() {
    let m = parse_expect("f _ = 0\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.params[0], Pattern::Wildcard(_))),
        _ => panic!(),
    }
}

#[test]
fn constructor_pattern_with_args() {
    let m = parse_expect("f x = case x of\n  Just n -> n\n  Nothing -> 0\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Case { arms, .. } => {
                assert!(matches!(arms[0].pattern, Pattern::Con { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn type_annotated_pattern() {
    let m = parse_expect("f (x : Int) = x\n");
    match &m.decls[0] {
        Decl::Value(v) => assert!(matches!(v.params[0], Pattern::Annot { .. })),
        _ => panic!(),
    }
}

#[test]
fn record_pattern() {
    // Record-pattern as the sole arm of a case must use explicit
    // braces to disambiguate from the case-alts block opener:
    // `case p of { pat -> e }` where the pat itself is a record
    // pattern `{ name = n }`.
    let m = parse_expect("f p = case p of { { name = n } -> n }\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Case { arms, .. } => {
                assert!(matches!(arms[0].pattern, Pattern::Record { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

#[test]
fn cons_pattern_in_case() {
    let m = parse_expect("f xs = case xs of\n  y :: ys -> y\n  [] -> 0\n");
    match &m.decls[0] {
        Decl::Value(v) => match &v.body {
            Expr::Case { arms, .. } => {
                assert!(matches!(arms[0].pattern, Pattern::Cons { .. }));
                assert!(matches!(arms[1].pattern, Pattern::List { .. }));
            }
            _ => panic!(),
        },
        _ => panic!(),
    }
}

// -------------------------------------------------------------------
//  Ruby embedding (spec 10)
// -------------------------------------------------------------------

#[test]
fn ruby_embed_single_line() {
    let m = parse_expect("rubyUpper s := \"s.upcase\"\n");
    match &m.decls[0] {
        Decl::RubyEmbed(r) => {
            assert_eq!(r.name, "rubyUpper");
            assert_eq!(r.params.len(), 1);
            assert_eq!(r.source, "s.upcase");
        }
        _ => panic!(),
    }
}

#[test]
fn ruby_embed_triple_quoted() {
    let m =
        parse_expect("rubyReadLines path := \"\"\"\n  File.readlines(path).map(&:chomp)\n\"\"\"\n");
    match &m.decls[0] {
        Decl::RubyEmbed(r) => {
            assert_eq!(r.name, "rubyReadLines");
            assert!(r.source.contains("File.readlines"));
        }
        _ => panic!(),
    }
}

// -------------------------------------------------------------------
//  Error cases
// -------------------------------------------------------------------

#[test]
fn unexpected_token_in_expr() {
    let e = parse_err("x = ,\n");
    assert!(matches!(e.kind, super::ParseErrorKind::Expected { .. }));
}

#[test]
fn missing_then_in_if() {
    let e = parse_err("x = if True 1 else 2\n");
    assert!(matches!(e.kind, super::ParseErrorKind::Expected { .. }));
}

#[test]
fn missing_equals_in_decl() {
    let e = parse_err("x 1\n");
    // `1` can't be a simple pat, causing an error at pattern parse
    // or clause-body detection. Just check we error out.
    assert!(matches!(
        e.kind,
        super::ParseErrorKind::Expected { .. } | super::ParseErrorKind::Unexpected(_)
    ));
}

#[test]
fn full_hello_ruby_sample_parses() {
    let src = include_str!("../../../../examples/sources/01-hello-ruby/Main.sp");
    let m = parse_expect(src);
    assert!(m.header.is_some());
    // 4 value/ruby declarations (main, greet, makeMessage, rubyPuts)
    // plus their signatures.
    assert!(m.decls.len() >= 4);
}

#[test]
fn full_parse_numbers_sample_parses() {
    let src = include_str!("../../../../examples/sources/02-parse-numbers/NumberSum.sp");
    parse_expect(src);
}

#[test]
fn full_students_records_sample_parses() {
    let src = include_str!("../../../../examples/sources/03-students-records/Students.sp");
    parse_expect(src);
}

#[test]
fn full_fetch_summarise_fetch_parses() {
    let src = include_str!("../../../../examples/sources/04-fetch-summarise/Fetch.sp");
    parse_expect(src);
}

#[test]
fn full_fetch_summarise_http_parses() {
    let src = include_str!("../../../../examples/sources/04-fetch-summarise/Http.sp");
    parse_expect(src);
}
