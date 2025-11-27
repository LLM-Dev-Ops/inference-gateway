//! # LLM Inference Gateway
//!
//! Enterprise-grade, OpenAI-compatible API gateway for Large Language Models.
//!
//! ## Features
//!
//! - Multi-provider support (OpenAI, Anthropic, etc.)
//! - Intelligent routing and load balancing
//! - Circuit breaker and retry patterns
//! - Prometheus metrics and distributed tracing
//! - Hot configuration reload
//!
//! ## Usage
//!
//! ```bash
//! # Start with default configuration
//! llm-inference-gateway
//!
//! # Start with custom config file
//! llm-inference-gateway --config /path/to/config.yaml
//!
//! # Start with environment overrides
//! GATEWAY_PORT=9000 llm-inference-gateway
//! ```

use gateway_config::{load_config, GatewayConfig};
use gateway_core::ProviderType;
use gateway_providers::{AnthropicProvider, OpenAIProvider, ProviderRegistry};
use gateway_resilience::RetryPolicy;
use gateway_routing::{Router, RouterConfig};
use gateway_server::{AppState, Server, ServerConfig};
use gateway_telemetry::{init_logging, LoggingConfig, Metrics, MetricsConfig};
use std::env;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Application entry point
#[tokio::main]
async fn main() {
    // Initialize logging first
    if let Err(e) = init_logging(&LoggingConfig::new().with_level("info")) {
        eprintln!("Failed to initialize logging: {}", e);
    }

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting LLM Inference Gateway"
    );

    // Run the application
    if let Err(e) = run().await {
        error!(error = %e, "Application failed");
        std::process::exit(1);
    }
}

/// Main application logic
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = load_config().await?;

    info!(
        host = %config.server.host,
        port = config.server.port,
        "Configuration loaded"
    );

    // Initialize metrics
    let metrics = Metrics::new(&MetricsConfig::default())?;

    // Create provider registry
    let registry = create_provider_registry(&config)?;

    info!(
        providers = registry.len(),
        "Provider registry initialized"
    );

    // Create router
    let router_config = RouterConfig::new().with_default_providers(registry.provider_ids());
    let router = Router::new(router_config);

    // Register providers with router
    for id in registry.provider_ids() {
        if let Some(provider) = registry.get(&id) {
            router.register_provider(provider, 100, 100);
            router.update_health(&id, gateway_core::HealthStatus::Healthy);
        }
    }

    // Create retry policy
    let retry_policy = RetryPolicy::with_defaults();

    // Build application state
    let state = AppState::builder()
        .config(config.clone())
        .providers(registry)
        .router(router)
        .retry_policy(retry_policy)
        .metrics(metrics)
        .build();

    // Create server
    let server_config = ServerConfig::new()
        .with_host(&config.server.host)
        .with_port(config.server.port);

    let server = Server::new(server_config, state);

    // Run server
    server.run().await?;

    Ok(())
}

/// Create provider registry from configuration
fn create_provider_registry(
    config: &GatewayConfig,
) -> Result<ProviderRegistry, Box<dyn std::error::Error>> {
    let registry = ProviderRegistry::new();

    // Register OpenAI provider if API key is available
    if let Ok(api_key) = env::var("OPENAI_API_KEY") {
        info!("Registering OpenAI provider from environment");

        let openai_config =
            gateway_providers::openai::OpenAIConfig::new("openai", api_key);
        let provider = OpenAIProvider::new(openai_config)?;
        registry.register(Arc::new(provider), 100, 100)?;
    } else {
        warn!("OPENAI_API_KEY not set, OpenAI provider not available");
    }

    // Register Anthropic provider if API key is available
    if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
        info!("Registering Anthropic provider from environment");

        let anthropic_config =
            gateway_providers::anthropic::AnthropicConfig::new(api_key);
        let provider = AnthropicProvider::new(anthropic_config)?;
        registry.register(Arc::new(provider), 100, 100)?;
    } else {
        warn!("ANTHROPIC_API_KEY not set, Anthropic provider not available");
    }

    // Register providers from config file
    for provider_config in &config.providers {
        if !provider_config.enabled {
            continue;
        }

        let api_key = provider_config
            .api_key
            .clone()
            .or_else(|| provider_config.api_key_env.as_ref().and_then(|var| env::var(var).ok()));

        if api_key.is_none() {
            warn!(
                provider = %provider_config.id,
                "Provider has no API key configured, skipping"
            );
            continue;
        }

        let api_key = api_key.expect("api key exists");

        match provider_config.provider_type {
            ProviderType::OpenAI => {
                if registry.get(&provider_config.id).is_none() {
                    let mut openai_config = gateway_providers::openai::OpenAIConfig::new(
                        &provider_config.id,
                        &api_key,
                    );
                    if !provider_config.endpoint.is_empty() {
                        openai_config = openai_config.with_base_url(&provider_config.endpoint);
                    }
                    let provider = OpenAIProvider::new(openai_config)?;
                    registry.register(
                        Arc::new(provider),
                        provider_config.priority,
                        provider_config.weight,
                    )?;
                }
            }
            ProviderType::Anthropic => {
                if registry.get(&provider_config.id).is_none() {
                    let mut anthropic_config =
                        gateway_providers::anthropic::AnthropicConfig::new(&api_key);
                    if !provider_config.endpoint.is_empty() {
                        anthropic_config =
                            anthropic_config.with_base_url(&provider_config.endpoint);
                    }
                    let provider =
                        AnthropicProvider::with_id(&provider_config.id, anthropic_config)?;
                    registry.register(
                        Arc::new(provider),
                        provider_config.priority,
                        provider_config.weight,
                    )?;
                }
            }
            _ => {
                warn!(
                    provider = %provider_config.id,
                    provider_type = ?provider_config.provider_type,
                    "Unknown or unsupported provider type"
                );
            }
        }
    }

    Ok(registry)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        // Basic test to ensure the binary compiles
        assert!(true);
    }
}
