# LLM Inference Gateway - Infrastructure Overview

## Quick Reference

This document provides a high-level overview of the LLM Inference Gateway infrastructure architecture, deployment options, and operational guidelines.

---

## Architecture at a Glance

```
┌─────────────────────────────────────────────────────────────────┐
│                        Client Layer                              │
│  Web Apps │ Mobile Apps │ Backend Services │ CLI Tools          │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Edge Layer (CDN/WAF)                          │
│  CloudFlare │ AWS CloudFront │ Fastly                           │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Load Balancer Layer (L7)                        │
│  AWS ALB │ GCP Load Balancer │ NGINX Ingress                    │
│  - TLS Termination                                               │
│  - Geographic Routing                                            │
│  - Health Checks                                                 │
│  - Rate Limiting                                                 │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              LLM Gateway Application Layer                       │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │  Gateway     │  │  Gateway     │  │  Gateway     │          │
│  │  Pod 1       │  │  Pod 2       │  │  Pod 3       │  ...     │
│  │              │  │              │  │              │          │
│  │ - Routing    │  │ - Routing    │  │ - Routing    │          │
│  │ - Transform  │  │ - Transform  │  │ - Transform  │          │
│  │ - Cache      │  │ - Cache      │  │ - Cache      │          │
│  │ - Metrics    │  │ - Metrics    │  │ - Metrics    │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         └──────────────────┼──────────────────┘                 │
└────────────────────────────┼────────────────────────────────────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
            ▼                ▼                ▼
┌───────────────────┐ ┌──────────────┐ ┌───────────────┐
│  Redis Cluster    │ │  Monitoring  │ │  Secrets      │
│  - Rate Limits    │ │  - Prometheus│ │  - Vault      │
│  - Caching        │ │  - Grafana   │ │  - AWS KMS    │
│  - Session State  │ │  - Jaeger    │ │               │
└───────────────────┘ └──────────────┘ └───────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    LLM Provider Layer                            │
│  ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌────────────┐       │
│  │ OpenAI  │  │Anthropic│  │  Azure   │  │   Google   │  ...  │
│  │  API    │  │  Claude │  │  OpenAI  │  │   Gemini   │       │
│  └─────────┘  └─────────┘  └──────────┘  └────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Deployment Models

### 1. Development Environment

**Recommended For:** Local development, testing, POC

**Infrastructure:**
- Docker Compose on local machine
- 2-4 CPU cores, 4-8GB RAM
- Local Redis instance
- Prometheus + Grafana (optional)

**Deployment Command:**
```bash
docker-compose -f deployment/docker-compose.dev.yml up
```

**Cost:** Free (local resources)

---

### 2. Small/Startup Environment

**Recommended For:** MVPs, small businesses, <1000 RPS

**Infrastructure:**
- Kubernetes cluster (3 nodes)
- 8 vCPUs total, 16GB RAM
- Managed Redis (2GB)
- Basic monitoring

**Cloud Costs (monthly):**
- AWS: ~$150-250
- GCP: ~$120-200
- Azure: ~$140-220

**Deployment:**
```bash
kubectl apply -k deployment/k8s/overlays/small
```

---

### 3. Medium/Production Environment

**Recommended For:** Established products, 1000-10,000 RPS

**Infrastructure:**
- Kubernetes cluster (6-10 nodes)
- 32-48 vCPUs total, 64-96GB RAM
- Managed Redis Cluster (8GB)
- Full monitoring stack
- Multi-AZ deployment

**Cloud Costs (monthly):**
- AWS: ~$800-1,200
- GCP: ~$600-1,000
- Azure: ~$700-1,100

**Deployment:**
```bash
terraform apply -var-file=environments/production.tfvars
kubectl apply -k deployment/k8s/overlays/production
```

---

### 4. Enterprise/Global Environment

**Recommended For:** Global scale, >10,000 RPS, multi-region

**Infrastructure:**
- Multi-region Kubernetes (3+ regions)
- 100+ vCPUs total, 200+ GB RAM
- Global Redis clusters
- Advanced monitoring & tracing
- Geographic load balancing
- Disaster recovery setup

**Cloud Costs (monthly):**
- AWS: ~$3,500-5,000
- GCP: ~$3,000-4,500
- Azure: ~$3,200-4,800

**Features:**
- <200ms latency worldwide
- 99.99% uptime SLA
- Automatic failover
- Data residency compliance (GDPR, etc.)

---

## Component Breakdown

### Core Components

| Component | Purpose | Scaling | Dependencies |
|-----------|---------|---------|--------------|
| **Gateway Pods** | Request routing, transformation, caching | Horizontal (HPA) | Redis, LLM APIs |
| **Redis Cluster** | Rate limiting, session state, caching | Vertical + Read replicas | None |
| **Load Balancer** | Traffic distribution, TLS termination | Auto-managed | Gateway Pods |
| **Prometheus** | Metrics collection and storage | Vertical | Gateway Pods |
| **Grafana** | Visualization and dashboards | Vertical | Prometheus |
| **AlertManager** | Alert routing and notifications | Vertical | Prometheus |

### Optional Components

| Component | Purpose | When to Use |
|-----------|---------|-------------|
| **Jaeger** | Distributed tracing | Debugging complex flows |
| **Loki** | Log aggregation | Centralized logging at scale |
| **CloudFront/CDN** | Edge caching | Global deployments |
| **WAF** | Web application firewall | Security-critical deployments |
| **External Secrets** | Secret management | Enterprise security requirements |

---

## Quick Start Guide

### Prerequisites

- Kubernetes cluster (1.24+)
- kubectl configured
- Helm 3.x installed (optional)
- Docker for local builds

### Rapid Deployment (5 minutes)

```bash
# 1. Clone repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# 2. Create namespace
kubectl create namespace llm-gateway

