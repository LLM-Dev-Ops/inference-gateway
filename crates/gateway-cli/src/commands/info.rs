//! Info command - show gateway version and information.

use anyhow::Result;
use clap::Args;
use serde::Serialize;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the info command.
#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Show build information
    #[arg(long)]
    pub build: bool,

    /// Show runtime information
    #[arg(long)]
    pub runtime: bool,
}

/// Info output.
#[derive(Debug, Serialize)]
pub struct InfoOutput {
    pub version: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerInfo>,
}

/// Build information.
#[derive(Debug, Serialize)]
pub struct BuildInfo {
    pub rust_version: String,
    pub target: String,
    pub profile: String,
}

/// Runtime information.
#[derive(Debug, Serialize)]
pub struct RuntimeInfo {
    pub os: String,
    pub arch: String,
    pub cpus: usize,
}

/// Server information.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,
}

/// Execute the info command.
pub async fn execute(args: InfoArgs, base_url: &str, json: bool) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    let mut info = InfoOutput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        name: "LLM Inference Gateway".to_string(),
        build: None,
        runtime: None,
        server: None,
    };

    if args.build {
        info.build = Some(BuildInfo {
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
            target: std::env::consts::ARCH.to_string(),
            profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
        });
    }

    if args.runtime {
        info.runtime = Some(RuntimeInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpus: num_cpus(),
        });
    }

    // Try to get server info
    let server_info = get_server_info(base_url).await;
    info.server = server_info;

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(info);
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("LLM Inference Gateway");
            output::key_value("Version", &info.version);

            if let Some(ref build) = info.build {
                output::section("Build Information");
                output::key_value("Rust Version", &build.rust_version);
                output::key_value("Target", &build.target);
                output::key_value("Profile", &build.profile);
            }

            if let Some(ref runtime) = info.runtime {
                output::section("Runtime Information");
                output::key_value("OS", &runtime.os);
                output::key_value("Architecture", &runtime.arch);
                output::key_value("CPUs", &runtime.cpus.to_string());
            }

            if let Some(ref server) = info.server {
                output::section("Server Status");
                output::status(&format!("Status: {}", server.status), server.status == "healthy");

                if let Some(ref version) = server.version {
                    output::key_value("Server Version", version);
                }

                if let Some(ref uptime) = server.uptime {
                    output::key_value("Uptime", uptime);
                }
            } else {
                output::section("Server Status");
                output::status("Server not reachable", false);
            }
        }
    }

    Ok(())
}

/// Get server information from the gateway.
async fn get_server_info(base_url: &str) -> Option<ServerInfo> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let url = format!("{}/health", base_url.trim_end_matches('/'));

    let response = client.get(&url).send().await.ok()?;

    if response.status().is_success() {
        let body: serde_json::Value = response.json().await.ok()?;

        Some(ServerInfo {
            status: body
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            version: body.get("version").and_then(|v| v.as_str()).map(String::from),
            uptime: body.get("uptime").and_then(|v| v.as_str()).map(String::from),
        })
    } else {
        Some(ServerInfo {
            status: "unhealthy".to_string(),
            version: None,
            uptime: None,
        })
    }
}

/// Get number of CPUs.
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1)
}
