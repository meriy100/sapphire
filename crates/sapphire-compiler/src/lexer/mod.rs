//! Sapphire lexer.
//!
//! Produces a flat token stream from Sapphire source. Layout
//! processing (virtual `{`, `;`, `}` insertion per the Haskell-style
//! off-side rule) is intentionally *not* performed here; it is the
//! responsibility of a later pass. The lexer does emit [`Newline`]
//! and [`Indent`] tokens so that the layout pass has the information
//! it needs.
//!
//! [`Newline`]: TokenKind::Newline
//! [`Indent`]: TokenKind::Indent
//!
//! The normative spec is `docs/spec/02-lexical-syntax.md`. The
//! operator-token subset promoted into dedicated kinds comes from
//! `docs/spec/05-operators-and-numbers.md`. Rationale for the
//! implementation shape (hand-written recursive descent, independent
//! `Newline`/`Indent`, ADT errors) lives in `docs/impl/09-lexer.md`.

mod error;
mod token;

#[cfg(test)]
mod tests;

pub use error::{LexError, LexErrorKind};
pub use token::{Span, Token, TokenKind};

/// Lex the whole input into a flat token vector. Appends a final
/// [`TokenKind::Eof`].
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer::new(source);
    lexer.run()
}

/// Hand-written recursive-descent lexer.
///
/// The public entry point is [`tokenize`]; `Lexer` is kept `pub` so
/// downstream crates can drive the lexer incrementally if needed
/// (the current LSP story, see I-OQ9, does not require this, but it
/// is cheap to expose).
pub struct Lexer<'a> {
    /// Source bytes. Sapphire source is UTF-8 (spec 02 §Source text);
    /// we keep the raw byte slice for O(1) span arithmetic and
    /// decode code points on demand.
    src: &'a [u8],
    /// Current byte offset into `src`.
    pos: usize,
    /// Whether the next emitted non-newline, non-whitespace token
    /// starts a new logical line (and therefore needs an `Indent`).
    /// Set to `true` at construction so that the first token of the
    /// file carries an `Indent`.
    at_line_start: bool,
}

impl<'a> Lexer<'a> {
    /// Construct a fresh lexer over `source`.
    pub fn new(source: &'a str) -> Self {
        let src = source.as_bytes();
        // Skip a leading UTF-8 BOM per spec 02 §Source text.
        let pos = if src.starts_with(b"\xEF\xBB\xBF") {
            3
        } else {
            0
        };
        Self {
            src,
            pos,
            at_line_start: true,
        }
    }

    /// Drive the lexer to completion.
    pub fn run(&mut self) -> Result<Vec<Token>, LexError> {
        let mut out = Vec::new();
        loop {
            // Handle indentation at the start of a logical line.
            if self.at_line_start {
                match self.scan_line_start()? {
                    LineStart::Eof => {
                        out.push(Token::new(TokenKind::Eof, self.here()));
                        return Ok(out);
                    }
                    LineStart::Blank => {
                        // Entire line was whitespace + optional comment
                        // ending in `\n`. Emit a `Newline` and keep
                        // looping; stay `at_line_start`.
                        out.push(Token::new(TokenKind::Newline, self.prev_newline_span()));
                        continue;
                    }
                    LineStart::Token { indent_col } => {
                        out.push(Token::new(TokenKind::Indent(indent_col), self.here()));
                        self.at_line_start = false;
                    }
                }
            }

            // Skip in-line whitespace and comments. If we hit a
            // newline we loop back up to re-enter `scan_line_start`.
            if self.skip_inline_trivia()? {
                // Consumed a newline; emit `Newline` and restart.
                out.push(Token::new(TokenKind::Newline, self.prev_newline_span()));
                self.at_line_start = true;
                continue;
            }

            if self.pos >= self.src.len() {
                out.push(Token::new(TokenKind::Eof, self.here()));
                return Ok(out);
            }

            let tok = self.scan_token()?;
            out.push(tok);
        }
    }

    // ---------------------------------------------------------------
    //  Line-start handling
    // ---------------------------------------------------------------

