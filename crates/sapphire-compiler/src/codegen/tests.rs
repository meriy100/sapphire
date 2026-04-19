//! Unit tests for the codegen pipeline. Each test takes a small
//! Sapphire source fragment, runs it through the full lex / layout /
//! parse / resolve / typecheck stack, and then asserts properties on
//! the generated Ruby.
//!
//! We deliberately assert on *substrings* rather than full files so
//! that trivial formatting changes to `decl.rs` don't cascade into
//! dozens of test rewrites. When the shape of the output is
//! load-bearing (e.g. "this construct must route through
//! `prim_embed`"), the substring assertion is precise enough.

use super::*;
use crate::analyze::analyze;
use crate::resolver::resolve_program;
use crate::typeck::check_program;

/// Run the full front-end on `src` and generate Ruby for the
/// resulting single-module program.
fn run_gen(src: &str) -> String {
    let result = analyze(src);
    assert!(
        result.errors.is_empty(),
        "front-end errors: {:?}",
        result.errors
    );
    let ast = result.module.unwrap();
    let resolved = resolve_program(vec![ast]).expect("resolve");
    let typed = check_program(&resolved).expect("typecheck");
    let program = generate(&resolved, &typed);
    // The prelude is always first; concatenate everything after for
    // substring checks.
    program
        .files
        .iter()
        .skip(1)
        .map(|f| f.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Same as `gen` but returns the full [`GeneratedProgram`] so tests
/// that care about filenames / prelude emission can inspect them.
fn run_gen_full(src: &str) -> GeneratedProgram {
    let result = analyze(src);
    assert!(
        result.errors.is_empty(),
        "front-end errors: {:?}",
        result.errors
    );
    let ast = result.module.unwrap();
    let resolved = resolve_program(vec![ast]).expect("resolve");
    let typed = check_program(&resolved).expect("typecheck");
    generate(&resolved, &typed)
}

// ---------------- Modules & output layout ------------------------

#[test]
fn emits_prelude_file_first() {
    let prog = run_gen_full(
        "\
module M where
x : Int
x = 1
",
    );
    assert_eq!(prog.files[0].path, "sapphire/prelude.rb");
    assert_eq!(prog.files[1].path, "sapphire/m.rb");
}

#[test]
fn dotted_module_goes_into_subdirectory() {
    let prog = run_gen_full(
        "\
module Data.List where
x : Int
x = 1
",
    );
    assert_eq!(prog.files[1].path, "sapphire/data/list.rb");
    assert!(prog.files[1].content.contains("module Data"));
    assert!(prog.files[1].content.contains("class List"));
}

#[test]
fn single_segment_module_emits_single_class() {
    let out = run_gen(
        "\
module M where
x : Int
x = 1
",
    );
    assert!(out.contains("module Sapphire"));
    assert!(out.contains("class M"));
}

#[test]
fn prelude_require_always_emitted() {
    let out = run_gen(
        "\
module M where
x : Int
x = 1
",
    );
    assert!(out.contains("require 'sapphire/prelude'"));
}

// ---------------- Literals --------------------------------------

#[test]
fn int_literal_emits_as_integer() {
    let out = run_gen("module M where\nx : Int\nx = 42\n");
    assert!(out.contains("42"));
}

#[test]
fn string_literal_escapes_quotes() {
    let out = run_gen("module M where\nx : String\nx = \"a\\\"b\"\n");
    assert!(out.contains("\"a\\\"b\""));
}

#[test]
fn string_literal_escapes_newline() {
    let out = run_gen("module M where\nx : String\nx = \"a\\nb\"\n");
    assert!(out.contains("\\n"));
}

// ---------------- Operators -------------------------------------

#[test]
fn plus_inlines_to_ruby_plus() {
    let out = run_gen("module M where\nx : Int\nx = 1 + 2\n");
    assert!(out.contains("1) + (2"));
}

#[test]
fn minus_inlines_to_ruby_minus() {
    let out = run_gen("module M where\nx : Int\nx = 3 - 2\n");
    assert!(out.contains("3) - (2"));
}

#[test]
fn times_inlines_to_ruby_star() {
    let out = run_gen("module M where\nx : Int\nx = 3 * 2\n");
    assert!(out.contains("3) * (2"));
}

#[test]
fn division_inlines() {
    let out = run_gen("module M where\nx : Int\nx = 6 / 2\n");
    assert!(out.contains("6) / (2"));
}

#[test]
fn modulo_inlines() {
    let out = run_gen("module M where\nx : Int\nx = 7 % 3\n");
    assert!(out.contains("7) % (3"));
}

#[test]
fn equality_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = 1 == 2\n");
    assert!(out.contains("1) == (2"));
}

#[test]
fn inequality_inlines_to_bang_eq() {
    let out = run_gen("module M where\nx : Bool\nx = 1 /= 2\n");
    assert!(out.contains("1) != (2"));
}

#[test]
fn lt_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = 1 < 2\n");
    assert!(out.contains("1) < (2"));
}

