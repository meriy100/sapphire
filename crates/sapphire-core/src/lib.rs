//! Shared types for the Sapphire compiler and language server.
//!
//! This crate hosts types that more than one downstream crate
//! consumes: source spans (`span`) and the surface AST (`ast`). It
//! deliberately stays dependency-free so that both
//! `sapphire-compiler` and `sapphire-lsp` can depend on it without
//! dragging in the rest of the compiler pipeline.
//!
//! The AST module mirrors the concrete syntax specified by
//! `docs/spec/01-core-expressions.md` through
//! `docs/spec/10-ruby-interop.md`; see `docs/impl/13-parser.md` for
//! the parser-strategy rationale and the AST layering choices.

pub mod ast;
pub mod span;
