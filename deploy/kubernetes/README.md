# LLM Inference Gateway - Kubernetes Deployment

Production-ready Kubernetes manifests for deploying the LLM Inference Gateway.

## Prerequisites

- Kubernetes cluster (1.25+)
- kubectl configured
- Container registry access
- (Optional) NGINX Ingress Controller
- (Optional) cert-manager for TLS
- (Optional) Prometheus for metrics

## Quick Start

### 1. Create the namespace and deploy

```bash
# Using kustomize (recommended)
kubectl apply -k deploy/kubernetes/

# Or apply individually
kubectl apply -f deploy/kubernetes/namespace.yaml
kubectl apply -f deploy/kubernetes/configmap.yaml
kubectl apply -f deploy/kubernetes/secret.yaml
kubectl apply -f deploy/kubernetes/rbac.yaml
kubectl apply -f deploy/kubernetes/deployment.yaml
kubectl apply -f deploy/kubernetes/service.yaml
kubectl apply -f deploy/kubernetes/hpa.yaml
```

### 2. Configure secrets

**Important:** Update the secrets before deploying to production.

```bash
# Create secrets from literal values
kubectl create secret generic llm-gateway-secrets \
  --namespace=llm-gateway \
  --from-literal=OPENAI_API_KEY='sk-your-key' \
  --from-literal=ANTHROPIC_API_KEY='sk-ant-your-key' \
  --from-literal=JWT_SECRET='your-32-char-secret' \
  --dry-run=client -o yaml | kubectl apply -f -
```

