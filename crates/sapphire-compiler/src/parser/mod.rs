//! Sapphire parser — hand-written recursive descent + Pratt.
//!
//! This module consumes the token stream produced by
//! [`crate::lexer::tokenize`] after it has been run through
//! [`crate::layout::resolve`] and produces a
//! [`sapphire_core::ast::Module`] tree. The parser is hand-written
//! (I-OQ2 DECIDED) because:
//!
//! - The language's front-end is small enough that a hand-written
//!   recursive-descent + Pratt operator parser is easy to reason
//!   about and fast to tune for diagnostics.
//! - Avoiding a parser-generator dependency keeps the compiler crate
//!   buildable on stock `cargo build` with nothing past the stdlib.
//! - `chumsky` / `lalrpop` / `nom` each have their own learning curve
//!   and, more importantly, their own preferred error-reporting
//!   shapes that collide with the bidirectional-judgement diagnostic
//!   story L2 will want.
//!
//! See `docs/impl/13-parser.md` for the full rationale.

mod error;

#[cfg(test)]
mod tests;

pub use error::{ParseError, ParseErrorKind};

use sapphire_core::ast::*;
use sapphire_core::span::Span;

use crate::layout;
use crate::lexer::{self, Token, TokenKind};

// ===================================================================
//  Public API
// ===================================================================

/// Parse a Sapphire source string end-to-end: lex, run the layout
/// pass, then parse.
///
/// Intended as the one-shot entry point for callers that have the
/// whole source in memory. Errors from any of the three stages are
/// projected into a uniform `ParseError` (the lex and layout errors
/// are wrapped with their spans; the textual description is carried
/// through).
pub fn parse(source: &str) -> Result<Module, ParseError> {
    let tokens = lexer::tokenize(source).map_err(|e| {
        ParseError::new(
            ParseErrorKind::Unexpected(TokenKind::Op(format!("lex: {}", e.kind))),
            e.span,
        )
    })?;
    let resolved = layout::resolve_with_source(tokens, source)
        .map_err(|e| ParseError::new(ParseErrorKind::MalformedLayout, e.span))?;
    parse_tokens(&resolved)
}

/// Parse a resolved token stream directly.
///
/// Exposed for tests that want to feed hand-crafted token vectors
/// through the parser without routing them through the lexer /
/// layout passes.
pub fn parse_tokens(tokens: &[Token]) -> Result<Module, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_module()
}

// ===================================================================
//  Parser state
// ===================================================================

struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
}

