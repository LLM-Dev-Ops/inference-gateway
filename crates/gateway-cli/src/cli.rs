//! CLI argument definitions using clap.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands;

/// LLM Inference Gateway - A unified gateway for LLM providers
#[derive(Parser, Debug)]
#[command(name = "llm-gateway")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Increase output verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Gateway server URL
    #[arg(short = 'u', long, env = "GATEWAY_URL", default_value = "http://localhost:8080", global = true)]
    pub url: String,

    /// API key for authentication
    #[arg(short = 'k', long, env = "GATEWAY_API_KEY", global = true)]
    pub api_key: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the gateway server
    #[command(visible_alias = "run")]
    Start(commands::start::StartArgs),

    /// Check gateway health
    Health(commands::health::HealthArgs),

    /// List available models
    Models(commands::models::ModelsArgs),

    /// Send a chat completion request
    Chat(commands::chat::ChatArgs),

    /// Manage gateway configuration
    Config(commands::config::ConfigArgs),

    /// Show gateway version and info
    Info(commands::info::InfoArgs),

    /// Validate configuration file
    Validate(commands::validate::ValidateArgs),

    /// Generate shell completions
    Completions(commands::completions::CompletionsArgs),

    /// Database migration management
    Migrate(commands::migrate::MigrateArgs),

    /// View latency metrics and statistics
    Latency(commands::latency::LatencyArgs),

    /// View cost tracking and analytics
    Cost(commands::cost::CostArgs),

    /// View token usage statistics
    #[command(name = "token-usage")]
    TokenUsage(commands::token_usage::TokenUsageArgs),

    /// Monitor backend health status
    #[command(name = "backend-health")]
    BackendHealth(commands::backend_health::BackendHealthArgs),

    /// Manage routing strategies
    #[command(name = "routing-strategy")]
    RoutingStrategy(commands::routing_strategy::RoutingStrategyArgs),

    /// View and manage cache status
    #[command(name = "cache-status")]
    CacheStatus(commands::cache_status::CacheStatusArgs),
}

impl Cli {
    /// Execute the CLI command.
    pub async fn execute(self) -> Result<()> {
        match self.command {
            Commands::Start(args) => commands::start::execute(args).await,
            Commands::Health(args) => commands::health::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::Models(args) => commands::models::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::Chat(args) => commands::chat::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::Config(args) => commands::config::execute(args, self.json).await,
            Commands::Info(args) => commands::info::execute(args, &self.url, self.json).await,
            Commands::Validate(args) => commands::validate::execute(args, self.json).await,
            Commands::Completions(args) => commands::completions::execute(args),
            Commands::Migrate(args) => commands::migrate::execute(args, self.json).await,
            Commands::Latency(args) => commands::latency::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::Cost(args) => commands::cost::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::TokenUsage(args) => commands::token_usage::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::BackendHealth(args) => commands::backend_health::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::RoutingStrategy(args) => commands::routing_strategy::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
            Commands::CacheStatus(args) => commands::cache_status::execute(args, &self.url, self.api_key.as_deref(), self.json).await,
        }
    }
}
