# LLM Inference Gateway - Configuration Reference

Complete configuration reference for the LLM Inference Gateway.

## Configuration Methods

The gateway supports multiple configuration methods, in order of precedence:

1. **Command-line arguments** (highest priority)
2. **Environment variables**
3. **Configuration file** (YAML/TOML)
4. **Default values** (lowest priority)

---

## Configuration File

### Location

The gateway looks for configuration files in the following locations:

1. Path specified by `--config` CLI argument
2. Path specified by `GATEWAY_CONFIG` environment variable
3. `./gateway.yaml` (current directory)
4. `/etc/llm-gateway/gateway.yaml`
5. `~/.config/llm-gateway/gateway.yaml`

### Format

Configuration files can be in YAML or TOML format:

```yaml
# gateway.yaml
server:
  host: "0.0.0.0"
  port: 8080
  metrics_port: 9090

providers:
  openai:
    enabled: true
    api_key: "${OPENAI_API_KEY}"
```

```toml
# gateway.toml
[server]
host = "0.0.0.0"
port = 8080
metrics_port = 9090

[providers.openai]
enabled = true
api_key = "${OPENAI_API_KEY}"
```

---

## Server Configuration

### Basic Settings

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `server.host` | `GATEWAY_HOST` | `0.0.0.0` | Server bind address |
| `server.port` | `GATEWAY_PORT` | `8080` | HTTP API port |
| `server.metrics_port` | `GATEWAY_METRICS_PORT` | `9090` | Prometheus metrics port |
| `server.graceful_shutdown_timeout` | `GATEWAY_SHUTDOWN_TIMEOUT` | `30s` | Graceful shutdown timeout |
| `server.request_timeout` | `GATEWAY_REQUEST_TIMEOUT` | `300s` | Maximum request timeout |
| `server.keep_alive_timeout` | `GATEWAY_KEEPALIVE_TIMEOUT` | `75s` | HTTP keep-alive timeout |

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  metrics_port: 9090
  graceful_shutdown_timeout: "30s"
  request_timeout: "300s"
  keep_alive_timeout: "75s"
```

### TLS Configuration

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `server.tls.enabled` | `GATEWAY_TLS_ENABLED` | `false` | Enable TLS |
| `server.tls.cert_path` | `GATEWAY_TLS_CERT` | - | Path to TLS certificate |
| `server.tls.key_path` | `GATEWAY_TLS_KEY` | - | Path to TLS private key |
| `server.tls.ca_path` | `GATEWAY_TLS_CA` | - | Path to CA certificate (for mTLS) |
| `server.tls.client_auth` | `GATEWAY_TLS_CLIENT_AUTH` | `none` | Client auth: `none`, `optional`, `required` |

```yaml
server:
  tls:
    enabled: true
    cert_path: "/etc/ssl/certs/gateway.crt"
    key_path: "/etc/ssl/private/gateway.key"
    ca_path: "/etc/ssl/certs/ca.crt"
    client_auth: "optional"
```

### Connection Limits

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `server.max_connections` | `GATEWAY_MAX_CONNECTIONS` | `10000` | Maximum concurrent connections |
| `server.max_request_body_size` | `GATEWAY_MAX_BODY_SIZE` | `10MB` | Maximum request body size |
| `server.connection_rate_limit` | `GATEWAY_CONN_RATE_LIMIT` | `1000/s` | New connections per second |

```yaml
server:
  max_connections: 10000
  max_request_body_size: "10MB"
  connection_rate_limit: "1000/s"
```

---

## Provider Configuration

### OpenAI

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `providers.openai.enabled` | `OPENAI_ENABLED` | `true` | Enable OpenAI provider |
| `providers.openai.api_key` | `OPENAI_API_KEY` | - | OpenAI API key (required) |
| `providers.openai.base_url` | `OPENAI_BASE_URL` | `https://api.openai.com/v1` | API base URL |
| `providers.openai.organization` | `OPENAI_ORG_ID` | - | Organization ID |
| `providers.openai.timeout` | `OPENAI_TIMEOUT` | `120s` | Request timeout |
| `providers.openai.max_retries` | `OPENAI_MAX_RETRIES` | `3` | Maximum retry attempts |

