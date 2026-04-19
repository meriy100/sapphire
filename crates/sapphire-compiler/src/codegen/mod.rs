//! Ruby code generation (I7).
//!
//! This module consumes a resolved + type-checked Sapphire program and
//! produces one Ruby source file per Sapphire module plus a generated
//! prelude file. The emitted code targets Ruby 3.3 and depends at run
//! time on the `sapphire-runtime` gem layout under `runtime/lib/` (R1
//! through R5).
//!
//! The design is split into four design notes under `docs/impl/`:
//!
//! - `docs/impl/24-codegen-expr.md` — I7a, expression-level translation.
//! - `docs/impl/25-codegen-adt-record.md` — I7b, ADT / record shapes.
//! - `docs/impl/26-codegen-effect-monad.md` — I7c, `:=`, `do`, and the
//!   `Ruby` monad.
//! - `docs/impl/27-cli.md` — I8, the CLI that drives this module.
//!
//! ## Inputs
//!
//! The code generator consumes two pieces of compiler state:
//!
//! - A [`ResolvedProgram`] from I5, whose reference side-tables tell us
//!   whether each `Expr::Var` is a local binding or points at a
//!   top-level definition in some module (including the synthetic
//!   `Prelude`).
//! - A [`TypedProgram`] from I6, whose per-binding schemes let us pick
//!   the right `pure` / `return` specialisation when we have to
//!   dispatch monad-polymorphic calls at code-emit time (see
//!   `docs/impl/26-codegen-effect-monad.md`).
//!
//! ## Outputs
//!
//! The entry point [`generate`] returns a [`GeneratedProgram`]:
//! a bundle of [`GeneratedFile`]s, each with a relative output path
//! (e.g. `sapphire/main.rb`, `sapphire/prelude.rb`) and its Ruby
//! source body. The CLI (I8) writes these under `--out-dir`; tests
//! pattern-match the bundle directly.

use sapphire_core::ast::Module as AstModule;

use crate::resolver::ResolvedProgram;
use crate::typeck::TypedProgram;

mod decl;
mod emit;
mod expr;
mod pattern;
mod prelude;
mod runtime;

#[cfg(test)]
mod tests;

/// A single Ruby file to be written at `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    /// Relative path under the output directory. Always uses `/` as
    /// the separator regardless of host OS; the CLI normalises to the
    /// host path separator before writing.
    pub path: String,
    /// The Ruby source body, UTF-8. Trailing newline included.
    pub content: String,
}

/// The full bundle of files that make up a compiled Sapphire program.
///
/// Stored in a stable order — the prelude first, then user modules in
/// dependency order — so that downstream consumers (CLI, tests,
/// snapshot examples) see the same output bytewise for identical
/// input.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GeneratedProgram {
    pub files: Vec<GeneratedFile>,
}

/// Generate a [`GeneratedProgram`] from a resolved program and its
/// typed twin.
///
/// The resolved program supplies the AST and reference side-table;
/// the typed program supplies return-type heads so that `pure` /
/// `return` calls can be specialised per binding per
/// `docs/impl/26-codegen-effect-monad.md` §`pure` / `return` / `>>=`
/// の dispatch.
pub fn generate(resolved: &ResolvedProgram, typed: &TypedProgram) -> GeneratedProgram {
    let mut files = Vec::new();

    // Always emit the prelude — every generated file `require`s it
    // and it is small enough to regenerate unconditionally.
    files.push(GeneratedFile {
        path: "sapphire/prelude.rb".into(),
        content: prelude::render_prelude(),
    });

    for rm in &resolved.modules {
        let typed_module = typed.modules.iter().find(|m| m.id == rm.id.display());
        let content = decl::render_module(&rm.ast, rm, typed_module);
        let path = module_output_path(&rm.ast);
        files.push(GeneratedFile { path, content });
    }

    GeneratedProgram { files }
}

/// Compute the relative output path for a given parsed Sapphire
/// module, per build 02 §Output tree.
fn module_output_path(m: &AstModule) -> String {
    let segments: Vec<String> = match &m.header {
        Some(h) => h.name.segments.iter().map(|s| to_snake_case(s)).collect(),
        None => vec!["main".into()],
    };
    format!("sapphire/{}.rb", segments.join("/"))
}

/// Convert a Sapphire `upper_ident` module segment to the snake_case
/// directory / file-basename form used by the output tree.
///
/// The rule: insert `_` before any uppercase letter that follows a
/// lowercase letter or digit, then lowercase the whole string. This
/// turns `DataList` into `data_list` and `HTTPServer` into
/// `h_t_t_p_server`. Acronym handling (`HTTPServer` →
/// `http_server`) is 10 OQ 1 territory and out of scope here.
pub(crate) fn to_snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && ch.is_ascii_uppercase() {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}
