//! L4 hover: translate a cursor position into an inferred-type
//! tooltip (`textDocument/hover`).
//!
//! The handler mirrors the L5 goto-definition pipeline:
//!
//! 1. Parse the buffer (`analyze`) to get an AST.
//! 2. Resolve names (`resolve`) to get the `references:
//!    HashMap<Span, Resolution>` side table.
//! 3. Run HM inference (`typeck::infer::check_module`) over the
//!    resolved module to populate the per-top-level `inferred:
//!    HashMap<String, Scheme>` table and the per-ctor
//!    [`CtorInfo`] registry.
//! 4. Translate the cursor position to a byte offset, pick the
//!    innermost enclosing reference span (reusing
//!    [`crate::definition::find_reference_span`]), and look up the
//!    [`Scheme`] for that name.
//! 5. Render the scheme as a Markdown code block in the `sapphire`
//!    language plus a one-line context note (`(prelude)`,
//!    `(constructor of T)`, `(local)`, …).
//!
//! ## Scope (L4)
//!
//! - **Same-file only.** Cross-module imports resolve via the
//!   current prelude tables (for built-in operators and ctors) but
//!   not via user-authored `.sp` sources we haven't opened. This
//!   mirrors the L5 goto scope; see I-OQ72 / I-OQ73.
//! - **Local binders show name only.** I6's back-annotated output
//!   is the per-top-level scheme + ctor registry. Local (lambda /
//!   `let` / pattern / `do`-bind) types are not retained in a side
//!   table. For those references the hover surfaces the identifier
//!   and the `(local)` tag; the inferred type will appear once I6
//!   grows a `HashMap<Span, Ty>` side table (tracked as I-OQ96).
//! - **Type-position hover is best-effort.** Type variables land
//!   in the resolver side table as `Resolution::Local { name }`
//!   with no associated scheme; the hover surfaces the variable's
//!   name and the `(local)` tag only. Binding the variable to its
//!   `forall` quantifier — or to its implicit binder — requires a
//!   decision we inherit from L5 goto (I-OQ75) and track for L4
//!   as I-OQ99.
//! - **Typecheck errors are tolerated.** If inference raises errors
//!   we still return whatever partial schemes `ctx.inferred`
//!   captured before the failure. The goal is "best-effort tooltip
//!   in an editor session" rather than "gate hover behind a clean
//!   compile".
//!
//! The design note is `docs/impl/28-lsp-hover.md`.

use std::collections::HashMap;

use sapphire_compiler::resolver::{DefKind, ModuleEnv, Resolution, ResolvedModule, ResolvedRef};
use sapphire_compiler::typeck::infer::{InferCtx, check_module, install_prelude};
use sapphire_compiler::typeck::{CtorInfo, GlobalId, Scheme};
use sapphire_core::ast::Module as AstModule;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::definition::find_reference_span;
use crate::diagnostics::LineMap;

/// Read-only view of the typeck output the hover handler needs.
///
/// Three projections, each keyed differently:
///
/// - `inferred` — current-module top-level value / `:=` bindings,
///   keyed by bare name (mirrors `InferCtx.inferred`).
/// - `ctors` — every registered data constructor's scheme, keyed by
///   bare ctor name (a projection of `TypeEnv.ctors`).
/// - `globals` — **every** globally-registered scheme, keyed by
///   fully-qualified `GlobalId { module, name }`. This is the
///   authoritative home for prelude operators (`+`, `++`, `>>=`),
///   prelude functions (`map`, `pure`), and user-defined class
///   methods, which `register_ast_class` writes into
///   `TypeEnv.globals` but **not** into `InferCtx.inferred`. Without
///   projecting `globals`, hover over those names would only see the
///   name and drop the scheme.
///
/// All three are populated by [`collect_hover_types`]; the wrapper
/// struct keeps the `find_hover_info` signature stable when I6 later
/// grows a local-type side table (tracked as I-OQ96). Whether to keep
/// projection or switch to holding the full `InferCtx` is tracked as
/// I-OQ100.
#[derive(Debug, Clone, Default)]
pub struct HoverTypes {
    /// Inferred top-level schemes, keyed by binding name (current
    /// module only).
    pub inferred: HashMap<String, Scheme>,
    /// Data-constructor metadata, keyed by ctor name. Distinct from
    /// `inferred` because ctors live in a separate namespace slot in
    /// `TypeEnv` and the I6 `check_module` entry point only writes
    /// value-binding schemes into `inferred`.
    pub ctors: HashMap<String, CtorInfo>,
    /// Fully-qualified global schemes, keyed by `GlobalId`. Covers
    /// prelude operators / functions (under `"Prelude"`) and user
    /// class methods (under the declaring module's name) which
    /// `inferred` alone does not record.
    pub globals: HashMap<GlobalId, Scheme>,
}