```yaml
providers:
  openai:
    enabled: true
    api_key: "${OPENAI_API_KEY}"
    base_url: "https://api.openai.com/v1"
    organization: "org-xxxx"
    timeout: "120s"
    max_retries: 3
    models:
      - "gpt-4o"
      - "gpt-4o-mini"
      - "gpt-4-turbo"
      - "gpt-3.5-turbo"
```

### Anthropic

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `providers.anthropic.enabled` | `ANTHROPIC_ENABLED` | `true` | Enable Anthropic provider |
| `providers.anthropic.api_key` | `ANTHROPIC_API_KEY` | - | Anthropic API key (required) |
| `providers.anthropic.base_url` | `ANTHROPIC_BASE_URL` | `https://api.anthropic.com` | API base URL |
| `providers.anthropic.version` | `ANTHROPIC_VERSION` | `2024-01-01` | API version |
| `providers.anthropic.timeout` | `ANTHROPIC_TIMEOUT` | `120s` | Request timeout |
| `providers.anthropic.max_retries` | `ANTHROPIC_MAX_RETRIES` | `3` | Maximum retry attempts |

```yaml
providers:
  anthropic:
    enabled: true
    api_key: "${ANTHROPIC_API_KEY}"
    base_url: "https://api.anthropic.com"
    version: "2024-01-01"
    timeout: "120s"
    max_retries: 3
    models:
      - "claude-3-5-sonnet-latest"
      - "claude-3-opus-latest"
      - "claude-3-haiku-latest"
```

### Google (Gemini)

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `providers.google.enabled` | `GOOGLE_ENABLED` | `true` | Enable Google provider |
| `providers.google.api_key` | `GOOGLE_API_KEY` | - | Google API key (required) |
| `providers.google.project_id` | `GOOGLE_PROJECT_ID` | - | GCP Project ID (for Vertex AI) |
| `providers.google.location` | `GOOGLE_LOCATION` | `us-central1` | Vertex AI location |
| `providers.google.timeout` | `GOOGLE_TIMEOUT` | `120s` | Request timeout |

```yaml
providers:
  google:
    enabled: true
    api_key: "${GOOGLE_API_KEY}"
    # For Vertex AI:
    # project_id: "my-project"
    # location: "us-central1"
    timeout: "120s"
    models:
      - "gemini-1.5-pro"
      - "gemini-1.5-flash"
```

### Azure OpenAI

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `providers.azure.enabled` | `AZURE_OPENAI_ENABLED` | `false` | Enable Azure OpenAI |
| `providers.azure.api_key` | `AZURE_OPENAI_API_KEY` | - | Azure API key |
| `providers.azure.endpoint` | `AZURE_OPENAI_ENDPOINT` | - | Azure endpoint URL |
| `providers.azure.api_version` | `AZURE_OPENAI_API_VERSION` | `2024-02-01` | API version |
| `providers.azure.deployments` | - | - | Model to deployment mapping |

```yaml
providers:
  azure:
    enabled: true
    api_key: "${AZURE_OPENAI_API_KEY}"
    endpoint: "https://my-resource.openai.azure.com"
    api_version: "2024-02-01"
    deployments:
      gpt-4o: "gpt-4o-deployment"
      gpt-4o-mini: "gpt-4o-mini-deployment"
```

### AWS Bedrock

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `providers.bedrock.enabled` | `BEDROCK_ENABLED` | `false` | Enable AWS Bedrock |
| `providers.bedrock.region` | `AWS_REGION` | `us-east-1` | AWS region |
| `providers.bedrock.access_key_id` | `AWS_ACCESS_KEY_ID` | - | AWS access key |
| `providers.bedrock.secret_access_key` | `AWS_SECRET_ACCESS_KEY` | - | AWS secret key |
| `providers.bedrock.profile` | `AWS_PROFILE` | - | AWS profile name |

```yaml
providers:
  bedrock:
    enabled: true
    region: "us-east-1"
    # Uses AWS credential chain by default
    # Optionally specify credentials:
    # access_key_id: "${AWS_ACCESS_KEY_ID}"
    # secret_access_key: "${AWS_SECRET_ACCESS_KEY}"
    models:
      - "anthropic.claude-3-sonnet-20240229-v1:0"
      - "amazon.titan-text-express-v1"
```