# 3. Create secrets
kubectl create secret generic llm-provider-secrets \
  --from-literal=openai-api-key="sk-..." \
  --from-literal=anthropic-api-key="sk-ant-..." \
  -n llm-gateway

# 4. Deploy
kubectl apply -k deployment/k8s/

# 5. Verify
kubectl get pods -n llm-gateway
kubectl logs -f -l app=llm-gateway -n llm-gateway

# 6. Access (port-forward for testing)
kubectl port-forward svc/llm-gateway-service 8080:80 -n llm-gateway

# Test
curl http://localhost:8080/health
```

### Production Deployment (Terraform)

```bash
# 1. Initialize Terraform
cd deployment/terraform
terraform init

# 2. Review plan
terraform plan -var-file=environments/production.tfvars

# 3. Apply infrastructure
terraform apply -var-file=environments/production.tfvars

# 4. Configure kubectl
aws eks update-kubeconfig --name llm-gateway-eks --region us-east-1

# 5. Deploy application
kubectl apply -k ../k8s/overlays/production

# 6. Verify deployment
./deployment/scripts/health-check.sh production
```

---

## Configuration Reference

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SERVER_HOST` | No | `0.0.0.0` | Server bind address |
| `SERVER_PORT` | No | `8080` | HTTP server port |
| `METRICS_PORT` | No | `9090` | Prometheus metrics port |
| `RUST_LOG` | No | `info` | Log level (debug, info, warn, error) |
| `REDIS_URL` | Yes | - | Redis connection URL |
| `OPENAI_API_KEY` | Yes* | - | OpenAI API key |
| `ANTHROPIC_API_KEY` | Yes* | - | Anthropic API key |
| `AZURE_API_KEY` | No | - | Azure OpenAI API key |
| `GOOGLE_API_KEY` | No | - | Google Gemini API key |

*At least one provider API key required

### Resource Requests/Limits

**Small Deployment (per pod):**
```yaml
resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 1000m
    memory: 2Gi
```

**Medium Deployment (per pod):**
```yaml
resources:
  requests:
    cpu: 1000m
    memory: 2Gi
  limits:
    cpu: 2000m
    memory: 4Gi
```

**Large Deployment (per pod):**
```yaml
resources:
  requests:
    cpu: 2000m
    memory: 4Gi
  limits:
    cpu: 4000m
    memory: 8Gi
```

---

## Monitoring & Observability

### Key Metrics

**Application Metrics:**
- `http_requests_total` - Total HTTP requests
- `http_request_duration_seconds` - Request latency histogram
- `llm_provider_requests_total` - Requests per provider
- `llm_provider_errors_total` - Errors per provider
- `llm_provider_latency_seconds` - Provider latency
- `llm_rate_limit_exceeded_total` - Rate limit hits

**Infrastructure Metrics:**
- `container_cpu_usage_seconds_total` - CPU usage
- `container_memory_working_set_bytes` - Memory usage
- `kube_pod_status_ready` - Pod health status
- `redis_connected_clients` - Redis connections

### Grafana Dashboards

Pre-configured dashboards available at `/deployment/monitoring/dashboards/`:

