//! Health check command.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the health command.
#[derive(Args, Debug)]
pub struct HealthArgs {
    /// Show detailed health information
    #[arg(short, long)]
    pub detailed: bool,

    /// Timeout in seconds
    #[arg(short, long, default_value = "5")]
    pub timeout: u64,

    /// Check specific endpoint (health, ready, live)
    #[arg(long, default_value = "health")]
    pub endpoint: String,
}

/// Health check response for output.
#[derive(Debug, Serialize)]
pub struct HealthOutput {
    pub status: String,
    pub endpoint: String,
    pub response_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Execute the health command.
pub async fn execute(
    args: HealthArgs,
    base_url: &str,
    api_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    let client = build_client(base_url, api_key, args.timeout)?;

    let endpoint_path = match args.endpoint.as_str() {
        "health" => "/health",
        "ready" | "readiness" => "/ready",
        "live" | "liveness" => "/live",
        other => {
            let result: CommandResult<()> =
                CommandResult::failure(format!("Unknown endpoint: {}", other));
            result.print(format)?;
            return Ok(());
        }
    };

    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint_path);

    if !json {
        let spinner = output::spinner(&format!("Checking {} endpoint...", args.endpoint));

        let start = std::time::Instant::now();
        let response = client.get(&url).send().await;
        let elapsed = start.elapsed();

        spinner.finish_and_clear();

        match response {
            Ok(resp) => {
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or_default();

                if status.is_success() {
                    output::success("Gateway is healthy");
                    output::key_value("Endpoint", &args.endpoint);
                    output::key_value(
                        "Response Time",
                        &format!("{}ms", elapsed.as_millis()),
                    );

                    if let Some(status_val) = body.get("status") {
                        output::key_value("Status", status_val.as_str().unwrap_or("unknown"));
                    }

                    if let Some(version) = body.get("version") {
                        output::key_value("Version", version.as_str().unwrap_or("unknown"));
                    }

                    if args.detailed {
                        output::section("Details");
                        println!("{}", serde_json::to_string_pretty(&body)?);
                    }
                } else {
                    output::error(&format!("Health check failed with status {}", status));
                }
            }
            Err(e) => {
                output::error(&format!("Failed to connect: {}", e));
            }
        }
    } else {
        let start = std::time::Instant::now();
        let response = client.get(&url).send().await;
        let elapsed = start.elapsed();

        match response {
            Ok(resp) => {
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or_default();

                let health_output = HealthOutput {
                    status: if status.is_success() {
                        "healthy".to_string()
                    } else {
                        "unhealthy".to_string()
                    },
                    endpoint: args.endpoint.clone(),
                    response_time_ms: elapsed.as_millis() as u64,
                    version: body.get("version").and_then(|v| v.as_str()).map(String::from),
                    details: if args.detailed { Some(body) } else { None },
                };

                let result = CommandResult::success(health_output);
                result.print(format)?;
            }
            Err(e) => {
                let result: CommandResult<HealthOutput> =
                    CommandResult::failure(format!("Connection failed: {}", e));
                result.print(format)?;
            }
        }
    }

    Ok(())
}

fn build_client(
    _base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout));

    if let Some(key) = api_key {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", key))?,
        );
        builder = builder.default_headers(headers);
    }

    Ok(builder.build()?)
}
