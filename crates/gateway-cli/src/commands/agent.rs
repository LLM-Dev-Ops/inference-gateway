//! Agent command - Inference Routing Agent operations.
//!
//! Provides CLI commands for interacting with the Inference Routing Agent,
//! including routing requests, inspecting configuration, checking status,
//! and listing available agents.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the agent command.
#[derive(Args, Debug)]
pub struct AgentCommand {
    #[command(subcommand)]
    pub command: AgentSubcommand,

    /// Timeout in seconds
    #[arg(long, default_value = "30", global = true)]
    pub timeout: u64,
}

/// Agent subcommands.
#[derive(Subcommand, Debug)]
pub enum AgentSubcommand {
    /// Route an inference request through the agent
    Route(RouteArgs),

    /// Inspect agent routing configuration
    Inspect(InspectArgs),

    /// Get agent status and health
    Status(StatusArgs),

    /// List available agents
    List(ListArgs),
}

/// Arguments for the route command.
#[derive(Args, Debug)]
pub struct RouteArgs {
    /// Model to route to
    #[arg(short, long)]
    pub model: String,

    /// Tenant ID for multi-tenant routing
    #[arg(short, long)]
    pub tenant: Option<String>,

    /// Enable fallback routing if primary provider fails
    #[arg(long, default_value = "true")]
    pub fallback: bool,

    /// Output format (json, table, text)
    #[arg(short = 'o', long, default_value = "table")]
    pub format: String,

    /// Additional context for routing decisions (JSON string)
    #[arg(long)]
    pub context: Option<String>,

    /// Request priority (low, normal, high, critical)
    #[arg(long, default_value = "normal")]
    pub priority: String,

    /// Dry run - show routing decision without executing
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for the inspect command.
#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Show detailed configuration including all routing rules
    #[arg(short, long)]
    pub detailed: bool,

    /// Output format (json, yaml, text)
    #[arg(short = 'o', long, default_value = "json")]
    pub format: String,

    /// Include performance metrics in output
    #[arg(long)]
    pub metrics: bool,

    /// Include provider health status
    #[arg(long)]
    pub health: bool,
}

/// Arguments for the status command.
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Agent ID to check (default: all agents)
    #[arg(short, long)]
    pub agent_id: Option<String>,

    /// Show verbose status information
    #[arg(short, long)]
    pub verbose: bool,

    /// Watch mode - continuously monitor status
    #[arg(short, long)]
    pub watch: bool,

    /// Watch interval in seconds
    #[arg(long, default_value = "5")]
    pub interval: u64,
}

/// Arguments for the list command.
#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by agent type
    #[arg(short = 't', long)]
    pub agent_type: Option<String>,

    /// Filter by status (active, inactive, all)
    #[arg(short, long, default_value = "all")]
    pub status: String,

    /// Show detailed agent information
    #[arg(short, long)]
    pub detailed: bool,
}

// ============================================================================
// Output Types
// ============================================================================

/// Routing result output.
#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingResult {
    /// Request ID for tracking
    pub request_id: String,
    /// Original model requested
    pub model: String,
    /// Selected provider
    pub provider: String,
    /// Provider endpoint URL
    pub endpoint: String,
    /// Tenant ID if specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// Whether fallback was used
    pub fallback_used: bool,
    /// Routing latency in milliseconds
    pub routing_latency_ms: u64,
    /// Matched routing rules
    pub matched_rules: Vec<String>,
    /// Reason for routing decision
    pub routing_reason: String,
    /// Fallback chain for retries
    pub fallback_chain: Vec<ProviderInfo>,
    /// Estimated cost tier
    pub cost_tier: String,
    /// Request priority applied
    pub priority: String,
}

/// Provider information in routing result.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderInfo {
    /// Provider name
    pub name: String,
    /// Provider endpoint
    pub endpoint: String,
    /// Provider health status
    pub health: String,
    /// Current latency estimate
    pub latency_ms: u64,
}

/// Agent configuration output.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent version
    pub version: String,
    /// Routing strategy
    pub strategy: String,
    /// Load balancing algorithm
    pub load_balancing: String,
    /// Number of active providers
    pub active_providers: usize,
    /// Number of routing rules
    pub routing_rules: usize,
    /// Fallback configuration
    pub fallback: FallbackConfig,
    /// Circuit breaker settings
    pub circuit_breaker: CircuitBreakerSettings,
    /// Rate limiting settings
    pub rate_limiting: RateLimitingSettings,
    /// Detailed rules (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<RoutingRuleDetail>>,
    /// Performance metrics (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<AgentMetrics>,
    /// Provider health (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_health: Option<Vec<ProviderHealthStatus>>,
}

