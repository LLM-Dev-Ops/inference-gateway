//! Start command - launches the gateway server.

use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use std::time::Duration;

use crate::output;

/// Arguments for the start command.
#[derive(Args, Debug)]
pub struct StartArgs {
    /// Configuration file path
    #[arg(short, long, env = "GATEWAY_CONFIG")]
    pub config: Option<PathBuf>,

    /// Host to bind to
    #[arg(long, env = "GATEWAY_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// Port to bind to
    #[arg(short, long, env = "GATEWAY_PORT", default_value = "8080")]
    pub port: u16,

    /// Enable hot reloading of configuration
    #[arg(long)]
    pub hot_reload: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "GATEWAY_LOG_LEVEL", default_value = "info")]
    pub log_level: String,

    /// Enable JSON logging
    #[arg(long)]
    pub json_logs: bool,

    /// Enable metrics endpoint
    #[arg(long, default_value = "true")]
    pub metrics: bool,

    /// Metrics port (if different from main port)
    #[arg(long, env = "GATEWAY_METRICS_PORT")]
    pub metrics_port: Option<u16>,

    /// Run in development mode (enables additional debugging)
    #[arg(long)]
    pub dev: bool,

    /// Request timeout in seconds
    #[arg(long, env = "GATEWAY_REQUEST_TIMEOUT", default_value = "120")]
    pub request_timeout: u64,
}

/// Execute the start command.
pub async fn execute(args: StartArgs) -> Result<()> {
    output::info(&format!(
        "Starting LLM Inference Gateway on {}:{}",
        args.host, args.port
    ));

    // Load configuration
    let config = load_config(&args).await?;

    output::info("Configuration loaded successfully");

    if args.hot_reload {
        output::info("Hot reloading enabled");
    }

    if args.dev {
        output::warning("Development mode enabled - not for production use");
    }

    // Start the server
    output::info("Initializing server...");

    output::success(&format!("Server listening on {}:{}", args.host, args.port));
    output::info("Press Ctrl+C to stop");

    // Create server configuration
    let server_config = gateway_server::ServerConfig::new()
        .with_host(&args.host)
        .with_port(args.port)
        .with_request_timeout(Duration::from_secs(args.request_timeout));

    // Create application state
    let state = gateway_server::AppState::builder()
        .config(config)
        .build();

    // Create and run the server
    let server = gateway_server::Server::new(server_config, state);
    server.run().await.context("Server error")?;

    output::info("Server stopped");
    Ok(())
}

/// Load configuration from file or defaults.
async fn load_config(args: &StartArgs) -> Result<gateway_config::GatewayConfig> {
    let config = if let Some(ref path) = args.config {
        output::info(&format!("Loading configuration from {:?}", path));
        gateway_config::ConfigLoader::new()
            .with_file(path.display().to_string())
            .load()
            .await
            .context("Failed to load configuration file")?
    } else {
        output::info("Using default configuration");
        gateway_config::GatewayConfig::default()
    };

    // Apply command-line overrides
    let mut config = config;

    // Override server settings
    config.server.host = args.host.clone();
    config.server.port = args.port;

    Ok(config)
}
