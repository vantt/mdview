//! mdview — multi-project markdown viewer for AI agent workflows.

mod cli;
mod doctor;
mod mcp;
mod runtime;
mod server;
mod views;
mod watch;

use clap::Parser;

fn main() {
    // MCP speaks JSON-RPC on stdout; keep tracing on stderr and quiet by default.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mdview=info,warn".into()),
        )
        .init();

    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