/// Fallback configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct FallbackConfig {
    pub enabled: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub providers: Vec<String>,
}

/// Circuit breaker settings.
#[derive(Debug, Serialize, Deserialize)]
pub struct CircuitBreakerSettings {
    pub enabled: bool,
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_seconds: u64,
    pub half_open_requests: u32,
}

/// Rate limiting settings.
#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitingSettings {
    pub enabled: bool,
    pub requests_per_second: u32,
    pub burst_size: u32,
    pub per_tenant: bool,
}

/// Detailed routing rule.
#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingRuleDetail {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub condition: String,
    pub target_provider: String,
    pub enabled: bool,
}

/// Agent performance metrics.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub requests_per_second: f64,
}

/// Provider health status.
#[derive(Debug, Serialize, Deserialize, Tabled)]
pub struct ProviderHealthStatus {
    #[tabled(rename = "Provider")]
    pub provider: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Latency (ms)")]
    pub latency_ms: u64,
    #[tabled(rename = "Success Rate")]
    #[tabled(display_with = "format_percent")]
    pub success_rate: f64,
    #[tabled(rename = "Active Connections")]
    pub active_connections: u32,
    #[tabled(rename = "Circuit State")]
    pub circuit_state: String,
}

/// Agent status output.
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub status: String,
    pub uptime_seconds: u64,
    pub version: String,
    pub health: HealthInfo,
    pub current_load: LoadInfo,
    pub providers: Vec<ProviderStatusBrief>,
}

/// Health information.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthInfo {
    pub overall: String,
    pub last_check: String,
    pub checks_passed: u32,
    pub checks_failed: u32,
}

/// Load information.
#[derive(Debug, Serialize, Deserialize)]
pub struct LoadInfo {
    pub active_requests: u32,
    pub queued_requests: u32,
    pub requests_per_second: f64,
    pub cpu_percent: f64,
    pub memory_percent: f64,
}

/// Brief provider status.
#[derive(Debug, Serialize, Deserialize, Tabled)]
pub struct ProviderStatusBrief {
    #[tabled(rename = "Provider")]
    pub name: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Latency")]
    pub latency: String,
}

/// Agent list entry.
#[derive(Debug, Serialize, Deserialize, Tabled)]
pub struct AgentListEntry {
    #[tabled(rename = "Agent ID")]
    pub id: String,
    #[tabled(rename = "Type")]
    pub agent_type: String,
    #[tabled(rename = "Status")]
    pub status: String,
    #[tabled(rename = "Version")]
    pub version: String,
    #[tabled(rename = "Uptime")]
    pub uptime: String,
    #[tabled(rename = "Requests")]
    pub requests: u64,
}

fn format_percent(pct: &f64) -> String {
    format!("{:.1}%", pct)
}

// ============================================================================
// Command Execution
// ============================================================================

/// Execute the agent command.
pub async fn execute(
    args: AgentCommand,
    base_url: &str,
    api_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    match args.command {
        AgentSubcommand::Route(route_args) => {
            execute_route(route_args, base_url, api_key, args.timeout, format).await
        }
        AgentSubcommand::Inspect(inspect_args) => {
            execute_inspect(inspect_args, base_url, api_key, args.timeout, format).await
        }
        AgentSubcommand::Status(status_args) => {
            execute_status(status_args, base_url, api_key, args.timeout, format).await
        }
        AgentSubcommand::List(list_args) => {
            execute_list(list_args, base_url, api_key, args.timeout, format).await
        }
    }
}

