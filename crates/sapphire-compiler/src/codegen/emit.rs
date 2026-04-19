//! Small helpers used by the Ruby emitter.
//!
//! These helpers do not know anything about Sapphire semantics; they
//! are purely mechanical bits that keep the rest of `codegen` terse:
//! string escaping for Ruby double-quoted literals, a simple
//! indentation accumulator, and identifier sanity checks.

/// Escape a Sapphire `String` value for emission inside a Ruby
/// double-quoted literal.
///
/// The input is already decoded (no `\n`-as-text escapes — those have
/// been turned into raw newlines by the lexer). We re-escape
/// newlines, carriage returns, tabs, backslashes, and double quotes
/// so that the emitted Ruby literal round-trips to the original
/// bytes. Other control characters pass through as-is; Ruby accepts
/// them in double-quoted strings.
pub fn escape_ruby_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '#' => out.push_str("\\#"),
            c => out.push(c),
        }
    }
    out
}

/// An indentation-aware buffer. Appending a line automatically
/// prefixes it with `indent * 2` spaces. Blocks can be opened and
/// closed to change the indentation level.
#[derive(Debug, Default)]
pub struct Buf {
    buf: String,
    indent: usize,
}

impl Buf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_line(&mut self, line: &str) {
        for _ in 0..self.indent {
            self.buf.push_str("  ");
        }
        self.buf.push_str(line);
        self.buf.push('\n');
    }

    /// Emit a blank line with no leading whitespace.
    pub fn blank(&mut self) {
        self.buf.push('\n');
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn dedent(&mut self) {
        debug_assert!(self.indent > 0, "dedent below zero");
        self.indent = self.indent.saturating_sub(1);
    }

    pub fn into_string(self) -> String {
        self.buf
    }
}

/// Emit a Ruby identifier for a Sapphire value binding.
///
/// For M9 scope (pure `lower_ident` top-level bindings), this is the
/// identity. Operator names and any mangling scheme (10-OQ1) are out
/// of scope; the compiler rejects user-defined operators at codegen
/// time by simply producing the raw name — the Ruby interpreter will
/// then complain if the name is not a valid method identifier, which
/// M9 sources never trigger.
pub fn value_ident(name: &str) -> String {
    name.to_string()
}
