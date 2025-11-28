# LLM Inference Gateway - Load Testing Suite

Comprehensive k6 load testing suite for validating performance, scalability, and reliability of the LLM Inference Gateway.

## Prerequisites

### Install k6

```bash
# macOS
brew install k6

# Ubuntu/Debian
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6

# Docker
docker pull grafana/k6
```

## Test Suite Overview

| Test | Purpose | Duration | VUs | Use Case |
|------|---------|----------|-----|----------|
| smoke-test.js | Basic validation | 30s | 1 | Pre-deployment check |
| baseline-test.js | Performance baseline | 5m | 10 | Establish metrics |
| stress-test.js | Find breaking points | 25m | 0→150 | Capacity planning |
| soak-test.js | Long-running stability | 2h | 20 | Memory leak detection |
| streaming-test.js | Streaming performance | 14m | 0→50 | TTFT validation |

## Quick Start

### 1. Start the Gateway

```bash
# Run the gateway locally
cargo run --release

# Or with Docker
docker-compose up -d
```

### 2. Run Smoke Test

```bash
k6 run tests/load/smoke-test.js
```

### 3. Run Baseline Test

```bash
k6 run tests/load/baseline-test.js
```

## Environment Variables

Configure tests via environment variables:

```bash
# Gateway URL (default: http://localhost:8080)
export GATEWAY_URL=http://localhost:8080

# API Key for authentication
export API_KEY=your-api-key

# Default model (default: gpt-3.5-turbo)
export MODEL=gpt-4

# Max tokens per request (default: 100)
export MAX_TOKENS=150

# Temperature (default: 0.7)
export TEMPERATURE=0.5
```

## Test Descriptions

### Smoke Test (`smoke-test.js`)

Quick validation that the gateway is functioning correctly.

**Checks:**
- Health endpoint responds with 200
- Models endpoint returns available models
- Chat completion returns valid response

**Run:**
```bash
k6 run tests/load/smoke-test.js
```

**Use when:**
- After deployment
- Before other load tests
- In CI/CD pipelines

### Baseline Test (`baseline-test.js`)

Establishes baseline performance metrics under moderate load.

**Metrics collected:**
- Request rate (requests/second)
- Response time (p50, p95, p99)
- Error rate
- Token generation statistics
- Time to first response (TTFT approximation)

**Run:**
```bash
k6 run tests/load/baseline-test.js
```

**Thresholds:**
- Error rate < 5%
- P95 latency < 15s
- P99 latency < 30s
- Success rate > 95%

### Stress Test (`stress-test.js`)

Gradually increases load to identify the system's breaking point.

**Load profile:**
```
VUs: 0 → 20 → 50 → 100 → 150 → 0
Duration: 25 minutes
```

**Stages:**
1. Ramp up to 20 VUs (2m)
2. Hold at 20 VUs (3m)
3. Ramp up to 50 VUs (2m)
4. Hold at 50 VUs (3m)
5. Ramp up to 100 VUs (2m)
6. Hold at 100 VUs (5m)
7. Ramp up to 150 VUs (2m)
8. Hold at 150 VUs (3m)
9. Ramp down (3m)

**Run:**
```bash
k6 run tests/load/stress-test.js
```

**What to look for:**
- Error rate spike at specific VU count
- Latency degradation pattern
- Rate limiting behavior
- Circuit breaker activations

### Soak Test (`soak-test.js`)

Extended test to detect memory leaks and performance degradation.

**Duration:** 2 hours
**Load:** 20 constant VUs

**Run:**
```bash
k6 run tests/load/soak-test.js
```

**Analysis:**
- Latency trend over time (should be stable)
- Memory usage (monitor externally)
- Connection pool behavior
- Error rate consistency

### Streaming Test (`streaming-test.js`)

Tests streaming chat completions with focus on Time to First Token (TTFT).

**Metrics:**
- Time to First Token (TTFT)
- Tokens per second
- Total tokens generated
- Streaming success rate

**Run:**
```bash
k6 run tests/load/streaming-test.js
```

**Thresholds:**
- TTFT < 5s at p95
- Streaming success rate > 95%

## Running with Docker

```bash
# Smoke test
docker run -i grafana/k6 run - <tests/load/smoke-test.js

# With environment variables
docker run -i \
  -e GATEWAY_URL=http://host.docker.internal:8080 \
  -e API_KEY=your-key \
  grafana/k6 run - <tests/load/baseline-test.js
```

## Output Formats

### JSON Output

```bash
k6 run --out json=results.json tests/load/baseline-test.js
```

### InfluxDB Output

```bash
k6 run --out influxdb=http://localhost:8086/k6 tests/load/baseline-test.js
```

### Prometheus Output

```bash
k6 run --out experimental-prometheus-rw=http://localhost:9090/api/v1/write tests/load/baseline-test.js
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Load Tests
on:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM

jobs:
  load-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup k6
        uses: grafana/setup-k6-action@v1

      - name: Run smoke test
        run: k6 run tests/load/smoke-test.js
        env:
          GATEWAY_URL: ${{ secrets.GATEWAY_URL }}
          API_KEY: ${{ secrets.API_KEY }}

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: load-test-results
          path: '*-summary.json'
```

### GitLab CI

```yaml
load_test:
  stage: test
  image: grafana/k6
  script:
    - k6 run tests/load/smoke-test.js
  artifacts:
    paths:
      - '*-summary.json'
```

## Interpreting Results

### Success Criteria

| Metric | Smoke | Baseline | Stress | Soak |
|--------|-------|----------|--------|------|
| Error Rate | <1% | <5% | <15% | <2% |
| P95 Latency | <15s | <15s | <30s | <15s |
| P99 Latency | - | <30s | - | <20s |
| Success Rate | >99% | >95% | >85% | >98% |

### Common Issues

**High Error Rate:**
- Check provider API limits
- Verify authentication
- Check circuit breaker status
- Review rate limiting configuration

**High Latency:**
- Provider response time
- Network latency
- Connection pool exhaustion
- Request queuing

**Degradation Over Time:**
- Memory leak in gateway
- Connection pool issues
- Resource exhaustion
- Provider throttling

## Customization

### Adding New Test Scenarios

Create a new file following the pattern:

```javascript
import { config, getHeaders, buildChatRequest } from './k6-config.js';

export const options = {
  scenarios: {
    custom: {
      executor: 'constant-vus',
      vus: 10,
      duration: '5m',
    },
  },
  thresholds: {
    http_req_failed: ['rate<0.05'],
  },
};

export default function () {
  // Your test logic
}
```

### Custom Thresholds

Modify thresholds in `k6-config.js` or per-test:

```javascript
export const options = {
  thresholds: {
    http_req_duration: ['p(95)<5000', 'p(99)<10000'],
    http_req_failed: ['rate<0.01'],
    'llm_tokens_per_second': ['avg>50'],
  },
};
```

## Troubleshooting

### Connection Refused

```
ERRO[0001] GoError: Get "http://localhost:8080/health": dial tcp: connection refused
```

**Solution:** Ensure the gateway is running and accessible.

### Authentication Errors

```
status is 200: false (401)
```

**Solution:** Set correct `API_KEY` environment variable.

### Timeout Errors

```
request timeout
```

**Solution:** Increase timeout in request options or check provider performance.

## License

Part of the LLM Inference Gateway project.