impl<'t> Parser<'t> {
    fn new(tokens: &'t [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    // --- primitive cursor helpers ------------------------------------

    fn peek(&self) -> &Token {
        // The layout pass guarantees a trailing `Eof` on the input,
        // so this never panics on a well-formed stream.
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_at(&self, off: usize) -> &TokenKind {
        let idx = (self.pos + off).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn at(&self, k: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(k)
    }

    #[allow(dead_code)]
    fn eat(&mut self, k: &TokenKind) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, k: &TokenKind, desc: &'static str) -> Result<Token, ParseError> {
        if self.at(k) {
            Ok(self.bump())
        } else {
            let tok = self.peek().clone();
            Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: desc,
                    found: tok.kind,
                },
                tok.span,
            ))
        }
    }

    fn expect_lower_ident(&mut self) -> Result<(String, Span), ParseError> {
        let tok = self.peek().clone();
        if let TokenKind::LowerIdent(name) = &tok.kind {
            let n = name.clone();
            self.bump();
            Ok((n, tok.span))
        } else {
            Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "lower-case identifier",
                    found: tok.kind,
                },
                tok.span,
            ))
        }
    }

    fn expect_upper_ident(&mut self) -> Result<(String, Span), ParseError> {
        let tok = self.peek().clone();
        if let TokenKind::UpperIdent(name) = &tok.kind {
            let n = name.clone();
            self.bump();
            Ok((n, tok.span))
        } else {
            Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "upper-case identifier",
                    found: tok.kind,
                },
                tok.span,
            ))
        }
    }

    // --- module header & top-level -----------------------------------

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        // Every file is wrapped by an implicit top-level layout
        // block `{ ... }` by the layout pass.
        let start = self.peek().span;
        self.expect(&TokenKind::LBrace, "`{`")?;

        // Parse optional `module Foo (exports) where` header.
        let header = if matches!(self.peek_kind(), TokenKind::Module) {
            Some(self.parse_module_header()?)
        } else {
            None
        };

        // When a header is present, the `where` opened its own
        // layout block. The header body now lives inside that
        // inner `{`. We continue parsing declarations at whichever
        // level we are at now.
        let mut imports = Vec::new();
        let mut decls = Vec::new();
        // The `where` block contains *everything* after the header.
        // Separators come from the layout pass as `Semicolon` or
        // `LBrace/RBrace` pairs.
        self.parse_decls_into(&mut imports, &mut decls)?;

        // Close every remaining `}` / `Eof` (the header-introduced
        // block plus the top-level one).
        while matches!(self.peek_kind(), TokenKind::RBrace) {
            self.bump();
        }
        // Allow trailing semicolons from the layout block.
        while matches!(self.peek_kind(), TokenKind::Semicolon) {
            self.bump();
        }

        let end = self.peek().span;
        Ok(Module {
            header,
            imports,
            decls,
            span: start.merge(end),
        })
    }

    fn parse_module_header(&mut self) -> Result<ModuleHeader, ParseError> {
        let start = self.expect(&TokenKind::Module, "`module`")?.span;
        let name = self.parse_mod_name()?;
        let exports = if matches!(self.peek_kind(), TokenKind::LParen) {
            Some(self.parse_export_list()?)
        } else {
            None
        };
        self.expect(&TokenKind::Where, "`where`")?;
        // `where` opens an implicit block — the layout pass has
        // emitted `{` which we now consume so the body declarations
        // are at the same level as the explicit-brace form.
        self.expect(&TokenKind::LBrace, "`{` after `where`")?;
        let end = self.peek().span;
        Ok(ModuleHeader {
            name,
            exports,
            span: start.merge(end),
        })
    }

    fn parse_mod_name(&mut self) -> Result<ModName, ParseError> {
        let (first, first_span) = self.expect_upper_ident()?;
        let mut segments = vec![first];
        let mut span = first_span;
        while matches!(self.peek_kind(), TokenKind::Dot)
            && matches!(self.peek_at(1), TokenKind::UpperIdent(_))
        {
            self.bump(); // `.`
            let (seg, seg_span) = self.expect_upper_ident()?;
            segments.push(seg);
            span = span.merge(seg_span);
        }
        Ok(ModName { segments, span })
    }

    fn parse_export_list(&mut self) -> Result<Vec<ExportItem>, ParseError> {
        self.expect(&TokenKind::LParen, "`(`")?;
        let mut items = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            self.bump();
            return Ok(items);
        }
        loop {
            items.push(self.parse_export_item()?);
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        self.expect(&TokenKind::RParen, "`)`")?;
        Ok(items)
    }

    fn parse_export_item(&mut self) -> Result<ExportItem, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::LowerIdent(name) => {
                let n = name.clone();
                self.bump();
                Ok(ExportItem::Value {
                    name: n,
                    operator: false,
                    span: tok.span,
                })
            }
            TokenKind::LParen => {
                // `(op)` operator-as-identifier form.
                let start = self.bump().span;
                let op = self.expect_operator_symbol()?;
                let end = self.expect(&TokenKind::RParen, "`)`")?.span;
                Ok(ExportItem::Value {
                    name: op,
                    operator: true,
                    span: start.merge(end),
                })
            }
            TokenKind::UpperIdent(name) => {
                let n = name.clone();
                self.bump();
                // Optional `(..)` or `(Ctor, Ctor)` tail.
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.bump();
                    if matches!(self.peek_kind(), TokenKind::DotDot) {
                        self.bump();
                        self.expect(&TokenKind::RParen, "`)`")?;
                        Ok(ExportItem::TypeAll {
                            name: n,
                            span: tok.span,
                        })
                    } else if matches!(self.peek_kind(), TokenKind::RParen) {
                        self.bump();
                        Ok(ExportItem::TypeWith {
                            name: n,
                            ctors: Vec::new(),
                            span: tok.span,
                        })
                    } else {
                        let mut ctors = Vec::new();
                        loop {
                            let (c, _) = self.expect_upper_ident()?;
                            ctors.push(c);
                            if matches!(self.peek_kind(), TokenKind::Comma) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                        self.expect(&TokenKind::RParen, "`)`")?;
                        Ok(ExportItem::TypeWith {
                            name: n,
                            ctors,
                            span: tok.span,
                        })
                    }
                } else {
                    Ok(ExportItem::Type {
                        name: n,
                        span: tok.span,
                    })
                }
            }
            TokenKind::Class => {
                self.bump();
                let (n, n_span) = self.expect_upper_ident()?;
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.bump();
                    self.expect(&TokenKind::DotDot, "`..`")?;
                    self.expect(&TokenKind::RParen, "`)`")?;
                    Ok(ExportItem::ClassAll {
                        name: n,
                        span: tok.span.merge(n_span),
                    })
                } else {
                    Ok(ExportItem::Class {
                        name: n,
                        span: tok.span.merge(n_span),
                    })
                }
            }
            TokenKind::Module => {
                self.bump();
                let name = self.parse_mod_name()?;
                let sp = name.span;
                Ok(ExportItem::ReExport {
                    name,
                    span: tok.span.merge(sp),
                })
            }
            _ => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "export item",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    fn expect_operator_symbol(&mut self) -> Result<String, ParseError> {
        let tok = self.peek().clone();
        let sym = op_token_symbol(&tok.kind);
        match sym {
            Some(s) => {
                self.bump();
                Ok(s)
            }
            None => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "operator",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    fn parse_decls_into(
        &mut self,
        imports: &mut Vec<ImportDecl>,
        decls: &mut Vec<Decl>,
    ) -> Result<(), ParseError> {
        loop {
            // Skip leading/separator semicolons.
            while matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
            if matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                return Ok(());
            }
            if matches!(self.peek_kind(), TokenKind::Import) {
                imports.push(self.parse_import()?);
            } else {
                decls.push(self.parse_decl()?);
            }
            // After a decl, the layout block separator is a
            // `Semicolon` or the closing `}`. Swallow any trailing
            // separator before the loop checks termination.
            if matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            } else if !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                // Without a separator between decls, we still want
                // to continue (layout may have failed to emit one
                // for an opener-followed-by-same-line-token pattern).
                continue;
            }
        }
    }

    // --- imports -----------------------------------------------------

    fn parse_import(&mut self) -> Result<ImportDecl, ParseError> {
        let start = self.expect(&TokenKind::Import, "`import`")?.span;
        let qualified = if matches!(self.peek_kind(), TokenKind::Qualified) {
            self.bump();
            true
        } else {
            false
        };
        let name = self.parse_mod_name()?;

        let mut alias: Option<ModName> = None;
        let mut items = ImportItems::All;

        if matches!(self.peek_kind(), TokenKind::As) {
            self.bump();
            alias = Some(self.parse_mod_name()?);
        }

        match self.peek_kind() {
            TokenKind::LParen => {
                let list = self.parse_import_item_list()?;
                items = ImportItems::Only(list);
            }
            TokenKind::Hiding => {
                self.bump();
                let list = self.parse_import_item_list()?;
                items = ImportItems::Hiding(list);
            }
            _ => {}
        }

        let end = self.peek().span;
        Ok(ImportDecl {
            name,
            qualified,
            alias,
            items,
            span: start.merge(end),
        })
    }

    fn parse_import_item_list(&mut self) -> Result<Vec<ImportItem>, ParseError> {
        self.expect(&TokenKind::LParen, "`(`")?;
        let mut items = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            self.bump();
            return Ok(items);
        }
        loop {
            items.push(self.parse_import_item()?);
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        self.expect(&TokenKind::RParen, "`)`")?;
        Ok(items)
    }

    fn parse_import_item(&mut self) -> Result<ImportItem, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::LowerIdent(name) => {
                let n = name.clone();
                self.bump();
                Ok(ImportItem::Value {
                    name: n,
                    operator: false,
                    span: tok.span,
                })
            }
            TokenKind::LParen => {
                let start = self.bump().span;
                let op = self.expect_operator_symbol()?;
                let end = self.expect(&TokenKind::RParen, "`)`")?.span;
                Ok(ImportItem::Value {
                    name: op,
                    operator: true,
                    span: start.merge(end),
                })
            }
            TokenKind::UpperIdent(name) => {
                let n = name.clone();
                self.bump();
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.bump();
                    if matches!(self.peek_kind(), TokenKind::DotDot) {
                        self.bump();
                        self.expect(&TokenKind::RParen, "`)`")?;
                        Ok(ImportItem::TypeAll {
                            name: n,
                            span: tok.span,
                        })
                    } else if matches!(self.peek_kind(), TokenKind::RParen) {
                        self.bump();
                        Ok(ImportItem::TypeWith {
                            name: n,
                            ctors: Vec::new(),
                            span: tok.span,
                        })
                    } else {
                        let mut ctors = Vec::new();
                        loop {
                            let (c, _) = self.expect_upper_ident()?;
                            ctors.push(c);
                            if matches!(self.peek_kind(), TokenKind::Comma) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                        self.expect(&TokenKind::RParen, "`)`")?;
                        Ok(ImportItem::TypeWith {
                            name: n,
                            ctors,
                            span: tok.span,
                        })
                    }
                } else {
                    Ok(ImportItem::Type {
                        name: n,
                        span: tok.span,
                    })
                }
            }
            TokenKind::Class => {
                self.bump();
                let (n, n_span) = self.expect_upper_ident()?;
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.bump();
                    self.expect(&TokenKind::DotDot, "`..`")?;
                    self.expect(&TokenKind::RParen, "`)`")?;
                    Ok(ImportItem::ClassAll {
                        name: n,
                        span: tok.span.merge(n_span),
                    })
                } else {
                    Ok(ImportItem::Class {
                        name: n,
                        span: tok.span.merge(n_span),
                    })
                }
            }
            _ => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "import item",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    // --- top-level declarations --------------------------------------

    fn parse_decl(&mut self) -> Result<Decl, ParseError> {
        match self.peek_kind() {
            TokenKind::Data => self.parse_data_decl(),
            TokenKind::Type => self.parse_type_alias_decl(),
            TokenKind::Class => self.parse_class_decl(),
            TokenKind::Instance => self.parse_instance_decl(),
            _ => self.parse_value_or_signature_decl(),
        }
    }

    fn parse_value_or_signature_decl(&mut self) -> Result<Decl, ParseError> {
        // Either:
        //   name : scheme
        //   name arg... = expr
        //   name arg... := ruby_string
        //   (op) : scheme
        //   (op) arg... = expr
        //   pat `op` pat = expr   (operator-method clause, class/inst)
        let start_tok = self.peek().clone();
        let (name, operator, name_span) = match &start_tok.kind {
            TokenKind::LowerIdent(n) => {
                let s = n.clone();
                self.bump();
                (s, false, start_tok.span)
            }
            TokenKind::LParen => {
                // `(op)` form.
                let start = self.bump().span;
                let op = self.expect_operator_symbol()?;
                let end = self.expect(&TokenKind::RParen, "`)`")?.span;
                (op, true, start.merge(end))
            }
            _ => {
                return Err(ParseError::new(
                    ParseErrorKind::Expected {
                        expected: "declaration",
                        found: start_tok.kind,
                    },
                    start_tok.span,
                ));
            }
        };

        // Signature form: `name : scheme`.
        if matches!(self.peek_kind(), TokenKind::Colon) {
            self.bump();
            let scheme = self.parse_scheme()?;
            let span = name_span.merge(scheme.span);
            return Ok(Decl::Signature {
                name,
                operator,
                scheme,
                span,
            });
        }

        // Otherwise a value clause or Ruby embedding. Parse any
        // argument patterns before `=` / `:=`.
        let mut params = Vec::new();
        while !self.at_clause_body_start() {
            params.push(self.parse_apat()?);
        }

        if matches!(self.peek_kind(), TokenKind::Equals) {
            self.bump();
            let body = self.parse_expr()?;
            let span = name_span.merge(body.span());
            Ok(Decl::Value(ValueClause {
                name,
                operator,
                params,
                body,
                span,
            }))
        } else if self.at_ruby_embed() {
            // `:=` op-token.
            self.bump();
            let (source, src_span) = self.parse_ruby_string_literal()?;
            // Ruby-embed form requires params be plain identifiers
            // (spec 10 §Embedding form).
            let mut simple_params = Vec::new();
            for p in &params {
                if let Pattern::Var { name: pn, span: ps } = p {
                    simple_params.push(Param {
                        name: pn.clone(),
                        span: *ps,
                    });
                } else {
                    return Err(ParseError::new(
                        ParseErrorKind::UnsupportedFeature(
                            "Ruby embedding `:=` requires plain identifier parameters",
                        ),
                        p.span(),
                    ));
                }
            }
            Ok(Decl::RubyEmbed(RubyEmbedDecl {
                name,
                params: simple_params,
                source,
                span: name_span.merge(src_span),
            }))
        } else {
            let tok = self.peek().clone();
            Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "`=` or `:=`",
                    found: tok.kind,
                },
                tok.span,
            ))
        }
    }

    /// Tokens that terminate a function clause's parameter list.
    fn at_clause_body_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Equals | TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
        ) || self.at_ruby_embed()
    }

    fn at_ruby_embed(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Op(s) if s == ":=")
    }

    fn parse_ruby_string_literal(&mut self) -> Result<(String, Span), ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Str(s) => {
                let v = s.clone();
                self.bump();
                Ok((v, tok.span))
            }
            _ => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "Ruby source string literal",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    // --- data / type / class / instance ------------------------------

    fn parse_data_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.expect(&TokenKind::Data, "`data`")?.span;
        let (name, _) = self.expect_upper_ident()?;
        let mut type_params = Vec::new();
        while let TokenKind::LowerIdent(tv) = self.peek_kind() {
            let tv = tv.clone();
            self.bump();
            type_params.push(tv);
        }
        // spec 03 §Abstract syntax: `data T a₁ … aₙ = C₁ … | … | Cₘ …`
        // は `=` と少なくとも 1 本の constructor を必須とする。GADT /
        // existential / abstract data decl / record-shaped ctor は
        // spec 03 §Out-of-scope で明示的にスコープ外。
        self.expect(&TokenKind::Equals, "`=`")?;
        let mut ctors = Vec::new();
        ctors.push(self.parse_data_ctor()?);
        while matches!(self.peek_kind(), TokenKind::Bar) {
            self.bump();
            ctors.push(self.parse_data_ctor()?);
        }
        let end = ctors.last().map(|c| c.span).unwrap_or(start);
        Ok(Decl::Data(DataDecl {
            name,
            type_params,
            ctors,
            span: start.merge(end),
        }))
    }

    fn parse_data_ctor(&mut self) -> Result<DataCtor, ParseError> {
        let (name, name_span) = self.expect_upper_ident()?;
        let mut args = Vec::new();
        while self.at_atype_start() {
            args.push(self.parse_atype()?);
        }
        let end = args.last().map(|a| a.span()).unwrap_or(name_span);
        Ok(DataCtor {
            name,
            args,
            span: name_span.merge(end),
        })
    }

    fn parse_type_alias_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.expect(&TokenKind::Type, "`type`")?.span;
        let (name, _) = self.expect_upper_ident()?;
        let mut type_params = Vec::new();
        while let TokenKind::LowerIdent(tv) = self.peek_kind() {
            let tv = tv.clone();
            self.bump();
            type_params.push(tv);
        }
        self.expect(&TokenKind::Equals, "`=`")?;
        let body = self.parse_type()?;
        let span = start.merge(body.span());
        Ok(Decl::TypeAlias(TypeAlias {
            name,
            type_params,
            body,
            span,
        }))
    }

    fn parse_class_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.expect(&TokenKind::Class, "`class`")?.span;
        let context = self.parse_optional_context()?;
        let (name, _) = self.expect_upper_ident()?;
        let (type_var, _) = self.expect_lower_ident()?;
        self.expect(&TokenKind::Where, "`where`")?;
        self.expect(&TokenKind::LBrace, "`{`")?;
        let mut items = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            while matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
            if matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            items.push(self.parse_class_item()?);
            if matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
        }
        let end = self.peek().span;
        self.expect(&TokenKind::RBrace, "`}`")?;
        Ok(Decl::Class(ClassDecl {
            context,
            name,
            type_var,
            items,
            span: start.merge(end),
        }))
    }

    fn parse_class_item(&mut self) -> Result<ClassItem, ParseError> {
        // Either `name : scheme` or a default clause
        // `name pat... = expr` / `x op y = expr`.
        //
        // Detect a signature by looking at the token AFTER the
        // identifier (or `(op)`). If it is `:`, parse a signature;
        // otherwise parse a clause.
        let lookahead_sig = match self.peek_kind() {
            TokenKind::LowerIdent(_) => matches!(self.peek_at(1), TokenKind::Colon),
            TokenKind::LParen => {
                // `(op) :` — operator signature. Need at least
                // `(`, op, `)`, `:` so look at offsets 3.
                op_token_symbol(self.peek_at(1)).is_some()
                    && matches!(self.peek_at(2), TokenKind::RParen)
                    && matches!(self.peek_at(3), TokenKind::Colon)
            }
            _ => false,
        };
        if lookahead_sig {
            let start_tok = self.peek().clone();
            let (name, operator, name_span) = match &start_tok.kind {
                TokenKind::LowerIdent(n) => {
                    let s = n.clone();
                    self.bump();
                    (s, false, start_tok.span)
                }
                TokenKind::LParen => {
                    let start = self.bump().span;
                    let op = self.expect_operator_symbol()?;
                    let end = self.expect(&TokenKind::RParen, "`)`")?.span;
                    (op, true, start.merge(end))
                }
                _ => unreachable!(),
            };
            self.expect(&TokenKind::Colon, "`:`")?;
            let scheme = self.parse_scheme()?;
            let span = name_span.merge(scheme.span);
            Ok(ClassItem::Signature {
                name,
                operator,
                scheme,
                span,
            })
        } else {
            let clause = self.parse_clause()?;
            Ok(ClassItem::Default(clause))
        }
    }

    fn parse_instance_decl(&mut self) -> Result<Decl, ParseError> {
        let start = self.expect(&TokenKind::Instance, "`instance`")?.span;
        let context = self.parse_optional_context()?;
        let (name, _) = self.expect_upper_ident()?;
        let head = self.parse_atype()?;
        self.expect(&TokenKind::Where, "`where`")?;
        self.expect(&TokenKind::LBrace, "`{`")?;
        let mut items = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            while matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
            if matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            items.push(self.parse_clause()?);
            if matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
        }
        let end = self.peek().span;
        self.expect(&TokenKind::RBrace, "`}`")?;
        Ok(Decl::Instance(InstanceDecl {
            context,
            name,
            head,
            items,
            span: start.merge(end),
        }))
    }

    /// Parse a class/instance-body clause. Handles both
    /// `name pat... = expr` and infix-LHS `x op y = expr` for
    /// operator methods.
    fn parse_clause(&mut self) -> Result<ValueClause, ParseError> {
        // Detect infix-LHS form: `pat op pat = ...` where `pat` is
        // an apat and `op` is an operator token.
        // Conservative: parse an apat; if followed by an operator
        // token and another apat and `=`, treat as infix LHS.
        let first_tok = self.peek().clone();
        let snapshot = self.pos;

        // Case A: operator in parenthesised prefix: `(op) x y = ...`.
        if matches!(first_tok.kind, TokenKind::LParen)
            && op_token_symbol(self.peek_at(1)).is_some()
            && matches!(self.peek_at(2), TokenKind::RParen)
        {
            let start = self.bump().span;
            let op = self.expect_operator_symbol()?;
            let _end = self.expect(&TokenKind::RParen, "`)`")?.span;
            let mut params = Vec::new();
            while !self.at_clause_body_start() {
                params.push(self.parse_apat()?);
            }
            self.expect(&TokenKind::Equals, "`=`")?;
            let body = self.parse_expr()?;
            let span = start.merge(body.span());
            return Ok(ValueClause {
                name: op,
                operator: true,
                params,
                body,
                span,
            });
        }

        // Case B: `name pat... = expr` form.
        //          or infix-LHS `p1 op p2 = expr`.
        //
        // Parse the first apat. If it's a bare `Var` and next token
        // is not an operator, treat it as a name-plus-params clause.
        // If we see an operator right after the first apat, switch
        // to infix-LHS interpretation.
        let first = self.parse_apat()?;
        if op_token_symbol(self.peek_kind()).is_some()
            && self.is_valid_infix_method(self.peek_kind())
        {
            // Infix LHS.
            let op = self.expect_operator_symbol()?;
            let second = self.parse_apat()?;
            self.expect(&TokenKind::Equals, "`=`")?;
            let body = self.parse_expr()?;
            return Ok(ValueClause {
                name: op,
                operator: true,
                params: vec![first, second],
                body: body.clone(),
                span: first_tok.span.merge(body.span()),
            });
        }

        // Standard name-plus-params clause. The first apat must be
        // a bare Var.
        let (name, name_span) = match first {
            Pattern::Var { name, span } => (name, span),
            _ => {
                // Roll back and error.
                self.pos = snapshot;
                let tok = self.peek().clone();
                return Err(ParseError::new(
                    ParseErrorKind::Expected {
                        expected: "clause name",
                        found: tok.kind,
                    },
                    tok.span,
                ));
            }
        };
        let mut params = Vec::new();
        while !self.at_clause_body_start() {
            params.push(self.parse_apat()?);
        }
        self.expect(&TokenKind::Equals, "`=`")?;
        let body = self.parse_expr()?;
        Ok(ValueClause {
            name,
            operator: false,
            params,
            body: body.clone(),
            span: name_span.merge(body.span()),
        })
    }

    fn is_valid_infix_method(&self, kind: &TokenKind) -> bool {
        // `::` and most operators are fine; restrict to actual
        // operator tokens. `Equals` is NOT infix because it would
        // clash with the clause body separator.
        matches!(
            kind,
            TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::EqEq
                | TokenKind::SlashEq
                | TokenKind::Lt
                | TokenKind::LtEq
                | TokenKind::Gt
                | TokenKind::GtEq
                | TokenKind::AndAnd
                | TokenKind::OrOr
                | TokenKind::PlusPlus
                | TokenKind::DoubleColon
                | TokenKind::Op(_)
        )
    }

    fn parse_optional_context(&mut self) -> Result<Vec<Constraint>, ParseError> {
        // Heuristic: a context appears as `Ctx =>` where Ctx is
        // either `ClassName τ` (single) or `( ClassName τ, ... )`.
        // We look ahead for `=>` after balancing parens.
        if !self.looks_like_context() {
            return Ok(Vec::new());
        }
        let mut constraints = Vec::new();
        if matches!(self.peek_kind(), TokenKind::LParen) {
            self.bump();
            if !matches!(self.peek_kind(), TokenKind::RParen) {
                loop {
                    constraints.push(self.parse_constraint()?);
                    if matches!(self.peek_kind(), TokenKind::Comma) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            self.expect(&TokenKind::RParen, "`)`")?;
        } else {
            constraints.push(self.parse_constraint()?);
        }
        self.expect(&TokenKind::FatArrow, "`=>`")?;
        Ok(constraints)
    }

    fn looks_like_context(&self) -> bool {
        // Scan tokens for a top-level `=>` (paren-depth 0) before
        // we hit something that terminates a type (like `=`,
        // `where`, `;`, `}`, Eof). This is a bounded lookahead by
        // the length of the tokens array.
        let mut depth: i32 = 0;
        let mut i = self.pos;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    depth += 1;
                }
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                TokenKind::FatArrow if depth == 0 => return true,
                TokenKind::Equals | TokenKind::Where | TokenKind::Semicolon if depth == 0 => {
                    return false;
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_constraint(&mut self) -> Result<Constraint, ParseError> {
        let (name, start) = self.expect_upper_ident()?;
        let mut args = Vec::new();
        while self.at_atype_start() {
            args.push(self.parse_atype()?);
        }
        let end = args.last().map(|t| t.span()).unwrap_or(start);
        Ok(Constraint {
            class_name: name,
            args,
            span: start.merge(end),
        })
    }

    // --- schemes and types -------------------------------------------

    fn parse_scheme(&mut self) -> Result<Scheme, ParseError> {
        let start = self.peek().span;
        let forall = if matches!(self.peek_kind(), TokenKind::Forall) {
            self.bump();
            let mut vars = Vec::new();
            while let TokenKind::LowerIdent(v) = self.peek_kind() {
                let v = v.clone();
                self.bump();
                vars.push(v);
            }
            self.expect(&TokenKind::Dot, "`.`")?;
            vars
        } else {
            Vec::new()
        };
        let context = self.parse_optional_context()?;
        let body = self.parse_type()?;
        let span = start.merge(body.span());
        Ok(Scheme {
            forall,
            context,
            body,
            span,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        // τ = btype ( -> τ )?
        let left = self.parse_btype()?;
        if matches!(self.peek_kind(), TokenKind::Arrow) {
            self.bump();
            let right = self.parse_type()?;
            let span = left.span().merge(right.span());
            Ok(Type::Fun {
                param: Box::new(left),
                result: Box::new(right),
                span,
            })
        } else {
            Ok(left)
        }
    }

    fn parse_btype(&mut self) -> Result<Type, ParseError> {
        // btype = atype atype*
        let mut t = self.parse_atype()?;
        while self.at_atype_start() {
            let arg = self.parse_atype()?;
            let span = t.span().merge(arg.span());
            t = Type::App {
                func: Box::new(t),
                arg: Box::new(arg),
                span,
            };
        }
        Ok(t)
    }

    fn parse_atype(&mut self) -> Result<Type, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::LowerIdent(name) => {
                let n = name.clone();
                self.bump();
                Ok(Type::Var {
                    name: n,
                    span: tok.span,
                })
            }
            TokenKind::UpperIdent(name) => {
                // Possibly qualified: `Foo.Bar.Ty` or `Foo.Ty`.
                let first = name.clone();
                self.bump();
                let mut segments = vec![first];
                let mut span = tok.span;
                // Allow dotted chain while followed by UpperIdent.
                while matches!(self.peek_kind(), TokenKind::Dot)
                    && matches!(self.peek_at(1), TokenKind::UpperIdent(_))
                {
                    self.bump();
                    let (seg, seg_span) = self.expect_upper_ident()?;
                    segments.push(seg);
                    span = span.merge(seg_span);
                }
                let name = segments.pop().unwrap();
                let module = if segments.is_empty() {
                    None
                } else {
                    Some(ModName {
                        segments,
                        span: tok.span,
                    })
                };
                Ok(Type::Con { module, name, span })
            }
            TokenKind::LParen => {
                self.bump();
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    // `()` is not admitted as a type at this layer
                    // (tuples / unit deferred to 09 OQ 1). Use `{}`.
                    return Err(ParseError::new(
                        ParseErrorKind::UnsupportedFeature(
                            "`()` unit type (use `{}` empty record instead)",
                        ),
                        tok.span,
                    ));
                }
                let ty = self.parse_type()?;
                self.expect(&TokenKind::RParen, "`)`")?;
                Ok(ty)
            }
            TokenKind::LBrace => {
                self.bump();
                let mut fields = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RBrace) {
                    loop {
                        let (fname, _) = self.expect_lower_ident()?;
                        self.expect(&TokenKind::Colon, "`:`")?;
                        let fty = self.parse_type()?;
                        fields.push((fname, fty));
                        if matches!(self.peek_kind(), TokenKind::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RBrace, "`}`")?.span;
                Ok(Type::Record {
                    fields,
                    span: tok.span.merge(end),
                })
            }
            _ => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "type",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    fn at_atype_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::LowerIdent(_)
                | TokenKind::UpperIdent(_)
                | TokenKind::LParen
                | TokenKind::LBrace
        )
    }

    // --- patterns ----------------------------------------------------

    /// Parse a full pattern: `apat`, constructor pattern with
    /// args, or a cons chain.
    fn parse_pat(&mut self) -> Result<Pattern, ParseError> {
        let head = self.parse_con_pat()?;
        // Cons pattern: right-associative.
        if matches!(self.peek_kind(), TokenKind::DoubleColon) {
            self.bump();
            let tail = self.parse_pat()?;
            let span = head.span().merge(tail.span());
            Ok(Pattern::Cons {
                head: Box::new(head),
                tail: Box::new(tail),
                span,
            })
        } else {
            Ok(head)
        }
    }

    /// Parse a potentially-applied constructor pattern.
    fn parse_con_pat(&mut self) -> Result<Pattern, ParseError> {
        if let TokenKind::UpperIdent(_) = self.peek_kind() {
            // Look ahead — if UpperIdent at start, try gathering
            // apat arguments.
            let tok = self.peek().clone();
            let (name, name_span) = self.expect_upper_ident()?;
            // Optional module qualifier: Foo.Bar.Ctor.
            let mut segments = vec![name];
            let mut span = name_span;
            while matches!(self.peek_kind(), TokenKind::Dot)
                && matches!(self.peek_at(1), TokenKind::UpperIdent(_))
            {
                self.bump();
                let (seg, seg_span) = self.expect_upper_ident()?;
                segments.push(seg);
                span = span.merge(seg_span);
            }
            let ctor_name = segments.pop().unwrap();
            let module = if segments.is_empty() {
                None
            } else {
                Some(ModName {
                    segments,
                    span: tok.span,
                })
            };
            let mut args = Vec::new();
            while self.at_apat_start() {
                args.push(self.parse_apat()?);
            }
            let end = args.last().map(|a| a.span()).unwrap_or(span);
            Ok(Pattern::Con {
                module,
                name: ctor_name,
                args,
                span: tok.span.merge(end),
            })
        } else {
            self.parse_apat()
        }
    }

    fn at_apat_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Underscore
                | TokenKind::LowerIdent(_)
                | TokenKind::UpperIdent(_)
                | TokenKind::Int(_)
                | TokenKind::Str(_)
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
        )
    }

    fn parse_apat(&mut self) -> Result<Pattern, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Underscore => {
                self.bump();
                Ok(Pattern::Wildcard(tok.span))
            }
            TokenKind::LowerIdent(name) => {
                let n = name.clone();
                self.bump();
                // `x@apat`?
                if matches!(self.peek_kind(), TokenKind::Op(s) if s == "@") {
                    self.bump();
                    let inner = self.parse_apat()?;
                    let span = tok.span.merge(inner.span());
                    Ok(Pattern::As {
                        name: n,
                        inner: Box::new(inner),
                        span,
                    })
                } else {
                    Ok(Pattern::Var {
                        name: n,
                        span: tok.span,
                    })
                }
            }
            TokenKind::UpperIdent(name) => {
                // Nullary constructor (or module-qualified).
                let n = name.clone();
                self.bump();
                let mut segments = vec![n];
                let mut span = tok.span;
                while matches!(self.peek_kind(), TokenKind::Dot)
                    && matches!(self.peek_at(1), TokenKind::UpperIdent(_))
                {
                    self.bump();
                    let (seg, seg_span) = self.expect_upper_ident()?;
                    segments.push(seg);
                    span = span.merge(seg_span);
                }
                let ctor_name = segments.pop().unwrap();
                let module = if segments.is_empty() {
                    None
                } else {
                    Some(ModName {
                        segments,
                        span: tok.span,
                    })
                };
                Ok(Pattern::Con {
                    module,
                    name: ctor_name,
                    args: Vec::new(),
                    span,
                })
            }
            TokenKind::Int(n) => {
                let v = *n;
                self.bump();
                Ok(Pattern::Lit(Literal::Int(v), tok.span))
            }
            TokenKind::Str(s) => {
                let v = s.clone();
                self.bump();
                Ok(Pattern::Lit(Literal::Str(v), tok.span))
            }
            TokenKind::LParen => {
                self.bump();
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    // `()` has no pattern meaning in spec 06.
                    return Err(ParseError::new(
                        ParseErrorKind::UnsupportedFeature("`()` empty-tuple pattern"),
                        tok.span,
                    ));
                }
                let p = self.parse_pat()?;
                // Optional type annotation `(pat : type)`.
                let out = if matches!(self.peek_kind(), TokenKind::Colon) {
                    self.bump();
                    let ty = self.parse_type()?;
                    let span = p.span().merge(ty.span());
                    Pattern::Annot {
                        inner: Box::new(p),
                        ty,
                        span,
                    }
                } else {
                    p
                };
                self.expect(&TokenKind::RParen, "`)`")?;
                Ok(out)
            }
            TokenKind::LBracket => {
                self.bump();
                let mut items = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RBracket) {
                    loop {
                        items.push(self.parse_pat()?);
                        if matches!(self.peek_kind(), TokenKind::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RBracket, "`]`")?.span;
                Ok(Pattern::List {
                    items,
                    span: tok.span.merge(end),
                })
            }
            TokenKind::LBrace => {
                self.bump();
                let mut fields = Vec::new();
                if !matches!(self.peek_kind(), TokenKind::RBrace) {
                    loop {
                        let (fname, _) = self.expect_lower_ident()?;
                        self.expect(&TokenKind::Equals, "`=`")?;
                        let fpat = self.parse_pat()?;
                        fields.push((fname, fpat));
                        if matches!(self.peek_kind(), TokenKind::Comma) {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                let end = self.expect(&TokenKind::RBrace, "`}`")?.span;
                Ok(Pattern::Record {
                    fields,
                    span: tok.span.merge(end),
                })
            }
            _ => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "pattern",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    // --- expressions (Pratt) -----------------------------------------

    /// Parse a full expression — operator-aware Pratt parse at the
    /// bottom precedence tier.
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_binop_expr(0)
    }

    /// Pratt driver. `min_prec` is the minimum precedence tier the
    /// caller will accept.
    fn parse_binop_expr(&mut self, min_prec: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary_expr()?;
        while let Some(op_info) = self.current_binop() {
            if op_info.prec < min_prec {
                break;
            }
            // Tier 4 (comparisons) is non-associative. Reject
            // chained comparisons like `a < b < c`.
            if op_info.assoc == Assoc::None {
                if let Expr::BinOp { op: l_op, .. } = &left {
                    if operator_info(l_op)
                        .is_some_and(|o| o.assoc == Assoc::None && o.prec == op_info.prec)
                    {
                        let tok = self.peek().clone();
                        return Err(ParseError::new(
                            ParseErrorKind::NonAssociativeChain,
                            tok.span,
                        ));
                    }
                }
            }
            let (op_symbol, _op_span) = self.take_operator_token()?;
            let next_min = match op_info.assoc {
                Assoc::Left => op_info.prec + 1,
                Assoc::Right => op_info.prec,
                Assoc::None => op_info.prec + 1,
            };
            let right = self.parse_binop_expr(next_min)?;
            let span = left.span().merge(right.span());
            left = Expr::BinOp {
                op: op_symbol,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn current_binop(&self) -> Option<OpInfo> {
        operator_info_for_token(self.peek_kind())
    }

    fn take_operator_token(&mut self) -> Result<(String, Span), ParseError> {
        let tok = self.peek().clone();
        match op_token_symbol(&tok.kind) {
            Some(sym) => {
                self.bump();
                Ok((sym, tok.span))
            }
            None => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "operator",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        // Unary minus is recognised here, before function
        // application. Per spec 05 §Unary minus it is sugar for
        // `negate e` which we preserve as `Expr::Neg`.
        if matches!(self.peek_kind(), TokenKind::Minus) {
            let start = self.bump().span;
            let inner = self.parse_app_expr()?;
            let span = start.merge(inner.span());
            return Ok(Expr::Neg {
                value: Box::new(inner),
                span,
            });
        }
        self.parse_app_expr()
    }

    fn parse_app_expr(&mut self) -> Result<Expr, ParseError> {
        // Application is left-associative: a b c = (a b) c.
        let mut head = self.parse_field_expr()?;
        while self.at_atom_start() {
            let arg = self.parse_field_expr()?;
            let span = head.span().merge(arg.span());
            head = Expr::App {
                func: Box::new(head),
                arg: Box::new(arg),
                span,
            };
        }
        Ok(head)
    }

    /// Parse an atom, followed by any number of `.field`
    /// selections.
    fn parse_field_expr(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_atom()?;
        while matches!(self.peek_kind(), TokenKind::Dot)
            && matches!(self.peek_at(1), TokenKind::LowerIdent(_))
        {
            self.bump(); // `.`
            let (fname, fspan) = self.expect_lower_ident()?;
            let span = e.span().merge(fspan);
            e = Expr::FieldAccess {
                record: Box::new(e),
                field: fname,
                span,
            };
        }
        Ok(e)
    }

    fn at_atom_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::LowerIdent(_)
                | TokenKind::UpperIdent(_)
                | TokenKind::Int(_)
                | TokenKind::Str(_)
                | TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::LBrace
                | TokenKind::Backslash
                | TokenKind::Let
                | TokenKind::If
                | TokenKind::Case
                | TokenKind::Do
        )
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        let tok = self.peek().clone();
        match &tok.kind {
            TokenKind::Int(n) => {
                let v = *n;
                self.bump();
                Ok(Expr::Lit(Literal::Int(v), tok.span))
            }
            TokenKind::Str(s) => {
                let v = s.clone();
                self.bump();
                Ok(Expr::Lit(Literal::Str(v), tok.span))
            }
            TokenKind::LowerIdent(name) => {
                let n = name.clone();
                self.bump();
                Ok(Expr::Var {
                    module: None,
                    name: n,
                    span: tok.span,
                })
            }
            TokenKind::UpperIdent(_) => self.parse_upper_ident_expr(),
            TokenKind::Backslash => self.parse_lambda(),
            TokenKind::Let => self.parse_let_expr(),
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Case => self.parse_case_expr(),
            TokenKind::Do => self.parse_do_expr(),
            TokenKind::LParen => self.parse_paren_expr(),
            TokenKind::LBracket => self.parse_list_expr(),
            TokenKind::LBrace => self.parse_record_expr(),
            _ => Err(ParseError::new(
                ParseErrorKind::Expected {
                    expected: "expression",
                    found: tok.kind,
                },
                tok.span,
            )),
        }
    }

    fn parse_upper_ident_expr(&mut self) -> Result<Expr, ParseError> {
        let start_tok = self.peek().clone();
        let (first, first_span) = self.expect_upper_ident()?;
        let mut segments = vec![first];
        let mut span = first_span;
        // Chain: Foo.Bar.Ctor or Foo.Bar.value or Foo.Bar.(op)?
        while matches!(self.peek_kind(), TokenKind::Dot) {
            match self.peek_at(1) {
                TokenKind::UpperIdent(_) => {
                    self.bump();
                    let (seg, seg_span) = self.expect_upper_ident()?;
                    segments.push(seg);
                    span = span.merge(seg_span);
                }
                TokenKind::LowerIdent(_) => {
                    self.bump(); // `.`
                    let (name, n_span) = self.expect_lower_ident()?;
                    let module = ModName {
                        segments,
                        span: start_tok.span,
                    };
                    return Ok(Expr::Var {
                        module: Some(module),
                        name,
                        span: span.merge(n_span),
                    });
                }
                _ => break,
            }
        }
        // Ended with only upper segments → constructor reference.
        let last = segments.pop().unwrap();
        let module = if segments.is_empty() {
            None
        } else {
            Some(ModName {
                segments,
                span: start_tok.span,
            })
        };
        Ok(Expr::Var {
            module,
            name: last,
            span,
        })
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::Backslash, "`\\`")?.span;
        let mut params = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::Arrow) {
            params.push(self.parse_apat()?);
        }
        self.expect(&TokenKind::Arrow, "`->`")?;
        let body = self.parse_expr()?;
        let span = start.merge(body.span());
        Ok(Expr::Lambda {
            params,
            body: Box::new(body),
            span,
        })
    }

    fn parse_let_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::Let, "`let`")?.span;
        // `let` opens an implicit layout block. We parse a single
        // binding (spec 01 is single-binding). The block contains
        // `name pat... = expr`.
        self.expect(&TokenKind::LBrace, "`{`")?;
        // Swallow any leading virtual `;`s.
        while matches!(self.peek_kind(), TokenKind::Semicolon) {
            self.bump();
        }
        // Parse the single binding.
        let binding_start = self.peek().clone();
        let (name, operator, _name_span) = match &binding_start.kind {
            TokenKind::LowerIdent(n) => {
                let s = n.clone();
                self.bump();
                (s, false, binding_start.span)
            }
            TokenKind::LParen
                if op_token_symbol(self.peek_at(1)).is_some()
                    && matches!(self.peek_at(2), TokenKind::RParen) =>
            {
                let start = self.bump().span;
                let op = self.expect_operator_symbol()?;
                let end = self.expect(&TokenKind::RParen, "`)`")?.span;
                (op, true, start.merge(end))
            }
            _ => {
                return Err(ParseError::new(
                    ParseErrorKind::Expected {
                        expected: "let binding name",
                        found: binding_start.kind,
                    },
                    binding_start.span,
                ));
            }
        };
        let mut params = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::Equals) {
            params.push(self.parse_apat()?);
        }
        self.expect(&TokenKind::Equals, "`=`")?;
        let value = self.parse_expr()?;
        // The layout pass closes the let-block on `in`, so we expect
        // `}` then `in`.
        while matches!(self.peek_kind(), TokenKind::Semicolon) {
            self.bump();
        }
        self.expect(&TokenKind::RBrace, "`}`")?;
        self.expect(&TokenKind::In, "`in`")?;
        let body = self.parse_expr()?;
        let span = start.merge(body.span());
        Ok(Expr::Let {
            name,
            operator,
            params,
            value: Box::new(value),
            body: Box::new(body),
            span,
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::If, "`if`")?.span;
        let cond = self.parse_expr()?;
        self.expect(&TokenKind::Then, "`then`")?;
        let then_branch = self.parse_expr()?;
        self.expect(&TokenKind::Else, "`else`")?;
        let else_branch = self.parse_expr()?;
        let span = start.merge(else_branch.span());
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
            span,
        })
    }

    fn parse_case_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::Case, "`case`")?.span;
        let scrutinee = self.parse_expr()?;
        self.expect(&TokenKind::Of, "`of`")?;
        self.expect(&TokenKind::LBrace, "`{`")?;
        let mut arms = Vec::new();
        loop {
            while matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
            if matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            let pattern = self.parse_pat()?;
            self.expect(&TokenKind::Arrow, "`->`")?;
            let body = self.parse_expr()?;
            let span = pattern.span().merge(body.span());
            arms.push(CaseArm {
                pattern,
                body,
                span,
            });
            if matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
        }
        let end = self.expect(&TokenKind::RBrace, "`}`")?.span;
        Ok(Expr::Case {
            scrutinee: Box::new(scrutinee),
            arms,
            span: start.merge(end),
        })
    }

    fn parse_do_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::Do, "`do`")?.span;
        self.expect(&TokenKind::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        loop {
            while matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
            if matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            stmts.push(self.parse_do_stmt()?);
            if matches!(self.peek_kind(), TokenKind::Semicolon) {
                self.bump();
            }
        }
        let end = self.expect(&TokenKind::RBrace, "`}`")?.span;
        Ok(Expr::Do {
            stmts,
            span: start.merge(end),
        })
    }

    fn parse_do_stmt(&mut self) -> Result<DoStmt, ParseError> {
        // Three forms:
        //   pat <- e    (bind)
        //   let x = e   (inline let, no `in`)
        //   e           (expression)
        if matches!(self.peek_kind(), TokenKind::Let) {
            let start = self.bump().span;
            // `let` inside `do` doesn't open a layout block the
            // way a top-level `let` does — it's a single binding
            // `let name pat... = expr`. The layout pass has pushed
            // an Implicit(Let) block though; we accept an optional
            // `{` ... `}` wrapping and close it.
            let used_block = matches!(self.peek_kind(), TokenKind::LBrace);
            if used_block {
                self.bump();
            }
            let binding_start = self.peek().clone();
            let (name, operator, _name_span) = match &binding_start.kind {
                TokenKind::LowerIdent(n) => {
                    let s = n.clone();
                    self.bump();
                    (s, false, binding_start.span)
                }
                _ => {
                    return Err(ParseError::new(
                        ParseErrorKind::Expected {
                            expected: "let binding name",
                            found: binding_start.kind,
                        },
                        binding_start.span,
                    ));
                }
            };
            let mut params = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::Equals) {
                params.push(self.parse_apat()?);
            }
            self.expect(&TokenKind::Equals, "`=`")?;
            let value = self.parse_expr()?;
            if used_block {
                while matches!(self.peek_kind(), TokenKind::Semicolon) {
                    self.bump();
                }
                self.expect(&TokenKind::RBrace, "`}`")?;
            }
            let span = start.merge(value.span());
            return Ok(DoStmt::Let {
                name,
                operator,
                params,
                value,
                span,
            });
        }

        // Try `pat <- e` vs `e`. Scan for a `<-` at paren depth 0
        // before a statement-terminating token.
        if self.looks_like_bind_stmt() {
            let pattern = self.parse_pat()?;
            self.expect(&TokenKind::LeftArrow, "`<-`")?;
            let expr = self.parse_expr()?;
            let span = pattern.span().merge(expr.span());
            return Ok(DoStmt::Bind {
                pattern,
                expr,
                span,
            });
        }

        let e = self.parse_expr()?;
        Ok(DoStmt::Expr(e))
    }

    fn looks_like_bind_stmt(&self) -> bool {
        let mut depth: i32 = 0;
        let mut i = self.pos;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    depth += 1;
                }
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                TokenKind::LeftArrow if depth == 0 => return true,
                TokenKind::Semicolon | TokenKind::Eof if depth == 0 => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_paren_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::LParen, "`(`")?.span;
        // `(op)` operator-as-value form.
        if op_token_symbol(self.peek_kind()).is_some()
            && matches!(self.peek_at(1), TokenKind::RParen)
        {
            let sym = op_token_symbol(self.peek_kind()).unwrap();
            self.bump();
            let end = self.expect(&TokenKind::RParen, "`)`")?.span;
            return Ok(Expr::OpRef {
                symbol: sym,
                span: start.merge(end),
            });
        }
        // `()` unit is not admitted.
        if matches!(self.peek_kind(), TokenKind::RParen) {
            let end = self.bump().span;
            return Err(ParseError::new(
                ParseErrorKind::UnsupportedFeature("`()` empty tuple (use `{}`)"),
                start.merge(end),
            ));
        }
        let e = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "`)`")?;
        Ok(e)
    }

    fn parse_list_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::LBracket, "`[`")?.span;
        let mut items = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RBracket) {
            loop {
                items.push(self.parse_expr()?);
                if matches!(self.peek_kind(), TokenKind::Comma) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        let end = self.expect(&TokenKind::RBracket, "`]`")?.span;
        Ok(Expr::ListLit {
            items,
            span: start.merge(end),
        })
    }

    fn parse_record_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.expect(&TokenKind::LBrace, "`{`")?.span;
        // Empty record `{}`.
        if matches!(self.peek_kind(), TokenKind::RBrace) {
            let end = self.bump().span;
            return Ok(Expr::RecordLit {
                fields: Vec::new(),
                span: start.merge(end),
            });
        }
        // Distinguish update (`{ e | f = ... }`) from literal
        // (`{ f = e, ... }`). Look for a top-level `|` before any
        // `=` at brace depth 0.
        if self.looks_like_record_update() {
            let record = self.parse_expr()?;
            self.expect(&TokenKind::Bar, "`|`")?;
            let fields = self.parse_record_field_bindings()?;
            let end = self.expect(&TokenKind::RBrace, "`}`")?.span;
            Ok(Expr::RecordUpdate {
                record: Box::new(record),
                fields,
                span: start.merge(end),
            })
        } else {
            let fields = self.parse_record_field_bindings()?;
            let end = self.expect(&TokenKind::RBrace, "`}`")?.span;
            Ok(Expr::RecordLit {
                fields,
                span: start.merge(end),
            })
        }
    }

    fn looks_like_record_update(&self) -> bool {
        // We have consumed `{`. Scan forward: if we encounter a
        // top-level `|` before the matching `}`, and the `|`
        // appears before any `=` at brace depth 0, it's an update.
        let mut depth: i32 = 0;
        let mut i = self.pos;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    depth += 1;
                }
                TokenKind::RParen | TokenKind::RBracket => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                TokenKind::RBrace => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                TokenKind::Bar if depth == 0 => return true,
                TokenKind::Equals if depth == 0 => return false,
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_record_field_bindings(&mut self) -> Result<Vec<(String, Expr)>, ParseError> {
        let mut out = Vec::new();
        loop {
            let (name, _) = self.expect_lower_ident()?;
            self.expect(&TokenKind::Equals, "`=`")?;
            let value = self.parse_expr()?;
            out.push((name, value));
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.bump();
                continue;
            }
            break;
        }
        Ok(out)
    }
}

