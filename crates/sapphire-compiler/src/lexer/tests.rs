//! Unit tests for the lexer. Covers each `TokenKind` variant at
//! least once, plus whitespace / newline / comment interactions and
//! the explicit error classes enumerated in `LexErrorKind`.

use super::error::LexErrorKind;
use super::token::TokenKind;
use super::{Token, tokenize};

/// Drop `Eof` from a token list for concise assertions.
fn kinds(tokens: &[Token]) -> Vec<TokenKind> {
    tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind.clone())
        .collect()
}

/// Drop `Newline` and `Indent` tokens as well; useful when a test
/// only cares about the lexical content, not the layout scaffolding.
fn content_kinds(tokens: &[Token]) -> Vec<TokenKind> {
    tokens
        .iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Eof | TokenKind::Newline | TokenKind::Indent(_)
            )
        })
        .map(|t| t.kind.clone())
        .collect()
}

// -------------------------------------------------------------------
//  Identifiers and keywords
// -------------------------------------------------------------------

#[test]
fn empty_input_is_only_eof() {
    let toks = tokenize("").unwrap();
    assert_eq!(toks.len(), 1);
    assert_eq!(toks[0].kind, TokenKind::Eof);
}

#[test]
fn lower_ident_accepted() {
    let toks = tokenize("foo").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("foo".into())]
    );
}

#[test]
fn lower_ident_with_prime_and_digits() {
    let toks = tokenize("x' foo_bar f1'").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::LowerIdent("x'".into()),
            TokenKind::LowerIdent("foo_bar".into()),
            TokenKind::LowerIdent("f1'".into()),
        ]
    );
}

#[test]
fn underscore_alone_is_wildcard() {
    let toks = tokenize("_").unwrap();
    assert_eq!(content_kinds(&toks), vec![TokenKind::Underscore]);
}

#[test]
fn underscore_prefix_is_lower_ident() {
    let toks = tokenize("_xs").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("_xs".into())]
    );
}

#[test]
fn upper_ident_accepted() {
    let toks = tokenize("Maybe True").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::UpperIdent("Maybe".into()),
            TokenKind::UpperIdent("True".into()),
        ]
    );
}

#[test]
fn reserved_words_all_recognised() {
    // Covers every word listed in spec 02 §Keywords (20 words).
    // Words that do not yet appear in any production (`forall`,
    // `qualified`, `export`) are still lexed as their reserved
    // kind so the "cannot appear as lower_ident" invariant holds.
    let src = "module import hiding as qualified export \
               data type class instance where let in if then else \
               case of do forall";
    let toks = tokenize(src).unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::Module,
            TokenKind::Import,
            TokenKind::Hiding,
            TokenKind::As,
            TokenKind::Qualified,
            TokenKind::Export,
            TokenKind::Data,
            TokenKind::Type,
            TokenKind::Class,
            TokenKind::Instance,
            TokenKind::Where,
            TokenKind::Let,
            TokenKind::In,
            TokenKind::If,
            TokenKind::Then,
            TokenKind::Else,
            TokenKind::Case,
            TokenKind::Of,
            TokenKind::Do,
            TokenKind::Forall,
        ]
    );
}

#[test]
fn exposing_is_lower_ident_not_reserved() {
    // spec 02 §Keywords does not list `exposing`. Guard against
    // accidental re-promotion to a reserved word.
    let toks = tokenize("exposing").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("exposing".to_string())]
    );
}

// -------------------------------------------------------------------
//  Integer literals
// -------------------------------------------------------------------

#[test]
fn int_literal_plain() {
    let toks = tokenize("0 42 1000").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::Int(0), TokenKind::Int(42), TokenKind::Int(1000)]
    );
}

#[test]
fn int_literal_with_underscores() {
    let toks = tokenize("1_000_000").unwrap();
    assert_eq!(content_kinds(&toks), vec![TokenKind::Int(1_000_000)]);
}

