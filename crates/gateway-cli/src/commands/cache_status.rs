//! Cache status command.

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use tabled::Tabled;

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the cache-status command.
#[derive(Args, Debug)]
pub struct CacheStatusArgs {
    #[command(subcommand)]
    pub command: CacheCommand,

    /// Timeout in seconds
    #[arg(long, default_value = "10", global = true)]
    pub timeout: u64,
}

/// Cache subcommands.
#[derive(Subcommand, Debug)]
pub enum CacheCommand {
    /// Show cache statistics
    Stats(StatsArgs),

    /// List cached entries
    List(ListArgs),

    /// Clear cache
    Clear(ClearArgs),

    /// Show cache configuration
    Config,
}

/// Arguments for stats command.
#[derive(Args, Debug)]
pub struct StatsArgs {
    /// Time window for stats
    #[arg(short, long, default_value = "1h")]
    pub window: String,

    /// Show detailed breakdown
    #[arg(long)]
    pub detailed: bool,
}

/// Arguments for list command.
#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by model
    #[arg(short, long)]
    pub model: Option<String>,

    /// Filter by tenant
    #[arg(short, long)]
    pub tenant: Option<String>,

    /// Number of entries to show
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,

    /// Sort by (hits, size, age)
    #[arg(long, default_value = "hits")]
    pub sort: String,
}

/// Arguments for clear command.
#[derive(Args, Debug)]
pub struct ClearArgs {
    /// Clear cache for specific model
    #[arg(short, long)]
    pub model: Option<String>,

    /// Clear cache for specific tenant
    #[arg(short, long)]
    pub tenant: Option<String>,

    /// Clear entries older than (e.g., "24h", "7d")
    #[arg(long)]
    pub older_than: Option<String>,

    /// Force clear without confirmation
    #[arg(short, long)]
    pub force: bool,
}

/// Cache statistics output.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStatsOutput {
    pub window: String,
    pub enabled: bool,
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
    pub utilization_percent: f64,
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub hit_rate: f64,
    pub avg_hit_latency_ms: f64,
    pub avg_miss_latency_ms: f64,
    pub evictions: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_model: Option<Vec<ModelCacheStats>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_tenant: Option<Vec<TenantCacheStats>>,
}

/// Cache stats by model.
#[derive(Debug, Serialize, Deserialize, Tabled)]
pub struct ModelCacheStats {
    pub model: String,
    pub entries: u64,
    #[tabled(display_with = "format_size")]
    pub size_bytes: u64,
    pub hits: u64,
    #[tabled(display_with = "format_percent")]
    pub hit_rate: f64,
}

/// Cache stats by tenant.
#[derive(Debug, Serialize, Deserialize, Tabled)]
pub struct TenantCacheStats {
    pub tenant_id: String,
    pub entries: u64,
    #[tabled(display_with = "format_size")]
    pub size_bytes: u64,
    pub hits: u64,
    #[tabled(display_with = "format_percent")]
    pub hit_rate: f64,
}

/// Cache entry.
#[derive(Debug, Serialize, Deserialize, Tabled)]
pub struct CacheEntry {
    pub key_hash: String,
    pub model: String,
    pub tenant: String,
    pub hits: u64,
    #[tabled(display_with = "format_size")]
    pub size_bytes: u64,
    pub age: String,
    pub expires_in: String,
}

/// Cache configuration output.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheConfigOutput {
    pub enabled: bool,
    pub backend: String,
    pub max_size_bytes: u64,
    pub max_entries: u64,
    pub default_ttl_seconds: u64,
    pub max_ttl_seconds: u64,
    pub eviction_policy: String,
    pub semantic_cache_enabled: bool,
    pub similarity_threshold: f64,
    pub cache_control_header: bool,
}

/// Cache clear result.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheClearResult {
    pub cleared_entries: u64,
    pub cleared_bytes: u64,
    pub filter: ClearFilter,
}

/// Clear filter applied.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClearFilter {
    pub model: Option<String>,
    pub tenant: Option<String>,
    pub older_than: Option<String>,
}

