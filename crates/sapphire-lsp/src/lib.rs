//! Sapphire Language Server.
//!
//! At L4 / L5 this crate exposes a `tower-lsp` based server that
//! handshakes with the client (`initialize` / `initialized` /
//! `shutdown`), keeps an in-memory document store, applies
//! incremental text edits on `didChange`, publishes lex / layout /
//! parse diagnostics, resolves `textDocument/definition` requests
//! against the I5 resolver side table, and answers
//! `textDocument/hover` with inferred-type tooltips from I6.
//! See `docs/impl/10-lsp-scaffold.md` for the L1 scaffold,
//! `docs/impl/17-lsp-diagnostics.md` for L2 design decisions,
//! `docs/impl/21-lsp-incremental-sync.md` for L3 incremental-sync
//! decisions, `docs/impl/22-lsp-goto-definition.md` for the L5
//! goto-definition design, `docs/impl/28-lsp-hover.md` for the L4
//! hover design, and `docs/impl/07-lsp-stack.md` for the underlying
//! stack choice.

pub mod definition;
pub mod diagnostics;
pub mod edit;
pub mod hover;
pub mod server;

pub use server::SapphireLanguageServer;