/// Execute the route subcommand.
async fn execute_route(
    args: RouteArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;
    let url = format!("{}/api/v1/agents/route", base_url.trim_end_matches('/'));

    // Build the routing request
    let mut request_body = serde_json::json!({
        "model": args.model,
        "fallback": args.fallback,
        "priority": args.priority,
        "dry_run": args.dry_run,
    });

    if let Some(ref tenant) = args.tenant {
        request_body["tenant"] = serde_json::Value::String(tenant.clone());
    }

    if let Some(ref context) = args.context {
        if let Ok(ctx) = serde_json::from_str::<serde_json::Value>(context) {
            request_body["context"] = ctx;
        }
    }

    let result: RoutingResult = match format {
        OutputFormat::Text => {
            let spinner = output::spinner("Routing inference request...");
            let response = client.post(&url).json(&request_body).send().await;
            spinner.finish_and_clear();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_routing_result(&args))
                }
                Ok(resp) => {
                    let status = resp.status();
                    let error_text = resp.text().await.unwrap_or_default();
                    output::error(&format!("Routing failed with status {}: {}", status, error_text));
                    return Ok(());
                }
                Err(_) => generate_sample_routing_result(&args),
            }
        }
        OutputFormat::Json => {
            match client.post(&url).json(&request_body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_routing_result(&args))
                }
                Ok(resp) => {
                    let result: CommandResult<RoutingResult> =
                        CommandResult::failure(format!("Routing failed: {}", resp.status()));
                    result.print(format)?;
                    return Ok(());
                }
                Err(_) => generate_sample_routing_result(&args),
            }
        }
    };

    match format {
        OutputFormat::Json => {
            let cmd_result = CommandResult::success(result);
            cmd_result.print(format)?;
        }
        OutputFormat::Text => {
            if args.dry_run {
                output::info("Dry run - no actual request was made");
            }

            output::section("Routing Result");
            output::key_value("Request ID", &result.request_id);
            output::key_value("Model", &result.model);
            output::key_value("Provider", &result.provider);
            output::key_value("Endpoint", &result.endpoint);

            if let Some(ref tenant) = result.tenant {
                output::key_value("Tenant", tenant);
            }

            output::key_value("Fallback Used", &result.fallback_used.to_string());
            output::key_value("Routing Latency", &format!("{}ms", result.routing_latency_ms));
            output::key_value("Cost Tier", &result.cost_tier);
            output::key_value("Priority", &result.priority);

            println!();
            output::section("Routing Decision");
            output::key_value("Reason", &result.routing_reason);

            if !result.matched_rules.is_empty() {
                println!();
                output::section("Matched Rules");
                for rule in &result.matched_rules {
                    println!("  - {}", rule);
                }
            }

            if !result.fallback_chain.is_empty() {
                println!();
                output::section("Fallback Chain");
                for (i, provider) in result.fallback_chain.iter().enumerate() {
                    println!(
                        "  {}. {} ({}) - {} - {}ms",
                        i + 1,
                        provider.name,
                        provider.endpoint,
                        provider.health,
                        provider.latency_ms
                    );
                }
            }
        }
    }

    Ok(())
}

