# Testing and Quality Assurance Architecture

**Enterprise Rust-Based LLM Gateway**

## 1. Testing Strategy

### Test Pyramid

```
           /\
          /E2E\         10% - Full system integration
         /------\
        /Integr.\      20% - Component integration
       /----------\
      /   Unit     \   70% - Module & function tests
     /--------------\
```

**Coverage Requirements:**
- **Overall Target:** 80%+ line coverage, 70%+ branch coverage
- **Core Modules:** 85%+ (provider abstraction, routing, rate limiting)
- **Critical Path:** 90%+ (request processing, authentication, failover)
- **Configuration/Utilities:** 75%+

**Testing Cadence:**
- Unit tests: Run on every commit (pre-commit hook)
- Integration tests: Run on PR creation/update
- E2E tests: Run on main branch merge
- Performance tests: Weekly + pre-release

## 2. Unit Testing

### Test Organization Structure

```
src/
├── providers/
│   ├── mod.rs
│   ├── openai.rs
│   └── anthropic.rs
tests/
├── unit/
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── openai_tests.rs
│   │   ├── anthropic_tests.rs
│   │   └── mock_provider.rs
│   ├── routing/
│   │   ├── load_balancer_tests.rs
│   │   └── failover_tests.rs
│   └── rate_limiting/
│       └── token_bucket_tests.rs
```

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_provider_validation_success() {
        let request = create_valid_request();
        let provider = MockProvider::new();

        assert!(provider.validate_request(&request).is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_enforces_limit() {
        let config = RateLimitConfig {
            requests_per_minute: Some(10),
            tokens_per_minute: Some(1000),
        };
        let limiter = RateLimiter::new(config);

        // Consume all tokens
        for _ in 0..10 {
            assert!(limiter.check_and_consume(100).await.is_none());
        }

        // Should be rate limited
        assert!(limiter.check_and_consume(100).await.is_some());
    }
}
```

### Mock Implementations

**Cargo.toml dependencies:**
```toml
[dev-dependencies]
mockall = "0.12"
tokio-test = "0.4"
fake = "2.9"
```

**Mock Provider:**
```rust
use mockall::mock;

mock! {
    pub Provider {}

    #[async_trait]
    impl LLMProvider for Provider {
        fn provider_id(&self) -> &str;
        fn capabilities(&self) -> &ProviderCapabilities;
        async fn health_check(&self) -> Result<HealthStatus>;
        async fn chat_completion(&self, request: &GatewayRequest)
            -> Result<GatewayResponse>;
    }
}
```

### Property-Based Testing

**Install proptest:**
```toml
[dev-dependencies]
proptest = "1.4"
```

**Example property tests:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_rate_limiter_never_negative(
        requests in 1..100u32,
        tokens in 1000..10000u32
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = RateLimitConfig {
            requests_per_minute: Some(requests),
            tokens_per_minute: Some(tokens),
        };
        let limiter = RateLimiter::new(config);

        rt.block_on(async {
            // Should never panic or return negative durations
            for _ in 0..requests * 2 {
                let wait = limiter.check_and_consume(1).await;
                prop_assert!(wait.is_none() || wait.unwrap().as_secs() < 3600);
            }
            Ok(())
        })?;
    }

    #[test]
    fn test_request_serialization_roundtrip(
        model in "gpt-[34].*",
        temperature in 0.0f32..2.0f32,
        max_tokens in 1u32..4096u32
    ) {
        let request = GatewayRequest {
            model,
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            ..Default::default()
        };

        let json = serde_json::to_string(&request)?;
        let deserialized: GatewayRequest = serde_json::from_str(&json)?;

        prop_assert_eq!(request.model, deserialized.model);
        prop_assert_eq!(request.temperature, deserialized.temperature);
    }
}
```

## 3. Integration Testing

### Component Integration Tests

**Test structure:**
```rust
// tests/integration/provider_registry_tests.rs
use llm_gateway::*;

#[tokio::test]
async fn test_multi_provider_registration() {
    let pool = Arc::new(ConnectionPool::new(Default::default()));
    let registry = ProviderRegistry::new(Duration::from_secs(60));

    let openai = OpenAIProvider::new(test_config(), pool.clone());
    let anthropic = AnthropicProvider::new(test_config(), pool.clone());

    registry.register(Arc::new(openai)).await.unwrap();
    registry.register(Arc::new(anthropic)).await.unwrap();

    let providers = registry.list_all().await;
    assert_eq!(providers.len(), 2);
}
```

### WireMock for HTTP Mocking

**Cargo.toml:**
```toml
[dev-dependencies]
wiremock = "0.6"
```

**Mock server example:**
```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

#[tokio::test]
async fn test_openai_provider_with_mock() {
    let mock_server = MockServer::start().await;

    // Setup mock response
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({
                "id": "chatcmpl-123",
                "model": "gpt-4",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello!"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15
                }
            })))
        .mount(&mock_server)
        .await;

    let config = OpenAIConfig {
        api_key: "test-key".to_string(),
        base_url: mock_server.uri(),
        ..Default::default()
    };

    let pool = Arc::new(ConnectionPool::new(Default::default()));
    let provider = OpenAIProvider::new(config, pool);

    let request = create_test_request();
    let response = provider.chat_completion(&request).await.unwrap();

    assert_eq!(response.choices[0].message.content, "Hello!");
}
```

### Provider Simulation Tests

```rust
// tests/integration/provider_simulation.rs

struct ProviderSimulator {
    latency: Duration,
    error_rate: f32,
    rate_limit_threshold: Option<u32>,
}

impl ProviderSimulator {
    async fn simulate_request(&mut self) -> Result<GatewayResponse> {
        tokio::time::sleep(self.latency).await;

        if rand::random::<f32>() < self.error_rate {
            return Err(ProviderError::ProviderInternalError(
                "Simulated error".into()
            ));
        }

        Ok(create_mock_response())
    }
}

#[tokio::test]
async fn test_failover_under_provider_degradation() {
    let mut simulator = ProviderSimulator {
        latency: Duration::from_millis(100),
        error_rate: 0.5, // 50% error rate
        rate_limit_threshold: Some(10),
    };

    // Test that failover mechanism activates correctly
    let results: Vec<_> = (0..20)
        .map(|_| simulator.simulate_request())
        .collect();

    let success_count = results.iter()
        .filter(|r| r.is_ok())
        .count();

    assert!(success_count >= 8); // At least 40% should succeed
}
```

## 4. End-to-End Testing

### Full Request Lifecycle Tests

```rust
// tests/e2e/full_lifecycle_test.rs

#[tokio::test]
async fn test_complete_request_flow() {
    // Start test gateway
    let gateway = Gateway::builder()
        .with_config(test_config())
        .build()
        .await
        .unwrap();

    let server_addr = gateway.start().await;

    // Make HTTP request
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", server_addr))
        .header("Authorization", "Bearer test-key")
        .json(&json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap()
        .len() > 0);
}
```

### Docker Compose Test Environment

**docker-compose.test.yml:**
```yaml
version: '3.8'

services:
  gateway:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=debug
      - CONFIG_PATH=/config/test-config.yaml
    volumes:
      - ./config:/config
    depends_on:
      - wiremock
      - redis

  wiremock:
    image: wiremock/wiremock:3.3.1
    ports:
      - "8081:8080"
    volumes:
      - ./tests/wiremock:/home/wiremock

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: gateway_test
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
    ports:
      - "5432:5432"
```

**E2E test script:**
```bash
#!/bin/bash
# tests/e2e/run-e2e.sh

set -e

echo "Starting test environment..."
docker-compose -f docker-compose.test.yml up -d

echo "Waiting for services..."
sleep 10

echo "Running E2E tests..."
cargo test --test e2e -- --test-threads=1

echo "Cleaning up..."
docker-compose -f docker-compose.test.yml down -v
```

## 5. Performance Testing

### Criterion.rs Benchmarks

**Cargo.toml:**
```toml
[[bench]]
name = "provider_benchmarks"
harness = false

[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }
```

**benches/provider_benchmarks.rs:**
```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use tokio::runtime::Runtime;

fn bench_request_transformation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let provider = create_test_provider();
    let request = create_test_request();

    c.bench_function("openai_transform_request", |b| {
        b.iter(|| {
            provider.transform_request(&request)
        });
    });
}

fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_requests");

    for concurrency in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&rt).iter(|| async {
                    let futures = (0..concurrency)
                        .map(|_| make_request())
                        .collect::<Vec<_>>();
                    futures::future::join_all(futures).await
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches,
    bench_request_transformation,
    bench_concurrent_requests
);
criterion_main!(benches);
```

### k6 Load Tests

**k6/load-test.js:**
```javascript
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 200 },   // Ramp up to 200 users
    { duration: '5m', target: 200 },   // Stay at 200 users
    { duration: '2m', target: 0 },     // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.01'],
  },
};

export default function() {
  const url = 'http://localhost:8080/v1/chat/completions';
  const payload = JSON.stringify({
    model: 'gpt-4',
    messages: [
      { role: 'user', content: 'Hello, how are you?' }
    ],
    max_tokens: 100,
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
      'Authorization': 'Bearer test-key',
    },
  };

  const response = http.post(url, payload, params);

  check(response, {
    'status is 200': (r) => r.status === 200,
    'response has content': (r) => r.json('choices.0.message.content') !== '',
    'latency < 500ms': (r) => r.timings.duration < 500,
  });

  sleep(1);
}
```

**Run k6:**
```bash
k6 run --out json=results.json k6/load-test.js
```

## 6. Security Testing

### Cargo Audit Integration

**.github/workflows/security.yml:**
```yaml
name: Security Audit

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]
  schedule:
    - cron: '0 0 * * 0'  # Weekly

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/audit@v1
        with:
          denyWarnings: true
```

### Cargo Fuzz Setup

**Cargo.toml:**
```toml
[dependencies]
arbitrary = { version = "1.3", features = ["derive"], optional = true }

[features]
fuzzing = ["arbitrary"]
```

**fuzz/fuzz_targets/fuzz_request_parser.rs:**
```rust
#![no_main]
use libfuzzer_sys::fuzz_target;
use llm_gateway::GatewayRequest;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<GatewayRequest>(s);
    }
});
```

**Run fuzzing:**
```bash
cargo install cargo-fuzz
cargo fuzz run fuzz_request_parser -- -max_total_time=300
```

### SAST Integration

**Clippy (strict mode):**
```bash
cargo clippy --all-targets --all-features -- \
  -D warnings \
  -D clippy::all \
  -D clippy::pedantic \
  -D clippy::cargo
```

**cargo-deny configuration (.cargo-deny.toml):**
```toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"

[licenses]
unlicensed = "deny"
allow = ["MIT", "Apache-2.0", "BSD-3-Clause"]
deny = ["GPL-3.0"]

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

## 7. Quality Gates

| Stage | Checks | Threshold | Blocking |
|-------|--------|-----------|----------|
| **Pre-Commit** | Rustfmt | 100% formatted | Yes |
| | Clippy warnings | 0 warnings | Yes |
| | Unit tests | Pass all | Yes |
| **PR Creation** | Code coverage | ≥80% | Yes |
| | Integration tests | Pass all | Yes |
| | License compliance | 100% approved | Yes |
| **Merge to Main** | E2E tests | Pass all | Yes |
| | Security audit | 0 vulnerabilities | Yes |
| | Performance regression | <5% degradation | Yes |
| | Documentation | Up to date | Yes |
| **Pre-Release** | Load tests (k6) | p95 <500ms, p99 <1s | Yes |
| | Fuzz testing | 5min no crashes | Yes |
| | Full security scan | 0 high/critical | Yes |
| | Dependency audit | 0 vulnerabilities | Yes |

## 8. CI/CD Integration

### GitHub Actions Pipeline

**.github/workflows/ci.yml:**
```yaml
name: CI/CD Pipeline

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  format-and-lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run Clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  test-unit:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust ${{ matrix.rust }}
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}

      - name: Build
        run: cargo build --verbose

      - name: Run unit tests
        run: cargo test --lib --bins --verbose

      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov

      - name: Generate coverage
        run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
          fail_ci_if_error: true

  test-integration:
    runs-on: ubuntu-latest
    services:
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_DB: gateway_test
          POSTGRES_USER: test
          POSTGRES_PASSWORD: test
        ports:
          - 5432:5432
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Run integration tests
        run: cargo test --test '*' --verbose
        env:
          DATABASE_URL: postgres://test:test@localhost:5432/gateway_test
          REDIS_URL: redis://localhost:6379

  test-e2e:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Start test environment
        run: docker-compose -f docker-compose.test.yml up -d

      - name: Wait for services
        run: sleep 30

      - name: Run E2E tests
        run: cargo test --test e2e -- --test-threads=1

      - name: Collect logs
        if: failure()
        run: docker-compose -f docker-compose.test.yml logs

      - name: Cleanup
        if: always()
        run: docker-compose -f docker-compose.test.yml down -v

  benchmark:
    runs-on: ubuntu-latest
    if: github.event_name == 'push'
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Run benchmarks
        run: cargo bench --no-fail-fast

      - name: Store benchmark results
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: target/criterion/results.json
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true

  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Run cargo-deny
        uses: EmbarkStudios/cargo-deny-action@v1

      - name: Run cargo-audit
        uses: actions-rust-lang/audit@v1
```

### Test Parallelization

**Makefile:**
```makefile
.PHONY: test-fast test-all

test-fast:
	cargo nextest run --jobs 8 --no-fail-fast

test-unit:
	cargo nextest run --lib --bins --jobs 8

test-integration:
	cargo nextest run --test '*' --jobs 4

test-e2e:
	cargo nextest run --test e2e --jobs 1

test-all: test-unit test-integration test-e2e
```

**Install nextest:**
```bash
cargo install cargo-nextest
```

---

**Document Version:** 1.0
**Last Updated:** 2025-11-27
**Owner:** QA Architecture Team
