//! Tokens and spans produced by the Sapphire lexer.
//!
//! The normative spec is `docs/spec/02-lexical-syntax.md` together
//! with the operator table from `docs/spec/05-operators-and-numbers.md`.
//! See `docs/impl/09-lexer.md` for the implementation-side rationale
//! (especially why `Newline` / `Indent` are independent tokens and
//! why an `Op` fallback kind exists alongside the spec-05 subset).

use std::fmt;

/// Byte-offset span into the source text.
///
/// Both `start` and `end` are inclusive–exclusive byte indices into
/// the UTF-8 source buffer. `end >= start` always holds; an empty
/// span (`start == end`) is used for synthetic tokens such as `Eof`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A lexed token with its source-byte span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The kind of a lexed token.
///
/// Variants mirror `docs/spec/02-lexical-syntax.md` (identifier
/// classes, keywords, literals, punctuation) and the operator
/// subset that `docs/spec/05-operators-and-numbers.md` promotes
/// out of the `op_char*` soup. Operator runs that are not in that
/// subset (e.g. `<>`, `:>`) are returned via [`TokenKind::Op`];
/// the parser is responsible for rejecting runs that are not
/// legal at the expression layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // --- Identifiers -------------------------------------------------
    /// `[a-z_][A-Za-z0-9_']*` not starting with a bare `_`.
    LowerIdent(String),
    /// `[A-Z][A-Za-z0-9_']*`.
    UpperIdent(String),
    /// Standalone `_` (wildcard pattern).
    Underscore,

    // --- Reserved words (spec 02 §Keywords) --------------------------
    Module,
    Import,
    Hiding,
    As,
    Qualified,
    Export,
    Data,
    Type,
    Class,
    Instance,
    Where,
    Let,
    In,
    If,
    Then,
    Else,
    Case,
    Of,
    Do,
    Forall,

    // --- Literals ----------------------------------------------------
    /// Decimal integer literal; underscores already stripped.
    Int(i64),
    /// String literal with escape sequences already decoded.
    Str(String),

    // --- Punctuation (spec 02 §Reserved punctuation + brackets) ------
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Dot,
    DotDot,
    Equals,      // `=`
    Arrow,       // `->`
    FatArrow,    // `=>`
    Bar,         // `|`
    DoubleColon, // `::`
    Colon,       // `:`
    LeftArrow,   // `<-`
    Backslash,   // `\`

    // --- Operators (spec 05 §Operator table) -------------------------
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    SlashEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    PlusPlus,

    /// Fallback for an `op_char+` run that is neither reserved
    /// punctuation nor one of the spec-05 operators. Kept so the
    /// lexer honours spec 02's maximal-munch rule even when the
    /// parser will later reject the token.
    Op(String),

    // --- Layout markers ---------------------------------------------
    /// Logical-line terminator (`\n` after CRLF normalisation).
    Newline,
    /// Column of the first non-whitespace token of a logical line.
    /// Columns are 0-based and count Unicode code points after any
    /// BOM has been stripped.
    Indent(usize),

    // --- End-of-input sentinel --------------------------------------
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::LowerIdent(s) => write!(f, "LowerIdent({s})"),
            TokenKind::UpperIdent(s) => write!(f, "UpperIdent({s})"),
            TokenKind::Underscore => write!(f, "Underscore"),
            TokenKind::Module => write!(f, "Module"),
            TokenKind::Import => write!(f, "Import"),
            TokenKind::Hiding => write!(f, "Hiding"),
            TokenKind::As => write!(f, "As"),
            TokenKind::Qualified => write!(f, "Qualified"),
            TokenKind::Export => write!(f, "Export"),
            TokenKind::Data => write!(f, "Data"),
            TokenKind::Type => write!(f, "Type"),
            TokenKind::Class => write!(f, "Class"),
            TokenKind::Instance => write!(f, "Instance"),
            TokenKind::Where => write!(f, "Where"),
            TokenKind::Let => write!(f, "Let"),
            TokenKind::In => write!(f, "In"),
            TokenKind::If => write!(f, "If"),
            TokenKind::Then => write!(f, "Then"),
            TokenKind::Else => write!(f, "Else"),
            TokenKind::Case => write!(f, "Case"),
            TokenKind::Of => write!(f, "Of"),
            TokenKind::Do => write!(f, "Do"),
            TokenKind::Forall => write!(f, "Forall"),
            TokenKind::Int(n) => write!(f, "Int({n})"),
            TokenKind::Str(s) => write!(f, "Str({s:?})"),
            TokenKind::LParen => write!(f, "LParen"),
            TokenKind::RParen => write!(f, "RParen"),
            TokenKind::LBracket => write!(f, "LBracket"),
            TokenKind::RBracket => write!(f, "RBracket"),
            TokenKind::LBrace => write!(f, "LBrace"),
            TokenKind::RBrace => write!(f, "RBrace"),
            TokenKind::Comma => write!(f, "Comma"),
            TokenKind::Semicolon => write!(f, "Semicolon"),
            TokenKind::Dot => write!(f, "Dot"),
            TokenKind::DotDot => write!(f, "DotDot"),
            TokenKind::Equals => write!(f, "Equals"),
            TokenKind::Arrow => write!(f, "Arrow"),
            TokenKind::FatArrow => write!(f, "FatArrow"),
            TokenKind::Bar => write!(f, "Bar"),
            TokenKind::DoubleColon => write!(f, "DoubleColon"),
            TokenKind::Colon => write!(f, "Colon"),
            TokenKind::LeftArrow => write!(f, "LeftArrow"),
            TokenKind::Backslash => write!(f, "Backslash"),
            TokenKind::Plus => write!(f, "Plus"),
            TokenKind::Minus => write!(f, "Minus"),
            TokenKind::Star => write!(f, "Star"),
            TokenKind::Slash => write!(f, "Slash"),
            TokenKind::Percent => write!(f, "Percent"),
            TokenKind::EqEq => write!(f, "EqEq"),
            TokenKind::SlashEq => write!(f, "SlashEq"),
            TokenKind::Lt => write!(f, "Lt"),
            TokenKind::LtEq => write!(f, "LtEq"),
            TokenKind::Gt => write!(f, "Gt"),
            TokenKind::GtEq => write!(f, "GtEq"),
            TokenKind::AndAnd => write!(f, "AndAnd"),
            TokenKind::OrOr => write!(f, "OrOr"),
            TokenKind::PlusPlus => write!(f, "PlusPlus"),
            TokenKind::Op(s) => write!(f, "Op({s})"),
            TokenKind::Newline => write!(f, "Newline"),
            TokenKind::Indent(col) => write!(f, "Indent({col})"),
            TokenKind::Eof => write!(f, "Eof"),
        }
    }
}
