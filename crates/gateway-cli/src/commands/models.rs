//! Models command - list and query available models.

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use tabled::Tabled;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the models command.
#[derive(Args, Debug)]
pub struct ModelsArgs {
    /// Filter models by provider (openai, anthropic, etc.)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Filter models by name pattern
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Show detailed model information
    #[arg(short, long)]
    pub detailed: bool,

    /// Get info for a specific model
    #[arg(long)]
    pub model: Option<String>,
}

/// Model information for table display.
#[derive(Debug, Tabled, Serialize)]
pub struct ModelRow {
    #[tabled(rename = "Model ID")]
    pub id: String,
    #[tabled(rename = "Provider")]
    pub owned_by: String,
    #[tabled(rename = "Created")]
    pub created: String,
}

/// Detailed model information.
#[derive(Debug, Serialize)]
pub struct ModelDetail {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub created_formatted: String,
    pub owned_by: String,
}

/// Execute the models command.
pub async fn execute(
    args: ModelsArgs,
    base_url: &str,
    api_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    let client = gateway_sdk::Client::builder()
        .base_url(base_url)
        .timeout(std::time::Duration::from_secs(30));

    let client = if let Some(key) = api_key {
        client.api_key(key)
    } else {
        client
    };

    let client = client.build()?;

    if !json {
        let spinner = output::spinner("Fetching models...");

        let result = client.list_models().await;

        spinner.finish_and_clear();

        match result {
            Ok(response) => {
                let mut models: Vec<ModelRow> = response
                    .data
                    .iter()
                    .map(|m| ModelRow {
                        id: m.id.clone(),
                        owned_by: m.owned_by.clone(),
                        created: output::format_timestamp(m.created),
                    })
                    .collect();

                // Apply filters
                if let Some(ref provider) = args.provider {
                    models.retain(|m| {
                        m.owned_by.to_lowercase().contains(&provider.to_lowercase())
                    });
                }

                if let Some(ref filter) = args.filter {
                    models.retain(|m| {
                        m.id.to_lowercase().contains(&filter.to_lowercase())
                    });
                }

                // Sort by ID
                models.sort_by(|a, b| a.id.cmp(&b.id));

                if models.is_empty() {
                    output::warning("No models found matching the criteria");
                } else {
                    output::success(&format!("Found {} models", models.len()));
                    println!();
                    output::table(&models);
                }
            }
            Err(e) => {
                output::error(&format!("Failed to fetch models: {}", e));
            }
        }
    } else {
        let result = client.list_models().await;

        match result {
            Ok(response) => {
                let mut models: Vec<ModelDetail> = response
                    .data
                    .iter()
                    .map(|m| ModelDetail {
                        id: m.id.clone(),
                        object: m.object.clone(),
                        created: m.created,
                        created_formatted: output::format_timestamp(m.created),
                        owned_by: m.owned_by.clone(),
                    })
                    .collect();

                // Apply filters
                if let Some(ref provider) = args.provider {
                    models.retain(|m| {
                        m.owned_by.to_lowercase().contains(&provider.to_lowercase())
                    });
                }

                if let Some(ref filter) = args.filter {
                    models.retain(|m| {
                        m.id.to_lowercase().contains(&filter.to_lowercase())
                    });
                }

                let result = CommandResult::success(models);
                result.print(format)?;
            }
            Err(e) => {
                let result: CommandResult<Vec<ModelDetail>> =
                    CommandResult::failure(format!("{}", e));
                result.print(format)?;
            }
        }
    }

    Ok(())
}