    fn scan_line_start(&mut self) -> Result<LineStart, LexError> {
        // Count leading spaces, rejecting tabs. Skip blank lines and
        // line / block comments that occupy the whole line.
        let mut col: usize = 0;
        loop {
            if self.pos >= self.src.len() {
                return Ok(LineStart::Eof);
            }
            let c = self.src[self.pos];
            match c {
                b' ' => {
                    self.pos += 1;
                    col += 1;
                }
                b'\t' => {
                    return Err(LexError::new(
                        LexErrorKind::TabInLayoutPosition,
                        Span::new(self.pos, self.pos + 1),
                    ));
                }
                b'\r' => {
                    // CRLF → LF. A bare \r is an error.
                    if self.src.get(self.pos + 1) == Some(&b'\n') {
                        self.pos += 2;
                        return Ok(LineStart::Blank);
                    }
                    return Err(LexError::new(
                        LexErrorKind::BareCarriageReturn,
                        Span::new(self.pos, self.pos + 1),
                    ));
                }
                b'\n' => {
                    self.pos += 1;
                    return Ok(LineStart::Blank);
                }
                b'-' if self.src.get(self.pos + 1) == Some(&b'-')
                    && !is_op_char_byte(self.src.get(self.pos + 2).copied().unwrap_or(0)) =>
                {
                    // Line comment that starts the line: runs to \n.
                    self.skip_line_comment();
                    // Fall through at this position; next iteration
                    // will see `\n` or EOF.
                }
                b'{' if self.src.get(self.pos + 1) == Some(&b'-') => {
                    self.skip_block_comment()?;
                    // Block comments are whitespace. They may span
                    // newlines, so the current line's indent column
                    // is whatever lies between the last newline
                    // (or BOF) and the current position, counted in
                    // code points. Re-derive it rather than try to
                    // keep `col` in sync inside the comment walker.
                    col = self.column_since_line_start();
                }
                _ => {
                    return Ok(LineStart::Token { indent_col: col });
                }
            }
        }
    }

    // ---------------------------------------------------------------
    //  Inline trivia (spaces, tabs, comments) within a logical line
    // ---------------------------------------------------------------

    /// Skip whitespace and comments *within* a logical line. Returns
    /// `true` if a newline was consumed (caller should emit
    /// `Newline` and re-enter line-start scanning); `false` if the
    /// lexer is poised on a token or on EOF on the same logical
    /// line.
    fn skip_inline_trivia(&mut self) -> Result<bool, LexError> {
        loop {
            if self.pos >= self.src.len() {
                return Ok(false);
            }
            let c = self.src[self.pos];
            match c {
                b' ' | b'\t' => {
                    // Tabs are ordinary whitespace after the first
                    // non-whitespace token of a logical line.
                    self.pos += 1;
                }
                b'\r' => {
                    if self.src.get(self.pos + 1) == Some(&b'\n') {
                        self.pos += 2;
                        return Ok(true);
                    }
                    return Err(LexError::new(
                        LexErrorKind::BareCarriageReturn,
                        Span::new(self.pos, self.pos + 1),
                    ));
                }
                b'\n' => {
                    self.pos += 1;
                    return Ok(true);
                }
                b'-' if self.src.get(self.pos + 1) == Some(&b'-')
                    && !is_op_char_byte(self.src.get(self.pos + 2).copied().unwrap_or(0)) =>
                {
                    self.skip_line_comment();
                    // Line comment consumed up to but not including
                    // the newline; loop lets the `\n` branch catch it.
                }
                b'{' if self.src.get(self.pos + 1) == Some(&b'-') => {
                    self.skip_block_comment()?;
                }
                _ => return Ok(false),
            }
        }
    }

