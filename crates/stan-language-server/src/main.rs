use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use stan_language_server::document::{Document, DocumentStore};
use stan_language_server::features;
use stan_language_server::stanc::StancRunner;
use stan_language_server::workspace::{ServerConfig, Workspace};
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tower_lsp_server::jsonrpc::{Error, Result};
use tower_lsp_server::ls_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CallHierarchyServerCapability, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, CompletionOptions, CompletionParams, CompletionResponse,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentFormattingParams,
    DocumentHighlight, DocumentHighlightParams, DocumentRangeFormattingParams,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeParams,
    FoldingRangeProviderCapability, GotoDefinitionParams, GotoDefinitionResponse, Hover,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, InlayHint,
    InlayHintParams, Location, OneOf, PositionEncodingKind, ReferenceParams, RenameParams,
    SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions, SemanticTokensLegend,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SignatureHelp,
    SignatureHelpOptions, SignatureHelpParams, SymbolInformation, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, TextEdit, WorkDoneProgressOptions, WorkspaceEdit,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tracing_subscriber::EnvFilter;

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: Arc<RwLock<DocumentStore>>,
    stanc: Arc<RwLock<Option<StancRunner>>>,
    config: Arc<RwLock<ServerConfig>>,
    workspace: Arc<RwLock<Workspace>>,
    compiler_tasks: Arc<Mutex<HashMap<tower_lsp_server::ls_types::Uri, JoinHandle<()>>>>,
    compiler_version: Arc<RwLock<Option<String>>>,
    metrics: Arc<Metrics>,
}