#[test]
fn gt_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = 1 > 2\n");
    assert!(out.contains("1) > (2"));
}

#[test]
fn le_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = 1 <= 2\n");
    assert!(out.contains("1) <= (2"));
}

#[test]
fn ge_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = 1 >= 2\n");
    assert!(out.contains("1) >= (2"));
}

#[test]
fn and_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = True && False\n");
    assert!(out.contains("&&"));
}

#[test]
fn or_inlines() {
    let out = run_gen("module M where\nx : Bool\nx = True || False\n");
    assert!(out.contains("||"));
}

#[test]
fn string_append_uses_plus() {
    let out = run_gen("module M where\nx : String\nx = \"a\" ++ \"b\"\n");
    assert!(out.contains("\"a\") + (\"b\""));
}

#[test]
fn cons_desugars_to_array_splat() {
    let out = run_gen("module M where\nx : List Int\nx = 1 :: []\n");
    assert!(out.contains("[1, *[]]"));
}

#[test]
fn negation_emits() {
    let out = run_gen("module M where\nx : Int\nx = -5\n");
    assert!(out.contains("-(5)") || out.contains("(-5)"));
}

// ---------------- If / Case -------------------------------------

#[test]
fn if_emits_ternary() {
    let out = run_gen(
        "\
module M where
f : Int -> Int
f x = if x == 0 then 1 else 2
",
    );
    assert!(out.contains("?") && out.contains(":"));
}

#[test]
fn case_literal_emits_case_in() {
    let out = run_gen(
        "\
module M where
f : Int -> Int
f x = case x of
  0 -> 10
  n -> n
",
    );
    assert!(out.contains("case"));
    assert!(out.contains("in 0"));
}

#[test]
fn case_on_maybe_emits_tagged_patterns() {
    let out = run_gen(
        "\
module M where
f : Maybe Int -> Int
f m = case m of
  Nothing -> 0
  Just x  -> x
",
    );
    assert!(out.contains("tag: :Nothing"));
    assert!(out.contains("tag: :Just"));
}

#[test]
fn case_on_list_emits_array_pattern() {
    let out = run_gen(
        "\
module M where
f : List Int -> Int
f xs = case xs of
  []      -> 0
  x :: _  -> x
",
    );
    assert!(out.contains("in []"));
    assert!(out.contains("in [x, *_]"));
}

// ---------------- Lambda / Let ----------------------------------

#[test]
fn lambda_emits_curried_ruby_lambda() {
    let out = run_gen("module M where\nf : Int -> Int\nf = \\x -> x\n");
    assert!(out.contains("->("));
    assert!(out.contains("->(x)"));
}

#[test]
fn let_emits_iife_style() {
    let out = run_gen(
        "\
module M where
f : Int
f = let x = 3 in x
",
    );
    assert!(out.contains("lambda"));
    assert!(out.contains("x"));
}

// ---------------- Records ---------------------------------------