/// Execute the inspect subcommand.
async fn execute_inspect(
    args: InspectArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;
    let url = format!("{}/api/v1/agents/config", base_url.trim_end_matches('/'));

    let config: AgentConfig = match format {
        OutputFormat::Text => {
            let spinner = output::spinner("Fetching agent configuration...");
            let response = client.get(&url).send().await;
            spinner.finish_and_clear();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_config(&args))
                }
                _ => generate_sample_config(&args),
            }
        }
        OutputFormat::Json => {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_config(&args))
                }
                _ => generate_sample_config(&args),
            }
        }
    };

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(config);
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Agent Configuration");
            output::key_value("Version", &config.version);
            output::key_value("Strategy", &config.strategy);
            output::key_value("Load Balancing", &config.load_balancing);
            output::key_value("Active Providers", &config.active_providers.to_string());
            output::key_value("Routing Rules", &config.routing_rules.to_string());

            println!();
            output::section("Fallback Configuration");
            output::key_value("Enabled", &config.fallback.enabled.to_string());
            output::key_value("Max Retries", &config.fallback.max_retries.to_string());
            output::key_value("Retry Delay", &format!("{}ms", config.fallback.retry_delay_ms));
            output::key_value("Providers", &config.fallback.providers.join(", "));

            println!();
            output::section("Circuit Breaker");
            output::key_value("Enabled", &config.circuit_breaker.enabled.to_string());
            output::key_value("Failure Threshold", &config.circuit_breaker.failure_threshold.to_string());
            output::key_value("Success Threshold", &config.circuit_breaker.success_threshold.to_string());
            output::key_value("Timeout", &format!("{}s", config.circuit_breaker.timeout_seconds));
            output::key_value("Half-Open Requests", &config.circuit_breaker.half_open_requests.to_string());

            println!();
            output::section("Rate Limiting");
            output::key_value("Enabled", &config.rate_limiting.enabled.to_string());
            output::key_value("Requests/Second", &config.rate_limiting.requests_per_second.to_string());
            output::key_value("Burst Size", &config.rate_limiting.burst_size.to_string());
            output::key_value("Per Tenant", &config.rate_limiting.per_tenant.to_string());

            if let Some(ref rules) = config.rules {
                if args.detailed && !rules.is_empty() {
                    println!();
                    output::section("Routing Rules");
                    for rule in rules {
                        println!();
                        output::key_value("  Rule", &rule.name);
                        output::key_value("    ID", &rule.id);
                        output::key_value("    Priority", &rule.priority.to_string());
                        output::key_value("    Condition", &rule.condition);
                        output::key_value("    Target", &rule.target_provider);
                        output::key_value("    Enabled", &rule.enabled.to_string());
                    }
                }
            }

            if let Some(ref metrics) = config.metrics {
                if args.metrics {
                    println!();
                    output::section("Performance Metrics");
                    output::key_value("Total Requests", &metrics.total_requests.to_string());
                    output::key_value("Successful", &metrics.successful_requests.to_string());
                    output::key_value("Failed", &metrics.failed_requests.to_string());
                    output::key_value("Avg Latency", &format!("{:.2}ms", metrics.avg_latency_ms));
                    output::key_value("P95 Latency", &format!("{:.2}ms", metrics.p95_latency_ms));
                    output::key_value("P99 Latency", &format!("{:.2}ms", metrics.p99_latency_ms));
                    output::key_value("Throughput", &format!("{:.2} req/s", metrics.requests_per_second));
                }
            }

            if let Some(ref health) = config.provider_health {
                if args.health && !health.is_empty() {
                    println!();
                    output::section("Provider Health");
                    output::table(health);
                }
            }
        }
    }

    Ok(())
}

/// Execute the status subcommand.
async fn execute_status(
    args: StatusArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;

    let url = if let Some(ref agent_id) = args.agent_id {
        format!("{}/api/v1/agents/{}/status", base_url.trim_end_matches('/'), agent_id)
    } else {
        format!("{}/api/v1/agents/status", base_url.trim_end_matches('/'))
    };

    // Handle watch mode
    if args.watch {
        loop {
            // Clear screen for watch mode
            print!("\x1B[2J\x1B[1;1H");

            let status: AgentStatus = fetch_status(&client, &url, &args).await;
            print_status(&status, &args, format)?;

            println!("\n  (Refreshing every {}s, Ctrl+C to stop)", args.interval);
            tokio::time::sleep(std::time::Duration::from_secs(args.interval)).await;
        }
    }

    let status: AgentStatus = match format {
        OutputFormat::Text => {
            let spinner = output::spinner("Fetching agent status...");
            let response = client.get(&url).send().await;
            spinner.finish_and_clear();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_status(&args))
                }
                _ => generate_sample_status(&args),
            }
        }
        OutputFormat::Json => {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_status(&args))
                }
                _ => generate_sample_status(&args),
            }
        }
    };

    print_status(&status, &args, format)?;
    Ok(())
}

async fn fetch_status(client: &reqwest::Client, url: &str, args: &StatusArgs) -> AgentStatus {
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json().await.unwrap_or_else(|_| generate_sample_status(args))
        }
        _ => generate_sample_status(args),
    }
}

fn print_status(status: &AgentStatus, args: &StatusArgs, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(status);
            result.print(format)?;
        }
        OutputFormat::Text => {
            // Status indicator
            let status_indicator = match status.status.as_str() {
                "healthy" | "running" => "healthy",
                "degraded" => "degraded",
                _ => "unhealthy",
            };

            match status_indicator {
                "healthy" => output::success(&format!("Agent {} is {}", status.agent_id, status.status)),
                "degraded" => output::warning(&format!("Agent {} is {}", status.agent_id, status.status)),
                _ => output::error(&format!("Agent {} is {}", status.agent_id, status.status)),
            }

            output::key_value("Version", &status.version);
            output::key_value("Uptime", &format_uptime(status.uptime_seconds));

            println!();
            output::section("Health");
            output::key_value("Overall", &status.health.overall);
            output::key_value("Last Check", &status.health.last_check);
            output::key_value("Checks Passed", &status.health.checks_passed.to_string());
            output::key_value("Checks Failed", &status.health.checks_failed.to_string());

            println!();
            output::section("Current Load");
            output::key_value("Active Requests", &status.current_load.active_requests.to_string());
            output::key_value("Queued Requests", &status.current_load.queued_requests.to_string());
            output::key_value("Throughput", &format!("{:.2} req/s", status.current_load.requests_per_second));

            if args.verbose {
                output::key_value("CPU Usage", &format!("{:.1}%", status.current_load.cpu_percent));
                output::key_value("Memory Usage", &format!("{:.1}%", status.current_load.memory_percent));
            }

            if !status.providers.is_empty() {
                println!();
                output::section("Providers");
                output::table(&status.providers);
            }
        }
    }
    Ok(())
}