    fn skip_line_comment(&mut self) {
        // Caller has verified `src[pos..pos+2] == "--"` and that
        // the third byte is not an op_char, so this run really is a
        // comment per the maximal-munch rule (spec 02).
        self.pos += 2;
        while self.pos < self.src.len() {
            let c = self.src[self.pos];
            if c == b'\n' || c == b'\r' {
                break;
            }
            // Walk by whole UTF-8 code points to keep `pos` aligned.
            self.pos += utf8_len(c);
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let start = self.pos;
        self.pos += 2; // consume `{-`
        let mut depth: usize = 1;
        while depth > 0 {
            if self.pos >= self.src.len() {
                return Err(LexError::new(
                    LexErrorKind::UnterminatedBlockComment,
                    Span::new(start, self.src.len()),
                ));
            }
            let c = self.src[self.pos];
            if c == b'{' && self.src.get(self.pos + 1) == Some(&b'-') {
                depth += 1;
                self.pos += 2;
            } else if c == b'-' && self.src.get(self.pos + 1) == Some(&b'}') {
                depth -= 1;
                self.pos += 2;
            } else if c == b'\r' {
                if self.src.get(self.pos + 1) == Some(&b'\n') {
                    self.pos += 2;
                } else {
                    return Err(LexError::new(
                        LexErrorKind::BareCarriageReturn,
                        Span::new(self.pos, self.pos + 1),
                    ));
                }
            } else {
                self.pos += utf8_len(c);
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------
    //  Token scanning
    // ---------------------------------------------------------------

    fn scan_token(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let c = self.src[self.pos];
        match c {
            b'(' => self.punct1(TokenKind::LParen),
            b')' => self.punct1(TokenKind::RParen),
            b'[' => self.punct1(TokenKind::LBracket),
            b']' => self.punct1(TokenKind::RBracket),
            b'{' => self.punct1(TokenKind::LBrace),
            b'}' => self.punct1(TokenKind::RBrace),
            b',' => self.punct1(TokenKind::Comma),
            b';' => self.punct1(TokenKind::Semicolon),
            b'\\' => self.punct1(TokenKind::Backslash),

            b'"' => self.scan_string(),

            b'0'..=b'9' => self.scan_int(),

            b'_' => self.scan_underscore_or_ident(),

            b'a'..=b'z' => self.scan_lower_ident(),
            b'A'..=b'Z' => self.scan_upper_ident(),

            c if is_op_char_byte(c) => self.scan_op_run(),

            _ => {
                // Unknown byte. Decode a whole UTF-8 code point so the
                // error span covers the logical character, not just
                // the lead byte.
                let (ch, ch_len) = decode_utf8(&self.src[self.pos..]);
                let span = Span::new(start, start + ch_len);
                // Non-ASCII at the start of what would otherwise be
                // an identifier position is specifically called out
                // (02 §Identifiers / OQ5 DECIDED).
                let kind = if !ch.is_ascii() && (ch.is_alphabetic() || ch == '_') {
                    LexErrorKind::NonAsciiIdentStart
                } else {
                    LexErrorKind::UnexpectedChar(ch)
                };
                Err(LexError::new(kind, span))
            }
        }
    }

    fn punct1(&mut self, kind: TokenKind) -> Result<Token, LexError> {
        let start = self.pos;
        self.pos += 1;
        Ok(Token::new(kind, Span::new(start, self.pos)))
    }

    // ---------- Identifiers and keywords ---------------------------

    fn scan_underscore_or_ident(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.pos += 1;
        if self.pos < self.src.len() && is_ident_cont_byte(self.src[self.pos]) {
            // `_foo`, `_1` etc. are ordinary `lower_ident`s per
            // spec 02 §Identifiers (the regex class starts with
            // `[a-z_]`). Continue the identifier and classify.
            while self.pos < self.src.len() && is_ident_cont_byte(self.src[self.pos]) {
                self.pos += 1;
            }
            let span = Span::new(start, self.pos);
            let text = self.slice(span);
            return Ok(Token::new(TokenKind::LowerIdent(text), span));
        }
        Ok(Token::new(
            TokenKind::Underscore,
            Span::new(start, self.pos),
        ))
    }

    fn scan_lower_ident(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.src.len() && is_ident_cont_byte(self.src[self.pos]) {
            self.pos += 1;
        }
        let span = Span::new(start, self.pos);
        let text = self.slice(span);
        let kind = match text.as_str() {
            "module" => TokenKind::Module,
            "import" => TokenKind::Import,
            "hiding" => TokenKind::Hiding,
            "as" => TokenKind::As,
            "qualified" => TokenKind::Qualified,
            "export" => TokenKind::Export,
            "data" => TokenKind::Data,
            "type" => TokenKind::Type,
            "class" => TokenKind::Class,
            "instance" => TokenKind::Instance,
            "where" => TokenKind::Where,
            "let" => TokenKind::Let,
            "in" => TokenKind::In,
            "if" => TokenKind::If,
            "then" => TokenKind::Then,
            "else" => TokenKind::Else,
            "case" => TokenKind::Case,
            "of" => TokenKind::Of,
            "do" => TokenKind::Do,
            "forall" => TokenKind::Forall,
            _ => TokenKind::LowerIdent(text),
        };
        Ok(Token::new(kind, span))
    }

    fn scan_upper_ident(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.src.len() && is_ident_cont_byte(self.src[self.pos]) {
            self.pos += 1;
        }
        let span = Span::new(start, self.pos);
        let text = self.slice(span);
        Ok(Token::new(TokenKind::UpperIdent(text), span))
    }

    // ---------- Integer literals -----------------------------------

    fn scan_int(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        let mut digits = String::new();
        digits.push(self.src[self.pos] as char);
        self.pos += 1;
        while self.pos < self.src.len() {
            let c = self.src[self.pos];
            if c.is_ascii_digit() {
                digits.push(c as char);
                self.pos += 1;
            } else if c == b'_' {
                // Digit separator. Ignored in value computation.
                self.pos += 1;
            } else {
                break;
            }
        }
        let span = Span::new(start, self.pos);
        if digits.is_empty() {
            return Err(LexError::new(LexErrorKind::MalformedIntLiteral, span));
        }
        let value: i64 = digits
            .parse::<i64>()
            .map_err(|_| LexError::new(LexErrorKind::IntegerOverflow, span))?;
        Ok(Token::new(TokenKind::Int(value), span))
    }

    // ---------- String literals ------------------------------------

    fn scan_string(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.pos += 1; // opening `"`
        let mut buf = String::new();
        loop {
            if self.pos >= self.src.len() {
                return Err(LexError::new(
                    LexErrorKind::UnterminatedString,
                    Span::new(start, self.src.len()),
                ));
            }
            let c = self.src[self.pos];
            match c {
                b'"' => {
                    self.pos += 1;
                    return Ok(Token::new(TokenKind::Str(buf), Span::new(start, self.pos)));
                }
                b'\n' | b'\r' => {
                    return Err(LexError::new(
                        LexErrorKind::NewlineInString,
                        Span::new(self.pos, self.pos + 1),
                    ));
                }
                b'\\' => {
                    self.pos += 1;
                    self.scan_escape(&mut buf)?;
                }
                _ => {
                    // Copy the whole UTF-8 code point.
                    let n = utf8_len(c);
                    let end = (self.pos + n).min(self.src.len());
                    let slice = &self.src[self.pos..end];
                    // Safety: source is valid UTF-8.
                    let s = std::str::from_utf8(slice).expect("source is UTF-8");
                    buf.push_str(s);
                    self.pos = end;
                }
            }
        }
    }

    fn scan_escape(&mut self, buf: &mut String) -> Result<(), LexError> {
        if self.pos >= self.src.len() {
            return Err(LexError::new(
                LexErrorKind::UnterminatedString,
                Span::new(self.pos, self.pos),
            ));
        }
        let c = self.src[self.pos];
        let esc_start = self.pos - 1; // position of the leading `\`
        match c {
            b'n' => {
                buf.push('\n');
                self.pos += 1;
            }
            b't' => {
                buf.push('\t');
                self.pos += 1;
            }
            b'r' => {
                buf.push('\r');
                self.pos += 1;
            }
            b'\\' => {
                buf.push('\\');
                self.pos += 1;
            }
            b'"' => {
                buf.push('"');
                self.pos += 1;
            }
            b'u' => {
                self.pos += 1;
                self.scan_unicode_escape(esc_start, buf)?;
            }
            _ => {
                // Non-ASCII escape characters are possible (e.g.
                // `"\α"`); advance by the whole code point and
                // span the backslash + the code point.
                let (ch, ch_len) = decode_utf8(&self.src[self.pos..]);
                let span = Span::new(esc_start, self.pos + ch_len);
                self.pos += ch_len;
                return Err(LexError::new(LexErrorKind::UnknownEscape(ch), span));
            }
        }
        Ok(())
    }

    fn scan_unicode_escape(&mut self, esc_start: usize, buf: &mut String) -> Result<(), LexError> {
        if self.pos >= self.src.len() || self.src[self.pos] != b'{' {
            return Err(LexError::new(
                LexErrorKind::MalformedUnicodeEscape,
                Span::new(esc_start, self.pos),
            ));
        }
        self.pos += 1;
        let hex_start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_hexdigit() {
            self.pos += 1;
        }
        let hex_len = self.pos - hex_start;
        if !(1..=6).contains(&hex_len) {
            return Err(LexError::new(
                LexErrorKind::MalformedUnicodeEscape,
                Span::new(esc_start, self.pos),
            ));
        }
        if self.pos >= self.src.len() || self.src[self.pos] != b'}' {
            return Err(LexError::new(
                LexErrorKind::MalformedUnicodeEscape,
                Span::new(esc_start, self.pos),
            ));
        }
        let hex = std::str::from_utf8(&self.src[hex_start..self.pos]).expect("ASCII hex");
        self.pos += 1; // `}`
        let value = u32::from_str_radix(hex, 16).map_err(|_| {
            LexError::new(
                LexErrorKind::MalformedUnicodeEscape,
                Span::new(esc_start, self.pos),
            )
        })?;
        let ch = char::from_u32(value).ok_or_else(|| {
            LexError::new(
                LexErrorKind::InvalidUnicodeScalar(value),
                Span::new(esc_start, self.pos),
            )
        })?;
        buf.push(ch);
        Ok(())
    }

    // ---------- Operator runs --------------------------------------

    fn scan_op_run(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        while self.pos < self.src.len() && is_op_char_byte(self.src[self.pos]) {
            self.pos += 1;
        }
        let span = Span::new(start, self.pos);
        let text = self.slice(span);
        let kind = classify_op_run(&text);
        Ok(Token::new(kind, span))
    }

    // ---------------------------------------------------------------
    //  Helpers
    // ---------------------------------------------------------------

    fn slice(&self, span: Span) -> String {
        // Source is guaranteed UTF-8; token spans align to code-point
        // boundaries by construction.
        std::str::from_utf8(&self.src[span.start..span.end])
            .expect("source is UTF-8 and spans align to code points")
            .to_owned()
    }

    fn here(&self) -> Span {
        Span::new(self.pos, self.pos)
    }

    fn prev_newline_span(&self) -> Span {
        // The `\n` was just consumed; it sits at `pos - 1`.
        Span::new(self.pos.saturating_sub(1), self.pos)
    }

    /// Number of Unicode code points between the byte after the
    /// most recent `\n` (or start of input) and `self.pos`. Used
    /// to compute the indent column of the first token on a line
    /// when the run-up to it included a multi-line block comment.
    fn column_since_line_start(&self) -> usize {
        // Find the byte just after the previous `\n`; fall back to 0.
        let line_start = self.src[..self.pos]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        // Count code points. `from_utf8` is guaranteed to succeed on
        // a valid UTF-8 source sliced at a code-point boundary, and
        // newlines are code-point boundaries.
        std::str::from_utf8(&self.src[line_start..self.pos])
            .expect("source is UTF-8 and line boundaries align")
            .chars()
            .count()
    }
}

enum LineStart {
    Eof,
    Blank,
    Token { indent_col: usize },
}

// -------------------------------------------------------------------
//  Classification helpers
// -------------------------------------------------------------------

fn is_op_char_byte(b: u8) -> bool {
    matches!(
        b,
        b'+' | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'<'
            | b'>'
            | b'='
            | b'!'
            | b'&'
            | b'|'
            | b'^'
            | b'?'
            | b'~'
            | b':'
            | b'.'
    )
}

fn is_ident_cont_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

/// Map a maximal-munch `op_char+` run to its `TokenKind`.
///
/// Reserved punctuation (`=`, `->`, `=>`, `<-`, `|`, `:`, `::`,
/// `.`, `..`) and the spec-05 operator set are each promoted to a
/// dedicated variant. Everything else falls back to `Op(String)`.
/// `@` is *not* an `op_char` in spec 02 and is not reachable here.
fn classify_op_run(run: &str) -> TokenKind {
    match run {
        "=" => TokenKind::Equals,
        "->" => TokenKind::Arrow,
        "=>" => TokenKind::FatArrow,
        "<-" => TokenKind::LeftArrow,
        "|" => TokenKind::Bar,
        ":" => TokenKind::Colon,
        "::" => TokenKind::DoubleColon,
        "." => TokenKind::Dot,
        ".." => TokenKind::DotDot,

        "+" => TokenKind::Plus,
        "-" => TokenKind::Minus,
        "*" => TokenKind::Star,
        "/" => TokenKind::Slash,
        "%" => TokenKind::Percent,
        "==" => TokenKind::EqEq,
        "/=" => TokenKind::SlashEq,
        "<" => TokenKind::Lt,
        "<=" => TokenKind::LtEq,
        ">" => TokenKind::Gt,
        ">=" => TokenKind::GtEq,
        "&&" => TokenKind::AndAnd,
        "||" => TokenKind::OrOr,
        "++" => TokenKind::PlusPlus,

        other => TokenKind::Op(other.to_owned()),
    }
}

/// UTF-8 byte length for the code point whose lead byte is `b`.
///
/// The source is assumed to be valid UTF-8 (the `&str` API
/// guarantees it), so the continuation-byte branch is only hit if
/// a caller mis-aligns `pos`, which would already be a logic bug.
/// Returning `1` for both ASCII and an unexpected continuation byte
/// merely keeps `pos` advancing instead of looping.
fn utf8_len(b: u8) -> usize {
    if b < 0xC0 {
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// Decode the leading code point of `bytes`, returning the character
/// and the number of bytes consumed.
///
/// Assumes `bytes` starts at a code-point boundary (true because the
/// caller holds it via `&str`).
fn decode_utf8(bytes: &[u8]) -> (char, usize) {
    match std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.chars().next())
    {
        Some(ch) => (ch, ch.len_utf8()),
        None => ('\u{FFFD}', 1),
    }
}