#[test]
fn int_literal_overflow_rejected() {
    let err = tokenize("99999999999999999999").unwrap_err();
    assert_eq!(err.kind, LexErrorKind::IntegerOverflow);
}

#[test]
fn negative_integer_tokenises_as_two_tokens() {
    // Spec 02 §Literals: negative integer literals are not a lexical
    // form; `-3` is two tokens. Spec 05 §Unary minus confirms the
    // parser-side disambiguation.
    let toks = tokenize("-3").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::Minus, TokenKind::Int(3)]
    );
}

// -------------------------------------------------------------------
//  String literals and escapes
// -------------------------------------------------------------------

#[test]
fn string_literal_simple() {
    let toks = tokenize(r#""hello""#).unwrap();
    assert_eq!(content_kinds(&toks), vec![TokenKind::Str("hello".into())]);
}

#[test]
fn string_literal_escapes() {
    let toks = tokenize(r#""a\nb\tc\\d\"e""#).unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::Str("a\nb\tc\\d\"e".into())]
    );
}

#[test]
fn string_literal_unicode_escape() {
    let toks = tokenize(r#""\u{1F600}""#).unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::Str("\u{1F600}".into())]
    );
}

#[test]
fn string_literal_newline_rejected() {
    let src = "\"abc\ndef\"";
    let err = tokenize(src).unwrap_err();
    assert_eq!(err.kind, LexErrorKind::NewlineInString);
}

#[test]
fn string_literal_unterminated_rejected() {
    let err = tokenize(r#""abc"#).unwrap_err();
    assert_eq!(err.kind, LexErrorKind::UnterminatedString);
}

#[test]
fn string_literal_unknown_escape_rejected() {
    let err = tokenize(r#""\q""#).unwrap_err();
    assert_eq!(err.kind, LexErrorKind::UnknownEscape('q'));
}

#[test]
fn string_literal_surrogate_rejected() {
    // U+D800 is a high surrogate and is not a valid scalar.
    let err = tokenize(r#""\u{D800}""#).unwrap_err();
    assert_eq!(err.kind, LexErrorKind::InvalidUnicodeScalar(0xD800));
}

#[test]
fn string_literal_malformed_unicode_escape_rejected() {
    let err = tokenize(r#""\u1234""#).unwrap_err();
    assert_eq!(err.kind, LexErrorKind::MalformedUnicodeEscape);
}

// -------------------------------------------------------------------
//  Punctuation and brackets
// -------------------------------------------------------------------

#[test]
fn punctuation_basics() {
    let toks = tokenize("( ) [ ] { } , ; \\").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Comma,
            TokenKind::Semicolon,
            TokenKind::Backslash,
        ]
    );
}

#[test]
fn reserved_punctuation_ops() {
    let toks = tokenize("= -> => <- | : :: . ..").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::Equals,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::LeftArrow,
            TokenKind::Bar,
            TokenKind::Colon,
            TokenKind::DoubleColon,
            TokenKind::Dot,
            TokenKind::DotDot,
        ]
    );
}

// -------------------------------------------------------------------
//  Operators
// -------------------------------------------------------------------

#[test]
fn spec_05_operator_set() {
    let toks = tokenize("+ - * / % == /= < <= > >= && || ++").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::EqEq,
            TokenKind::SlashEq,
            TokenKind::Lt,
            TokenKind::LtEq,
            TokenKind::Gt,
            TokenKind::GtEq,
            TokenKind::AndAnd,
            TokenKind::OrOr,
            TokenKind::PlusPlus,
        ]
    );
}

#[test]
fn maximal_munch_captures_unknown_op_run() {
    // `<>` is not in the spec-05 subset but is a valid `op_char+`
    // run at the lexer layer. Spec 02 §Operator tokens requires
    // maximal munch; the parser will reject it if expression
    // position appears.
    let toks = tokenize("<>").unwrap();
    assert_eq!(content_kinds(&toks), vec![TokenKind::Op("<>".into())]);
}

