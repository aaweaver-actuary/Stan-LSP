use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::{
    InitializeParams, InitializeResult, ServerCapabilities, ServerInfo,
};
use tower_lsp_server::{LanguageServer, LspService, Server};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Default)]
struct Backend;

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities::default(),
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
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|_| Backend);
    Server::new(stdin, stdout, socket).serve(service).await;
}