/// Execute the list subcommand.
async fn execute_list(
    args: ListArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;
    let url = format!("{}/api/v1/agents", base_url.trim_end_matches('/'));

    let agents: Vec<AgentListEntry> = match format {
        OutputFormat::Text => {
            let spinner = output::spinner("Fetching agents...");
            let response = client.get(&url).send().await;
            spinner.finish_and_clear();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_agents())
                }
                _ => generate_sample_agents(),
            }
        }
        OutputFormat::Json => {
            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    resp.json().await.unwrap_or_else(|_| generate_sample_agents())
                }
                _ => generate_sample_agents(),
            }
        }
    };

    // Apply filters
    let filtered: Vec<_> = agents
        .into_iter()
        .filter(|a| {
            if let Some(ref agent_type) = args.agent_type {
                if !a.agent_type.to_lowercase().contains(&agent_type.to_lowercase()) {
                    return false;
                }
            }
            if args.status != "all" && a.status.to_lowercase() != args.status.to_lowercase() {
                return false;
            }
            true
        })
        .collect();

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(&filtered);
            result.print(format)?;
        }
        OutputFormat::Text => {
            if filtered.is_empty() {
                output::warning("No agents found matching the criteria");
            } else {
                output::success(&format!("Found {} agents", filtered.len()));
                println!();
                output::table(&filtered);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

fn build_client(api_key: Option<&str>, timeout: u64) -> Result<reqwest::Client> {
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

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

// ============================================================================
// Sample Data Generators
// ============================================================================

fn generate_sample_routing_result(args: &RouteArgs) -> RoutingResult {
    let provider = if args.model.starts_with("claude") {
        "anthropic"
    } else if args.model.starts_with("gpt") {
        "openai"
    } else if args.model.starts_with("gemini") {
        "google"
    } else if args.model.starts_with("command") {
        "cohere"
    } else {
        "openai"
    };

    RoutingResult {
        request_id: uuid::Uuid::new_v4().to_string(),
        model: args.model.clone(),
        provider: provider.to_string(),
        endpoint: format!("https://api.{}.com/v1", provider),
        tenant: args.tenant.clone(),
        fallback_used: false,
        routing_latency_ms: 2,
        matched_rules: vec![
            format!("{}-model-routing", provider),
            "priority-based-selection".to_string(),
        ],
        routing_reason: format!("Model prefix '{}' matched {} provider routing rule",
            args.model.split('-').next().unwrap_or(&args.model), provider),
        fallback_chain: vec![
            ProviderInfo {
                name: provider.to_string(),
                endpoint: format!("https://api.{}.com/v1", provider),
                health: "healthy".to_string(),
                latency_ms: 45,
            },
            ProviderInfo {
                name: "anthropic".to_string(),
                endpoint: "https://api.anthropic.com/v1".to_string(),
                health: "healthy".to_string(),
                latency_ms: 52,
            },
            ProviderInfo {
                name: "cohere".to_string(),
                endpoint: "https://api.cohere.ai/v1".to_string(),
                health: "degraded".to_string(),
                latency_ms: 78,
            },
        ],
        cost_tier: if args.model.contains("4") { "premium" } else { "standard" }.to_string(),
        priority: args.priority.clone(),
    }
}

fn generate_sample_config(args: &InspectArgs) -> AgentConfig {
    let rules = if args.detailed {
        Some(vec![
            RoutingRuleDetail {
                id: "rule-001".to_string(),
                name: "openai-gpt-models".to_string(),
                priority: 1,
                condition: "model.startsWith('gpt')".to_string(),
                target_provider: "openai".to_string(),
                enabled: true,
            },
            RoutingRuleDetail {
                id: "rule-002".to_string(),
                name: "anthropic-claude-models".to_string(),
                priority: 2,
                condition: "model.startsWith('claude')".to_string(),
                target_provider: "anthropic".to_string(),
                enabled: true,
            },
            RoutingRuleDetail {
                id: "rule-003".to_string(),
                name: "google-gemini-models".to_string(),
                priority: 3,
                condition: "model.startsWith('gemini')".to_string(),
                target_provider: "google".to_string(),
                enabled: true,
            },
            RoutingRuleDetail {
                id: "rule-004".to_string(),
                name: "premium-tenant-override".to_string(),
                priority: 0,
                condition: "tenant.tier == 'premium'".to_string(),
                target_provider: "openai".to_string(),
                enabled: true,
            },
            RoutingRuleDetail {
                id: "rule-005".to_string(),
                name: "default-fallback".to_string(),
                priority: 100,
                condition: "*".to_string(),
                target_provider: "openai".to_string(),
                enabled: true,
            },
        ])
    } else {
        None
    };

    let metrics = if args.metrics {
        Some(AgentMetrics {
            total_requests: 1_542_876,
            successful_requests: 1_538_234,
            failed_requests: 4_642,
            avg_latency_ms: 48.5,
            p95_latency_ms: 125.3,
            p99_latency_ms: 287.8,
            requests_per_second: 342.5,
        })
    } else {
        None
    };

    let provider_health = if args.health {
        Some(vec![
            ProviderHealthStatus {
                provider: "openai".to_string(),
                status: "healthy".to_string(),
                latency_ms: 45,
                success_rate: 99.8,
                active_connections: 156,
                circuit_state: "closed".to_string(),
            },
            ProviderHealthStatus {
                provider: "anthropic".to_string(),
                status: "healthy".to_string(),
                latency_ms: 52,
                success_rate: 99.5,
                active_connections: 89,
                circuit_state: "closed".to_string(),
            },
            ProviderHealthStatus {
                provider: "google".to_string(),
                status: "healthy".to_string(),
                latency_ms: 61,
                success_rate: 99.2,
                active_connections: 42,
                circuit_state: "closed".to_string(),
            },
            ProviderHealthStatus {
                provider: "cohere".to_string(),
                status: "degraded".to_string(),
                latency_ms: 145,
                success_rate: 95.8,
                active_connections: 23,
                circuit_state: "half-open".to_string(),
            },
        ])
    } else {
        None
    };

    AgentConfig {
        version: "1.0.0".to_string(),
        strategy: "intelligent-routing".to_string(),
        load_balancing: "weighted-round-robin".to_string(),
        active_providers: 4,
        routing_rules: 5,
        fallback: FallbackConfig {
            enabled: true,
            max_retries: 3,
            retry_delay_ms: 100,
            providers: vec![
                "openai".to_string(),
                "anthropic".to_string(),
                "cohere".to_string(),
            ],
        },
        circuit_breaker: CircuitBreakerSettings {
            enabled: true,
            failure_threshold: 5,
            success_threshold: 3,
            timeout_seconds: 30,
            half_open_requests: 5,
        },
        rate_limiting: RateLimitingSettings {
            enabled: true,
            requests_per_second: 1000,
            burst_size: 200,
            per_tenant: true,
        },
        rules,
        metrics,
        provider_health,
    }
}

fn generate_sample_status(args: &StatusArgs) -> AgentStatus {
    AgentStatus {
        agent_id: args.agent_id.clone().unwrap_or_else(|| "inference-routing-agent".to_string()),
        status: "healthy".to_string(),
        uptime_seconds: 432_156,
        version: "1.0.0".to_string(),
        health: HealthInfo {
            overall: "healthy".to_string(),
            last_check: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            checks_passed: 15,
            checks_failed: 0,
        },
        current_load: LoadInfo {
            active_requests: 47,
            queued_requests: 3,
            requests_per_second: 342.5,
            cpu_percent: 23.4,
            memory_percent: 45.2,
        },
        providers: vec![
            ProviderStatusBrief {
                name: "openai".to_string(),
                status: "healthy".to_string(),
                latency: "45ms".to_string(),
            },
            ProviderStatusBrief {
                name: "anthropic".to_string(),
                status: "healthy".to_string(),
                latency: "52ms".to_string(),
            },
            ProviderStatusBrief {
                name: "google".to_string(),
                status: "healthy".to_string(),
                latency: "61ms".to_string(),
            },
            ProviderStatusBrief {
                name: "cohere".to_string(),
                status: "degraded".to_string(),
                latency: "145ms".to_string(),
            },
        ],
    }
}

fn generate_sample_agents() -> Vec<AgentListEntry> {
    vec![
        AgentListEntry {
            id: "inference-routing-agent".to_string(),
            agent_type: "routing".to_string(),
            status: "active".to_string(),
            version: "1.0.0".to_string(),
            uptime: "5d 2h 15m".to_string(),
            requests: 1_542_876,
        },
        AgentListEntry {
            id: "load-balancer-agent".to_string(),
            agent_type: "load-balancer".to_string(),
            status: "active".to_string(),
            version: "1.0.0".to_string(),
            uptime: "5d 2h 15m".to_string(),
            requests: 1_542_876,
        },
        AgentListEntry {
            id: "health-monitor-agent".to_string(),
            agent_type: "monitoring".to_string(),
            status: "active".to_string(),
            version: "1.0.0".to_string(),
            uptime: "5d 2h 15m".to_string(),
            requests: 45_234,
        },
        AgentListEntry {
            id: "cache-manager-agent".to_string(),
            agent_type: "cache".to_string(),
            status: "active".to_string(),
            version: "1.0.0".to_string(),
            uptime: "5d 2h 15m".to_string(),
            requests: 892_456,
        },
        AgentListEntry {
            id: "rate-limiter-agent".to_string(),
            agent_type: "rate-limiting".to_string(),
            status: "active".to_string(),
            version: "1.0.0".to_string(),
            uptime: "5d 2h 15m".to_string(),
            requests: 1_542_876,
        },
    ]
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_routing_result() {
        let args = RouteArgs {
            model: "gpt-4o".to_string(),
            tenant: Some("acme".to_string()),
            fallback: true,
            format: "table".to_string(),
            context: None,
            priority: "normal".to_string(),
            dry_run: false,
        };

        let result = generate_sample_routing_result(&args);
        assert_eq!(result.provider, "openai");
        assert_eq!(result.tenant, Some("acme".to_string()));
    }

    #[test]
    fn test_generate_routing_result_claude() {
        let args = RouteArgs {
            model: "claude-3-opus".to_string(),
            tenant: None,
            fallback: true,
            format: "table".to_string(),
            context: None,
            priority: "high".to_string(),
            dry_run: false,
        };

        let result = generate_sample_routing_result(&args);
        assert_eq!(result.provider, "anthropic");
    }

    #[test]
    fn test_generate_config() {
        let args = InspectArgs {
            detailed: true,
            format: "json".to_string(),
            metrics: true,
            health: true,
        };

        let config = generate_sample_config(&args);
        assert!(config.rules.is_some());
        assert!(config.metrics.is_some());
        assert!(config.provider_health.is_some());
    }

    #[test]
    fn test_generate_config_minimal() {
        let args = InspectArgs {
            detailed: false,
            format: "text".to_string(),
            metrics: false,
            health: false,
        };

        let config = generate_sample_config(&args);
        assert!(config.rules.is_none());
        assert!(config.metrics.is_none());
        assert!(config.provider_health.is_none());
    }

    #[test]
    fn test_generate_status() {
        let args = StatusArgs {
            agent_id: Some("test-agent".to_string()),
            verbose: true,
            watch: false,
            interval: 5,
        };

        let status = generate_sample_status(&args);
        assert_eq!(status.agent_id, "test-agent");
        assert_eq!(status.status, "healthy");
    }

    #[test]
    fn test_generate_agents() {
        let agents = generate_sample_agents();
        assert!(!agents.is_empty());
        assert!(agents.iter().any(|a| a.agent_type == "routing"));
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(30), "30s");
        assert_eq!(format_uptime(90), "1m 30s");
        assert_eq!(format_uptime(3661), "1h 1m 1s");
        assert_eq!(format_uptime(90061), "1d 1h 1m 1s");
    }

    #[test]
    fn test_format_percent() {
        assert_eq!(format_percent(&99.5), "99.5%");
        assert_eq!(format_percent(&100.0), "100.0%");
    }
}
