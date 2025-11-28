# LLM Inference Gateway - Deployment Guide

Comprehensive deployment guide for the LLM Inference Gateway across different environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Docker Deployment](#docker-deployment)
- [Kubernetes Deployment](#kubernetes-deployment)
- [Cloud Deployments](#cloud-deployments)
- [Production Checklist](#production-checklist)
- [Monitoring & Observability](#monitoring--observability)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| Memory | 1 GB | 4+ GB |
| Disk | 1 GB | 10+ GB |
| Network | 100 Mbps | 1 Gbps |

### Software Requirements

- **Docker**: 20.10+ (for container deployment)
- **Kubernetes**: 1.25+ (for K8s deployment)
- **Rust**: 1.75+ (for building from source)

### API Keys

At minimum, you need one LLM provider API key:

| Provider | Environment Variable | Get API Key |
|----------|---------------------|-------------|
| OpenAI | `OPENAI_API_KEY` | https://platform.openai.com/api-keys |
| Anthropic | `ANTHROPIC_API_KEY` | https://console.anthropic.com/ |
| Google | `GOOGLE_API_KEY` | https://makersuite.google.com/app/apikey |

---

## Quick Start

### Option 1: Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# Set up environment
cp .env.example .env
# Edit .env and add your API keys

# Start the gateway
docker-compose up -d

# Verify it's running
curl http://localhost:8080/health
```

### Option 2: Binary

```bash
# Download the latest release
curl -LO https://github.com/your-org/llm-inference-gateway/releases/latest/download/llm-gateway-linux-amd64.tar.gz
tar xzf llm-gateway-linux-amd64.tar.gz

# Set environment variables
export OPENAI_API_KEY=sk-your-key

# Run the gateway
./llm-gateway --config gateway.yaml
```

### Option 3: Build from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway
cargo build --release

# Run
./target/release/llm-gateway
```

---

## Docker Deployment

### Basic Deployment

```bash
# Start core services (gateway + Redis)
docker-compose up -d

# Verify services
docker-compose ps
docker-compose logs gateway
```

### With Monitoring Stack

```bash
# Start with Prometheus and Grafana
docker-compose --profile monitoring up -d

# Access Grafana
open http://localhost:3000
# Default credentials: admin/admin
```

### With Full Observability

```bash
# Start with monitoring + tracing
docker-compose --profile monitoring --profile tracing up -d

# Access services
open http://localhost:3000   # Grafana
open http://localhost:16686  # Jaeger
```

### Docker Compose Configuration

```yaml
# docker-compose.override.yml - Custom overrides
version: '3.8'

services:
  gateway:
    environment:
      - LOG_LEVEL=debug
      - CACHE_TTL=7200
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 4G
    volumes:
      - ./custom-config.yaml:/etc/llm-gateway/gateway.yaml:ro
```

### Building Custom Images

```bash
# Build production image
docker build -t llm-gateway:latest .

# Build development image
docker build --target development -t llm-gateway:dev .

# Multi-platform build
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t your-registry/llm-gateway:latest \
  --push .
```

---

## Kubernetes Deployment

### Prerequisites

- Kubernetes cluster 1.25+
- kubectl configured
- Helm 3.0+ (optional)

### Quick Start

```bash
# Create namespace
kubectl create namespace llm-gateway

# Create secrets
kubectl create secret generic llm-gateway-secrets \
  --namespace llm-gateway \
  --from-literal=openai-api-key=sk-your-key \
  --from-literal=anthropic-api-key=sk-ant-your-key

# Apply manifests
kubectl apply -k deploy/kubernetes/

# Verify deployment
kubectl -n llm-gateway get pods
kubectl -n llm-gateway get services
```

### Using Kustomize

```bash
# Base deployment
kubectl apply -k deploy/kubernetes/

# Production overlay
kubectl apply -k deploy/kubernetes/overlays/production/

# With custom patches
cat > kustomization.yaml <<EOF
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - deploy/kubernetes
patches:
  - path: custom-patch.yaml
EOF
kubectl apply -k .
```

### Manifest Structure

```
deploy/kubernetes/
├── kustomization.yaml      # Kustomize config
├── namespace.yaml          # Namespace definition
├── configmap.yaml          # Gateway configuration
├── secret.yaml             # API keys (template)
├── deployment.yaml         # Gateway deployment
├── service.yaml            # Service definitions
├── hpa.yaml               # Horizontal Pod Autoscaler
├── ingress.yaml           # Ingress configuration
├── rbac.yaml              # RBAC policies
├── redis.yaml             # Redis StatefulSet
└── README.md              # Kubernetes-specific docs
```

### Scaling

```bash
# Manual scaling
kubectl -n llm-gateway scale deployment llm-gateway --replicas=5

# HPA is configured automatically
# Check HPA status
kubectl -n llm-gateway get hpa

# View scaling events
kubectl -n llm-gateway describe hpa llm-gateway
```

### Ingress Configuration

```yaml
# Custom ingress with TLS
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: llm-gateway
  namespace: llm-gateway
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
spec:
  ingressClassName: nginx
  tls:
    - hosts:
        - llm-gateway.example.com
      secretName: llm-gateway-tls
  rules:
    - host: llm-gateway.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: llm-gateway
                port:
                  number: 8080
```

### Resource Recommendations

| Environment | Replicas | CPU Request | CPU Limit | Memory Request | Memory Limit |
|-------------|----------|-------------|-----------|----------------|--------------|
| Development | 1 | 100m | 500m | 256Mi | 512Mi |
| Staging | 2 | 250m | 1000m | 512Mi | 1Gi |
| Production | 3+ | 500m | 2000m | 1Gi | 4Gi |

---

## Cloud Deployments

### AWS EKS

```bash
# Create EKS cluster
eksctl create cluster \
  --name llm-gateway \
  --region us-west-2 \
  --nodegroup-name standard \
  --node-type t3.large \
  --nodes 3

# Install AWS Load Balancer Controller
helm repo add eks https://aws.github.io/eks-charts
helm install aws-load-balancer-controller eks/aws-load-balancer-controller \
  -n kube-system \
  --set clusterName=llm-gateway

# Deploy gateway
kubectl apply -k deploy/kubernetes/overlays/aws/

# Create ElastiCache Redis (recommended for production)
aws elasticache create-cache-cluster \
  --cache-cluster-id llm-gateway-redis \
  --engine redis \
  --cache-node-type cache.t3.medium \
  --num-cache-nodes 1
```

### Google GKE

```bash
# Create GKE cluster
gcloud container clusters create llm-gateway \
  --region us-central1 \
  --num-nodes 3 \
  --machine-type e2-standard-4

# Get credentials
gcloud container clusters get-credentials llm-gateway --region us-central1

# Deploy gateway
kubectl apply -k deploy/kubernetes/overlays/gcp/

# Create Memorystore Redis
gcloud redis instances create llm-gateway-redis \
  --size=1 \
  --region=us-central1 \
  --redis-version=redis_7_0
```

### Azure AKS

```bash
# Create AKS cluster
az aks create \
  --resource-group llm-gateway-rg \
  --name llm-gateway \
  --node-count 3 \
  --node-vm-size Standard_D4s_v3 \
  --enable-managed-identity

# Get credentials
az aks get-credentials --resource-group llm-gateway-rg --name llm-gateway

# Deploy gateway
kubectl apply -k deploy/kubernetes/overlays/azure/

# Create Azure Cache for Redis
az redis create \
  --name llm-gateway-redis \
  --resource-group llm-gateway-rg \
  --location eastus \
  --sku Standard \
  --vm-size c1
```

### AWS ECS (Fargate)

```bash
# Create ECS cluster
aws ecs create-cluster --cluster-name llm-gateway

# Create task definition
aws ecs register-task-definition \
  --cli-input-json file://deploy/ecs/task-definition.json

# Create service
aws ecs create-service \
  --cluster llm-gateway \
  --service-name llm-gateway \
  --task-definition llm-gateway:1 \
  --desired-count 3 \
  --launch-type FARGATE \
  --network-configuration "awsvpcConfiguration={subnets=[subnet-xxx],securityGroups=[sg-xxx],assignPublicIp=ENABLED}"
```

### Cloud Run (GCP)

```bash
# Build and push image
gcloud builds submit --tag gcr.io/PROJECT_ID/llm-gateway

# Deploy to Cloud Run
gcloud run deploy llm-gateway \
  --image gcr.io/PROJECT_ID/llm-gateway \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --set-env-vars "OPENAI_API_KEY=sk-xxx" \
  --memory 2Gi \
  --cpu 2 \
  --min-instances 1 \
  --max-instances 10
```

---

## Production Checklist

### Security

- [ ] **TLS Enabled**: All traffic encrypted with TLS 1.2+
- [ ] **Authentication Enabled**: API key or JWT authentication configured
- [ ] **Secrets Management**: API keys stored in secrets manager (Vault, AWS Secrets Manager, etc.)
- [ ] **Network Policies**: Pod-to-pod traffic restricted
- [ ] **PII Redaction**: Enabled for logs and traces
- [ ] **RBAC**: Kubernetes RBAC policies configured
- [ ] **Container Security**: Non-root user, read-only filesystem

### Reliability

- [ ] **Health Checks**: Liveness and readiness probes configured
- [ ] **Resource Limits**: CPU and memory limits set
- [ ] **HPA Configured**: Auto-scaling based on CPU/memory
- [ ] **PDB Configured**: Pod Disruption Budget for availability
- [ ] **Circuit Breakers**: Enabled for upstream providers
- [ ] **Retry Policies**: Configured with exponential backoff
- [ ] **Rate Limiting**: Enabled to protect upstream providers

### Observability

- [ ] **Logging**: Structured JSON logging enabled
- [ ] **Metrics**: Prometheus metrics exposed
- [ ] **Tracing**: OpenTelemetry tracing enabled
- [ ] **Alerting**: Alerts configured for key metrics
- [ ] **Dashboards**: Grafana dashboards deployed

### Performance

- [ ] **Caching**: Redis caching enabled
- [ ] **Connection Pooling**: HTTP/2 enabled
- [ ] **Compression**: Response compression enabled
- [ ] **CDN**: Consider CDN for static assets (if any)

### Operations

- [ ] **Backup**: Redis data backed up (if using persistent cache)
- [ ] **Disaster Recovery**: DR plan documented
- [ ] **Runbook**: Operational runbook created
- [ ] **Incident Response**: On-call procedures defined

---

## Monitoring & Observability

### Prometheus Metrics

Key metrics to monitor:

```promql
# Request rate
rate(llm_gateway_requests_total[5m])

# Error rate
sum(rate(llm_gateway_requests_total{status="error"}[5m]))
  / sum(rate(llm_gateway_requests_total[5m]))

# Latency p99
histogram_quantile(0.99, rate(llm_gateway_request_duration_seconds_bucket[5m]))

# Cache hit ratio
sum(rate(llm_gateway_cache_hits_total[5m]))
  / (sum(rate(llm_gateway_cache_hits_total[5m])) + sum(rate(llm_gateway_cache_misses_total[5m])))

# Token usage rate
rate(llm_gateway_tokens_total[5m])

# Cost per hour
sum(increase(llm_gateway_cost_dollars_total[1h]))
```

### Alerting Rules

```yaml
# prometheus-alerts.yaml
groups:
  - name: llm-gateway
    rules:
      - alert: HighErrorRate
        expr: |
          sum(rate(llm_gateway_requests_total{status="error"}[5m]))
          / sum(rate(llm_gateway_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: High error rate detected
          description: Error rate is above 5% for the last 5 minutes

      - alert: HighLatency
        expr: |
          histogram_quantile(0.99, rate(llm_gateway_request_duration_seconds_bucket[5m])) > 5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: High latency detected
          description: P99 latency is above 5 seconds

      - alert: ProviderUnhealthy
        expr: llm_gateway_provider_health == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: Provider is unhealthy
          description: "Provider {{ $labels.provider }} is unhealthy"

      - alert: RateLimitExceeded
        expr: rate(llm_gateway_rate_limit_exceeded_total[5m]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: Rate limit frequently exceeded
          description: Users are hitting rate limits frequently
```

### Grafana Dashboards

Import pre-built dashboards from `deploy/monitoring/grafana/provisioning/dashboards/json/`:

1. **Gateway Overview**: Request rates, latencies, error rates
2. **Provider Health**: Per-provider metrics and health status
3. **Cache Performance**: Hit ratios, eviction rates
4. **Cost Tracking**: Token usage and cost by model

### Log Analysis

```bash
# View gateway logs
kubectl -n llm-gateway logs -f deployment/llm-gateway

# Filter error logs
kubectl -n llm-gateway logs deployment/llm-gateway | jq 'select(.level == "ERROR")'

# Search for specific request ID
kubectl -n llm-gateway logs deployment/llm-gateway | jq 'select(.request_id == "req_xxx")'
```

---

## Troubleshooting

### Common Issues

#### Gateway Won't Start

```bash
# Check logs
docker-compose logs gateway
kubectl -n llm-gateway logs deployment/llm-gateway

# Common causes:
# 1. Missing API keys - check environment variables
# 2. Invalid config file - validate YAML syntax
# 3. Port already in use - check with netstat
# 4. Insufficient permissions - check file permissions
```

#### Connection Refused to Providers

```bash
# Check network connectivity
curl -I https://api.openai.com
curl -I https://api.anthropic.com

# Check DNS resolution
nslookup api.openai.com

# Check firewall rules
# Ensure outbound HTTPS (443) is allowed
```

#### High Memory Usage

```bash
# Check current usage
kubectl -n llm-gateway top pods

# Possible causes:
# 1. Large cache size - reduce cache.memory.max_size
# 2. Many concurrent connections - reduce max_connections
# 3. Memory leak - check for increasing usage over time
```

#### Slow Response Times

```bash
# Check provider latency
curl -w "@curl-format.txt" -s -o /dev/null http://localhost:8080/v1/chat/completions

# Check cache hit ratio
curl http://localhost:9090/metrics | grep cache_hits

# Possible causes:
# 1. Cache misses - increase cache TTL
# 2. Provider slowness - check provider status pages
# 3. Network latency - check network path
```

#### Redis Connection Issues

```bash
# Test Redis connectivity
redis-cli -h redis ping

# Check Redis logs
docker-compose logs redis
kubectl -n llm-gateway logs statefulset/redis

# Verify Redis URL in config
echo $REDIS_URL
```

### Debug Mode

Enable debug logging for more detailed information:

```bash
# Environment variable
export LOG_LEVEL=debug

# Docker Compose
docker-compose exec gateway sh -c 'LOG_LEVEL=debug ./llm-gateway'

# Kubernetes
kubectl -n llm-gateway set env deployment/llm-gateway LOG_LEVEL=debug
```

### Health Check Endpoints

```bash
# Basic health
curl http://localhost:8080/health

# Readiness (includes provider checks)
curl http://localhost:8080/ready

# Liveness
curl http://localhost:8080/live

# Detailed status
curl http://localhost:8080/admin/stats
```

### Getting Help

1. **Check Documentation**: Review this guide and related docs
2. **Search Issues**: Check GitHub issues for similar problems
3. **Debug Logs**: Enable debug logging and capture relevant logs
4. **Report Issue**: Open a GitHub issue with:
   - Gateway version
   - Configuration (redact secrets)
   - Error messages and logs
   - Steps to reproduce

---

## Upgrade Guide

### Docker

```bash
# Pull latest image
docker-compose pull

# Stop current deployment
docker-compose down

# Start with new version
docker-compose up -d

# Verify
curl http://localhost:8080/health
```

### Kubernetes

```bash
# Update image tag in kustomization.yaml or deployment.yaml
# Then apply:
kubectl apply -k deploy/kubernetes/

# Or use rolling update:
kubectl -n llm-gateway set image deployment/llm-gateway \
  gateway=your-registry/llm-gateway:new-version

# Watch rollout
kubectl -n llm-gateway rollout status deployment/llm-gateway

# Rollback if needed
kubectl -n llm-gateway rollout undo deployment/llm-gateway
```

### Database Migrations

If upgrading requires Redis data migration:

```bash
# Backup Redis data
redis-cli -h redis BGSAVE
kubectl -n llm-gateway exec redis-0 -- redis-cli BGSAVE

# The gateway handles cache invalidation automatically
# Old cache entries will expire based on TTL
```

---

## Rollback Procedures

### Docker

```bash
# Stop current version
docker-compose down

# Use specific version tag
docker-compose -f docker-compose.yml \
  -e GATEWAY_IMAGE=llm-gateway:previous-version \
  up -d
```

### Kubernetes

```bash
# View rollout history
kubectl -n llm-gateway rollout history deployment/llm-gateway

# Rollback to previous version
kubectl -n llm-gateway rollout undo deployment/llm-gateway

# Rollback to specific revision
kubectl -n llm-gateway rollout undo deployment/llm-gateway --to-revision=2
```