1. **Gateway Overview** - Request rate, latency, error rate
2. **Provider Health** - Per-provider metrics and health
3. **Infrastructure** - CPU, memory, network metrics
4. **Redis Metrics** - Cache hit rate, memory usage
5. **Cost Tracking** - Token usage and estimated costs

### Alerts

Critical alerts configured in AlertManager:

- **HighErrorRate** - Error rate >5% for 5 minutes
- **HighLatency** - P99 latency >10s for 10 minutes
- **PodDown** - Pod unavailable for 2 minutes
- **ProviderUnhealthy** - Provider health check failing
- **RedisDown** - Redis connection lost
- **HighMemoryUsage** - Memory usage >90%

---

## Security Best Practices

### Network Security

1. **Network Policies:** Restrict pod-to-pod communication
2. **TLS Everywhere:** Encrypt all external communication
3. **Private Subnets:** Run pods in private subnets
4. **WAF:** Use Web Application Firewall for edge protection
5. **DDoS Protection:** Enable CloudFlare or AWS Shield

### Secret Management

1. **External Secrets:** Use AWS Secrets Manager, HashiCorp Vault
2. **Rotation:** Rotate API keys regularly (90 days)
3. **Encryption at Rest:** Enable KMS encryption for secrets
4. **RBAC:** Limit secret access to specific service accounts
5. **Audit Logging:** Log all secret access

### Container Security

1. **Non-Root:** Run containers as non-root user (UID 1000)
2. **Read-Only FS:** Use read-only root filesystem
3. **No Privileged:** Never run privileged containers
4. **Distroless:** Use distroless base images for production
5. **Scanning:** Scan images with Trivy/Snyk before deployment
6. **Signed Images:** Sign and verify container images

---

## Disaster Recovery

### Backup Strategy

**What to Backup:**
- Redis data (automated snapshots every 5 min)
- Kubernetes configurations (GitOps with ArgoCD)
- Secrets (AWS Secrets Manager with versioning)
- Prometheus data (7-day retention, then S3)
- Application logs (30-day retention in Loki/S3)

**Backup Frequency:**
- Redis: Every 5 minutes
- Config: On every change (GitOps)
- Metrics: Continuous to Prometheus, daily to S3
- Logs: Real-time streaming

### Recovery Procedures

**RTO (Recovery Time Objective):** 15 minutes
**RPO (Recovery Point Objective):** 5 minutes

**Failure Scenarios:**

1. **Single Pod Failure**
   - Automatic: Kubernetes restarts pod
   - RTO: <1 minute
   - No manual intervention required

2. **Availability Zone Failure**
   - Automatic: Pods redistribute to healthy AZs
   - RTO: 2-5 minutes
   - Traffic automatically routed

3. **Region Failure**
   - Manual/Automatic: DNS failover to secondary region
   - RTO: 15 minutes
   - Requires manual approval (configurable)

4. **Redis Cluster Failure**
   - Automatic: ElastiCache automatic failover
   - RTO: 1-2 minutes
   - Data restored from last snapshot (RPO: 5 min)

5. **Complete Cluster Loss**
   - Manual: Restore from Terraform + GitOps
   - RTO: 30-60 minutes
   - Requires manual intervention

---

## Cost Optimization

### Strategies

1. **Right-Sizing**
   - Use VPA (Vertical Pod Autoscaler) to optimize requests/limits
   - Monitor actual usage vs allocated resources
   - Reduce pod count during off-peak hours

2. **Spot/Preemptible Instances**
   - Use spot instances for non-critical workloads
   - Mix spot (60%) + on-demand (40%) for cost savings
   - Potential savings: 50-70%

3. **Reserved Instances**
   - Commit to 1-3 year reservations for baseline capacity
   - Savings: 30-60% vs on-demand
   - Recommended for production

4. **Caching**
   - Enable aggressive caching for repeated requests
   - Cache provider model lists (1 hour TTL)
   - Cache embeddings and common prompts

5. **Provider Selection**
   - Route to cheaper providers when quality is acceptable
   - Use tiered routing (GPT-3.5 → GPT-4 escalation)
   - Implement cost-aware load balancing

### Cost Monitoring

Monitor costs with:
- AWS Cost Explorer / GCP Billing / Azure Cost Management
- Grafana cost dashboards (token usage × pricing)
- Alerts for unusual spending patterns
- Monthly cost reports

**Estimated Cost Breakdown (Medium Deployment):**
- Compute (EKS nodes): 60%
- Load Balancer: 10%
- Redis: 15%
- Data transfer: 10%
- Monitoring: 5%