---

## Routing Configuration

### Routing Strategy

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `routing.strategy` | `ROUTING_STRATEGY` | `model_based` | Routing strategy |
| `routing.default_provider` | `DEFAULT_PROVIDER` | - | Default provider if model not found |
| `routing.fallback_enabled` | `FALLBACK_ENABLED` | `true` | Enable provider fallback |

```yaml
routing:
  # Strategy: model_based, round_robin, weighted, least_latency, cost_optimized
  strategy: "model_based"
  default_provider: "openai"
  fallback_enabled: true
  fallback_chain:
    - "openai"
    - "anthropic"
    - "google"
```

### Model Mapping

Map custom model names to provider models:

```yaml
routing:
  model_mapping:
    # Custom name -> provider/model
    "fast": "openai/gpt-4o-mini"
    "smart": "anthropic/claude-3-5-sonnet-latest"
    "vision": "openai/gpt-4o"
```

### Weighted Routing

```yaml
routing:
  strategy: "weighted"
  weights:
    openai: 0.6
    anthropic: 0.3
    google: 0.1
```

---

## Cache Configuration

### General Cache Settings

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `cache.enabled` | `CACHE_ENABLED` | `true` | Enable caching |
| `cache.default_ttl` | `CACHE_TTL` | `3600s` | Default cache TTL |
| `cache.max_entry_size` | `CACHE_MAX_ENTRY_SIZE` | `1MB` | Maximum cached entry size |
| `cache.key_strategy` | `CACHE_KEY_STRATEGY` | `full_request` | Cache key generation |

```yaml
cache:
  enabled: true
  default_ttl: "3600s"
  max_entry_size: "1MB"
  key_strategy: "full_request"  # or "messages_only"
```

### In-Memory Cache (L1)

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `cache.memory.enabled` | `CACHE_MEMORY_ENABLED` | `true` | Enable memory cache |
| `cache.memory.max_entries` | `CACHE_MEMORY_MAX_ENTRIES` | `10000` | Maximum entries |
| `cache.memory.max_size` | `CACHE_MEMORY_MAX_SIZE` | `512MB` | Maximum total size |

```yaml
cache:
  memory:
    enabled: true
    max_entries: 10000
    max_size: "512MB"
```

### Redis Cache (L2)

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `cache.redis.enabled` | `REDIS_ENABLED` | `false` | Enable Redis cache |
| `cache.redis.url` | `REDIS_URL` | `redis://localhost:6379` | Redis connection URL |
| `cache.redis.password` | `REDIS_PASSWORD` | - | Redis password |
| `cache.redis.database` | `REDIS_DATABASE` | `0` | Redis database number |
| `cache.redis.pool_size` | `REDIS_POOL_SIZE` | `10` | Connection pool size |
| `cache.redis.timeout` | `REDIS_TIMEOUT` | `5s` | Connection timeout |
| `cache.redis.key_prefix` | `REDIS_KEY_PREFIX` | `llm-gateway:` | Key prefix |

```yaml
cache:
  redis:
    enabled: true
    url: "redis://redis:6379"
    password: "${REDIS_PASSWORD}"
    database: 0
    pool_size: 10
    timeout: "5s"
    key_prefix: "llm-gateway:"
    tls:
      enabled: false
      # cert_path: "/etc/ssl/certs/redis.crt"
      # key_path: "/etc/ssl/private/redis.key"
```

### Cache Exclusions

```yaml
cache:
  exclude:
    # Don't cache specific models
    models:
      - "gpt-4o"  # Always want fresh responses
    # Don't cache if temperature > 0
    high_temperature: true
    # Don't cache streaming requests
    streaming: true
```

---

## Rate Limiting Configuration

### General Rate Limiting

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `rate_limit.enabled` | `RATE_LIMIT_ENABLED` | `true` | Enable rate limiting |
| `rate_limit.strategy` | `RATE_LIMIT_STRATEGY` | `sliding_window` | Rate limit strategy |
| `rate_limit.default_rpm` | `RATE_LIMIT_DEFAULT_RPM` | `1000` | Default requests per minute |
| `rate_limit.default_tpm` | `RATE_LIMIT_DEFAULT_TPM` | `100000` | Default tokens per minute |

