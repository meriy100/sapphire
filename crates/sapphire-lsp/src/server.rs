//! `tower-lsp` server implementation for Sapphire (L2 diagnostics).
//!
//! The server now wires enough of the protocol to publish parser /
//! lexer / layout diagnostics back to the editor:
//!
//! - `initialize` / `initialized` / `shutdown` respond with the
//!   minimum required by LSP 3.17.
//! - `textDocument/didOpen` / `didChange` / `didClose` drive an
//!   in-memory [`Document`] store keyed by `Url`. Every open or
//!   change runs the `sapphire_compiler::analyze` pipeline and
//!   publishes the resulting diagnostics via
//!   `textDocument/publishDiagnostics`. A `didClose` publishes an
//!   empty diagnostic set so stale markers disappear in the client.
//!
//! Richer capabilities (hover, goto-def, completion, …) land in
//! Track L's later milestones. Incremental sync is deferred as
//! I-OQ9; every change runs a full reparse for now.

use dashmap::DashMap;
use sapphire_compiler::analyze::{AnalysisResult, analyze};
use sapphire_compiler::error::CompileError;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

use crate::diagnostics::{build_line_map, compile_error_to_diagnostic};

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
                    TextDocumentSyncKind::FULL,
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

    /// Update the document store and publish the resulting
    /// diagnostics. Called from `did_open` / `did_change`.
    ///
    /// Order of operations is important: we insert into the document
    /// store **before** running analysis so a concurrent `did_change`
    /// cannot write back an older version on top of ours. We also
    /// check the stored version before publishing — if a newer change
    /// arrived while we were parsing, drop the stale diagnostics and
    /// let the newer `refresh` call publish its own.
    async fn refresh(&self, uri: Url, text: String, version: i32) {
        self.documents.insert(
            uri.clone(),
            Document {
                text: text.clone(),
                version,
            },
        );
        let diagnostics = Self::diagnostics_for(&text);
        let still_current = self
            .documents
            .get(&uri)
            .map(|entry| entry.version == version)
            .unwrap_or(false);
        if !still_current {
            // A newer change landed during analysis; skip publishing
            // these diagnostics — the newer refresh will publish its
            // own, and LSP requires the latest to win.
            return;
        }
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
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
        self.refresh(uri, params.text_document.text, version).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let version = params.text_document.version;
        tracing::info!(
            uri = %uri,
            version,
            changes = params.content_changes.len(),
            "textDocument/didChange",
        );
        // We advertise TextDocumentSyncKind::Full, so the client
        // always sends a single content change with the complete
        // document text. Pick the last one defensively (the spec
        // permits multiple entries; the last one is the final state
        // for Full sync).
        let Some(change) = params.content_changes.into_iter().next_back() else {
            return;
        };
        self.refresh(uri, change.text, version).await;
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
    use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString};

    /// Smoke test: `initialize_result` advertises the L1 capability
    /// surface (Full text-document sync, named `sapphire-lsp` with
    /// the crate version). This guards the invariant tested via
    /// `LanguageServer::initialize` without needing a mock client.
    #[tokio::test]
    async fn initialize_result_is_minimal_and_named() {
        let result = SapphireLanguageServer::initialize_result();

        let info = result.server_info.expect("server_info present");
        assert_eq!(info.name, "sapphire-lsp");
        assert_eq!(info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));

        match result.capabilities.text_document_sync {
            Some(TextDocumentSyncCapability::Kind(kind)) => {
                assert_eq!(kind, TextDocumentSyncKind::FULL);
            }
            other => panic!("expected Full text-document sync, got {other:?}"),
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
}