---

## Troubleshooting Guide

### Common Issues

#### Issue: Pods Not Starting

**Symptoms:**
- Pods stuck in `Pending` or `CrashLoopBackOff`

**Investigation:**
```bash
kubectl describe pod <pod-name> -n llm-gateway
kubectl logs <pod-name> -n llm-gateway
```

**Solutions:**
- Check resource requests vs cluster capacity
- Verify secrets exist and are accessible
- Check image pull permissions
- Review init container logs

---

#### Issue: High Latency

**Symptoms:**
- P99 latency >5 seconds
- AlertManager firing `HighLatency` alert

**Investigation:**
```bash
# Check pod resources
kubectl top pods -n llm-gateway

# Check provider latency
kubectl exec -it <pod-name> -n llm-gateway -- curl localhost:9090/metrics | grep provider_latency

# Check logs for slow requests
kubectl logs -f <pod-name> -n llm-gateway | grep "duration_ms"
```

**Solutions:**
- Scale up pods: `kubectl scale deployment llm-gateway --replicas=8`
- Increase resource limits
- Enable caching
- Switch to faster LLM provider

---

#### Issue: Provider Errors

**Symptoms:**
- 5xx errors from gateway
- Provider health checks failing

**Investigation:**
```bash
# Check provider health
curl https://api.llmgateway.example.com/health | jq '.providers'

# Check provider-specific errors
kubectl logs -f -l app=llm-gateway | grep "provider_error"
```

**Solutions:**
- Verify API keys are valid
- Check provider API status pages
- Implement circuit breaker (automatic)
- Route traffic to healthy providers

---

## Performance Tuning

### Optimization Checklist

- [ ] Enable HTTP/2 and keep-alive connections
- [ ] Configure connection pooling (32 idle connections per provider)
- [ ] Set appropriate timeout values (60s OpenAI, 300s Anthropic)
- [ ] Enable response caching for repeated queries
- [ ] Use read replicas for Redis in high-traffic scenarios
- [ ] Configure HPA for automatic scaling
- [ ] Implement request batching where possible
- [ ] Enable compression for responses
- [ ] Use CDN for static content
- [ ] Monitor and optimize database queries

### Benchmarking

Run load tests with K6:
```bash
k6 run deployment/loadtest/k6-test.js
```

Expected Results (Medium Deployment):
- Throughput: 5,000 RPS
- P95 Latency: <500ms
- P99 Latency: <2s
- Error Rate: <0.1%

---

## Compliance & Certifications

### Supported Compliance Standards

- **SOC 2 Type II** - Security, availability, processing integrity
- **ISO 27001** - Information security management
- **GDPR** - European data protection (with EU deployment)
- **HIPAA** - Healthcare data (with BAA from providers)
- **PCI DSS** - Payment card industry (infrastructure level)

### Data Residency

Configure region-specific deployments for:
- EU (GDPR compliance)
- US (HIPAA/SOC 2)
- APAC (local data laws)

---

## Support & Documentation

### Documentation

- **Full Deployment Guide:** [DEPLOYMENT.md](./DEPLOYMENT.md)
- **Architecture Details:** [ARCHITECTURE.md](./ARCHITECTURE.md)
- **API Documentation:** [API-DESIGN-AND-VERSIONING.md](./API-DESIGN-AND-VERSIONING.md)
- **Provider Integration:** `/plans/provider-implementations.rs`

### Getting Help

- **GitHub Issues:** https://github.com/your-org/llm-gateway/issues
- **Slack Channel:** #llm-gateway-support
- **Email:** devops@example.com
- **On-Call:** PagerDuty integration for critical issues

---

## Roadmap

### Upcoming Features

- **Q1 2024:**
  - gRPC support
  - Enhanced prompt caching
  - Cost optimization features
  - Multi-model routing

- **Q2 2024:**
  - Streaming improvements
  - Function calling enhancements
  - Advanced analytics dashboard
  - Plugin system

- **Q3 2024:**
  - Edge deployment support
  - Vector database integration
  - RAG (Retrieval-Augmented Generation) support
  - A/B testing framework

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines on:
- Code style and standards
- Pull request process
- Testing requirements
- Documentation updates

---

## License

This project is licensed under the LLM Dev Ops Commercial License - see [LICENSE.md](./LICENSE.md) for details.

---

**Last Updated:** November 2024
**Version:** 1.0.0
**Maintained By:** DevOps Team