```yaml
rate_limit:
  enabled: true
  strategy: "sliding_window"  # fixed_window, sliding_window, token_bucket
  default_rpm: 1000
  default_tpm: 100000
  burst_multiplier: 1.5
```

### Per-User Rate Limiting

```yaml
rate_limit:
  per_user:
    enabled: true
    default_rpm: 100
    default_tpm: 10000
    # Custom limits per user/API key
    overrides:
      "api-key-premium":
        rpm: 1000
        tpm: 100000
      "api-key-basic":
        rpm: 50
        tpm: 5000
```

### Per-Model Rate Limiting

```yaml
rate_limit:
  per_model:
    "gpt-4o":
      rpm: 500
      tpm: 50000
    "gpt-4o-mini":
      rpm: 2000
      tpm: 200000
```

### Redis-Based Rate Limiting

```yaml
rate_limit:
  backend: "redis"  # or "memory"
  redis:
    url: "redis://redis:6379"
    key_prefix: "llm-gateway:ratelimit:"
```

---

## Authentication Configuration

### API Key Authentication

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `auth.enabled` | `AUTH_ENABLED` | `false` | Enable authentication |
| `auth.api_key.enabled` | `AUTH_API_KEY_ENABLED` | `true` | Enable API key auth |
| `auth.api_key.header` | `AUTH_API_KEY_HEADER` | `X-API-Key` | API key header name |

```yaml
auth:
  enabled: true
  api_key:
    enabled: true
    header: "X-API-Key"
    # Static API keys (for development)
    keys:
      - key: "sk-gateway-dev-key"
        name: "Development"
        rate_limit_tier: "default"
      - key: "sk-gateway-premium-key"
        name: "Premium User"
        rate_limit_tier: "premium"
```

### JWT Authentication

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `auth.jwt.enabled` | `AUTH_JWT_ENABLED` | `false` | Enable JWT auth |
| `auth.jwt.secret` | `AUTH_JWT_SECRET` | - | JWT signing secret |
| `auth.jwt.issuer` | `AUTH_JWT_ISSUER` | - | Expected JWT issuer |
| `auth.jwt.audience` | `AUTH_JWT_AUDIENCE` | - | Expected JWT audience |

```yaml
auth:
  jwt:
    enabled: true
    secret: "${JWT_SECRET}"
    # Or use public key for RS256:
    # public_key_path: "/etc/ssl/jwt-public.pem"
    algorithm: "HS256"  # or RS256, ES256
    issuer: "https://auth.example.com"
    audience: "llm-gateway"
    claims_mapping:
      user_id: "sub"
      rate_limit_tier: "tier"
```

### OAuth2/OIDC

```yaml
auth:
  oauth2:
    enabled: true
    issuer_url: "https://auth.example.com"
    client_id: "${OAUTH_CLIENT_ID}"
    # Token introspection endpoint
    introspection_url: "https://auth.example.com/oauth/introspect"
```

---

## Telemetry Configuration

### Logging

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `telemetry.log.level` | `LOG_LEVEL` | `info` | Log level |
| `telemetry.log.format` | `LOG_FORMAT` | `json` | Log format |
| `telemetry.log.output` | `LOG_OUTPUT` | `stdout` | Log output destination |

```yaml
telemetry:
  log:
    level: "info"  # trace, debug, info, warn, error
    format: "json"  # json, pretty, compact
    output: "stdout"  # stdout, stderr, file
    file:
      path: "/var/log/llm-gateway/gateway.log"
      rotation: "daily"
      max_files: 7
```

### Metrics (Prometheus)

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `telemetry.metrics.enabled` | `METRICS_ENABLED` | `true` | Enable metrics |
| `telemetry.metrics.port` | `METRICS_PORT` | `9090` | Metrics server port |
| `telemetry.metrics.path` | `METRICS_PATH` | `/metrics` | Metrics endpoint path |

```yaml
telemetry:
  metrics:
    enabled: true
    port: 9090
    path: "/metrics"
    # Include additional labels
    labels:
      environment: "production"
      region: "us-west-2"
    # Histogram buckets for latency
    latency_buckets: [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
```

