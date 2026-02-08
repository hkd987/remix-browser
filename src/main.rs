use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

/// remix-browser: Headless Chrome automation via CDP
#[derive(Parser)]
#[command(name = "remix-browser", version, about)]
struct Cli {
    /// Run Chrome with a visible window (default: headless)
    #[arg(long)]
    headed: bool,
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

    tracing::info!(
        "Starting remix-browser MCP server (headless: {})",
        headless
    );

    let server = remix_browser::server::RemixBrowserServer::new(headless);
    let service = server.clone().serve(stdio()).await?;
    service.waiting().await?;

    // Explicitly kill Chrome before exiting
    server.shutdown().await;

    tracing::info!("remix-browser MCP server shut down");
    Ok(())
}
