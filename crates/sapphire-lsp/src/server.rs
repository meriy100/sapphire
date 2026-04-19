//! `tower-lsp` server implementation for Sapphire (L1 scaffold).
//!
//! Only the protocol skeleton is handled at this stage:
//!
//! - `initialize` / `initialized` / `shutdown` respond with the
//!   minimum required by LSP 3.17.
//! - `textDocument/didOpen` / `didChange` / `didClose` are logged but
//!   otherwise ignored. A document store, diagnostics and richer
//!   capabilities land in L2/L3.
//!
//! The advertised capabilities are intentionally minimal: only
//! `textDocumentSync = Full`. Incremental sync, hover, completion,
//! goto-definition etc. are broadened as later Track L milestones
//! (L3/L4/L5/L6) land.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, MessageType, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use tower_lsp::{Client, LanguageServer};

/// The Sapphire language server.
///
/// Holds the `tower-lsp` client handle so that later milestones can
/// push diagnostics and log messages to the editor. The L1 scaffold
/// does not itself publish any diagnostics.
#[derive(Debug)]
pub struct SapphireLanguageServer {
    client: Client,
}

impl SapphireLanguageServer {
    /// Create a new server bound to the given `tower-lsp` client
    /// handle. The handle is used for `window/showMessage`-style
    /// notifications and, later, `textDocument/publishDiagnostics`.
    pub fn new(client: Client) -> Self {
        Self { client }
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
        tracing::info!(
            uri = %params.text_document.uri,
            version = params.text_document.version,
            language = %params.text_document.language_id,
            "textDocument/didOpen",
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        tracing::info!(
            uri = %params.text_document.uri,
            version = params.text_document.version,
            changes = params.content_changes.len(),
            "textDocument/didChange",
        );
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        tracing::info!(
            uri = %params.text_document.uri,
            "textDocument/didClose",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
