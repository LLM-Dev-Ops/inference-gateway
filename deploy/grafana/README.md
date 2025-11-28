# Grafana Dashboards for LLM Inference Gateway

This directory contains pre-built Grafana dashboards and Prometheus alerting rules for monitoring the LLM Inference Gateway.

## Dashboards

### 1. LLM Gateway Overview (`llm-gateway-overview.json`)

A comprehensive overview dashboard providing high-level visibility into:

- **Request Metrics**: Request rate, error rate, P95 latency
- **Token Metrics**: Token throughput, input/output distribution
- **Provider Health**: Health status, circuit breaker states
- **Error Analysis**: Error rates by provider and type

**Key Panels:**
- Request Rate (requests/second)
- Error Rate (percentage)
- P95 Latency
- Active Requests
- Token Rate
- Healthy Provider Count
- Request Rate by Provider/Model
- Latency Percentiles (p50, p95, p99)
- Time to First Token (TTFT)
- Cache Operations
- Rate Limit Hits

### 2. Provider Details (`llm-gateway-providers.json`)

Drill-down dashboard for analyzing individual provider performance:

- Per-provider health status and circuit breaker state
- Model-level request distribution
- Latency analysis by model
- Token usage patterns
- Error breakdown

**Variables:**
- `provider`: Select specific provider to analyze

## Alerting Rules (`alerts.yaml`)

Prometheus alerting rules covering:

### Availability Alerts
- `LLMGatewayProviderUnhealthy`: Provider health check failing
- `LLMGatewayAllProvidersUnhealthy`: No healthy providers available
- `LLMGatewayCircuitBreakerOpen`: Circuit breaker tripped

### Latency Alerts
- `LLMGatewayHighLatencyP95`: P95 latency > 10s
- `LLMGatewayHighLatencyP99`: P99 latency > 30s
- `LLMGatewayHighTTFT`: Time to first token > 5s

### Error Alerts
- `LLMGatewayHighErrorRate`: Error rate > 5%
- `LLMGatewayCriticalErrorRate`: Error rate > 20%
- `LLMGatewayAuthenticationErrors`: Auth errors detected
- `LLMGatewayRateLimitErrors`: Provider rate limiting

### Capacity Alerts
- `LLMGatewayHighActiveRequests`: High concurrent requests
- `LLMGatewayRateLimitHits`: Tenant rate limiting

### Token Alerts
- `LLMGatewayHighTokenUsage`: High token consumption
- `LLMGatewayLowTokenThroughput`: Low generation speed

### Cache Alerts
- `LLMGatewayLowCacheHitRate`: Cache efficiency below 30%

## Installation

### Grafana Dashboard Import

1. Open Grafana and navigate to **Dashboards** > **Import**
2. Upload the JSON file or paste its contents
3. Select your Prometheus data source
4. Click **Import**

### Prometheus Alerting Rules

Add the rules to your Prometheus configuration:

```yaml
# prometheus.yml
rule_files:
  - /etc/prometheus/rules/llm-gateway-alerts.yaml
```

Or for Grafana Alerting:

1. Navigate to **Alerting** > **Alert rules**
2. Click **Import** and upload `alerts.yaml`

### Kubernetes ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: grafana-dashboards
  labels:
    grafana_dashboard: "1"
data:
  llm-gateway-overview.json: |
    <contents of llm-gateway-overview.json>
  llm-gateway-providers.json: |
    <contents of llm-gateway-providers.json>
```

## Metrics Reference

The dashboards query the following Prometheus metrics:

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `llm_gateway_llm_gateway_requests_total` | Counter | Total requests | model, provider, status, streaming |
| `llm_gateway_llm_gateway_request_duration_seconds` | Histogram | Request latency | model, provider, streaming |
| `llm_gateway_llm_gateway_tokens_total` | Counter | Total tokens processed | model, provider, type |
| `llm_gateway_llm_gateway_active_requests` | Gauge | Currently active requests | provider |
| `llm_gateway_llm_gateway_provider_health` | Gauge | Provider health (1=healthy, 0=unhealthy) | provider |
| `llm_gateway_llm_gateway_errors_total` | Counter | Total errors | provider, error_type |
| `llm_gateway_llm_gateway_circuit_breaker_state` | Gauge | Circuit breaker state (0=closed, 1=open, 2=half-open) | provider |
| `llm_gateway_llm_gateway_rate_limit_hits_total` | Counter | Rate limit hits | tenant, limit_type |
| `llm_gateway_llm_gateway_cache_operations_total` | Counter | Cache operations | operation, result |
| `llm_gateway_llm_gateway_ttft_seconds` | Histogram | Time to first token | model, provider |
| `llm_gateway_llm_gateway_tokens_per_second` | Gauge | Token generation rate | model, provider |

## Customization

### Adjusting Alert Thresholds

Edit `alerts.yaml` and modify the `expr` values. Common adjustments:

```yaml
# Example: Increase error rate threshold to 10%
- alert: LLMGatewayHighErrorRate
  expr: |
    (...) > 0.10  # Changed from 0.05
```

### Adding Custom Panels

1. Open the dashboard in Grafana
2. Click **Add** > **Visualization**
3. Configure your PromQL query
4. Export the updated dashboard JSON

### Multi-Cluster Support

Add a `cluster` variable to filter metrics:

```json
{
  "name": "cluster",
  "type": "query",
  "query": "label_values(llm_gateway_llm_gateway_requests_total, cluster)"
}
```

## Troubleshooting

### No Data in Dashboards

1. Verify the Prometheus data source is configured correctly
2. Check that the gateway is exposing metrics at `/metrics`
3. Ensure Prometheus is scraping the gateway
4. Verify metric names match (check namespace prefix)

### Missing Providers

1. Ensure providers are registered and active
2. Check that health checks are running
3. Verify provider configuration in gateway settings

### Alert Noise

1. Adjust `for` duration to increase tolerance
2. Modify thresholds based on your SLOs
3. Add exclusion labels for known maintenance windows

## License

These dashboards are part of the LLM Inference Gateway project.
