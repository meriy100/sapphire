//! One-shot front-end analysis: lex → layout → parse.
//!
//! [`analyze`] glues the three front-end passes together and returns
//! an [`AnalysisResult`] that is convenient for IDE-style consumers
//! (the LSP server, `sapphire check`): it surfaces whichever stage
//! failed as a uniform [`CompileError`] without losing the AST if
//! the parser managed to produce one.
//!
//! ## Single-error-per-run, for now
//!
//! The first implementation deliberately **stops at the first error**.
//! The parser is strict — there is no error recovery pass yet — so
//! there is at most one entry in [`AnalysisResult::errors`]. This
//! mirrors the per-stage `Result<T, E>` APIs and keeps the shape
//! simple; once the parser grows a recovery mode, [`analyze`] can
//! return multiple errors without breaking its callers. See
//! `docs/impl/17-lsp-diagnostics.md` §Error recovery for the
//! rationale and I-OQ52 for the follow-up.

use sapphire_core::ast::Module;

use crate::error::CompileError;
use crate::{layout, lexer, parser};

/// Outcome of running [`analyze`] over a source string.
///
/// `module` is `Some` iff every front-end pass succeeded. When an
/// earlier pass fails `module` is `None` and `errors` carries a
/// single entry describing the failure. The two fields are
/// independently useful: consumers that only care about diagnostics
/// (e.g. LSP `publishDiagnostics`) can ignore `module`, and
/// consumers that only care about the AST (e.g. snapshot tests)
/// can assert `errors.is_empty()` and then unwrap `module`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResult {
    pub module: Option<Module>,
    pub errors: Vec<CompileError>,
}

impl AnalysisResult {
    /// True iff no front-end error was raised.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Run the lex / layout / parse pipeline end-to-end.
///
/// On success the returned `AnalysisResult` carries the parsed
/// module and an empty `errors` vector. On failure `errors` contains
/// a single `CompileError` whose span points into `source`, and
/// `module` is `None`.
pub fn analyze(source: &str) -> AnalysisResult {
    let tokens = match lexer::tokenize(source) {
        Ok(ts) => ts,
        Err(e) => {
            return AnalysisResult {
                module: None,
                errors: vec![CompileError::from_lex(e)],
            };
        }
    };

    let resolved = match layout::resolve_with_source(tokens, source) {
        Ok(ts) => ts,
        Err(e) => {
            return AnalysisResult {
                module: None,
                errors: vec![CompileError::from_layout(e)],
            };
        }
    };

    match parser::parse_tokens(&resolved) {
        Ok(module) => AnalysisResult {
            module: Some(module),
            errors: Vec::new(),
        },
        Err(e) => AnalysisResult {
            module: None,
            errors: vec![CompileError::from_parse(e)],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CompileErrorKind;

    const GOOD: &str = "\
module M (x) where

x : Int
x = 1
";

    #[test]
    fn analyze_ok_produces_module() {
        let result = analyze(GOOD);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        assert!(result.module.is_some());
    }

    #[test]
    fn analyze_reports_lex_error() {
        // A bare CR is a lex error per spec 02 §Source text.
        let src = "module M where\n\rx = 1\n";
        let result = analyze(src);
        assert!(!result.is_ok());
        assert!(result.module.is_none());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0].kind, CompileErrorKind::Lex(_)));
    }

    #[test]
    fn analyze_reports_parse_error() {
        // `data T` missing `=` is a parse error.
        let src = "module M where\n\ndata T\n";
        let result = analyze(src);
        assert!(!result.is_ok());
        assert!(result.module.is_none());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0].kind, CompileErrorKind::Parse(_)));
    }
}
