//! Sapphire Language Server.
//!
//! L1 scaffold: this crate exposes a minimal `tower-lsp` based server
//! that handles `initialize` / `initialized` / `shutdown` and logs
//! `textDocument/didOpen` / `didChange` / `didClose` without acting on
//! them. Real capabilities (diagnostics, hover, completion, ...) land
//! in L2 onwards. See `docs/impl/10-lsp-scaffold.md` for the design
//! intent and `docs/impl/07-lsp-stack.md` for the stack decision.

pub mod server;

pub use server::SapphireLanguageServer;