// ===================================================================
//  Operator table
// ===================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Assoc {
    Left,
    Right,
    None,
}

#[derive(Debug, Clone, Copy)]
struct OpInfo {
    prec: u8,
    assoc: Assoc,
}

/// Operator table for the parser. Tiers mirror spec 05 §Operator
/// table exactly; `>>=`/`>>` from spec 07 are added at tier 1 left.
fn operator_info(sym: &str) -> Option<OpInfo> {
    match sym {
        "||" => Some(OpInfo {
            prec: 2,
            assoc: Assoc::Right,
        }),
        "&&" => Some(OpInfo {
            prec: 3,
            assoc: Assoc::Right,
        }),
        "==" | "/=" | "<" | ">" | "<=" | ">=" => Some(OpInfo {
            prec: 4,
            assoc: Assoc::None,
        }),
        "++" | "::" => Some(OpInfo {
            prec: 5,
            assoc: Assoc::Right,
        }),
        "+" | "-" => Some(OpInfo {
            prec: 6,
            assoc: Assoc::Left,
        }),
        "*" | "/" | "%" => Some(OpInfo {
            prec: 7,
            assoc: Assoc::Left,
        }),
        ">>=" | ">>" => Some(OpInfo {
            prec: 1,
            assoc: Assoc::Left,
        }),
        _ => None,
    }
}

fn operator_info_for_token(kind: &TokenKind) -> Option<OpInfo> {
    let sym = op_token_symbol(kind)?;
    operator_info(&sym)
}

/// Convert an operator-ish token into its textual symbol.
fn op_token_symbol(kind: &TokenKind) -> Option<String> {
    Some(
        match kind {
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::EqEq => "==",
            TokenKind::SlashEq => "/=",
            TokenKind::Lt => "<",
            TokenKind::LtEq => "<=",
            TokenKind::Gt => ">",
            TokenKind::GtEq => ">=",
            TokenKind::AndAnd => "&&",
            TokenKind::OrOr => "||",
            TokenKind::PlusPlus => "++",
            TokenKind::DoubleColon => "::",
            TokenKind::Op(s) => s.as_str(),
            _ => return None,
        }
        .to_string(),
    )
}