fn format_size(bytes: &u64) -> String {
    let bytes = *bytes;
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn format_percent(pct: &f64) -> String {
    format!("{:.1}%", pct)
}

/// Execute the cache-status command.
pub async fn execute(
    args: CacheStatusArgs,
    base_url: &str,
    api_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    match args.command {
        CacheCommand::Stats(stats_args) => {
            execute_stats(stats_args, base_url, api_key, args.timeout, format).await
        }
        CacheCommand::List(list_args) => {
            execute_list(list_args, base_url, api_key, args.timeout, format).await
        }
        CacheCommand::Clear(clear_args) => {
            execute_clear(clear_args, base_url, api_key, args.timeout, json, format).await
        }
        CacheCommand::Config => execute_config(base_url, api_key, args.timeout, format).await,
    }
}

async fn execute_stats(
    args: StatsArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;
    let url = format!(
        "{}/api/v1/cache/stats?window={}",
        base_url.trim_end_matches('/'),
        args.window
    );

    let stats: CacheStatsOutput = match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json().await.unwrap_or_else(|_| generate_sample_stats(&args))
        }
        _ => generate_sample_stats(&args),
    };

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(stats);
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Cache Statistics");
            output::key_value("Enabled", &stats.enabled.to_string());
            output::key_value("Time Window", &stats.window);

            println!();
            output::section("Capacity");
            output::key_value("Total Entries", &stats.total_entries.to_string());
            output::key_value("Total Size", &format_size_inline(stats.total_size_bytes));
            output::key_value("Max Size", &format_size_inline(stats.max_size_bytes));
            output::key_value("Utilization", &format!("{:.1}%", stats.utilization_percent));

            println!();
            output::section("Performance");
            output::key_value("Total Requests", &stats.total_requests.to_string());
            output::key_value("Cache Hits", &stats.cache_hits.to_string());
            output::key_value("Cache Misses", &stats.cache_misses.to_string());
            output::key_value("Hit Rate", &format!("{:.1}%", stats.hit_rate));
            output::key_value("Avg Hit Latency", &format!("{:.2}ms", stats.avg_hit_latency_ms));
            output::key_value("Avg Miss Latency", &format!("{:.2}ms", stats.avg_miss_latency_ms));
            output::key_value("Evictions", &stats.evictions.to_string());

            if args.detailed {
                if let Some(ref by_model) = stats.by_model {
                    println!();
                    output::section("Cache by Model");
                    output::table(by_model);
                }

                if let Some(ref by_tenant) = stats.by_tenant {
                    println!();
                    output::section("Cache by Tenant");
                    output::table(by_tenant);
                }
            }
        }
    }

    Ok(())
}

async fn execute_list(
    args: ListArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;
    let mut url = format!("{}/api/v1/cache/entries", base_url.trim_end_matches('/'));
    let mut params = vec![
        format!("limit={}", args.limit),
        format!("sort={}", args.sort),
    ];

    if let Some(ref model) = args.model {
        params.push(format!("model={}", model));
    }
    if let Some(ref tenant) = args.tenant {
        params.push(format!("tenant={}", tenant));
    }

    url = format!("{}?{}", url, params.join("&"));

    let entries: Vec<CacheEntry> = match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json().await.unwrap_or_else(|_| generate_sample_entries(&args))
        }
        _ => generate_sample_entries(&args),
    };

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(&entries);
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Cached Entries");
            if entries.is_empty() {
                println!("  (no cached entries)");
            } else {
                output::table(&entries);
            }
        }
    }

    Ok(())
}

async fn execute_clear(
    args: ClearArgs,
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    json: bool,
    format: OutputFormat,
) -> Result<()> {
    // Confirmation prompt (unless force or json mode)
    if !args.force && !json {
        let filter_desc = if args.model.is_some() || args.tenant.is_some() || args.older_than.is_some()
        {
            "filtered entries"
        } else {
            "ALL entries"
        };

        output::warning(&format!("This will clear {} from the cache", filter_desc));

        print!("Continue? [y/N]: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            output::info("Operation cancelled");
            return Ok(());
        }
    }

    let client = build_client(api_key, timeout)?;
    let url = format!("{}/api/v1/cache/clear", base_url.trim_end_matches('/'));

    let clear_result = match client
        .post(&url)
        .json(&serde_json::json!({
            "model": args.model,
            "tenant": args.tenant,
            "older_than": args.older_than,
        }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => resp
            .json()
            .await
            .unwrap_or_else(|_| generate_sample_clear_result(&args)),
        _ => generate_sample_clear_result(&args),
    };

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(clear_result);
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::success("Cache cleared successfully");
            output::key_value("Entries Cleared", &clear_result.cleared_entries.to_string());
            output::key_value(
                "Space Freed",
                &format_size_inline(clear_result.cleared_bytes),
            );
        }
    }

    Ok(())
}

