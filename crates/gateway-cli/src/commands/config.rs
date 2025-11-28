//! Config command - manage gateway configuration.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the config command.
#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

/// Config subcommands.
#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show current configuration
    Show(ShowArgs),

    /// Generate a sample configuration file
    Init(InitArgs),

    /// Check configuration paths
    Path(PathArgs),
}

/// Arguments for config show.
#[derive(Args, Debug)]
pub struct ShowArgs {
    /// Configuration file to show
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Show only a specific section
    #[arg(short, long)]
    pub section: Option<String>,
}

/// Arguments for config init.
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Output file path
    #[arg(short, long, default_value = "gateway.yaml")]
    pub output: PathBuf,

    /// Output format (yaml, json, toml)
    #[arg(short, long, default_value = "yaml")]
    pub format: String,

    /// Overwrite existing file
    #[arg(long)]
    pub force: bool,

    /// Include all optional fields with defaults
    #[arg(long)]
    pub full: bool,
}

/// Arguments for config path.
#[derive(Args, Debug)]
pub struct PathArgs {
    /// Show all configuration search paths
    #[arg(long)]
    pub all: bool,
}

/// Configuration output for JSON.
#[derive(Debug, Serialize)]
pub struct ConfigOutput {
    pub path: Option<String>,
    pub content: serde_json::Value,
}

/// Execute the config command.
pub async fn execute(args: ConfigArgs, json: bool) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    match args.command {
        ConfigCommand::Show(show_args) => execute_show(show_args, format).await,
        ConfigCommand::Init(init_args) => execute_init(init_args, format).await,
        ConfigCommand::Path(path_args) => execute_path(path_args, format).await,
    }
}

/// Execute config show.
async fn execute_show(args: ShowArgs, format: OutputFormat) -> Result<()> {
    let config = if let Some(ref path) = args.file {
        gateway_config::ConfigLoader::new()
            .with_file(path.display().to_string())
            .load()
            .await?
    } else {
        gateway_config::GatewayConfig::default()
    };

    let config_json = serde_json::to_value(&config)?;

    let content = if let Some(ref section) = args.section {
        config_json
            .get(section)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    } else {
        config_json
    };

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(ConfigOutput {
                path: args.file.map(|p| p.display().to_string()),
                content,
            });
            result.print(format)?;
        }
        OutputFormat::Text => {
            if let Some(ref path) = args.file {
                output::info(&format!("Configuration from: {:?}", path));
            } else {
                output::info("Default configuration");
            }

            if let Some(ref section) = args.section {
                output::section(&format!("Section: {}", section));
            }

            println!("\n{}", serde_yaml::to_string(&content)?);
        }
    }

    Ok(())
}

/// Execute config init.
async fn execute_init(args: InitArgs, format: OutputFormat) -> Result<()> {
    // Check if file exists
    if args.output.exists() && !args.force {
        let result: CommandResult<()> = CommandResult::failure(format!(
            "File {:?} already exists. Use --force to overwrite.",
            args.output
        ));
        result.print(format)?;
        return Ok(());
    }

    // Generate config
    let config = if args.full {
        generate_full_config()
    } else {
        generate_minimal_config()
    };

    // Serialize to requested format
    let content = match args.format.as_str() {
        "yaml" | "yml" => serde_yaml::to_string(&config)?,
        "json" => serde_json::to_string_pretty(&config)?,
        "toml" => toml::to_string_pretty(&config)?,
        other => {
            let result: CommandResult<()> =
                CommandResult::failure(format!("Unknown format: {}", other));
            result.print(format)?;
            return Ok(());
        }
    };

    // Write file
    std::fs::write(&args.output, &content)?;

    match format {
        OutputFormat::Json => {
            let result: CommandResult<serde_json::Value> =
                CommandResult::success(serde_json::json!({
                    "path": args.output.display().to_string(),
                    "format": args.format,
                }));
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::success(&format!(
                "Created configuration file: {:?}",
                args.output
            ));
            output::info(&format!("Format: {}", args.format));

            if args.full {
                output::info("Generated full configuration with all options");
            } else {
                output::info("Generated minimal configuration");
            }
        }
    }

    Ok(())
}

