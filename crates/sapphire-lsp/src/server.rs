//! `tower-lsp` server implementation for Sapphire (L3 incremental
//! document sync).
//!
//! The server now wires enough of the protocol to keep an
//! incrementally updated in-memory buffer and publish parser /
//! lexer / layout diagnostics back to the editor:
//!
//! - `initialize` / `initialized` / `shutdown` respond with the
//!   minimum required by LSP 3.17.
//! - `initialize` now advertises `TextDocumentSyncKind::INCREMENTAL`
//!   (upgraded from `Full` at L2). Clients may still fall back to a
//!   whole-document replacement by sending a change with no `range`,
//!   which the server handles transparently.
//! - `textDocument/didOpen` / `didChange` / `didClose` drive an
//!   in-memory [`Document`] store keyed by `Url`. Each `didChange`
//!   applies the incremental edits via [`crate::edit::apply_change`],
//!   stores the updated buffer, runs the `sapphire_compiler::analyze`
//!   pipeline, and publishes the resulting diagnostics via
//!   `textDocument/publishDiagnostics`. A `didClose` publishes an
//!   empty diagnostic set so stale markers disappear in the client.
//! - Document version monotonicity — LSP 3.17 mandates strictly
//!   increasing versions from the client. The store rejects
//!   `didChange` notifications whose version is not strictly greater
//!   than the stored one, and publishing is gated so a stale analysis
//!   cannot overwrite a newer one's diagnostics.
//!
//! Richer capabilities (hover, goto-def, completion, …) land in
//! Track L's later milestones. Reparse strategy stays full-reparse
//! even though text sync is incremental; see
//! `docs/impl/21-lsp-incremental-sync.md` §Why split text-sync from
//! reparse.

use dashmap::DashMap;
use sapphire_compiler::analyze::{AnalysisResult, analyze};
use sapphire_compiler::error::CompileError;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities,
    ServerInfo, TextDocumentContentChangeEvent, TextDocumentSyncCapability, TextDocumentSyncKind,
    Url,
};
use tower_lsp::{Client, LanguageServer};

use crate::diagnostics::{build_line_map, compile_error_to_diagnostic};
use crate::edit::{ApplyError, apply_change};

/// A single open text document as the server sees it.
///
/// Text is owned so that the analyzer can run without borrowing the
/// `DashMap` entry guard for the duration of a (potentially slow)
/// parse. `version` is the client-assigned monotonically increasing
/// version counter LSP requires us to echo back on
/// `publishDiagnostics`.
#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub version: i32,
}

/// The Sapphire language server.
///
/// Holds the `tower-lsp` client handle and an in-memory document
/// store. The client handle is used for `window/showMessage`-style
/// notifications and for `textDocument/publishDiagnostics`. The
/// document store is keyed by the client-assigned `Url` and is
/// updated on every `did_open` / `did_change`.
#[derive(Debug)]
pub struct SapphireLanguageServer {
    client: Client,
    documents: DashMap<Url, Document>,
}