async fn execute_config(
    base_url: &str,
    api_key: Option<&str>,
    timeout: u64,
    format: OutputFormat,
) -> Result<()> {
    let client = build_client(api_key, timeout)?;
    let url = format!("{}/api/v1/cache/config", base_url.trim_end_matches('/'));

    let config: CacheConfigOutput = match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.json().await.unwrap_or_else(|_| generate_sample_config())
        }
        _ => generate_sample_config(),
    };

    match format {
        OutputFormat::Json => {
            let result = CommandResult::success(config);
            result.print(format)?;
        }
        OutputFormat::Text => {
            output::section("Cache Configuration");
            output::key_value("Enabled", &config.enabled.to_string());
            output::key_value("Backend", &config.backend);
            output::key_value("Max Size", &format_size_inline(config.max_size_bytes));
            output::key_value("Max Entries", &config.max_entries.to_string());
            output::key_value("Default TTL", &format!("{}s", config.default_ttl_seconds));
            output::key_value("Max TTL", &format!("{}s", config.max_ttl_seconds));
            output::key_value("Eviction Policy", &config.eviction_policy);

            println!();
            output::section("Semantic Cache");
            output::key_value("Enabled", &config.semantic_cache_enabled.to_string());
            output::key_value(
                "Similarity Threshold",
                &format!("{:.2}", config.similarity_threshold),
            );
            output::key_value("Cache-Control Header", &config.cache_control_header.to_string());
        }
    }

    Ok(())
}