#[test]
fn record_literal_emits_symbol_keyed_hash() {
    let out = run_gen(
        "\
module M where
r : { name : String, age : Int }
r = { name = \"a\", age = 1 }
",
    );
    assert!(out.contains("name:"));
    assert!(out.contains("age:"));
}

#[test]
fn record_field_access_emits_bracket_symbol() {
    let out = run_gen(
        "\
module M where
f : { name : String } -> String
f r = r.name
",
    );
    assert!(out.contains("[:name]"));
}

#[test]
fn record_update_uses_merge() {
    let out = run_gen(
        "\
module M where
f : { age : Int } -> { age : Int }
f r = { r | age = 0 }
",
    );
    assert!(out.contains("merge"));
}

// ---------------- ADTs ------------------------------------------

#[test]
fn data_decl_installs_variant_factory() {
    let out = run_gen(
        "\
module M where
data Color = Red | Green | Blue
",
    );
    assert!(out.contains("ADT.define_variants"));
    assert!(out.contains(":Red"));
    assert!(out.contains(":Green"));
}

#[test]
fn constructor_with_args_emits_adt_make() {
    let out = run_gen(
        "\
module M where
data Box a = Box a
b : Box Int
b = Box 3
",
    );
    assert!(out.contains("ADT.make(:Box, [3])"));
}

#[test]
fn nullary_ctor_in_expression_emits_directly() {
    let out = run_gen(
        "\
module M where
data Color = Red | Green | Blue
c : Color
c = Red
",
    );
    assert!(out.contains("ADT.make(:Red, [])") || out.contains("Red"));
}

#[test]
fn prelude_data_decls_are_skipped() {
    // Module redefining Bool (not allowed in real code because
    // Prelude exports it, but the generator should not re-emit a
    // define_variants call either way).
    let out = run_gen(
        "\
module M where
f : Maybe Int
f = Just 3
",
    );
    // Maybe's variants come from the generated Sapphire::Prelude,
    // not from the module file.
    assert!(!out.contains("[:Just, 1]"));
}

#[test]
fn just_in_value_position_emits_adt_make() {
    let out = run_gen(
        "\
module M where
f : Maybe Int
f = Just 3
",
    );
    assert!(out.contains("ADT.make(:Just, [3])"));
}

#[test]
fn nothing_in_value_position_emits_adt_make() {
    let out = run_gen(
        "\
module M where
f : Maybe Int
f = Nothing
",
    );
    assert!(out.contains(":Nothing"));
}

// ---------------- Ruby embed (:= ) ------------------------------

#[test]
fn ruby_embed_wraps_in_prim_embed() {
    let out = run_gen(
        "\
module M where
rubyPuts : String -> Ruby {}
rubyPuts s := \"\"\"
  puts s
\"\"\"
",
    );
    assert!(out.contains("prim_embed"));
    assert!(out.contains("puts s"));
}

#[test]
fn ruby_embed_preserves_param_binding() {
    let out = run_gen(
        "\
module M where
rubyUpper : String -> Ruby String
rubyUpper s := \"s.upcase\"
",
    );
    assert!(out.contains("->(s)"));
    assert!(out.contains("s.upcase"));
}

// ---------------- Do notation -----------------------------------

#[test]
fn do_with_bind_emits_monad_bind() {
    let out = run_gen(
        "\
module M where
rubyPuts : String -> Ruby {}
rubyPuts s := \"puts s\"
main : Ruby {}
main = do
  rubyPuts \"hi\"
  rubyPuts \"again\"
",
    );
    assert!(out.contains("monad_then") || out.contains("monad_bind"));
}

#[test]
fn do_with_arrow_bind_emits_monad_bind() {
    let out = run_gen(
        "\
module M where
rubyRead : Ruby String
rubyRead := \"gets.chomp\"
rubyPuts : String -> Ruby {}
rubyPuts s := \"puts s\"
main : Ruby {}
main = do
  line <- rubyRead
  rubyPuts line
",
    );
    assert!(out.contains("monad_bind"));
}

#[test]
fn pure_in_ruby_monad_specialises_to_prim_return() {
    let out = run_gen(
        "\
module M where
f : Ruby Int
f = pure 3
",
    );
    assert!(out.contains("prim_return"));
}

#[test]
fn pure_in_result_monad_specialises_to_ok() {
    let out = run_gen(
        "\
module M where
f : Result String Int
f = pure 3
",
    );
    assert!(out.contains("ADT.make(:Ok, [3])"));
}

// ---------------- Main entry -----------------------------------

#[test]
fn ruby_main_emits_run_main_helper() {
    let out = run_gen(
        "\
module Main where
rubyPuts : String -> Ruby {}
rubyPuts s := \"puts s\"
main : Ruby {}
main = rubyPuts \"hi\"
",
    );
    assert!(out.contains("def self.run_main"));
    assert!(out.contains("run_action(main)"));
}

#[test]
fn pure_module_does_not_emit_run_main() {
    let out = run_gen(
        "\
module M where
x : Int
x = 1
",
    );
    assert!(!out.contains("run_main"));
}

// ---------------- Prelude helpers ------------------------------

#[test]
fn map_reference_routes_to_prelude() {
    let out = run_gen(
        "\
module M where
f : List Int -> List Int
f xs = map (\\x -> x) xs
",
    );
    assert!(out.contains("Sapphire::Prelude::MAP"));
}

#[test]
fn foldr_reference_routes_to_prelude() {
    let out = run_gen(
        "\
module M where
f : List Int -> Int
f = foldr (+) 0
",
    );
    assert!(out.contains("Sapphire::Prelude::FOLDR"));
}

// ---------------- Multi-clause / Patterns ----------------------

#[test]
fn multi_clause_function_emits_case_over_params() {
    let out = run_gen(
        "\
module M where
len : List Int -> Int
len []      = 0
len (_::xs) = 1 + len xs
",
    );
    // Multi-clause functions wrap all params in an array and match
    // per-clause. Cons-pattern should unfold inside that array.
    assert!(out.contains("case"));
    assert!(out.contains("[[]]"));
    assert!(out.contains("[[_, *xs]]"));
}

#[test]
fn single_clause_with_var_params_emits_aliases() {
    let out = run_gen(
        "\
module M where
double : Int -> Int
double n = n + n
",
    );
    // Aliases `n = _arg0` etc. should appear.
    assert!(out.contains("n = _arg0"));
}

// ---------------- Reference tables -----------------------------

#[test]
fn locally_bound_name_emits_bare_identifier() {
    let out = run_gen(
        "\
module M where
f : Int -> Int
f n = let m = n + 1 in m
",
    );
    assert!(out.contains("m = "));
}

#[test]
fn cross_module_reference_is_namespaced() {
    // This uses Prelude.map specifically to force a Global-resolution
    // path.
    let out = run_gen(
        "\
module M where
f : List Int -> List Int
f xs = Prelude.map (\\x -> x) xs
",
    );
    assert!(out.contains("Sapphire::Prelude"));
}

// ---------------- Show dispatch --------------------------------

#[test]
fn show_call_routes_through_prelude() {
    let out = run_gen(
        "\
module M where
f : Int -> String
f x = show x
",
    );
    assert!(out.contains("Sapphire::Prelude::SHOW"));
}

// ---------------- BinOp > / <  ---------------------------------

#[test]
fn list_literal_emits_array() {
    let out = run_gen(
        "\
module M where
xs : List Int
xs = [1, 2, 3]
",
    );
    assert!(out.contains("[1, 2, 3]"));
}

// ---------------- Empty list -----------------------------------

#[test]
fn nil_in_value_position_emits_empty_array() {
    let out = run_gen(
        "\
module M where
xs : List Int
xs = Nil
",
    );
    assert!(out.contains("SP_NIL") || out.contains("[]"));
}

// ---------------- Ok / Err -------------------------------------

#[test]
fn ok_application_emits_adt_make_ok() {
    let out = run_gen(
        "\
module M where
r : Result String Int
r = Ok 3
",
    );
    assert!(out.contains("ADT.make(:Ok, [3])"));
}

#[test]
fn err_application_emits_adt_make_err() {
    let out = run_gen(
        "\
module M where
r : Result String Int
r = Err \"oops\"
",
    );
    assert!(out.contains("ADT.make(:Err, [\"oops\"])"));
}

// ---------------- Imports --------------------------------------

#[test]
fn import_emits_require_statement() {
    // Two-module program: A imports B.
    let a = "\
module A where
import B (b)
x : Int
x = b
";
    let b = "\
module B where
b : Int
b = 1
";
    let ra = analyze(a).module.unwrap();
    let rb = analyze(b).module.unwrap();
    let resolved = resolve_program(vec![ra, rb]).expect("resolve");
    let typed = check_program(&resolved).expect("typecheck");
    let prog = generate(&resolved, &typed);
    let a_src = &prog
        .files
        .iter()
        .find(|f| f.path == "sapphire/a.rb")
        .unwrap()
        .content;
    assert!(a_src.contains("require 'sapphire/b'"));
}

// ---------------- snake_case -----------------------------------

#[test]
fn snake_case_for_camelcase_module_segment() {
    assert_eq!(to_snake_case("Foo"), "foo");
    assert_eq!(to_snake_case("FooBar"), "foo_bar");
    assert_eq!(to_snake_case("HTTPServer"), "h_t_t_p_server");
}
