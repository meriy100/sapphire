//! Emit the generated `Sapphire::Prelude` Ruby source.
//!
//! The content lives in `prelude.rb.tpl` next to this file; it is
//! baked into the compiler binary at compile time via `include_str!`
//! and rendered verbatim on every `sapphire build`. Keeping the
//! prelude as a separate, codegen-side artefact (rather than shipping
//! it in the `sapphire-runtime` gem) matches the split recorded in
//! `docs/impl/24-codegen-expr.md` §prelude の取り扱い: runtime stays
//! tiny, prelude is a rebuilt surface per compile.

const PRELUDE_TEMPLATE: &str = include_str!("prelude.rb.tpl");

pub fn render_prelude() -> String {
    PRELUDE_TEMPLATE.to_string()
}