impl HoverTypes {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Run HM inference against `module` and project the results into a
/// [`HoverTypes`] view.
///
/// On typeck errors the returned `HoverTypes` carries whatever the
/// inferencer managed to register before the first failure. The
/// prelude module name is hard-coded to `Prelude` to match the
/// `install_prelude` convention. The `module_name` argument is
/// intentionally borrowed rather than pulled out of the resolved
/// module's `env.id` because tests exercise `find_hover_info`
/// against synthetic modules whose resolver-level identity is
/// incidental.
pub fn collect_hover_types(module_name: &str, module: &AstModule) -> HoverTypes {
    let mut ctx = InferCtx::new(module_name);
    // `install_prelude` hard-codes `GlobalId::new("Prelude", _)` for
    // its inserts, so it is safe to run against any `ctx.module`;
    // the prelude entries always land under the `Prelude` module
    // key regardless.
    install_prelude(&mut ctx);
    // Ignore errors — `ctx.inferred` is populated incrementally as
    // bindings check out, and a partial map is still useful for
    // hover. A clean-compile gate is the wrong UX for an editor.
    let _ = check_module(&mut ctx, module);
    HoverTypes {
        inferred: ctx.inferred,
        ctors: ctx.type_env.ctors,
        globals: ctx.type_env.globals,
    }
}

/// Given a byte offset into `source`, return the LSP [`Hover`] for
/// the identifier at that offset, if one can be located.
///
/// The returned [`Hover::range`] is always the narrow reference span
/// the resolver recorded (e.g. the identifier itself, not the
/// enclosing expression). `contents` is a [`MarkupContent`] in
/// Markdown form: a fenced code block with the Sapphire scheme text
/// followed by a one-line italic context note.
///
/// Returns `None` when `byte_offset` does not rest on any reference
/// recorded by the resolver.
pub fn find_hover_info(
    module: &AstModule,
    resolved: &ResolvedModule,
    typed: &HoverTypes,
    _source: &str,
    byte_offset: usize,
    line_map: &LineMap<'_>,
) -> Option<Hover> {
    let _ = module; // reserved for future local-binder type lookup
    let span = find_reference_span(&resolved.references, byte_offset)?;
    let resolution = resolved.references.get(&span)?;
    let info = build_hover_info(&resolved.env, typed, resolution)?;
    let markdown = render_markdown(&info);
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: Some(line_map.range(span)),
    })
}

// ---------------------------------------------------------------------
//  Internal helpers
// ---------------------------------------------------------------------

