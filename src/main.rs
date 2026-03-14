use clap::Parser;
use rmcp::transport::stdio;
use rmcp::ServiceExt;

/// remix-browser: Headless Chrome automation via CDP
#[derive(Parser)]
#[command(name = "remix-browser", version, about)]
struct Cli {
    /// Run Chrome with a visible window (default: headless)
    #[arg(long)]
    headed: bool,

    /// Connect to an existing browser via CDP WebSocket URL (ws:// or http://).
    /// When using http://, the WebSocket URL is auto-discovered from /json/version.
    #[arg(long, env = "CDP_URL")]
    cdp_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stderr only — stdout is the MCP transport
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .init();

    let cli = Cli::parse();
    let headless = !cli.headed;

    match &cli.cdp_url {
        Some(url) => tracing::info!("Starting remix-browser MCP server (connecting to {})", url),
        None => tracing::info!("Starting remix-browser MCP server (headless: {})", headless),
    }

    let server = remix_browser::server::RemixBrowserServer::new(headless, cli.cdp_url);
    let service = server.clone().serve(stdio()).await?;

    // Wait for MCP service to finish OR a termination signal — whichever comes first
    tokio::select! {
        result = service.waiting() => { result?; }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received interrupt signal, shutting down");
        }
    }

    // Shut down browser session (disconnects from remote or kills local Chrome)
    server.shutdown().await;

    tracing::info!("remix-browser MCP server shut down");
    Ok(())
}
