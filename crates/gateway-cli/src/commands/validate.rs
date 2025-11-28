//! Validate command - validate configuration files.

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use std::path::PathBuf;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the validate command.
#[derive(Args, Debug)]
pub struct ValidateArgs {
    /// Configuration file to validate
    #[arg(short, long, default_value = "gateway.yaml")]
    pub file: PathBuf,

    /// Strict validation mode
    #[arg(long)]
    pub strict: bool,
}

/// Validation result.
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub file: String,
    pub warnings: Vec<ValidationMessage>,
    pub errors: Vec<ValidationMessage>,
}

/// Validation message.
#[derive(Debug, Serialize)]
pub struct ValidationMessage {
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Execute the validate command.
pub async fn execute(args: ValidateArgs, json: bool) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    let mut result = ValidationResult {
        valid: true,
        file: args.file.display().to_string(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    // Check if file exists
    if !args.file.exists() {
        result.valid = false;
        result.errors.push(ValidationMessage {
            level: "error".to_string(),
            message: format!("File not found: {:?}", args.file),
            path: None,
        });

        return print_result(result, format);
    }

    // Try to load the configuration
    match gateway_config::ConfigLoader::new()
        .with_file(args.file.display().to_string())
        .load()
        .await
    {
        Ok(config) => {
            // Validate configuration
            validate_config(&config, &mut result, args.strict);
        }
        Err(e) => {
            result.valid = false;
            result.errors.push(ValidationMessage {
                level: "error".to_string(),
                message: format!("Failed to parse configuration: {}", e),
                path: None,
            });
        }
    }

    print_result(result, format)
}

/// Validate the configuration.
fn validate_config(
    config: &gateway_config::GatewayConfig,
    result: &mut ValidationResult,
    strict: bool,
) {
    // Validate server configuration
    if config.server.port == 0 {
        result.errors.push(ValidationMessage {
            level: "error".to_string(),
            message: "Server port cannot be 0".to_string(),
            path: Some("server.port".to_string()),
        });
        result.valid = false;
    }

    // Validate providers
    let enabled_providers: Vec<_> = config
        .providers
        .iter()
        .filter(|p| p.enabled)
        .collect();

    if enabled_providers.is_empty() {
        result.warnings.push(ValidationMessage {
            level: "warning".to_string(),
            message: "No providers are enabled".to_string(),
            path: Some("providers".to_string()),
        });

        if strict {
            result.valid = false;
        }
    }

    // Check provider API keys
    for provider in &config.providers {
        if provider.enabled {
            if let Some(ref api_key_env) = provider.api_key_env {
                if std::env::var(api_key_env).is_err() {
                    result.warnings.push(ValidationMessage {
                        level: "warning".to_string(),
                        message: format!(
                            "Environment variable '{}' for provider '{}' is not set",
                            api_key_env, provider.id
                        ),
                        path: Some(format!("providers.{}.api_key_env", provider.id)),
                    });
                }
            } else if provider.api_key.is_none() {
                result.warnings.push(ValidationMessage {
                    level: "warning".to_string(),
                    message: format!(
                        "Provider '{}' has no API key configured",
                        provider.id
                    ),
                    path: Some(format!("providers.{}", provider.id)),
                });
            }
        }
    }

    // Validate rate limiting
    if config.security.rate_limiting.enabled {
        if config.security.rate_limiting.default_rpm == 0 {
            result.warnings.push(ValidationMessage {
                level: "warning".to_string(),
                message: "Rate limiting is enabled but default_rpm is 0".to_string(),
                path: Some("security.rate_limiting.default_rpm".to_string()),
            });
        }
    }

    // Additional strict validations
    if strict {
        // Check for recommended settings
        if !config.observability.metrics.enabled {
            result.warnings.push(ValidationMessage {
                level: "warning".to_string(),
                message: "Metrics are disabled (recommended for production)".to_string(),
                path: Some("observability.metrics.enabled".to_string()),
            });
        }

        if config.security.cors.allowed_origins.contains(&"*".to_string()) {
            result.warnings.push(ValidationMessage {
                level: "warning".to_string(),
                message: "CORS allows all origins (not recommended for production)".to_string(),
                path: Some("security.cors.allowed_origins".to_string()),
            });
        }
    }
}

/// Print the validation result.
fn print_result(result: ValidationResult, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let cmd_result = if result.valid {
                CommandResult::success(result)
            } else {
                CommandResult {
                    success: false,
                    data: Some(result),
                    error: Some("Validation failed".to_string()),
                    message: None,
                }
            };
            cmd_result.print(format)?;
        }
        OutputFormat::Text => {
            if result.valid {
                output::success(&format!("Configuration file is valid: {}", result.file));
            } else {
                output::error(&format!("Configuration file is invalid: {}", result.file));
            }

            if !result.errors.is_empty() {
                output::section("Errors");
                for error in &result.errors {
                    if let Some(ref path) = error.path {
                        output::error(&format!("[{}] {}", path, error.message));
                    } else {
                        output::error(&error.message);
                    }
                }
            }

            if !result.warnings.is_empty() {
                output::section("Warnings");
                for warning in &result.warnings {
                    if let Some(ref path) = warning.path {
                        output::warning(&format!("[{}] {}", path, warning.message));
                    } else {
                        output::warning(&warning.message);
                    }
                }
            }

            let total_issues = result.errors.len() + result.warnings.len();
            if total_issues > 0 {
                println!(
                    "\nFound {} error(s) and {} warning(s)",
                    result.errors.len(),
                    result.warnings.len()
                );
            }
        }
    }

    Ok(())
}