/// What the rendered hover will display for a given reference.
///
/// Kept as a small intermediate value so the renderer stays a pure
/// string-assembly helper; unit tests can pin the renderer in
/// isolation from the pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HoverInfo {
    /// Display name (e.g. `x`, `Just`, `map`). For operators the
    /// resolver hands us the symbol as-is (`+`, `>>=`).
    name: String,
    /// `Some(scheme_pretty)` when we resolved a type scheme for the
    /// reference; `None` when we only know the name (e.g. local
    /// binders at L4 — I6 does not expose per-span types yet).
    scheme: Option<String>,
    /// One-line qualifier rendered in italics under the code block.
    context: HoverContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HoverContext {
    /// Prelude-defined binding (`Prelude.map`, `Prelude.+`).
    Prelude,
    /// Top-level value binding defined in this module.
    TopLevelValue,
    /// Constructor of a named data type.
    Constructor { type_name: String },
    /// Class method bound to a class name.
    ClassMethod { class_name: String },
    /// Ruby-embedded (`:=`) binding.
    RubyEmbed,
    /// Nominal data type in type position.
    DataType,
    /// `type T = τ` transparent alias.
    TypeAlias,
    /// Class name in type position.
    Class,
    /// Locally-bound identifier (lambda / let / pattern / do-bind).
    Local,
    /// A reference we recognise but cannot classify (e.g. imported
    /// name from a module whose metadata is outside this module's
    /// env).
    External { module: String },
}

fn build_hover_info(
    env: &ModuleEnv,
    typed: &HoverTypes,
    resolution: &Resolution,
) -> Option<HoverInfo> {
    match resolution {
        Resolution::Local { name } => Some(HoverInfo {
            name: name.clone(),
            scheme: None,
            context: HoverContext::Local,
        }),
        Resolution::Global(r) => {
            let is_prelude = r.module.segments == ["Prelude"];
            // 1. Try the current-module top-level table. This is the
            //    primary source for value hovers and also lets us
            //    discover whether the name is a ctor / class method
            //    / type alias / data-type / class.
            if let Some(def) = env.top_lookup(&r.name, r.namespace) {
                return Some(HoverInfo {
                    name: r.name.clone(),
                    scheme: scheme_for_def(typed, r, &def.kind),
                    context: context_from_def(&def.kind, is_prelude),
                });
            }
            // 2. Imported / prelude name. Try the qualified global
            //    table first — this is where `install_prelude` and
            //    `register_ast_class` deposit prelude operators,
            //    prelude functions, and class methods. Fall back to
            //    the ctor / inferred tables keyed by bare name for
            //    anything that slipped through (e.g. user-defined
            //    `import` re-exports under the same `InferCtx`).
            if let Some(cinfo) = typed.ctors.get(&r.name) {
                let type_name = cinfo.type_name.clone();
                return Some(HoverInfo {
                    name: r.name.clone(),
                    scheme: Some(cinfo.scheme.pretty()),
                    context: if is_prelude {
                        HoverContext::Prelude
                    } else {
                        HoverContext::Constructor { type_name }
                    },
                });
            }
            if let Some(scheme) = scheme_for_global(typed, r) {
                return Some(HoverInfo {
                    name: r.name.clone(),
                    scheme: Some(scheme),
                    context: if is_prelude {
                        HoverContext::Prelude
                    } else {
                        HoverContext::External {
                            module: r.module.display(),
                        }
                    },
                });
            }
            if let Some(scheme) = typed.inferred.get(&r.name) {
                return Some(HoverInfo {
                    name: r.name.clone(),
                    scheme: Some(scheme.pretty()),
                    context: if is_prelude {
                        HoverContext::Prelude
                    } else {
                        HoverContext::External {
                            module: r.module.display(),
                        }
                    },
                });
            }
            // 3. A known reference whose scheme we don't have. Show
            //    the name with an "external" tag so hover still fires
            //    rather than silently disappearing.
            Some(HoverInfo {
                name: r.name.clone(),
                scheme: None,
                context: if is_prelude {
                    HoverContext::Prelude
                } else {
                    HoverContext::External {
                        module: r.module.display(),
                    }
                },
            })
        }
    }
}

/// Look up a `ResolvedRef` in the qualified globals table. The
/// resolver's `ModuleId.segments` is dot-joined to match the
/// `GlobalId.module: String` key `InferCtx` uses (prelude entries
/// land under `"Prelude"`; user-module entries land under the
/// module's `ctx.module` which is set via `InferCtx::new(&id.display())`
/// in [`collect_hover_types`]).
fn scheme_for_global(typed: &HoverTypes, r: &ResolvedRef) -> Option<String> {
    let gid = GlobalId::new(r.module.display(), r.name.clone());
    typed.globals.get(&gid).map(|s| s.pretty())
}