impl SapphireLanguageServer {
    /// Create a new server bound to the given `tower-lsp` client
    /// handle. The handle is used for `window/showMessage`-style
    /// notifications and for `textDocument/publishDiagnostics`.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
        }
    }

    /// Build the `InitializeResult` advertised to the editor.
    ///
    /// Exposed so unit tests can assert the capability surface
    /// without spinning up a real LSP transport.
    pub fn initialize_result() -> InitializeResult {
        InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "sapphire-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        }
    }

    /// Run the front-end pipeline over `text` and project the
    /// resulting errors into LSP diagnostics.
    ///
    /// Exposed for tests — a real `tower-lsp::Client` is awkward to
    /// mock, so we keep the pure "source → diagnostics" function
    /// separately testable.
    pub fn diagnostics_for(text: &str) -> Vec<Diagnostic> {
        let AnalysisResult { errors, .. } = analyze(text);
        compile_errors_to_diagnostics(&errors, text)
    }

    /// Apply a sequence of incremental `didChange` edits on top of
    /// `existing` and return the resulting buffer plus any per-change
    /// error. Exposed for tests so the `did_change` race guard can
    /// stay in `refresh_incremental`.
    ///
    /// Each change is applied in order; the first [`ApplyError`]
    /// aborts the batch and the partially mutated buffer is returned
    /// alongside the error (the caller decides whether to commit
    /// anyway). See `docs/impl/21-lsp-incremental-sync.md`
    /// §Error handling for the rationale.
    pub fn apply_changes(
        existing: &str,
        changes: &[TextDocumentContentChangeEvent],
    ) -> (String, Option<ApplyError>) {
        let mut buf = existing.to_owned();
        for change in changes {
            if let Err(e) = apply_change(&mut buf, change) {
                return (buf, Some(e));
            }
        }
        (buf, None)
    }

    /// Run analysis against `text` and publish diagnostics for
    /// `(uri, version)` — but only if `version` still matches the
    /// currently stored version for `uri`. If a newer `did_change`
    /// landed while analysis was running, the newer refresh will
    /// publish its own diagnostics; publishing stale ones here would
    /// race the LSP "latest wins" contract.
    async fn analyze_and_publish(&self, uri: Url, text: String, version: i32) {
        let diagnostics = Self::diagnostics_for(&text);
        let still_current = self
            .documents
            .get(&uri)
            .map(|entry| entry.version == version)
            .unwrap_or(false);
        if !still_current {
            tracing::debug!(
                uri = %uri,
                version,
                "dropping stale diagnostics; newer version in store",
            );
            return;
        }
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }

    /// Open-path refresh: the client sent us a whole document and a
    /// starting version. Stored unconditionally (open replaces any
    /// previous state for the same URI).
    async fn refresh_full(&self, uri: Url, text: String, version: i32) {
        self.documents.insert(
            uri.clone(),
            Document {
                text: text.clone(),
                version,
            },
        );
        self.analyze_and_publish(uri, text, version).await;
    }

    /// Incremental-path refresh: apply `changes` on top of the
    /// currently stored buffer, store the new `(text, version)`,
    /// and publish diagnostics if we are still the latest writer.
    ///
    /// Monotonicity guard: if the current stored version is already
    /// ≥ `version` the batch is dropped. LSP 3.17 §TextDocumentItem
    /// requires clients to send strictly increasing versions on
    /// `didChange`; seeing an older version here is a client bug
    /// or a late reorder we must not "undo" by writing back.
    async fn refresh_incremental(
        &self,
        uri: Url,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
    ) {
        // Pull the current buffer out of the store. If we have no
        // record of this URI the client has sent `didChange`
        // without a prior `didOpen` — the LSP spec forbids this,
        // but we fall back to an empty buffer so a misbehaving
        // client does not hang the server.
        let starting_text = match self.documents.get(&uri) {
            Some(entry) => {
                if entry.version >= version {
                    tracing::warn!(
                        uri = %uri,
                        stored = entry.version,
                        incoming = version,
                        "dropping didChange: version not strictly increasing",
                    );
                    return;
                }
                entry.text.clone()
            }
            None => {
                tracing::warn!(
                    uri = %uri,
                    version,
                    "didChange for document we never saw a didOpen for; treating as empty"
                );
                String::new()
            }
        };

        let (new_text, err) = Self::apply_changes(&starting_text, &changes);
        if let Some(e) = err {
            tracing::warn!(uri = %uri, error = %e, "apply_change failed; dropping remaining changes in batch");
        }

        // Commit the new buffer + version. Do this *before* running
        // analysis so a concurrent `did_change` sees our write and
        // does not racily write an older version back on top.
        self.documents.insert(
            uri.clone(),
            Document {
                text: new_text.clone(),
                version,
            },
        );
        // Monotonicity invariant: whatever is stored after our
        // insert must be at least our version. A concurrent newer
        // writer is allowed to bump it further; a stale write must
        // never land below us. `analyze_and_publish` will drop our
        // diagnostics if the store moved past `version`.
        debug_assert!(
            self.documents
                .get(&uri)
                .map(|e| e.version >= version)
                .unwrap_or(false),
            "post-insert version regressed for {uri}"
        );

        self.analyze_and_publish(uri, new_text, version).await;
    }
}

/// Batch-render a slice of [`CompileError`]s into LSP diagnostics
/// against a shared [`LineMap`] built from `source`.
fn compile_errors_to_diagnostics(errors: &[CompileError], source: &str) -> Vec<Diagnostic> {
    let map = build_line_map(source);
    errors
        .iter()
        .map(|e| compile_error_to_diagnostic(e, &map))
        .collect()
}