### Tracing (OpenTelemetry)

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `telemetry.tracing.enabled` | `OTEL_ENABLED` | `false` | Enable tracing |
| `telemetry.tracing.exporter` | `OTEL_EXPORTER` | `otlp` | Trace exporter |
| `telemetry.tracing.endpoint` | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP endpoint |
| `telemetry.tracing.sample_rate` | `OTEL_SAMPLE_RATE` | `1.0` | Sampling rate (0.0-1.0) |

```yaml
telemetry:
  tracing:
    enabled: true
    exporter: "otlp"  # otlp, jaeger, zipkin
    endpoint: "http://otel-collector:4317"
    sample_rate: 0.1  # Sample 10% of traces
    propagation: "tracecontext,baggage"  # W3C Trace Context
    resource:
      service.name: "llm-gateway"
      service.version: "0.1.0"
      deployment.environment: "production"
```

### PII Redaction

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `telemetry.pii.enabled` | `PII_REDACTION_ENABLED` | `true` | Enable PII redaction |
| `telemetry.pii.style` | `PII_REDACTION_STYLE` | `mask` | Redaction style |

```yaml
telemetry:
  pii:
    enabled: true
    style: "mask"  # mask, hash, remove, partial_mask
    patterns:
      email: true
      phone: true
      ssn: true
      credit_card: true
      api_key: true
      jwt: true
      url_credentials: true
    custom_patterns:
      - name: "internal_id"
        pattern: "INT-[0-9]{8}"
        replacement: "[INTERNAL_ID]"
```

### Cost Tracking

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `telemetry.cost.enabled` | `COST_TRACKING_ENABLED` | `true` | Enable cost tracking |
| `telemetry.cost.currency` | `COST_CURRENCY` | `USD` | Cost currency |

```yaml
telemetry:
  cost:
    enabled: true
    currency: "USD"
    # Custom pricing (overrides defaults)
    pricing:
      "gpt-4o":
        input_per_1k: 0.005
        output_per_1k: 0.015
      "claude-3-5-sonnet-latest":
        input_per_1k: 0.003
        output_per_1k: 0.015
```

---

## Retry and Resilience

### Retry Configuration

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `resilience.retry.max_retries` | `MAX_RETRIES` | `3` | Maximum retry attempts |
| `resilience.retry.initial_backoff` | `RETRY_INITIAL_BACKOFF` | `100ms` | Initial backoff |
| `resilience.retry.max_backoff` | `RETRY_MAX_BACKOFF` | `10s` | Maximum backoff |
| `resilience.retry.multiplier` | `RETRY_MULTIPLIER` | `2.0` | Backoff multiplier |

```yaml
resilience:
  retry:
    max_retries: 3
    initial_backoff: "100ms"
    max_backoff: "10s"
    multiplier: 2.0
    jitter: 0.1
    retry_on:
      - 429  # Too Many Requests
      - 502  # Bad Gateway
      - 503  # Service Unavailable
      - 504  # Gateway Timeout
```

### Circuit Breaker

| Option | Environment Variable | Default | Description |
|--------|---------------------|---------|-------------|
| `resilience.circuit_breaker.enabled` | `CIRCUIT_BREAKER_ENABLED` | `true` | Enable circuit breaker |
| `resilience.circuit_breaker.failure_threshold` | `CB_FAILURE_THRESHOLD` | `5` | Failures before opening |
| `resilience.circuit_breaker.success_threshold` | `CB_SUCCESS_THRESHOLD` | `3` | Successes to close |
| `resilience.circuit_breaker.timeout` | `CB_TIMEOUT` | `30s` | Half-open timeout |

```yaml
resilience:
  circuit_breaker:
    enabled: true
    failure_threshold: 5
    success_threshold: 3
    timeout: "30s"
    # Per-provider circuit breakers
    per_provider: true
```

### Timeout Configuration

```yaml
resilience:
  timeouts:
    connect: "5s"
    read: "120s"
    write: "30s"
    overall: "300s"
```

---

## Health Check Configuration

