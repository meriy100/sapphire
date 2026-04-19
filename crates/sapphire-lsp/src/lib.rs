//! Sapphire Language Server.
//!
//! At L2 this crate exposes a `tower-lsp` based server that
//! handshakes with the client (`initialize` / `initialized` /
//! `shutdown`), keeps an in-memory document store, and publishes
//! lex / layout / parse diagnostics on every `didOpen` / `didChange`
//! / `didClose`. Richer capabilities (hover, goto-def, completion)
//! land in later Track L milestones. See
//! `docs/impl/10-lsp-scaffold.md` for the L1 scaffold,
//! `docs/impl/17-lsp-diagnostics.md` for L2 design decisions, and
//! `docs/impl/07-lsp-stack.md` for the underlying stack choice.

pub mod diagnostics;
pub mod server;

pub use server::SapphireLanguageServer;