fn scheme_for_def(typed: &HoverTypes, r: &ResolvedRef, kind: &DefKind) -> Option<String> {
    match kind {
        // Value / RubyEmbed bindings live in `inferred`; class methods
        // are registered only in `type_env.globals` (and not in
        // `inferred`), so try the qualified global first and fall back
        // to the bare-name `inferred` lookup.
        DefKind::Value | DefKind::RubyEmbed | DefKind::ClassMethod { .. } => {
            scheme_for_global(typed, r).or_else(|| typed.inferred.get(&r.name).map(|s| s.pretty()))
        }
        DefKind::Ctor { .. } => typed.ctors.get(&r.name).map(|c| c.scheme.pretty()),
        // Types / aliases / classes live in the type namespace and
        // have no value-level scheme.
        DefKind::DataType | DefKind::TypeAlias | DefKind::Class => None,
    }
}

fn context_from_def(kind: &DefKind, is_prelude: bool) -> HoverContext {
    if is_prelude {
        return HoverContext::Prelude;
    }
    match kind {
        DefKind::Value => HoverContext::TopLevelValue,
        DefKind::Ctor { parent_type } => HoverContext::Constructor {
            type_name: parent_type.clone(),
        },
        DefKind::ClassMethod { parent_class } => HoverContext::ClassMethod {
            class_name: parent_class.clone(),
        },
        DefKind::RubyEmbed => HoverContext::RubyEmbed,
        DefKind::DataType => HoverContext::DataType,
        DefKind::TypeAlias => HoverContext::TypeAlias,
        DefKind::Class => HoverContext::Class,
    }
}

fn render_markdown(info: &HoverInfo) -> String {
    let mut out = String::new();
    // Fenced code block. The language tag `sapphire` lines up with
    // the TextMate grammar scope so editors can highlight the type
    // line the same way they highlight a declaration.
    out.push_str("```sapphire\n");
    match &info.scheme {
        Some(scheme) => {
            // `name : scheme` mirrors the surface signature form.
            // For operators the name is a symbol — we do NOT wrap in
            // `(...)` here because the user can already see the glyph
            // in the source; keeping it bare avoids confusion with
            // Haskell's section syntax.
            out.push_str(&info.name);
            out.push_str(" : ");
            out.push_str(scheme);
            out.push('\n');
        }
        None => {
            // No known scheme — still show the name so the tooltip
            // is not blank.
            out.push_str(&info.name);
            out.push('\n');
        }
    }
    out.push_str("```\n");
    out.push_str(&context_line(&info.context));
    // Suppress the "type info not retrieved" fallback for contexts
    // where a value-level scheme is meaningless by design (type-side
    // names). Surfacing the note there is a lie: the type checker
    // did its job — the entity simply has no scheme.
    if info.scheme.is_none() && context_expects_scheme(&info.context) {
        out.push_str("\n\n");
        out.push_str("_型情報未取得_");
    }
    out
}

/// Whether a given hover context is expected to carry a value-level
/// scheme. Type-side references (`data` / `type` / `class`) do not,
/// and showing a "scheme missing" note for them is misleading.
fn context_expects_scheme(ctx: &HoverContext) -> bool {
    !matches!(
        ctx,
        HoverContext::DataType | HoverContext::TypeAlias | HoverContext::Class
    )
}