```yaml
health:
  # Liveness probe
  liveness:
    enabled: true
    path: "/live"

  # Readiness probe
  readiness:
    enabled: true
    path: "/ready"
    # Check provider connectivity
    check_providers: true
    # Check Redis connectivity
    check_redis: true

  # Startup probe
  startup:
    enabled: true
    path: "/startup"
    # Fail if providers aren't ready within timeout
    timeout: "60s"
```

---

## Complete Example Configuration

```yaml
# gateway.yaml - Complete production configuration example

server:
  host: "0.0.0.0"
  port: 8080
  metrics_port: 9090
  graceful_shutdown_timeout: "30s"
  request_timeout: "300s"
  max_connections: 10000
  max_request_body_size: "10MB"
  tls:
    enabled: false

providers:
  openai:
    enabled: true
    api_key: "${OPENAI_API_KEY}"
    timeout: "120s"
    max_retries: 3
    models:
      - "gpt-4o"
      - "gpt-4o-mini"

  anthropic:
    enabled: true
    api_key: "${ANTHROPIC_API_KEY}"
    timeout: "120s"
    max_retries: 3
    models:
      - "claude-3-5-sonnet-latest"
      - "claude-3-opus-latest"

  google:
    enabled: true
    api_key: "${GOOGLE_API_KEY}"
    timeout: "120s"
    models:
      - "gemini-1.5-pro"
      - "gemini-1.5-flash"

routing:
  strategy: "model_based"
  fallback_enabled: true
  fallback_chain:
    - "openai"
    - "anthropic"

cache:
  enabled: true
  default_ttl: "3600s"
  memory:
    enabled: true
    max_entries: 10000
  redis:
    enabled: true
    url: "redis://redis:6379"
    pool_size: 10

rate_limit:
  enabled: true
  strategy: "sliding_window"
  default_rpm: 1000
  default_tpm: 100000
  backend: "redis"

auth:
  enabled: false  # Enable for production

telemetry:
  log:
    level: "info"
    format: "json"
  metrics:
    enabled: true
    port: 9090
  tracing:
    enabled: true
    endpoint: "http://otel-collector:4317"
    sample_rate: 0.1
  pii:
    enabled: true
    style: "mask"
  cost:
    enabled: true

resilience:
  retry:
    max_retries: 3
    initial_backoff: "100ms"
    max_backoff: "10s"
  circuit_breaker:
    enabled: true
    failure_threshold: 5
    timeout: "30s"

health:
  readiness:
    check_providers: true
    check_redis: true
```

---

## Environment Variable Reference

All configuration options can be set via environment variables. The naming convention is:

- Uppercase with underscores
- Nested options use underscores as separators
- Example: `server.tls.enabled` → `GATEWAY_TLS_ENABLED`

### Quick Reference Table

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `GATEWAY_HOST` | Server bind address | `0.0.0.0` |
| `GATEWAY_PORT` | HTTP API port | `8080` |
| `GATEWAY_METRICS_PORT` | Metrics port | `9090` |
| `GATEWAY_CONFIG` | Config file path | - |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `GOOGLE_API_KEY` | Google API key | - |
| `REDIS_URL` | Redis connection URL | `redis://localhost:6379` |
| `REDIS_ENABLED` | Enable Redis | `false` |
| `LOG_LEVEL` | Log level | `info` |
| `LOG_FORMAT` | Log format | `json` |
| `OTEL_ENABLED` | Enable tracing | `false` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP endpoint | - |
| `RATE_LIMIT_ENABLED` | Enable rate limiting | `true` |
| `CACHE_ENABLED` | Enable caching | `true` |
| `AUTH_ENABLED` | Enable authentication | `false` |

---

## Validation

The gateway validates configuration on startup. Common validation errors:

```
ERROR: Missing required configuration: providers.openai.api_key
ERROR: Invalid port number: 99999 (must be 1-65535)
ERROR: Invalid log level: verbose (must be trace|debug|info|warn|error)
ERROR: Redis URL is invalid: not a valid URL
```

### Testing Configuration

```bash
# Validate configuration file
llm-gateway --config gateway.yaml --validate

# Show effective configuration
llm-gateway --config gateway.yaml --show-config

# Test with dry run (don't start server)
llm-gateway --config gateway.yaml --dry-run
```