#[derive(Debug, Default)]
struct Metrics {
    analysis_runs: AtomicU64,
    compiler_runs: AtomicU64,
    compiler_cancellations: AtomicU64,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentStore::default())),
            stanc: Arc::new(RwLock::new(StancRunner::discover())),
            config: Arc::new(RwLock::new(ServerConfig::default())),
            workspace: Arc::new(RwLock::new(Workspace::default())),
            compiler_tasks: Arc::new(Mutex::new(HashMap::new())),
            compiler_version: Arc::new(RwLock::new(None)),
            metrics: Arc::new(Metrics::default()),
        }
    }

    async fn publish(&self, document: &Document) {
        self.metrics.analysis_runs.fetch_add(1, Ordering::Relaxed);
        let config = self.config.read().await.clone();
        let mut published = stan_language_server::diagnostics::diagnostics_with_lints(
            document,
            &config.lint_config(),
        );
        if let Some(path) = document.uri.to_file_path() {
            let workspace = self.workspace.read().await;
            let (_, mut include_diagnostics) =
                workspace.resolve_includes(&path, &document.text, &config);
            if let Some(cycle) = workspace
                .include_cycles()
                .into_iter()
                .find(|cycle| cycle.iter().any(|entry| entry == path.as_ref()))
            {
                include_diagnostics.push(stan_language::Diagnostic::error(
                    "workspace.include-cycle",
                    format!(
                        "include cycle: {}",
                        cycle
                            .iter()
                            .map(|entry| entry.display().to_string())
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    ),
                    stan_language::TextRange::new(0, 0),
                ));
            }
            published.extend(include_diagnostics.into_iter().filter_map(|diagnostic| {
                stan_language_server::diagnostics::to_lsp(document, diagnostic)
            }));
        }
        self.client
            .publish_diagnostics(document.uri.clone(), published, Some(document.version))
            .await;
    }

    async fn document(&self, uri: &tower_lsp_server::ls_types::Uri) -> Option<Document> {
        self.documents.read().await.get(uri).cloned()
    }

    async fn schedule_compiler(&self, document: Document) {
        let config = self.config.read().await.clone();
        if !config.compiler_on_change {
            return;
        }
        let Some(runner) = self.stanc.read().await.clone() else {
            return;
        };
        let Some(path) = document.uri.to_file_path().map(|path| path.into_owned()) else {
            return;
        };
        let uri = document.uri.clone();
        let version = document.version;
        let source = document.text.clone();
        let documents = self.documents.clone();
        let client = self.client.clone();
        let metrics = self.metrics.clone();
        let lint_config = config.lint_config();
        let debounce = config.compiler_debounce();
        let task = tokio::spawn(async move {
            tokio::time::sleep(debounce).await;
            metrics.compiler_runs.fetch_add(1, Ordering::Relaxed);
            let Ok(compiler_diagnostics) = runner.check_source(&path, &source).await else {
                return;
            };
            let current = documents.read().await.get(&uri).cloned();
            let Some(current) = current.filter(|current| current.version == version) else {
                return;
            };
            let diagnostics = stan_language_server::diagnostics::with_compiler_and_lints(
                &current,
                compiler_diagnostics,
                &lint_config,
            );
            client
                .publish_diagnostics(uri, diagnostics, Some(version))
                .await;
        });
        if let Some(previous) = self.compiler_tasks.lock().await.insert(document.uri, task) {
            previous.abort();
            self.metrics
                .compiler_cancellations
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let config = params
            .initialization_options
            .and_then(|options| serde_json::from_value::<ServerConfig>(options).ok())
            .unwrap_or_default();
        let version = config
            .stan_version
            .parse::<stan_language::StanVersion>()
            .map_err(|error| Error::invalid_params(error.to_string()))?;
        if stan_language::FunctionCatalog::for_version(version).is_none() {
            return Err(Error::invalid_params(format!(
                "Stan catalog {} is not embedded; supported versions: {}",
                version,
                stan_language::FunctionCatalog::supported_versions()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let mut roots = params
            .workspace_folders
            .unwrap_or_default()
            .into_iter()
            .filter_map(|folder| folder.uri.to_file_path().map(|path| path.into_owned()))
            .collect::<Vec<_>>();
        #[allow(deprecated)]
        if roots.is_empty() {
            if let Some(path) = params
                .root_uri
                .and_then(|uri| uri.to_file_path().map(|path| path.into_owned()))
            {
                roots.push(path);
            }
        }
        if let Some(path) = config
            .stanc_path
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
        {
            *self.stanc.write().await = Some(StancRunner::with_path(path));
        }
        self.workspace.write().await.set_roots(roots, &config);
        *self.config.write().await = config.clone();
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF16),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..TextDocumentSyncOptions::default()
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                    retrigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    SemanticTokenType::KEYWORD,
                                    SemanticTokenType::TYPE,
                                    SemanticTokenType::FUNCTION,
                                    SemanticTokenType::VARIABLE,
                                    SemanticTokenType::OPERATOR,
                                    SemanticTokenType::NUMBER,
                                    SemanticTokenType::STRING,
                                    SemanticTokenType::COMMENT,
                                    SemanticTokenType::MACRO,
                                ],
                                token_modifiers: vec![SemanticTokenModifier::DEPRECATED],
                            },
                            range: None,
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                experimental: Some(serde_json::json!({
                    "stanCatalogVersion": config.stan_version,
                    "nativeFormatter": true,
                    "stanlint": true,
                    "stancIntegration": self.stanc.read().await.is_some()
                })),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "stan-language-server".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            offset_encoding: None,
        })
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!(
            analysis_runs = self.metrics.analysis_runs.load(Ordering::Relaxed),
            compiler_runs = self.metrics.compiler_runs.load(Ordering::Relaxed),
            compiler_cancellations = self.metrics.compiler_cancellations.load(Ordering::Relaxed),
            "Stan LSP session metrics"
        );
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let item = params.text_document;
        let document = Document::new(item.uri, item.version, item.text);
        self.documents.write().await.open(document.clone());
        if let Some(path) = document.uri.to_file_path() {
            let config = self.config.read().await.clone();
            self.workspace.write().await.set_overlay(
                path.into_owned(),
                document.text.clone(),
                &config,
            );
        }
        self.publish(&document).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let identifier = params.text_document;
        let [change] = params.content_changes.as_slice() else {
            tracing::warn!("ignoring change that is not one full-document replacement");
            return;
        };
        if change.range.is_some() {
            tracing::warn!("ignoring incremental change while full synchronization is active");
            return;
        }
        let document = {
            let mut documents = self.documents.write().await;
            documents
                .replace(&identifier.uri, identifier.version, change.text.clone())
                .cloned()
        };
        if let Some(document) = document {
            if let Some(path) = document.uri.to_file_path() {
                let config = self.config.read().await.clone();
                self.workspace.write().await.set_overlay(
                    path.into_owned(),
                    document.text.clone(),
                    &config,
                );
            }
            self.publish(&document).await;
            self.schedule_compiler(document).await;
        } else {
            tracing::warn!("ignoring stale change or change for an unopened document");
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.close(&uri);
        if let Some(task) = self.compiler_tasks.lock().await.remove(&uri) {
            task.abort();
            self.metrics
                .compiler_cancellations
                .fetch_add(1, Ordering::Relaxed);
        }
        if let Some(path) = uri.to_file_path() {
            let config = self.config.read().await.clone();
            self.workspace.write().await.clear_overlay(&path, &config);
        }
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(task) = self.compiler_tasks.lock().await.remove(&uri) {
            task.abort();
            self.metrics
                .compiler_cancellations
                .fetch_add(1, Ordering::Relaxed);
        }
        if !self.config.read().await.compiler_on_save {
            return;
        }
        let Some(runner) = self.stanc.read().await.clone() else {
            return;
        };
        let Some(document) = self.document(&uri).await else {
            return;
        };
        let Some(path) = uri.to_file_path() else {
            return;
        };
        self.metrics.compiler_runs.fetch_add(1, Ordering::Relaxed);
        let compiler_version = if let Some(version) = self.compiler_version.read().await.clone() {
            Some(version)
        } else {
            match runner.version().await {
                Ok(version) => {
                    *self.compiler_version.write().await = Some(version.clone());
                    Some(version)
                }
                Err(error) => {
                    tracing::warn!(%error, "could not determine stanc version");
                    None
                }
            }
        };
        match runner.check(&path).await {
            Ok(mut compiler_diagnostics) => {
                let configured = self.config.read().await.stan_version.clone();
                if compiler_version
                    .as_deref()
                    .is_some_and(|version| !version.contains(&configured))
                {
                    compiler_diagnostics.push(stan_language::Diagnostic {
                        code: stan_language::DiagnosticCode("stanc3.version-mismatch"),
                        severity: stan_language::Severity::Information,
                        message: format!(
                            "catalog targets Stan {configured}, but compiler reports {}",
                            compiler_version.as_deref().unwrap_or_default()
                        ),
                        primary_range: stan_language::TextRange::new(0, 0),
                        related: Vec::new(),
                        fixes: Vec::new(),
                        source: stan_language::DiagnosticSource::Stanc3,
                    });
                }
                let Some(current) = self.document(&uri).await else {
                    return;
                };
                if current.version != document.version {
                    return;
                }
                let diagnostics = stan_language_server::diagnostics::with_compiler_and_lints(
                    &current,
                    compiler_diagnostics,
                    &self.config.read().await.lint_config(),
                );
                self.client
                    .publish_diagnostics(uri, diagnostics, Some(current.version))
                    .await;
            }
            Err(error) => tracing::warn!(%error, "stanc validation unavailable"),
        }
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let settings = params
            .settings
            .get("stanLsp")
            .cloned()
            .unwrap_or(params.settings);
        let Ok(config) = serde_json::from_value::<ServerConfig>(settings) else {
            tracing::warn!("ignoring invalid Stan LSP configuration");
            return;
        };
        *self.stanc.write().await = config
            .stanc_path
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
            .map(StancRunner::with_path)
            .or_else(StancRunner::discover);
        *self.compiler_version.write().await = None;
        self.workspace.write().await.reindex(&config);
        *self.config.write().await = config;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        Ok(self
            .document(&params.text_document.uri)
            .await
            .as_ref()
            .map(features::document_symbols))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        Ok(self
            .document(&params.text_document.uri)
            .await
            .as_ref()
            .map(features::folding_ranges))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        Ok(self
            .document(&params.text_document.uri)
            .await
            .as_ref()
            .map(features::semantic_tokens))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let params = params.text_document_position_params;
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, params.position)
        else {
            return Ok(None);
        };
        Ok(features::hover(&document, offset))
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let config = self.config.read().await.formatter_config();
        Ok(features::formatting_with_config(&document, &config))
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let config = self.config.read().await.formatter_config();
        Ok(features::range_formatting_with_config(
            &document,
            params.range,
            &config,
        ))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let params = params.text_document_position;
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, params.position)
        else {
            return Ok(None);
        };
        Ok(Some(features::completion(&document, offset)))
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let params = params.text_document_position_params;
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, params.position)
        else {
            return Ok(None);
        };
        Ok(features::signature_help(&document, offset))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let params = params.text_document_position_params;
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, params.position)
        else {
            return Ok(None);
        };
        Ok(features::goto_definition(&document, offset))
    }

    async fn references(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<tower_lsp_server::ls_types::Location>>> {
        let position = params.text_document_position;
        let Some(document) = self.document(&position.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, position.position)
        else {
            return Ok(None);
        };
        Ok(features::references(
            &document,
            offset,
            params.context.include_declaration,
        ))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let params = params.text_document_position_params;
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, params.position)
        else {
            return Ok(None);
        };
        Ok(features::highlights(&document, offset))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let position = params.text_document_position;
        let Some(document) = self.document(&position.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, position.position)
        else {
            return Ok(None);
        };
        Ok(features::rename(&document, offset, &params.new_name))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let config = self.config.read().await.lint_config();
        Ok(Some(features::code_actions_with_config(
            &document,
            params.range,
            &config,
        )))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<WorkspaceSymbolResponse>> {
        let query = params.query.to_lowercase();
        let workspace = self.workspace.read().await;
        let mut symbols = Vec::new();
        for file in workspace.files() {
            let Some(uri) = tower_lsp_server::ls_types::Uri::from_file_path(&file.path) else {
                continue;
            };
            let analysis = stan_language::analyze(&file.text);
            let index = stan_language_server::line_index::LineIndex::new(&file.text);
            for symbol in analysis
                .semantics
                .symbols
                .iter()
                .filter(|symbol| symbol.name.to_lowercase().contains(&query))
            {
                let Ok(start) =
                    index.offset_to_position(&file.text, symbol.name_range.start as usize)
                else {
                    continue;
                };
                let Ok(end) = index.offset_to_position(&file.text, symbol.name_range.end as usize)
                else {
                    continue;
                };
                #[allow(deprecated)]
                symbols.push(SymbolInformation {
                    name: symbol.name.clone(),
                    kind: match symbol.kind {
                        stan_language::SemanticSymbolKind::Function => SymbolKind::FUNCTION,
                        _ => SymbolKind::VARIABLE,
                    },
                    tags: None,
                    deprecated: None,
                    location: Location::new(
                        uri.clone(),
                        tower_lsp_server::ls_types::Range::new(start, end),
                    ),
                    container_name: None,
                });
            }
        }
        Ok(Some(WorkspaceSymbolResponse::Flat(symbols)))
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        let params = params.text_document_position_params;
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        let Ok(offset) = document
            .line_index
            .position_to_offset(&document.text, params.position)
        else {
            return Ok(None);
        };
        Ok(features::prepare_call_hierarchy(&document, offset))
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        let Some(document) = self.document(&params.item.uri).await else {
            return Ok(None);
        };
        Ok(Some(features::incoming_calls(&document, &params.item)))
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        let Some(document) = self.document(&params.item.uri).await else {
            return Ok(None);
        };
        Ok(Some(features::outgoing_calls(&document, &params.item)))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let Some(document) = self.document(&params.text_document.uri).await else {
            return Ok(None);
        };
        Ok(Some(features::inlay_hints(&document, params.range)))
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
