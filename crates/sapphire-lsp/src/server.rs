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
//! L4 adds `textDocument/hover`: the handler runs analyze →
//! resolve → typeck and returns a Markdown tooltip with the
//! inferred scheme for the identifier under the cursor. See
//! `docs/impl/28-lsp-hover.md` for the design notes.
//!
//! L6 adds `textDocument/completion`: the handler reuses the same
//! analyze → resolve → typeck pipeline and collects in-scope
//! identifiers (local binders, top-level names, prelude / imported
//! names, module qualifiers) into a list of `CompletionItem`s. See
//! `docs/impl/31-lsp-completion.md` for the design notes. Reparse
//! strategy stays full-reparse even though text sync is incremental;
//! see `docs/impl/21-lsp-incremental-sync.md` §Why split text-sync
//! from reparse.

use dashmap::DashMap;
use sapphire_compiler::analyze::{AnalysisResult, analyze};
use sapphire_compiler::error::CompileError;
use sapphire_compiler::resolver::resolve;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionOptions, CompletionParams, CompletionResponse, Diagnostic,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, MessageType, OneOf, Position,
    ServerCapabilities, ServerInfo, TextDocumentContentChangeEvent, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

use crate::completion::find_completion_items;
use crate::definition::find_definition;
use crate::diagnostics::{build_line_map, compile_error_to_diagnostic};
use crate::edit::{ApplyError, apply_change};
use crate::hover::{collect_hover_types, find_hover_info};

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
                // L5: advertise `textDocument/definition` support.
                // We return a `Location` (single site) rather than a
                // `LocationLink` array for now; see
                // `docs/impl/22-lsp-goto-definition.md` for the
                // rationale.
                definition_provider: Some(OneOf::Left(true)),
                // L4: advertise `textDocument/hover` support. The
                // handler runs analyze → resolve → typeck and
                // returns a Markdown-formatted type scheme tooltip.
                // See `docs/impl/28-lsp-hover.md`.
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // L6: advertise `textDocument/completion` support.
                // Clients trigger completion on every identifier
                // keypress by default; `trigger_characters` only
                // needs to list non-identifier characters that should
                // also pop the list. We add `.` for module-qualified
                // references (`Http.ma|` → propose `map`). Snippet
                // completion is covered by the L7 VSCode extension
                // `snippets/` bundle, not the server.
                // See `docs/impl/31-lsp-completion.md`.
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string()]),
                    ..CompletionOptions::default()
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "sapphire-lsp".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        }
    }

    /// Resolve a `(uri, position)` pair into a same-file definition
    /// site, if any. Exposed as a pure `&self` helper so tests can
    /// exercise the lookup without spinning up a real LSP client.
    ///
    /// Returns `None` when the document is not in the store, when
    /// `analyze` fails, when the resolver fails to produce a module
    /// env, when the position does not rest on a reference site, or
    /// when the reference resolves to a definition outside the
    /// current file (see `docs/impl/22-lsp-goto-definition.md`
    /// §Scope).
    pub fn resolve_position_to_location(&self, uri: &Url, position: Position) -> Option<Location> {
        let text = self.documents.get(uri).map(|e| e.text.clone())?;
        let analysis = analyze(&text);
        let module = analysis.module?;
        // The resolver may fail on this snapshot (e.g. an unrelated
        // unresolved name elsewhere in the file). When it does, we
        // drop goto rather than risk returning a bogus span — the
        // reference side table is the authoritative source, and we
        // lose it if `resolve` errors. A future enhancement could
        // keep partial reference information across resolve errors
        // (tracked in the design note); today we fall back to `None`.
        let resolved = resolve(module.clone()).ok()?;
        let line_map = build_line_map(&text);
        let byte_offset = line_map.byte_offset(position)?;
        let range = find_definition(&module, &resolved, &text, byte_offset, &line_map)?;
        Some(Location {
            uri: uri.clone(),
            range,
        })
    }

    /// Resolve a `(uri, position)` pair into a `Vec<CompletionItem>`
    /// by running the same `analyze → resolve → typeck` pipeline L4
    /// hover uses, then walking the resolver tables via
    /// [`crate::completion::find_completion_items`]. Exposed as a
    /// pure `&self` helper so tests can exercise the lookup without
    /// spinning up a real LSP client.
    ///
    /// Returns `None` when the document is not in the store or when
    /// analysis cannot produce a module. An empty `Vec` is a valid
    /// return (cursor on whitespace with no in-scope names); the
    /// caller is expected to forward it to the client as
    /// `Some(CompletionResponse::Array(vec![]))`.
    pub fn resolve_completion_at(
        &self,
        uri: &Url,
        position: Position,
    ) -> Option<Vec<CompletionItem>> {
        let text = self.documents.get(uri).map(|e| e.text.clone())?;
        let analysis = analyze(&text);
        let module = analysis.module?;
        let resolved = resolve(module.clone()).ok()?;
        let module_name = resolved.env.id.display();
        let typed = collect_hover_types(&module_name, &module);
        let line_map = build_line_map(&text);
        let byte_offset = line_map.byte_offset(position)?;
        Some(find_completion_items(
            &module,
            &resolved,
            &typed,
            &text,
            byte_offset,
        ))
    }

    /// Resolve a `(uri, position)` pair into an LSP [`Hover`] by
    /// running the front-end + resolver + typeck pipeline over the
    /// stored buffer. Exposed as a pure `&self` helper so tests can
    /// exercise the lookup without spinning up a real LSP client.
    ///
    /// Returns `None` when the document is not in the store, when
    /// `analyze` fails, when the resolver fails to produce a module
    /// env, or when the position does not rest on a recognised
    /// reference site. Typecheck errors are tolerated — partial
    /// scheme info is still useful for hover, so we do not gate the
    /// tooltip on a clean compile. See `docs/impl/28-lsp-hover.md`
    /// §Scope for the full rationale.
    pub fn resolve_position_to_hover(&self, uri: &Url, position: Position) -> Option<Hover> {
        let text = self.documents.get(uri).map(|e| e.text.clone())?;
        let analysis = analyze(&text);
        let module = analysis.module?;
        let resolved = resolve(module.clone()).ok()?;
        let module_name = resolved.env.id.display();
        let typed = collect_hover_types(&module_name, &module);
        let line_map = build_line_map(&text);
        let byte_offset = line_map.byte_offset(position)?;
        find_hover_info(&module, &resolved, &typed, &text, byte_offset, &line_map)
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

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let position = params.text_document_position_params.position;
        tracing::trace!(
            uri = %uri,
            line = position.line,
            character = position.character,
            "textDocument/definition",
        );
        let Some(location) = self.resolve_position_to_location(&uri, position) else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(location)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .clone();
        let position = params.text_document_position_params.position;
        tracing::trace!(
            uri = %uri,
            line = position.line,
            character = position.character,
            "textDocument/hover",
        );
        Ok(self.resolve_position_to_hover(&uri, position))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.clone();
        let position = params.text_document_position.position;
        tracing::trace!(
            uri = %uri,
            line = position.line,
            character = position.character,
            "textDocument/completion",
        );
        let items = self
            .resolve_completion_at(&uri, position)
            .unwrap_or_default();
        Ok(Some(CompletionResponse::Array(items)))
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

    /// Smoke test: `initialize_result` advertises the L3 / L4 / L5
    /// capability surface (Incremental text-document sync, hover
    /// provider, definition provider, named `sapphire-lsp` with the
    /// crate version). This guards the invariants tested via
    /// `LanguageServer::initialize` without needing a mock client.
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

        // L4: `textDocument/hover` advertised as a boolean
        // capability (no server-side registration options yet).
        match result.capabilities.hover_provider {
            Some(HoverProviderCapability::Simple(true)) => {}
            other => panic!("expected Simple(true) hover_provider, got {other:?}"),
        }

        // L6: `textDocument/completion` advertised with `.` as the
        // only explicit trigger character.
        let completion = result
            .capabilities
            .completion_provider
            .expect("completion_provider present");
        let triggers = completion
            .trigger_characters
            .expect("completion_provider.trigger_characters set");
        assert_eq!(
            triggers,
            vec![".".to_string()],
            "expected `.` as sole trigger char",
        );
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