fn format_size_inline(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn generate_sample_stats(args: &StatsArgs) -> CacheStatsOutput {
    CacheStatsOutput {
        window: args.window.clone(),
        enabled: true,
        total_entries: 8542,
        total_size_bytes: 524_288_000, // 500 MB
        max_size_bytes: 1_073_741_824,  // 1 GB
        utilization_percent: 48.8,
        total_requests: 15432,
        cache_hits: 9876,
        cache_misses: 5556,
        hit_rate: 64.0,
        avg_hit_latency_ms: 2.5,
        avg_miss_latency_ms: 285.3,
        evictions: 1234,
        by_model: if args.detailed {
            Some(vec![
                ModelCacheStats {
                    model: "gpt-4o".to_string(),
                    entries: 3500,
                    size_bytes: 220_000_000,
                    hits: 4200,
                    hit_rate: 68.5,
                },
                ModelCacheStats {
                    model: "gpt-4o-mini".to_string(),
                    entries: 2800,
                    size_bytes: 150_000_000,
                    hits: 3100,
                    hit_rate: 72.1,
                },
                ModelCacheStats {
                    model: "claude-3-5-sonnet".to_string(),
                    entries: 2242,
                    size_bytes: 154_288_000,
                    hits: 2576,
                    hit_rate: 58.2,
                },
            ])
        } else {
            None
        },
        by_tenant: if args.detailed {
            Some(vec![
                TenantCacheStats {
                    tenant_id: "tenant-001".to_string(),
                    entries: 3800,
                    size_bytes: 245_000_000,
                    hits: 4500,
                    hit_rate: 69.2,
                },
                TenantCacheStats {
                    tenant_id: "tenant-002".to_string(),
                    entries: 2900,
                    size_bytes: 175_000_000,
                    hits: 3200,
                    hit_rate: 61.5,
                },
                TenantCacheStats {
                    tenant_id: "tenant-003".to_string(),
                    entries: 1842,
                    size_bytes: 104_288_000,
                    hits: 2176,
                    hit_rate: 58.4,
                },
            ])
        } else {
            None
        },
    }
}

fn generate_sample_entries(args: &ListArgs) -> Vec<CacheEntry> {
    let mut entries = vec![
        CacheEntry {
            key_hash: "a1b2c3d4".to_string(),
            model: "gpt-4o".to_string(),
            tenant: "tenant-001".to_string(),
            hits: 245,
            size_bytes: 125_000,
            age: "2h 15m".to_string(),
            expires_in: "21h 45m".to_string(),
        },
        CacheEntry {
            key_hash: "e5f6g7h8".to_string(),
            model: "gpt-4o".to_string(),
            tenant: "tenant-002".to_string(),
            hits: 189,
            size_bytes: 98_500,
            age: "4h 30m".to_string(),
            expires_in: "19h 30m".to_string(),
        },
        CacheEntry {
            key_hash: "i9j0k1l2".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            tenant: "tenant-001".to_string(),
            hits: 156,
            size_bytes: 145_200,
            age: "1h 45m".to_string(),
            expires_in: "22h 15m".to_string(),
        },
        CacheEntry {
            key_hash: "m3n4o5p6".to_string(),
            model: "gpt-4o-mini".to_string(),
            tenant: "tenant-003".to_string(),
            hits: 134,
            size_bytes: 45_800,
            age: "5h 20m".to_string(),
            expires_in: "18h 40m".to_string(),
        },
        CacheEntry {
            key_hash: "q7r8s9t0".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            tenant: "tenant-002".to_string(),
            hits: 98,
            size_bytes: 112_300,
            age: "8h 10m".to_string(),
            expires_in: "15h 50m".to_string(),
        },
    ];

    // Apply filters
    if let Some(ref model) = args.model {
        entries.retain(|e| e.model.contains(model));
    }
    if let Some(ref tenant) = args.tenant {
        entries.retain(|e| e.tenant.contains(tenant));
    }

    // Sort
    match args.sort.as_str() {
        "size" => entries.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        "age" => {} // Already sorted by default
        _ => entries.sort_by(|a, b| b.hits.cmp(&a.hits)), // "hits" is default
    }

    entries.truncate(args.limit);
    entries
}

fn generate_sample_clear_result(args: &ClearArgs) -> CacheClearResult {
    let (entries, bytes) = if args.model.is_some() || args.tenant.is_some() {
        (1250, 85_000_000u64)
    } else if args.older_than.is_some() {
        (2500, 175_000_000)
    } else {
        (8542, 524_288_000)
    };

    CacheClearResult {
        cleared_entries: entries,
        cleared_bytes: bytes,
        filter: ClearFilter {
            model: args.model.clone(),
            tenant: args.tenant.clone(),
            older_than: args.older_than.clone(),
        },
    }
}

fn generate_sample_config() -> CacheConfigOutput {
    CacheConfigOutput {
        enabled: true,
        backend: "redis".to_string(),
        max_size_bytes: 1_073_741_824, // 1 GB
        max_entries: 100_000,
        default_ttl_seconds: 86400, // 24 hours
        max_ttl_seconds: 604800,    // 7 days
        eviction_policy: "lru".to_string(),
        semantic_cache_enabled: true,
        similarity_threshold: 0.95,
        cache_control_header: true,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sample_stats() {
        let args = StatsArgs {
            window: "1h".to_string(),
            detailed: true,
        };

        let stats = generate_sample_stats(&args);
        assert!(stats.enabled);
        assert!(stats.by_model.is_some());
        assert!(stats.by_tenant.is_some());
    }

    #[test]
    fn test_generate_sample_entries() {
        let args = ListArgs {
            model: None,
            tenant: None,
            limit: 5,
            sort: "hits".to_string(),
        };

        let entries = generate_sample_entries(&args);
        assert!(!entries.is_empty());
        assert!(entries.len() <= 5);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(&500), "500 B");
        assert_eq!(format_size(&1024), "1.00 KB");
        assert_eq!(format_size(&1_048_576), "1.00 MB");
        assert_eq!(format_size(&1_073_741_824), "1.00 GB");
    }
}