fn context_line(ctx: &HoverContext) -> String {
    match ctx {
        HoverContext::Prelude => "_(prelude)_".to_string(),
        HoverContext::TopLevelValue => "_(top-level value)_".to_string(),
        HoverContext::Constructor { type_name } => {
            format!("_(constructor of `{type_name}`)_")
        }
        HoverContext::ClassMethod { class_name } => {
            format!("_(method of class `{class_name}`)_")
        }
        HoverContext::RubyEmbed => "_(`:=`-binding)_".to_string(),
        HoverContext::DataType => "_(data type)_".to_string(),
        HoverContext::TypeAlias => "_(type alias)_".to_string(),
        HoverContext::Class => "_(class)_".to_string(),
        HoverContext::Local => "_(local)_".to_string(),
        HoverContext::External { module } => format!("_(imported from `{module}`)_"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sapphire_compiler::analyze::analyze;
    use sapphire_compiler::resolver::resolve;

    use crate::diagnostics::build_line_map;

    /// Compile `src` through analyze → resolve → typeck and return
    /// the bundle the hover handler needs. Panics on analyze /
    /// resolve failure so the surrounding test is easier to read.
    fn prepare(src: &str) -> (AstModule, ResolvedModule, HoverTypes) {
        let analysis = analyze(src);
        assert!(analysis.is_ok(), "analyze failed: {:?}", analysis.errors);
        let module = analysis.module.expect("module present");
        let resolved = resolve(module.clone()).expect("resolve ok");
        let module_name = resolved.env.id.display();
        let typed = collect_hover_types(&module_name, &module);
        (module, resolved, typed)
    }

    fn byte_first(src: &str, needle: &str) -> usize {
        src.find(needle)
            .unwrap_or_else(|| panic!("needle `{needle}` not found in source"))
    }

    fn markdown(h: &Hover) -> &str {
        match &h.contents {
            HoverContents::Markup(m) => &m.value,
            other => panic!("expected Markup contents, got {other:?}"),
        }
    }

    #[test]
    fn hover_for_top_level_value_reference_shows_scheme() {
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "y = x") + 4;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("```sapphire"), "missing code fence: {md}");
        assert!(md.contains("x : Int"), "missing scheme: {md}");
        assert!(md.contains("(top-level value)"), "missing ctx tag: {md}");
    }

    #[test]
    fn hover_range_matches_reference_span_not_enclosing_expr() {
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "y = x") + 4;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let range = hover.range.expect("range present");
        // Single-character identifier `x` on the last line (0-based).
        assert_eq!(range.start.line, range.end.line);
        assert_eq!(range.end.character - range.start.character, 1);
    }

    #[test]
    fn hover_for_constructor_shows_constructor_scheme() {
        let src = "\
module M where

data Pair = P Int Int

first : Pair
first = P 1 2
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "= P 1 2") + 2;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("P : "), "missing ctor name in code fence: {md}");
        assert!(md.contains("Pair"), "missing result type: {md}");
        assert!(
            md.contains("constructor of `Pair`"),
            "missing ctor tag: {md}",
        );
    }

    #[test]
    fn hover_for_prelude_operator_shows_prelude_tag() {
        let src = "\
module M where

two : Int
two = 1 + 1
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let plus_off = byte_first(src, "1 + 1") + 2;
        let hover = find_hover_info(&module, &resolved, &typed, src, plus_off, &map)
            .expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(prelude)"), "expected prelude tag: {md}");
        // Must-fix #1: the scheme for `+` lives in `type_env.globals`
        // under `GlobalId::new("Prelude", "+")`; the hover must
        // surface it rather than the bare-name fallback.
        assert!(
            md.contains("+ : Int -> Int -> Int"),
            "expected `+` scheme: {md}",
        );
        assert!(
            !md.contains("_型情報未取得_"),
            "prelude operator scheme must be populated, got fallback note: {md}",
        );
    }

    #[test]
    fn hover_for_prelude_list_append_shows_scheme() {
        // `++` is registered as a prelude global whose scheme lives
        // only in `type_env.globals`. Must-fix #1 regression.
        let src = "\
module M where

greet : String
greet = \"hi \" ++ \"there\"
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let append_off = byte_first(src, "++");
        let hover = find_hover_info(&module, &resolved, &typed, src, append_off, &map)
            .expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(prelude)"), "expected prelude tag: {md}");
        assert!(
            md.contains("++ : String -> String -> String"),
            "expected `++` scheme: {md}",
        );
        assert!(
            !md.contains("_型情報未取得_"),
            "`++` scheme must be populated: {md}",
        );
    }

    #[test]
    fn hover_for_user_class_method_shows_scheme() {
        // Must-fix #2: user-defined class methods land in
        // `type_env.globals` keyed by the declaring module, but NOT
        // in `InferCtx.inferred`. The hover (exercised at a *use*
        // site of the method) must follow the global table to
        // recover the scheme.
        let src = "\
module M where

class MyEq a where
  eqq : a -> a -> Bool

useEq : MyEq a => a -> a -> Bool
useEq x y = eqq x y
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        // Point at the `eqq` occurrence in `useEq x y = eqq x y`.
        let method_off = byte_first(src, "= eqq x y") + 2;
        let hover = find_hover_info(&module, &resolved, &typed, src, method_off, &map)
            .expect("hover present");
        let md = markdown(&hover);
        assert!(
            md.contains("method of class `MyEq`"),
            "expected class method tag: {md}",
        );
        assert!(md.contains("eqq :"), "expected method scheme line: {md}");
        assert!(
            !md.contains("_型情報未取得_"),
            "class method scheme must be populated: {md}",
        );
    }

    #[test]
    fn hover_for_prelude_ctor_shows_prelude_tag() {
        let src = "\
module M where

x : Maybe Int
x = Just 1
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "= Just 1") + 2;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("Just : "), "missing Just scheme: {md}");
        assert!(md.contains("(prelude)"), "expected prelude tag: {md}");
    }

    #[test]
    fn hover_for_local_let_binder_shows_name_only() {
        let src = "\
module M where

f : Int
f = let a = 1 in a
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "in a") + 3;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(local)"), "expected local tag: {md}");
        assert!(
            md.contains("_型情報未取得_"),
            "expected fallback note: {md}"
        );
    }

    #[test]
    fn hover_for_lambda_parameter_shows_local_tag() {
        let src = "\
module M where

f : Int -> Int
f = \\x -> x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = src.rfind('x').unwrap();
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(local)"), "expected local tag: {md}");
    }

    #[test]
    fn hover_for_function_parameter_shows_local_tag() {
        let src = "\
module M where

id : Int -> Int
id x = x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = src.rfind('x').unwrap();
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(local)"), "expected local tag: {md}");
    }

    #[test]
    fn hover_for_data_type_reference_in_signature_shows_data_tag() {
        let src = "\
module M where

data Pair = P Int Int

mkp : Pair
mkp = P 1 2
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, ": Pair") + 2;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(data type)"), "expected data tag: {md}");
        assert!(md.contains("Pair"), "expected type name: {md}");
        // Must-fix #3: data types are type-side and have no
        // value-level scheme by design — the hover must not append
        // the "type info not retrieved" fallback note.
        assert!(
            !md.contains("_型情報未取得_"),
            "data type hover must not emit scheme-missing fallback: {md}",
        );
    }

    #[test]
    fn hover_outside_any_reference_returns_none() {
        let src = "\
module M where

x : Int
x = 1
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        // Byte 0 — on the `module` keyword, not a reference site.
        assert!(find_hover_info(&module, &resolved, &typed, src, 0, &map).is_none());
    }

    #[test]
    fn hover_past_source_end_returns_none() {
        let src = "\
module M where

x : Int
x = 1
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let past = src.len();
        assert!(find_hover_info(&module, &resolved, &typed, src, past, &map).is_none());
    }

    #[test]
    fn hover_for_type_alias_in_signature_shows_alias_tag() {
        let src = "\
module M where

type Age = Int

mkage : Age
mkage = 0
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, ": Age") + 2;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(type alias)"), "expected alias tag: {md}");
        // Must-fix #3: aliases are type-side and carry no scheme.
        assert!(
            !md.contains("_型情報未取得_"),
            "type alias hover must not emit scheme-missing fallback: {md}",
        );
    }

    #[test]
    fn hover_for_class_reference_in_context_suppresses_fallback() {
        // A class name appearing in a class context `C a => ...`
        // reaches the hover as a type-side reference. Must-fix #3
        // covers this.
        let src = "\
module M where

class Show a where
  showMe : a -> String

shout : Show a => a -> String
shout x = showMe x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        // Point at the `Show` in the signature context.
        let use_off = byte_first(src, "Show a =>");
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(class)"), "expected class tag: {md}");
        assert!(
            !md.contains("_型情報未取得_"),
            "class hover must not emit scheme-missing fallback: {md}",
        );
    }

    #[test]
    fn hover_prefers_innermost_span_on_overlap() {
        // `y = x + x` — clicking on the first `x` should resolve
        // to `x`, not to the enclosing BinOp span.
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x + x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "x + x");
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("x : Int"), "expected x scheme: {md}");
    }

    #[test]
    fn hover_for_case_arm_pattern_binder_shows_local_tag() {
        let src = "\
module M where

only : Int -> Int
only n = case n of
  k -> k
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = src.rfind('k').unwrap();
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("(local)"), "expected local tag: {md}");
    }

    #[test]
    fn hover_for_ruby_embed_binding_shows_scheme_and_tag() {
        let src = "\
module M where

greet : Ruby {}
greet := \"\"\"
  puts \"hello\"
\"\"\"

main : Ruby {}
main = greet
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "main = greet") + "main = ".len();
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        let md = markdown(&hover);
        assert!(md.contains("greet : "), "expected scheme line: {md}");
        assert!(md.contains("Ruby"), "expected Ruby in type: {md}");
        assert!(
            md.contains("`:=`-binding") || md.contains("top-level value"),
            "expected embed/top-level tag: {md}",
        );
    }

    #[test]
    fn hover_returns_markdown_content_kind() {
        let src = "\
module M where

x : Int
x = 1

y : Int
y = x
";
        let (module, resolved, typed) = prepare(src);
        let map = build_line_map(src);
        let use_off = byte_first(src, "y = x") + 4;
        let hover =
            find_hover_info(&module, &resolved, &typed, src, use_off, &map).expect("hover present");
        match hover.contents {
            HoverContents::Markup(MarkupContent { kind, .. }) => {
                assert_eq!(kind, MarkupKind::Markdown);
            }
            other => panic!("expected Markdown Markup, got {other:?}"),
        }
    }

    #[test]
    fn render_markdown_formats_name_and_context() {
        // Pure-function smoke test on the renderer without a full
        // pipeline: guards against accidentally dropping the code
        // fence or the trailing italic note.
        let info = HoverInfo {
            name: "foo".to_string(),
            scheme: Some("Int -> Int".to_string()),
            context: HoverContext::TopLevelValue,
        };
        let md = render_markdown(&info);
        assert!(md.starts_with("```sapphire\n"));
        assert!(md.contains("foo : Int -> Int"));
        assert!(md.contains("(top-level value)"));
        assert!(!md.contains("_型情報未取得_"));
    }

    #[test]
    fn render_markdown_suppresses_fallback_for_type_side_contexts() {
        // Must-fix #3 renderer pin: `DataType` / `TypeAlias` / `Class`
        // never carry a scheme, and the fallback note must be
        // suppressed for them.
        for ctx in [
            HoverContext::DataType,
            HoverContext::TypeAlias,
            HoverContext::Class,
        ] {
            let info = HoverInfo {
                name: "T".to_string(),
                scheme: None,
                context: ctx.clone(),
            };
            let md = render_markdown(&info);
            assert!(
                !md.contains("_型情報未取得_"),
                "fallback must be suppressed for {ctx:?}, got:\n{md}",
            );
        }
    }

    #[test]
    fn render_markdown_keeps_fallback_for_local_context() {
        // Local binders legitimately have no scheme today (I-OQ96);
        // the fallback note must still appear for them so the UX
        // signal stays.
        let info = HoverInfo {
            name: "x".to_string(),
            scheme: None,
            context: HoverContext::Local,
        };
        let md = render_markdown(&info);
        assert!(
            md.contains("_型情報未取得_"),
            "expected local fallback: {md}"
        );
    }
}