#[test]
fn monadic_bind_operators_fall_back_to_op_variant() {
    // `>>=` and `>>` appear in spec 05 §Operator table but are
    // bound to the `Monad` class at M7. Until dedicated variants
    // land, the lexer hands them to the parser as `Op`.
    let toks = tokenize(">>= >>").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::Op(">>=".into()), TokenKind::Op(">>".into())]
    );
}

#[test]
fn comment_punch_beats_line_comment_when_op_run_extends() {
    // `-->` is a single operator token, not `-` then `-`-comment.
    // Spec 02 §Whitespace and comments / maximal munch.
    let toks = tokenize("a --> b").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::LowerIdent("a".into()),
            TokenKind::Op("-->".into()),
            TokenKind::LowerIdent("b".into()),
        ]
    );
}

#[test]
fn plain_double_dash_starts_line_comment() {
    let toks = tokenize("a -- trailing comment\nb").unwrap();
    // Expected: Indent(0), a, Newline, Indent(0), b, Eof
    let ks = kinds(&toks);
    assert_eq!(
        ks,
        vec![
            TokenKind::Indent(0),
            TokenKind::LowerIdent("a".into()),
            TokenKind::Newline,
            TokenKind::Indent(0),
            TokenKind::LowerIdent("b".into()),
        ]
    );
}

// -------------------------------------------------------------------
//  Layout markers
// -------------------------------------------------------------------

#[test]
fn first_token_carries_indent_zero() {
    let toks = tokenize("foo").unwrap();
    assert_eq!(toks[0].kind, TokenKind::Indent(0));
    assert_eq!(toks[1].kind, TokenKind::LowerIdent("foo".into()));
}

#[test]
fn newline_and_indent_emitted_across_lines() {
    let src = "foo\n  bar";
    let toks = tokenize(src).unwrap();
    let ks = kinds(&toks);
    assert_eq!(
        ks,
        vec![
            TokenKind::Indent(0),
            TokenKind::LowerIdent("foo".into()),
            TokenKind::Newline,
            TokenKind::Indent(2),
            TokenKind::LowerIdent("bar".into()),
        ]
    );
}

#[test]
fn blank_lines_emit_newlines_each() {
    let src = "foo\n\n\nbar";
    let toks = tokenize(src).unwrap();
    let ks = kinds(&toks);
    assert_eq!(
        ks,
        vec![
            TokenKind::Indent(0),
            TokenKind::LowerIdent("foo".into()),
            TokenKind::Newline,
            TokenKind::Newline,
            TokenKind::Newline,
            TokenKind::Indent(0),
            TokenKind::LowerIdent("bar".into()),
        ]
    );
}

#[test]
fn crlf_normalises_to_lf() {
    let toks = tokenize("foo\r\nbar").unwrap();
    let ks = kinds(&toks);
    assert_eq!(
        ks,
        vec![
            TokenKind::Indent(0),
            TokenKind::LowerIdent("foo".into()),
            TokenKind::Newline,
            TokenKind::Indent(0),
            TokenKind::LowerIdent("bar".into()),
        ]
    );
}

#[test]
fn bare_cr_rejected() {
    let err = tokenize("foo\rbar").unwrap_err();
    assert_eq!(err.kind, LexErrorKind::BareCarriageReturn);
}

#[test]
fn bom_is_skipped() {
    let mut src = String::new();
    src.push('\u{FEFF}');
    src.push_str("foo");
    let toks = tokenize(&src).unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("foo".into())]
    );
}

// -------------------------------------------------------------------
//  Comments
// -------------------------------------------------------------------

#[test]
fn line_comment_skipped() {
    let toks = tokenize("-- just a comment\nfoo").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("foo".into())]
    );
}

#[test]
fn block_comment_skipped() {
    let toks = tokenize("{- ignore me -}foo").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("foo".into())]
    );
}

#[test]
fn nested_block_comments_skipped() {
    let toks = tokenize("{- outer {- inner -} still outer -}foo").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::LowerIdent("foo".into())]
    );
}

