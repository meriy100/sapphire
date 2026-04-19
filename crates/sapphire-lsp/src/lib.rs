//! Sapphire Language Server.
//!
//! At L5 this crate exposes a `tower-lsp` based server that
//! handshakes with the client (`initialize` / `initialized` /
//! `shutdown`), keeps an in-memory document store, applies
//! incremental text edits on `didChange`, publishes lex / layout /
//! parse diagnostics, and resolves `textDocument/definition`
//! requests against the I5 resolver side table. Richer capabilities
//! (hover, completion) land in later Track L milestones. See
//! `docs/impl/10-lsp-scaffold.md` for the L1 scaffold,
//! `docs/impl/17-lsp-diagnostics.md` for L2 design decisions,
//! `docs/impl/21-lsp-incremental-sync.md` for L3 incremental-sync
//! decisions, `docs/impl/22-lsp-goto-definition.md` for the L5
//! goto-definition design, and `docs/impl/07-lsp-stack.md` for the
//! underlying stack choice.

pub mod definition;
pub mod diagnostics;
pub mod edit;
pub mod server;

pub use server::SapphireLanguageServer;