For production, use:
- [External Secrets Operator](https://external-secrets.io/)
- [Sealed Secrets](https://sealed-secrets.netlify.app/)
- [SOPS](https://github.com/mozilla/sops)
- Cloud provider secrets management (AWS Secrets Manager, GCP Secret Manager, Azure Key Vault)

### 3. Verify deployment

```bash
# Check pods
kubectl get pods -n llm-gateway

# Check services
kubectl get svc -n llm-gateway

# View logs
kubectl logs -n llm-gateway -l app.kubernetes.io/name=llm-inference-gateway -f

# Test health endpoint
kubectl port-forward -n llm-gateway svc/llm-gateway 8080:80
curl http://localhost:8080/health
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Kubernetes Cluster                      │
│                                                              │
│  ┌─────────────┐     ┌─────────────────────────────────┐   │
│  │   Ingress   │────▶│        LLM Gateway Service       │   │
│  │  (NGINX)    │     │         (ClusterIP:80)          │   │
│  └─────────────┘     └─────────────────────────────────┘   │
│                                    │                        │
│                     ┌──────────────┼──────────────┐        │
│                     ▼              ▼              ▼        │
│              ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│              │  Pod 1   │  │  Pod 2   │  │  Pod 3   │     │
│              │ Gateway  │  │ Gateway  │  │ Gateway  │     │
│              └──────────┘  └──────────┘  └──────────┘     │
│                     │              │              │        │
│                     └──────────────┼──────────────┘        │
│                                    ▼                        │
│                          ┌─────────────────┐               │
│                          │     Redis       │               │
│                          │ (Distributed    │               │
│                          │     Cache)      │               │
│                          └─────────────────┘               │
└─────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────┐
                    │    External LLM APIs      │
                    │  (OpenAI, Anthropic, etc) │
                    └───────────────────────────┘
```

## Components

| File | Description |
|------|-------------|
| `namespace.yaml` | Dedicated namespace for isolation |
| `configmap.yaml` | Non-sensitive configuration |
| `secret.yaml` | API keys and sensitive data (template) |
| `rbac.yaml` | ServiceAccount, Role, RoleBinding, NetworkPolicy |
| `deployment.yaml` | Gateway deployment with health probes, PDB |
| `service.yaml` | ClusterIP and headless services |
| `hpa.yaml` | Horizontal Pod Autoscaler |
| `ingress.yaml` | NGINX Ingress with TLS |
| `redis.yaml` | Redis for distributed caching (optional) |
| `kustomization.yaml` | Kustomize configuration |

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GATEWAY_HOST` | Server bind address | `0.0.0.0` |
| `GATEWAY_PORT` | HTTP port | `8080` |
| `GATEWAY_METRICS_PORT` | Metrics port | `9090` |
| `LOG_LEVEL` | Logging level | `info` |
| `CACHE_ENABLED` | Enable response caching | `true` |
| `REDIS_ENABLED` | Enable Redis cache | `false` |
| `RATE_LIMIT_ENABLED` | Enable rate limiting | `true` |

### Resource Limits

Default resource configuration:

```yaml
resources:
  requests:
    cpu: "500m"
    memory: "512Mi"
  limits:
    cpu: "2000m"
    memory: "2Gi"
```

Adjust based on your workload. For high-traffic deployments, consider:

```yaml
resources:
  requests:
    cpu: "1000m"
    memory: "1Gi"
  limits:
    cpu: "4000m"
    memory: "4Gi"
```

### Scaling

The HPA is configured to scale between 3-20 replicas based on:
- CPU utilization (target: 70%)
- Memory utilization (target: 80%)

Customize in `hpa.yaml`:

```yaml
spec:
  minReplicas: 3
  maxReplicas: 20
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

## Production Checklist

- [ ] Configure proper secrets management
- [ ] Set resource limits appropriate for workload
- [ ] Configure TLS certificates
- [ ] Set up monitoring and alerting
- [ ] Configure backup for Redis (if used)
- [ ] Review and apply NetworkPolicy
- [ ] Configure proper ingress domain
- [ ] Set up log aggregation
- [ ] Configure proper replica count
- [ ] Review security context settings

## Monitoring

### Prometheus Metrics

The gateway exposes Prometheus metrics on port 9090:

```bash
# Port forward metrics
kubectl port-forward -n llm-gateway svc/llm-gateway 9090:9090
curl http://localhost:9090/metrics
```

### Key Metrics

- `llm_gateway_requests_total` - Total request count
- `llm_gateway_request_duration_seconds` - Request latency histogram
- `llm_gateway_tokens_total` - Token usage
- `llm_gateway_cache_hits_total` - Cache hit ratio
- `llm_gateway_provider_errors_total` - Provider error count

### Grafana Dashboard

Import the provided dashboard or create custom visualizations:

```bash
kubectl port-forward -n monitoring svc/grafana 3000:80
```

## Troubleshooting

### Pods not starting

```bash
# Check events
kubectl describe pod -n llm-gateway <pod-name>

# Check logs
kubectl logs -n llm-gateway <pod-name> --previous
```

### Connection issues

```bash
# Test internal connectivity
kubectl run test --rm -it --image=curlimages/curl -- \
  curl http://llm-gateway.llm-gateway.svc.cluster.local/health

# Check endpoints
kubectl get endpoints -n llm-gateway
```

### Performance issues

```bash
# Check resource usage
kubectl top pods -n llm-gateway

# Check HPA status
kubectl describe hpa -n llm-gateway llm-gateway-hpa
```

## Overlays

Create environment-specific overlays:

```
deploy/kubernetes/
├── base/
│   └── kustomization.yaml
├── overlays/
│   ├── development/
│   │   └── kustomization.yaml
│   ├── staging/
│   │   └── kustomization.yaml
│   └── production/
│       └── kustomization.yaml
```

Example production overlay:

```yaml
# overlays/production/kustomization.yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

resources:
  - ../../base

replicas:
  - name: llm-gateway
    count: 10

patches:
  - patch: |-
      - op: replace
        path: /spec/template/spec/containers/0/resources/requests/cpu
        value: "2000m"
    target:
      kind: Deployment
      name: llm-gateway
```

## Security

### Network Policies

The included NetworkPolicy restricts:
- Ingress: Only from ingress-nginx and monitoring namespaces
- Egress: DNS, HTTPS (external APIs), Redis, OpenTelemetry

### Pod Security

- Runs as non-root user (UID 1000)
- Read-only root filesystem
- No privilege escalation
- All capabilities dropped
- Seccomp profile enabled

### Secrets Management

For production, integrate with:

1. **AWS Secrets Manager**:
   ```yaml
   apiVersion: external-secrets.io/v1beta1
   kind: ExternalSecret
   spec:
     secretStoreRef:
       kind: ClusterSecretStore
       name: aws-secrets-manager
   ```

2. **HashiCorp Vault**:
   ```yaml
   apiVersion: external-secrets.io/v1beta1
   kind: ExternalSecret
   spec:
     secretStoreRef:
       kind: ClusterSecretStore
       name: vault-backend
   ```

## License

See LICENSE file in the repository root.