/// Execute config path.
async fn execute_path(args: PathArgs, format: OutputFormat) -> Result<()> {
    let paths = get_config_paths();

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(paths.clone());
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Configuration Search Paths");

            for path in &paths {
                let exists = std::path::Path::new(path).exists();
                output::status(path, exists);
            }

            if args.all {
                output::section("Environment Variables");
                output::key_value(
                    "GATEWAY_CONFIG",
                    &std::env::var("GATEWAY_CONFIG").unwrap_or_else(|_| "(not set)".to_string()),
                );
                output::key_value(
                    "GATEWAY_URL",
                    &std::env::var("GATEWAY_URL").unwrap_or_else(|_| "(not set)".to_string()),
                );
                output::key_value(
                    "GATEWAY_API_KEY",
                    &std::env::var("GATEWAY_API_KEY")
                        .map(|_| "(set)".to_string())
                        .unwrap_or_else(|_| "(not set)".to_string()),
                );
            }
        }
    }

    Ok(())
}

/// Get configuration search paths.
fn get_config_paths() -> Vec<String> {
    let mut paths = vec![
        "./gateway.yaml".to_string(),
        "./gateway.yml".to_string(),
        "./gateway.json".to_string(),
        "./gateway.toml".to_string(),
        "./config/gateway.yaml".to_string(),
    ];

    // Add home directory config
    if let Some(home) = dirs::home_dir() {
        paths.push(
            home.join(".config/llm-gateway/gateway.yaml")
                .display()
                .to_string(),
        );
    }

    // Add system config paths
    #[cfg(unix)]
    {
        paths.push("/etc/llm-gateway/gateway.yaml".to_string());
    }

    paths
}

/// Generate a minimal configuration.
fn generate_minimal_config() -> serde_json::Value {
    serde_json::json!({
        "server": {
            "host": "0.0.0.0",
            "port": 8080
        },
        "providers": [{
            "id": "openai",
            "type": "openai",
            "endpoint": "https://api.openai.com/v1",
            "enabled": true,
            "api_key_env": "OPENAI_API_KEY"
        }],
        "routing": {
            "default_strategy": "round_robin"
        }
    })
}

/// Generate a full configuration with all options.
fn generate_full_config() -> serde_json::Value {
    serde_json::json!({
        "server": {
            "host": "0.0.0.0",
            "port": 8080,
            "workers": 4,
            "request_timeout": "300s",
            "graceful_shutdown_timeout": "30s",
            "keep_alive_timeout": "75s"
        },
        "providers": [
            {
                "id": "openai",
                "type": "openai",
                "endpoint": "https://api.openai.com/v1",
                "enabled": true,
                "api_key_env": "OPENAI_API_KEY",
                "models": ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo"],
                "timeout": "120s",
                "priority": 1,
                "weight": 100
            },
            {
                "id": "anthropic",
                "type": "anthropic",
                "endpoint": "https://api.anthropic.com",
                "enabled": true,
                "api_key_env": "ANTHROPIC_API_KEY",
                "models": ["claude-3-5-sonnet-latest", "claude-3-opus-latest", "claude-3-haiku-20240307"],
                "timeout": "120s",
                "priority": 1,
                "weight": 100
            }
        ],
        "routing": {
            "default_strategy": "round_robin",
            "health_aware": true,
            "rules": []
        },
        "resilience": {
            "circuit_breaker": {
                "enabled": true,
                "failure_threshold": 5,
                "reset_timeout": "30s"
            },
            "retry": {
                "enabled": true,
                "max_attempts": 3,
                "initial_delay": "1s",
                "max_delay": "30s"
            },
            "timeout": {
                "default": "120s",
                "streaming": "300s"
            }
        },
        "observability": {
            "logging": {
                "level": "info",
                "format": "text"
            },
            "metrics": {
                "enabled": true,
                "path": "/metrics"
            },
            "tracing": {
                "enabled": false
            }
        },
        "security": {
            "auth": {
                "enabled": false
            },
            "rate_limiting": {
                "enabled": true,
                "default_rpm": 60,
                "default_tpm": 100000
            },
            "cors": {
                "enabled": true,
                "allowed_origins": ["*"]
            }
        }
    })
}

/// Get home directory (cross-platform).
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            std::env::var_os("USERPROFILE").map(PathBuf::from)
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("HOME").map(PathBuf::from)
        }
    }
}
