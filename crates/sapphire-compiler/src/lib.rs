//! The Sapphire compiler.
//!
//! This crate hosts the front-end pipeline of the Sapphire compiler.
//! The modules are layered so that each stage consumes the previous
//! stage's output:
//!
//! - [`lexer`] — byte-level tokenisation (spec 02).
//!   `docs/impl/09-lexer.md` records the design rationale.
//! - [`layout`] — off-side-rule resolution (Haskell-98-ish), turning
//!   `Newline`/`Indent` markers into virtual `{`, `;`, `}` tokens.
//!   Rationale in `docs/impl/13-parser.md` §Layout.
//! - [`parser`] — hand-written recursive descent + Pratt operator
//!   parsing, producing an AST from `sapphire_core::ast`.
//!   Rationale in `docs/impl/13-parser.md`.
//!
//! The AST itself lives in [`sapphire_core::ast`] so that the LSP
//! crate can share it with the compiler without depending on the
//! compiler pipeline.
//!
//! Two auxiliary modules sit alongside the pipeline stages:
//!
//! - [`error`] — [`error::CompileError`], a unifying envelope over
//!   lex / layout / parse errors that downstream code (CLI, LSP)
//!   can consume without matching on three different per-stage
//!   types.
//! - [`analyze`] — the one-shot entry point that runs every front-end
//!   pass and returns an `AnalysisResult` carrying the module (on
//!   success) and the single first error (on failure).

pub mod analyze;
pub mod error;
pub mod layout;
pub mod lexer;
pub mod parser;
