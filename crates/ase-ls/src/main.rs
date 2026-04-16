//! # SAP ASE Language Server
//!
//! VSCode / Zed 用の SAP ASE (Sybase) T-SQL Language Server。

mod config;
mod server;

use clap::Parser;
use tower_lsp::LspService;

/// SAP ASE Language Server
#[derive(Parser, Debug)]
#[command(name = "ase-ls", version, about = "SAP ASE Language Server")]
struct Args {
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let server_config = config::ServerConfig {
        log_level: args.log_level,
    };
    server_config.init_logging();

    tracing::info!(
        "Starting ASE Language Server v{}",
        env!("CARGO_PKG_VERSION")
    );

    let (service, socket) = LspService::new(server::AseLanguageServer::new);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
