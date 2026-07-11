use std::sync::Arc;

use stan_language_server::diagnostics::diagnostics;
use stan_language_server::document::{Document, DocumentStore};
use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, PositionEncodingKind, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
};
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use tracing_subscriber::EnvFilter;

#[derive(Debug)]
struct Backend {
    client: Client,
    documents: Arc<RwLock<DocumentStore>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentStore::default())),
        }
    }

    async fn publish(&self, document: &Document) {
        self.client
            .publish_diagnostics(
                document.uri.clone(),
                diagnostics(document),
                Some(document.version),
            )
            .await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF16),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        ..TextDocumentSyncOptions::default()
                    },
                )),
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
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let item = params.text_document;
        let document = Document::new(item.uri, item.version, item.text);
        self.documents.write().await.open(document.clone());
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
            self.publish(&document).await;
        } else {
            tracing::warn!("ignoring stale change or change for an unopened document");
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.close(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
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
