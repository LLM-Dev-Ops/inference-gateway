# Provider Abstraction Layer - Quick Start Guide

## Table of Contents
1. [Basic Usage](#basic-usage)
2. [Provider Configuration](#provider-configuration)
3. [Advanced Features](#advanced-features)
4. [Common Patterns](#common-patterns)
5. [Error Handling](#error-handling)

---

## Basic Usage

### Initialize the System

```rust
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Create connection pool
    let pool_config = ConnectionPoolConfig::default();
    let connection_pool = Arc::new(ConnectionPool::new(pool_config));

    // 2. Create provider registry
    let registry = Arc::new(ProviderRegistry::new(Duration::from_secs(60)));

    // 3. Start health monitoring
    registry.start_health_checks().await;

    // 4. Create provider factory
    let factory = ProviderFactory::new(Arc::clone(&connection_pool));

    // Ready to register providers!
    Ok(())
}
```

### Register a Provider

```rust
// OpenAI
let openai = factory.create_openai(OpenAIConfig {
    api_key: std::env::var("OPENAI_API_KEY")?,
    base_url: "https://api.openai.com".to_string(),
    organization: None,
    timeout: Duration::from_secs(60),
    retry_config: RetryConfig::default(),
});

registry.register(openai).await?;

// Anthropic
let anthropic = factory.create_anthropic(AnthropicConfig {
    api_key: std::env::var("ANTHROPIC_API_KEY")?,
    base_url: "https://api.anthropic.com".to_string(),
    api_version: "2023-06-01".to_string(),
    timeout: Duration::from_secs(60),
    retry_config: RetryConfig::default(),
});

registry.register(anthropic).await?;
```

### Make a Simple Request

```rust
use std::collections::HashMap;

// Get a provider
let provider = registry.get("openai").await
    .ok_or_else(|| anyhow!("Provider not found"))?;

// Create request
let request = GatewayRequest {
    request_id: uuid::Uuid::new_v4().to_string(),
    model: "gpt-4-turbo".to_string(),
    messages: vec![
        Message {
            role: MessageRole::User,
            content: MessageContent::Text("Explain quantum computing".to_string()),
            name: None,
        }
    ],
    temperature: Some(0.7),
    max_tokens: Some(500),
    top_p: None,
    top_k: None,
    stop_sequences: None,
    stream: false,
    system: Some("You are a helpful physics tutor.".to_string()),
    tools: None,
    tool_choice: None,
    metadata: HashMap::new(),
    timeout: None,
};

// Get response
let response = provider.chat_completion(&request).await?;

println!("Response: {}",
    match &response.choices[0].message.content {
        MessageContent::Text(text) => text,
        _ => "Non-text response",
    }
);

println!("Tokens used: {}", response.usage.total_tokens);
```

### Streaming Request

```rust
use futures::stream::StreamExt;

let mut request = GatewayRequest {
    // ... same as above
    stream: true,  // Enable streaming
};

let mut stream = provider.chat_completion_stream(&request).await?;

print!("Response: ");
while let Some(chunk_result) = stream.next().await {
    match chunk_result {
        Ok(chunk) => {
            if let Some(content) = chunk.delta.content {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout())?;
            }

            if let Some(usage) = chunk.usage {
                println!("\n\nTokens: {}", usage.total_tokens);
            }
        }
        Err(e) => eprintln!("Stream error: {}", e),
    }
}
println!();
```

---

## Provider Configuration

### OpenAI with Full Options

```rust
let openai_config = OpenAIConfig {
    api_key: std::env::var("OPENAI_API_KEY")?,
    base_url: "https://api.openai.com".to_string(),
    organization: Some("org-xyz123".to_string()),
    timeout: Duration::from_secs(120),
    retry_config: RetryConfig {
        max_retries: 5,
        initial_backoff: Duration::from_millis(200),
        max_backoff: Duration::from_secs(30),
        backoff_multiplier: 2.0,
    },
};

let openai = factory.create_openai(openai_config);
```

### Anthropic with Claude Models

```rust
let anthropic_config = AnthropicConfig {
    api_key: std::env::var("ANTHROPIC_API_KEY")?,
    base_url: "https://api.anthropic.com".to_string(),
    api_version: "2023-06-01".to_string(),
    timeout: Duration::from_secs(300), // Longer for big contexts
    retry_config: RetryConfig::default(),
};

let request = GatewayRequest {
    model: "claude-3-opus-20240229".to_string(),
    // ... rest of config
};
```

### vLLM (Local/Self-Hosted)

```rust
let vllm_config = VLLMConfig {
    base_url: "http://localhost:8000".to_string(),
    api_key: None, // Usually no auth for local
    timeout: Duration::from_secs(60),
    available_models: vec![
        "meta-llama/Meta-Llama-3-70B-Instruct".to_string(),
    ],
};

let vllm = VLLMProvider::new(vllm_config, Arc::clone(&connection_pool));
registry.register(Arc::new(vllm)).await?;
```

### Ollama (Local)

```rust
let ollama_config = OllamaConfig {
    base_url: "http://localhost:11434".to_string(),
    timeout: Duration::from_secs(120),
};

let ollama = OllamaProvider::new(ollama_config, Arc::clone(&connection_pool));

// Check available models
let models = ollama.list_models().await?;
println!("Available Ollama models: {:?}", models);

registry.register(Arc::new(ollama)).await?;
```

### Azure OpenAI

```rust
let azure_config = AzureOpenAIConfig {
    api_key: std::env::var("AZURE_OPENAI_API_KEY")?,
    endpoint: "https://your-resource.openai.azure.com".to_string(),
    deployment_name: "gpt-4-deployment".to_string(),
    api_version: "2024-02-15-preview".to_string(),
    timeout: Duration::from_secs(120),
};

let azure = AzureOpenAIProvider::new(azure_config, Arc::clone(&connection_pool));
registry.register(Arc::new(azure)).await?;
```

### AWS Bedrock

```rust
let bedrock_config = BedrockConfig {
    region: "us-east-1".to_string(),
    model_id: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
    timeout: Duration::from_secs(120),
};

let bedrock = BedrockProvider::new(bedrock_config).await;
registry.register(Arc::new(bedrock)).await?;
```

---

## Advanced Features

### Circuit Breaker

```rust
// Wrap provider with circuit breaker
let resilient_provider = CircuitBreakerProvider::with_config(
    base_provider,
    5,  // Open after 5 failures
    2,  // Close after 2 successes in half-open
    Duration::from_secs(30), // Retry after 30s
);

registry.register(Arc::new(resilient_provider)).await?;

// Check circuit state
let state = resilient_provider.circuit_breaker.get_state().await;
println!("Circuit state: {:?}", state);
```

### Response Caching

```rust
// Create cache
let cache = Arc::new(ResponseCache::new(
    Duration::from_secs(300), // 5 minute TTL
    1000, // Max 1000 entries
));

// Wrap provider
let cached_provider = CachedProvider::new(base_provider, Arc::clone(&cache));

registry.register(Arc::new(cached_provider)).await?;

// Cache stats
println!("Cache size: {}", cache.size().await);

// Manual cache operations
let cache_key = CacheKey::from_request(&request);
cache.invalidate(&cache_key).await; // Clear specific entry
cache.clear().await; // Clear all
```

### Load Balancing

```rust
// Create load balancer (choose strategy)
let balancer: Arc<dyn LoadBalancer> = Arc::new(
    LatencyWeightedBalancer::new(100) // Track last 100 requests
);

// Alternative strategies:
// Arc::new(RoundRobinBalancer::new())
// Arc::new(LeastConnectionsBalancer::new())

// Create load-balanced pool
let pool = LoadBalancedProvider::new(Arc::clone(&balancer));

// Add providers
pool.add_provider(openai_provider).await;
pool.add_provider(anthropic_provider).await;
pool.add_provider(azure_provider).await;

// Use pool - automatically selects best provider
let response = pool.chat_completion(&request).await?;
```

### Observability

```rust
use prometheus::Registry;

// Create metrics
let metrics_registry = Registry::new();
let provider_metrics = Arc::new(
    ProviderMetrics::new("openai", &metrics_registry)?
);

// Wrap provider
let observable_provider = ObservableProvider::new(
    base_provider,
    Arc::clone(&provider_metrics)
);

registry.register(Arc::new(observable_provider)).await?;

// Metrics are automatically recorded
// - llm_requests_total
// - llm_requests_success
// - llm_requests_failure
// - llm_request_duration_seconds
// - llm_tokens_prompt
// - llm_tokens_completion
// - llm_active_connections
// - llm_cache_hits
// - llm_cache_misses

// Expose metrics endpoint
use prometheus::Encoder;
let encoder = prometheus::TextEncoder::new();
let metric_families = metrics_registry.gather();
let mut buffer = Vec::new();
encoder.encode(&metric_families, &mut buffer)?;
println!("{}", String::from_utf8(buffer)?);
```

### Distributed Tracing

```rust
use tracing::{info, instrument};

// Wrap provider with tracing
let traced_provider = TracedProvider::new(base_provider);

registry.register(Arc::new(traced_provider)).await?;

// All requests are automatically traced with:
// - provider ID
// - model name
// - request ID
// - duration
// - token counts
// - errors
```

### Complete Production Stack

```rust
let production_provider = ProviderStackBuilder::new(base_provider)
    .with_tracing()          // Distributed tracing
    .with_circuit_breaker()  // Fault tolerance
    .with_cache(cache)       // Response caching
    .with_metrics(metrics)   // Prometheus metrics
    .build();

registry.register(production_provider).await?;
```

### Fallback Chain

```rust
// Create fallback chain
let fallback_provider = FallbackProvider::new(openai_provider)
    .add_fallback(anthropic_provider)
    .add_fallback(azure_provider);

registry.register(Arc::new(fallback_provider)).await?;

// If OpenAI fails, tries Anthropic, then Azure
let response = fallback_provider.chat_completion(&request).await?;
```

---

## Common Patterns

### Multi-Modal Requests (Vision)

```rust
use base64::Engine;

// Read image
let image_data = std::fs::read("image.jpg")?;
let base64_image = base64::engine::general_purpose::STANDARD.encode(&image_data);

let request = GatewayRequest {
    model: "gpt-4-vision-preview".to_string(),
    messages: vec![
        Message {
            role: MessageRole::User,
            content: MessageContent::MultiModal(vec![
                ContentPart::Text {
                    text: "What's in this image?".to_string(),
                },
                ContentPart::Image {
                    source: ImageSource::Base64 {
                        media_type: "image/jpeg".to_string(),
                        data: base64_image,
                    },
                    detail: Some("high".to_string()),
                },
            ]),
            name: None,
        }
    ],
    max_tokens: Some(300),
    // ... rest of config
};

let response = provider.chat_completion(&request).await?;
```

### Function/Tool Calling

```rust
let tools = vec![
    Tool {
        name: "get_weather".to_string(),
        description: "Get current weather for a location".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City name"
                },
                "units": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"]
                }
            },
            "required": ["location"]
        }),
    }
];

let request = GatewayRequest {
    model: "gpt-4-turbo".to_string(),
    messages: vec![
        Message {
            role: MessageRole::User,
            content: MessageContent::Text(
                "What's the weather in Paris?".to_string()
            ),
            name: None,
        }
    ],
    tools: Some(tools),
    tool_choice: Some(ToolChoice::Auto),
    // ... rest of config
};

let response = provider.chat_completion(&request).await?;

// Check if model wants to call a function
if matches!(response.finish_reason, FinishReason::ToolCalls) {
    // Extract tool calls from response and execute them
    println!("Model wants to call a tool!");
}
```

### Conversation with History

```rust
let mut conversation = vec![
    Message {
        role: MessageRole::System,
        content: MessageContent::Text(
            "You are a helpful coding assistant.".to_string()
        ),
        name: None,
    },
    Message {
        role: MessageRole::User,
        content: MessageContent::Text(
            "Write a function to reverse a string in Rust".to_string()
        ),
        name: None,
    },
];

let response = provider.chat_completion(&GatewayRequest {
    model: "gpt-4-turbo".to_string(),
    messages: conversation.clone(),
    // ... rest
}).await?;

// Add assistant response to history
conversation.push(response.choices[0].message.clone());

// Continue conversation
conversation.push(Message {
    role: MessageRole::User,
    content: MessageContent::Text(
        "Now add error handling".to_string()
    ),
    name: None,
});

let response2 = provider.chat_completion(&GatewayRequest {
    messages: conversation,
    // ... rest
}).await?;
```

### Health Monitoring

```rust
// Check single provider health
let health = provider.health_check().await?;
println!("Provider healthy: {}", health.is_healthy);
println!("Latency: {:?}ms", health.latency_ms);
println!("Error rate: {:.2}%", health.error_rate * 100.0);

// Check all providers
let all_providers = registry.list_all().await;
for (id, provider) in all_providers {
    match provider.health_check().await {
        Ok(health) => {
            println!("{}: {} ({}ms)",
                id,
                if health.is_healthy { "✓" } else { "✗" },
                health.latency_ms.unwrap_or(0)
            );
        }
        Err(e) => println!("{}: Error - {}", id, e),
    }
}

// Get only healthy providers
let healthy = registry.list_healthy().await;
println!("Healthy providers: {}", healthy.len());
```

### Provider Selection by Capability

```rust
// Find providers with specific features
let vision_providers = registry.get_providers_with_capability(
    |caps| caps.supports_multimodal
).await;

let tool_providers = registry.get_providers_with_capability(
    |caps| caps.supports_tools
).await;

let high_context_providers = registry.get_providers_with_capability(
    |caps| caps.max_context_tokens >= 100_000
).await;
```

---

## Error Handling

### Comprehensive Error Handling

```rust
match provider.chat_completion(&request).await {
    Ok(response) => {
        println!("Success!");
        println!("Response: {:?}", response);
    }
    Err(ProviderError::RateLimitExceeded(msg)) => {
        eprintln!("Rate limit hit: {}", msg);
        // Wait and retry
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    Err(ProviderError::Timeout(msg)) => {
        eprintln!("Request timeout: {}", msg);
        // Try different provider or retry
    }
    Err(ProviderError::AuthenticationFailed(msg)) => {
        eprintln!("Auth failed: {}", msg);
        // Check API key, don't retry
    }
    Err(ProviderError::InvalidRequest(msg)) => {
        eprintln!("Invalid request: {}", msg);
        // Fix request, don't retry
    }
    Err(ProviderError::ProviderInternalError(msg)) => {
        eprintln!("Provider error: {}", msg);
        // Try fallback provider
    }
    Err(ProviderError::NetworkError(msg)) => {
        eprintln!("Network error: {}", msg);
        // Retry with backoff
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

### Retry with Exponential Backoff

```rust
use tokio::time::sleep;

async fn request_with_retry(
    provider: Arc<dyn LLMProvider>,
    request: &GatewayRequest,
    max_retries: u32,
) -> Result<GatewayResponse> {
    let mut backoff = Duration::from_millis(100);

    for attempt in 0..=max_retries {
        match provider.chat_completion(request).await {
            Ok(response) => return Ok(response),
            Err(e) if attempt < max_retries => {
                eprintln!("Attempt {} failed: {}", attempt + 1, e);
                sleep(backoff).await;
                backoff *= 2; // Exponential backoff
            }
            Err(e) => return Err(e),
        }
    }

    unreachable!()
}
```

### Graceful Degradation

```rust
async fn get_completion_with_fallback(
    primary: Arc<dyn LLMProvider>,
    fallback: Arc<dyn LLMProvider>,
    request: &GatewayRequest,
) -> Result<GatewayResponse> {
    // Try primary
    match primary.chat_completion(request).await {
        Ok(response) => Ok(response),
        Err(e) => {
            eprintln!("Primary provider failed: {}, trying fallback", e);

            // Try fallback
            fallback.chat_completion(request).await
        }
    }
}
```

### Timeout Handling

```rust
use tokio::time::timeout;

// Request-level timeout
let result = timeout(
    Duration::from_secs(30),
    provider.chat_completion(&request)
).await;

match result {
    Ok(Ok(response)) => println!("Success: {:?}", response),
    Ok(Err(e)) => eprintln!("Provider error: {}", e),
    Err(_) => eprintln!("Timeout after 30s"),
}
```

---

## Best Practices

### 1. Always Use Connection Pooling
```rust
// ✓ Good - Shared pool
let pool = Arc::new(ConnectionPool::new(config));
let provider1 = factory.create_openai_with_pool(Arc::clone(&pool));
let provider2 = factory.create_anthropic_with_pool(Arc::clone(&pool));

// ✗ Bad - New connection for each provider
let provider1 = factory.create_openai(); // Creates own pool
let provider2 = factory.create_anthropic(); // Creates another pool
```

### 2. Use Production Stack in Production
```rust
// ✓ Good - Full stack
let provider = ProviderStackBuilder::new(base)
    .with_tracing()
    .with_circuit_breaker()
    .with_cache(cache)
    .with_metrics(metrics)
    .build();

// ✗ Bad - Bare provider in production
let provider = base_provider; // No resilience, observability
```

### 3. Set Appropriate Timeouts
```rust
// ✓ Good - Different timeouts for different use cases
let fast_config = OpenAIConfig {
    timeout: Duration::from_secs(30), // Quick responses
    // ...
};

let slow_config = OpenAIConfig {
    timeout: Duration::from_secs(300), // Long-form content
    // ...
};

// ✗ Bad - One-size-fits-all
let config = OpenAIConfig {
    timeout: Duration::from_secs(60), // May be too long or too short
    // ...
};
```

### 4. Monitor Health Continuously
```rust
// ✓ Good - Automated health checks
registry.start_health_checks().await; // Background task

// ✗ Bad - No health monitoring
// Just hope providers stay healthy
```

### 5. Handle Streaming Errors
```rust
// ✓ Good - Handle each chunk error
while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(c) => process_chunk(c),
        Err(e) => {
            eprintln!("Stream error: {}", e);
            break; // Or retry
        }
    }
}

// ✗ Bad - Unwrap streaming errors
while let Some(chunk) = stream.next().await {
    let c = chunk.unwrap(); // May panic mid-stream
    process_chunk(c);
}
```

---

## Performance Tips

1. **Reuse providers**: Create once, use many times
2. **Enable HTTP/2**: Connection multiplexing for better throughput
3. **Use caching**: Avoid redundant API calls
4. **Monitor latency**: Use metrics to identify slow providers
5. **Tune connection pool**: Match pool size to workload
6. **Batch requests**: If provider supports it
7. **Use load balancing**: Distribute across multiple providers

---

## Troubleshooting

### Provider Not Responding
```rust
// Check health
let health = provider.health_check().await?;
if !health.is_healthy {
    eprintln!("Provider unhealthy: {:?}", health.details);
}

// Check circuit breaker state
let state = circuit_breaker.get_state().await;
if state == CircuitState::Open {
    eprintln!("Circuit breaker is open, provider disabled");
}
```

### Rate Limit Issues
```rust
// Check rate limit before request
if let Some(wait) = provider.check_rate_limit().await {
    println!("Rate limited, waiting {:?}", wait);
    tokio::time::sleep(wait).await;
}
```

### High Latency
```rust
// Check pool stats
let stats = connection_pool.stats("openai").await;
if stats.available_connections == 0 {
    eprintln!("Connection pool exhausted!");
    eprintln!("Active: {}, Max: {}",
        stats.active_connections,
        stats.max_connections
    );
}
```

---

This quick start guide should get you up and running with the Provider Abstraction Layer. For more detailed information, see the full documentation in the architecture files.
