# LLM-Inference-Gateway Deployment Guide

## Service Topology

**Service Name:** `llm-inference-gateway`

**Deployment Model:** Single unified Cloud Run service

**Agent Endpoints:**
| Endpoint | Method | Description |
|----------|--------|-------------|
| `/agents/route` | POST | Route inference request to provider |
| `/agents/inspect` | GET | Inspect routing configuration |
| `/agents/status` | GET | Get agent health and status |
| `/agents` | GET | List available agents |

**OpenAI-Compatible Endpoints:**
| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/chat/completions` | POST | Chat completion (streaming/non-streaming) |
| `/v1/models` | GET | List available models |

**Health Endpoints:**
| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Service health |
| `/live` | GET | Liveness probe |
| `/ready` | GET | Readiness probe |
| `/metrics` | GET | Prometheus metrics |

## Architecture Compliance

### What LLM-Inference-Gateway DOES:
- ✅ Receives inference requests from clients
- ✅ Selects providers and models using routing rules
- ✅ Applies routing decisions deterministically
- ✅ Emits DecisionEvents to ruvector-service
- ✅ Emits telemetry to LLM-Observatory
- ✅ Acts as controlled ingress to LLM execution

### What LLM-Inference-Gateway DOES NOT:
- ❌ Execute model inference
- ❌ Generate or modify prompts
- ❌ Analyze outputs
- ❌ Optimize configurations (LLM-Auto-Optimizer does this)
- ❌ Enforce policies beyond routing (LLM-Policy-Engine does this)
- ❌ Perform analytics or forecasting
- ❌ Connect directly to databases

## Environment Configuration

### Required Environment Variables
```bash
# Service Identity
SERVICE_NAME=llm-inference-gateway
SERVICE_VERSION=1.0.0
PLATFORM_ENV=dev|staging|prod

# RuVector Service (Persistence Layer)
RUVECTOR_SERVICE_URL=https://ruvector-service-{project}.run.app
RUVECTOR_API_KEY=<from-secret-manager>

# Telemetry (LLM-Observatory)
TELEMETRY_ENDPOINT=https://llm-observatory-{project}.run.app/v1/telemetry
OTEL_ENABLED=true
OTEL_SERVICE_NAME=llm-inference-gateway
OTEL_EXPORTER_OTLP_ENDPOINT=https://otel-collector-{project}.run.app

# Provider API Keys (from Secret Manager)
OPENAI_API_KEY=<from-secret-manager>
ANTHROPIC_API_KEY=<from-secret-manager>
GOOGLE_API_KEY=<from-secret-manager>
```

### Secrets (Secret Manager)
| Secret Name | Description | Required |
|-------------|-------------|----------|
| `ruvector-api-key` | RuVector service auth | Yes |
| `openai-api-key` | OpenAI provider | Yes |
| `anthropic-api-key` | Anthropic provider | Yes |
| `google-api-key` | Google/Gemini provider | No |

## Deployment Commands

### Quick Deploy (Dev)
```bash
./deploy/scripts/deploy.sh --env dev
```

### Full Deploy (Production)
```bash
./deploy/scripts/deploy.sh --project agentics-prod --env prod --region us-central1
```

### Using Cloud Build
```bash
gcloud builds submit --config=deploy/cloud-run/cloudbuild.yaml \
  --substitutions="_PLATFORM_ENV=prod"
```

### Manual Deploy
```bash
# Build and push image
docker build -t gcr.io/agentics-dev/llm-inference-gateway:latest \
  -f deploy/cloud-run/Dockerfile .
docker push gcr.io/agentics-dev/llm-inference-gateway:latest

# Deploy to Cloud Run
gcloud run deploy llm-inference-gateway \
  --image gcr.io/agentics-dev/llm-inference-gateway:latest \
  --region us-central1 \
  --platform managed \
  --allow-unauthenticated
```

## CLI Commands

### Agent Route
```bash
# Route a request
llm-gateway agent route --model gpt-4 --tenant acme-corp

# With fallback
llm-gateway agent route --model claude-3-opus --fallback --format json

# Dry run
llm-gateway agent route --model gpt-4 --dry-run
```

### Agent Inspect
```bash
# Basic inspection
llm-gateway agent inspect

# Detailed with metrics
llm-gateway agent inspect --detailed --metrics --health
```

### Agent Status
```bash
# Current status
llm-gateway agent status

# Watch mode
llm-gateway agent status --watch --interval 5
```

## Verification

### Run Verification Checklist
```bash
./deploy/scripts/verify.sh
```

### Manual Verification
```bash
SERVICE_URL=$(gcloud run services describe llm-inference-gateway \
  --region us-central1 --format='value(status.url)')

# Health check
curl -s "$SERVICE_URL/health"

# Agent route
curl -X POST "$SERVICE_URL/agents/route" \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-4","request_id":"test-123"}'

# Inspect
curl -s "$SERVICE_URL/agents/inspect"
```

## Rollback

### Automatic Rollback
```bash
./deploy/scripts/rollback.sh
```

### Manual Rollback
```bash
# List revisions
gcloud run revisions list --service llm-inference-gateway --region us-central1

# Rollback to specific revision
gcloud run services update-traffic llm-inference-gateway \
  --region us-central1 \
  --to-revisions=llm-inference-gateway-00002-abc=100
```

## Platform Integration

### Systems that INVOKE Inference Gateway:
- **LLM-Orchestrator** - Explicit invocation for inference requests
- **Direct clients** - API consumers via OpenAI-compatible endpoints

### Systems that PROVIDE to Inference Gateway:
- **LLM-Policy-Engine / Shield** - Routing constraints
- **LLM-Auto-Optimizer** - Routing rule updates (async)

### Systems that CONSUME from Inference Gateway:
- **ruvector-service** - DecisionEvent persistence
- **LLM-Observatory** - Routing telemetry
- **LLM-CostOps** - Cost metadata from routing decisions
- **Governance/Audit** - Compliance events

## IAM Requirements

### Service Account: `inference-gateway@{project}.iam.gserviceaccount.com`

**Required Roles:**
- `roles/secretmanager.secretAccessor` - Access API keys
- `roles/run.invoker` - Invoke other Cloud Run services
- `roles/logging.logWriter` - Write logs
- `roles/monitoring.metricWriter` - Write metrics
- `roles/cloudtrace.agent` - Send traces

## Failure Modes

| Failure | Detection | Resolution |
|---------|-----------|------------|
| Service not responding | Health check fails | Check logs, restart |
| Invalid routing decisions | Incorrect provider selection | Check routing rules |
| Schema mismatch | DecisionEvent rejection | Validate against agentics-contracts |
| Missing telemetry | No data in Observatory | Check OTEL config |
| Provider failures | Circuit breaker opens | Check provider status |

## Common Issues

### No healthy providers
```
Error: NoHealthyProviders
```
Solution: Configure at least one provider with valid API key.

### Schema validation failed
```
Error: Validation failed for DecisionEvent
```
Solution: Ensure agentics-contracts version matches.

### RuVector connection failed
```
Error: Failed to persist DecisionEvent
```
Solution: Verify RUVECTOR_SERVICE_URL and API key.