#[tower_lsp::async_trait]
impl LanguageServer for SapphireLanguageServer {
    async fn initialize(&self, _params: InitializeParams) -> Result<InitializeResult> {
        tracing::info!("initialize received");
        Ok(Self::initialize_result())
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("initialized");
        self.client
            .log_message(MessageType::INFO, "sapphire-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("shutdown received");
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        tracing::info!(
            uri = %uri,
            version,
            language = %params.text_document.language_id,
            "textDocument/didOpen",
        );
        self.refresh_full(uri, params.text_document.text, version)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        tracing::trace!(
            uri = %uri,
            version,
            changes = params.content_changes.len(),
            "textDocument/didChange range-based",
        );
        self.refresh_incremental(uri, params.content_changes, version)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        tracing::info!(uri = %uri, "textDocument/didClose");
        self.documents.remove(&uri);
        // Clear any lingering diagnostics so the client doesn't keep
        // stale squiggles when the buffer is closed.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Position, Range};

    fn change_full(text: &str) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_owned(),
        }
    }

    fn change_range(
        sl: u32,
        sc: u32,
        el: u32,
        ec: u32,
        text: &str,
    ) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(sl, sc),
                end: Position::new(el, ec),
            }),
            range_length: None,
            text: text.to_owned(),
        }
    }

    /// Smoke test: `initialize_result` advertises the L3 capability
    /// surface (Incremental text-document sync, named `sapphire-lsp`
    /// with the crate version). This guards the invariant tested
    /// via `LanguageServer::initialize` without needing a mock
    /// client.
    #[tokio::test]
    async fn initialize_result_advertises_incremental_sync() {
        let result = SapphireLanguageServer::initialize_result();

        let info = result.server_info.expect("server_info present");
        assert_eq!(info.name, "sapphire-lsp");
        assert_eq!(info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

        match result.capabilities.text_document_sync {
            Some(TextDocumentSyncCapability::Kind(kind)) => {
                assert_eq!(kind, TextDocumentSyncKind::INCREMENTAL);
            }
            other => panic!("expected Incremental text-document sync, got {other:?}"),
        }
    }

    #[test]
    fn diagnostics_for_valid_source_is_empty() {
        // A single top-level let with a signature is the smallest
        // thing the pipeline consistently accepts.
        let src = "\
module M (x) where

x : Int
x = 1
";
        let diags = SapphireLanguageServer::diagnostics_for(src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn diagnostics_for_lex_error_reports_error_severity() {
        // Non-ASCII identifier start is a lex error per spec 02.
        let src = "module M where\n\nαβ = 1\n";
        let diags = SapphireLanguageServer::diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].source.as_deref(), Some("sapphire"));
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("sapphire/lex-error".to_owned()))
        );
    }

    #[test]
    fn diagnostics_for_parse_error_reports_parse_code() {
        // `data T` without `=` is a parse error.
        let src = "module M where\n\ndata T\n";
        let diags = SapphireLanguageServer::diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("sapphire/parse-error".to_owned()))
        );
    }

    #[test]
    fn apply_changes_full_replacement() {
        let (buf, err) = SapphireLanguageServer::apply_changes("old", &[change_full("new")]);
        assert!(err.is_none());
        assert_eq!(buf, "new");
    }

    #[test]
    fn apply_changes_two_inserts_compose() {
        // Starting buffer: "" — insert "hello" then " world".
        let (buf, err) = SapphireLanguageServer::apply_changes(
            "",
            &[
                change_range(0, 0, 0, 0, "hello"),
                change_range(0, 5, 0, 5, " world"),
            ],
        );
        assert!(err.is_none());
        assert_eq!(buf, "hello world");
    }

    #[test]
    fn apply_changes_mixed_full_then_range() {
        // Range=None clears the buffer, then a subsequent edit runs
        // against the replacement.
        let (buf, err) = SapphireLanguageServer::apply_changes(
            "stale",
            &[change_full("abc"), change_range(0, 1, 0, 2, "X")],
        );
        assert!(err.is_none());
        assert_eq!(buf, "aXc");
    }

    #[test]
    fn apply_changes_first_error_aborts_batch() {
        // Second edit is out-of-range; first still applies.
        let (buf, err) = SapphireLanguageServer::apply_changes(
            "abc",
            &[change_range(0, 1, 0, 2, "X"), change_range(9, 0, 9, 0, "Y")],
        );
        assert_eq!(buf, "aXc");
        assert_eq!(err, Some(ApplyError::StartOutOfRange));
    }
}
