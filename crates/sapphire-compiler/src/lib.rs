//! The Sapphire compiler.
//!
//! This crate hosts the lexer and (in later milestones) the parser,
//! name resolver, type checker, and code generator. The lexer is the
//! first real module and lives under [`lexer`]; see
//! `docs/spec/02-lexical-syntax.md` for the normative specification
//! and `docs/impl/09-lexer.md` for the implementation rationale.

pub mod lexer;