#[test]
fn block_comment_spans_lines() {
    let src = "foo {- mid\n  still comment\n-} bar";
    let toks = tokenize(src).unwrap();
    // The newlines inside the block comment must NOT produce
    // Newline/Indent tokens — comments behave as whitespace.
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::LowerIdent("foo".into()),
            TokenKind::LowerIdent("bar".into()),
        ]
    );
}

#[test]
fn unterminated_block_comment_rejected() {
    let err = tokenize("{- oops").unwrap_err();
    assert_eq!(err.kind, LexErrorKind::UnterminatedBlockComment);
}

#[test]
fn indent_column_accounts_for_inline_block_comment() {
    // The first non-whitespace token on the line is `foo`. Per
    // spec 02 §Layout its column is measured from the line start,
    // so the leading spaces plus the block-comment body contribute
    // to the indent column.
    let src = "  {- x -} foo";
    let toks = tokenize(src).unwrap();
    let ks = kinds(&toks);
    assert_eq!(
        ks,
        vec![TokenKind::Indent(10), TokenKind::LowerIdent("foo".into()),]
    );
}

#[test]
fn indent_column_after_multiline_block_comment_is_current_line_only() {
    // After a block comment that spans newlines, the indent column
    // is that of the *post-comment* line — spec 02 §Layout treats
    // the comment as whitespace.
    let src = "{- first\n  second -}   foo";
    let toks = tokenize(src).unwrap();
    let ks = kinds(&toks);
    assert_eq!(
        ks,
        vec![TokenKind::Indent(14), TokenKind::LowerIdent("foo".into()),]
    );
}

// -------------------------------------------------------------------
//  Layout-position tabs and non-ASCII identifiers
// -------------------------------------------------------------------

#[test]
fn tab_at_line_start_rejected() {
    let err = tokenize("\tfoo").unwrap_err();
    assert_eq!(err.kind, LexErrorKind::TabInLayoutPosition);
}

#[test]
fn tab_after_first_token_is_whitespace() {
    let toks = tokenize("foo\tbar").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::LowerIdent("foo".into()),
            TokenKind::LowerIdent("bar".into()),
        ]
    );
}

#[test]
fn non_ascii_ident_start_rejected() {
    // Greek lowercase alpha — would start a Unicode lower_ident
    // under a future extension, but is not admitted here.
    let err = tokenize("α").unwrap_err();
    assert_eq!(err.kind, LexErrorKind::NonAsciiIdentStart);
}

#[test]
fn non_ascii_inside_string_is_fine() {
    let toks = tokenize("\"こんにちは\"").unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![TokenKind::Str("こんにちは".into())]
    );
}

// -------------------------------------------------------------------
//  Realistic snippet
// -------------------------------------------------------------------

#[test]
fn realistic_module_snippet() {
    // spec 08 module syntax: `module Name (exports) where`.
    let src = "module Main (main) where\n\nmain = putStrLn \"Hello!\"\n";
    let toks = tokenize(src).unwrap();
    assert_eq!(
        content_kinds(&toks),
        vec![
            TokenKind::Module,
            TokenKind::UpperIdent("Main".into()),
            TokenKind::LParen,
            TokenKind::LowerIdent("main".into()),
            TokenKind::RParen,
            TokenKind::Where,
            TokenKind::LowerIdent("main".into()),
            TokenKind::Equals,
            TokenKind::LowerIdent("putStrLn".into()),
            TokenKind::Str("Hello!".into()),
        ]
    );
}

// -------------------------------------------------------------------
//  Span sanity checks
// -------------------------------------------------------------------

#[test]
fn spans_cover_token_text() {
    let src = "foo bar";
    let toks = tokenize(src).unwrap();
    let content: Vec<&Token> = toks
        .iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Eof | TokenKind::Newline | TokenKind::Indent(_)
            )
        })
        .collect();
    assert_eq!(content.len(), 2);
    assert_eq!(&src[content[0].span.start..content[0].span.end], "foo");
    assert_eq!(&src[content[1].span.start..content[1].span.end], "bar");
}
