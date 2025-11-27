# LLM Inference Gateway - Deployment and Infrastructure Architecture

## Table of Contents

- [1. Deployment Topologies](#1-deployment-topologies)
  - [Single Instance (Development)](#single-instance-development)
  - [High Availability (Production)](#high-availability-production)
  - [Multi-Region (Enterprise)](#multi-region-enterprise)
- [2. Kubernetes Architecture](#2-kubernetes-architecture)
- [3. Infrastructure Components](#3-infrastructure-components)
- [4. Container Architecture](#4-container-architecture)
- [5. CI/CD Pipeline](#5-cicd-pipeline)
- [6. Monitoring Infrastructure](#6-monitoring-infrastructure)
- [7. Resource Sizing Guide](#7-resource-sizing-guide)
- [8. Security Hardening](#8-security-hardening)
- [9. Operational Procedures](#9-operational-procedures)

---

## 1. Deployment Topologies

### Single Instance (Development)

**Use Case:** Local development, testing, and proof-of-concept deployments

#### Local Development Setup

```bash
# Clone repository
git clone https://github.com/your-org/llm-inference-gateway.git
cd llm-inference-gateway

# Build the project
cargo build --release

# Set environment variables
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export LOG_LEVEL="info"
export SERVER_PORT="8080"

# Run locally
cargo run --release
```

#### Docker Compose Configuration

Create `docker-compose.dev.yml`:

```yaml
version: '3.8'

services:
  gateway:
    build:
      context: .
      dockerfile: Dockerfile
      target: runtime
    ports:
      - "8080:8080"
      - "9090:9090"  # Metrics endpoint
    environment:
      - RUST_LOG=info
      - SERVER_HOST=0.0.0.0
      - SERVER_PORT=8080
      - METRICS_PORT=9090
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - REDIS_URL=redis://redis:6379
    depends_on:
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    restart: unless-stopped
    networks:
      - gateway-net

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - gateway-net

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./deployment/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
    networks:
      - gateway-net

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    volumes:
      - grafana-data:/var/lib/grafana
      - ./deployment/grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./deployment/grafana/datasources:/etc/grafana/provisioning/datasources
    depends_on:
      - prometheus
    networks:
      - gateway-net

volumes:
  redis-data:
  prometheus-data:
  grafana-data:

networks:
  gateway-net:
    driver: bridge
```

#### Resource Requirements (Development)

| Component | CPU | Memory | Storage |
|-----------|-----|--------|---------|
| Gateway | 1 core | 512MB | 1GB |
| Redis | 0.5 core | 256MB | 2GB |
| Prometheus | 0.5 core | 512MB | 10GB |
| Grafana | 0.25 core | 256MB | 1GB |
| **Total** | **2.25 cores** | **1.5GB** | **14GB** |

---

### High Availability (Production)

**Use Case:** Production workloads requiring 99.9% uptime with automatic failover

#### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                   Load Balancer (L7)                    │
│              (AWS ALB / GCP LB / NGINX)                 │
└────────────┬────────────────────────┬───────────────────┘
             │                        │
    ┌────────▼─────────┐     ┌────────▼─────────┐
    │  Gateway Pod 1   │     │  Gateway Pod 2   │
    │  (Active)        │     │  (Active)        │
    │  - Rate Limiting │     │  - Rate Limiting │
    │  - Caching       │     │  - Caching       │
    │  - Health Checks │     │  - Health Checks │
    └────────┬─────────┘     └────────┬─────────┘
             │                        │
    ┌────────▼─────────┐     ┌────────▼─────────┐
    │  Gateway Pod 3   │     │  Gateway Pod 4   │
    │  (Active)        │     │  (Active)        │
    └────────┬─────────┘     └────────┬─────────┘
             │                        │
             └────────┬───────────────┘
                      │
         ┌────────────▼────────────┐
         │   Redis Cluster         │
         │   (Master + Replicas)   │
         │   - Session Storage     │
         │   - Rate Limit State    │
         └─────────────────────────┘
                      │
         ┌────────────▼────────────┐
         │   LLM Providers         │
         │   - OpenAI              │
         │   - Anthropic           │
         │   - Azure OpenAI        │
         │   - AWS Bedrock         │
         └─────────────────────────┘
```

#### Load Balancer Configuration

**AWS Application Load Balancer (ALB)**

```yaml
# ALB Target Group Configuration
TargetGroup:
  Protocol: HTTP
  Port: 8080
  HealthCheck:
    Protocol: HTTP
    Path: /health
    Interval: 30
    Timeout: 5
    HealthyThreshold: 2
    UnhealthyThreshold: 3
    Matcher:
      HttpCode: 200
  Stickiness:
    Enabled: false  # Stateless gateway, no session affinity needed
    Type: lb_cookie
    Duration: 0
  DeregistrationDelay: 30
  Attributes:
    - Key: deregistration_delay.connection_termination.enabled
      Value: true
```

**NGINX Load Balancer Configuration**

```nginx
upstream llm_gateway {
    least_conn;  # Least connections algorithm

    server gateway-1.internal:8080 max_fails=3 fail_timeout=30s;
    server gateway-2.internal:8080 max_fails=3 fail_timeout=30s;
    server gateway-3.internal:8080 max_fails=3 fail_timeout=30s;
    server gateway-4.internal:8080 max_fails=3 fail_timeout=30s;

    keepalive 32;
}

server {
    listen 80;
    listen 443 ssl http2;
    server_name api.llmgateway.example.com;

    ssl_certificate /etc/nginx/ssl/cert.pem;
    ssl_certificate_key /etc/nginx/ssl/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Security headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api_limit:10m rate=100r/s;
    limit_req zone=api_limit burst=200 nodelay;

    # Connection limits
    limit_conn_zone $binary_remote_addr zone=conn_limit:10m;
    limit_conn conn_limit 20;

    location / {
        proxy_pass http://llm_gateway;
        proxy_http_version 1.1;

        # Headers
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Connection "";

        # Timeouts
        proxy_connect_timeout 10s;
        proxy_send_timeout 300s;
        proxy_read_timeout 300s;

        # Buffering
        proxy_buffering off;  # Important for SSE streaming
        proxy_request_buffering off;

        # Health check bypass
        if ($request_uri = "/health") {
            access_log off;
        }
    }

    location /metrics {
        # Restrict metrics to internal network
        allow 10.0.0.0/8;
        deny all;

        proxy_pass http://llm_gateway;
    }
}
```

#### Health Check Integration

Create `/workspaces/llm-inference-gateway/src/health.rs`:

```rust
use axum::{
    extract::State,
    response::Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub providers: Vec<ProviderHealth>,
    pub dependencies: DependencyHealth,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub name: String,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub redis: bool,
    pub disk_space_gb: f64,
}

pub async fn health_check(
    State(registry): State<Arc<ProviderRegistry>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    // Check all registered providers
    let providers = registry.list_all().await;
    let mut provider_health = Vec::new();

    for (name, provider) in providers {
        let health = provider.health_check().await
            .unwrap_or_else(|_| HealthStatus {
                is_healthy: false,
                latency_ms: None,
                error_rate: 1.0,
                last_check: Instant::now(),
                details: HashMap::new(),
            });

        provider_health.push(ProviderHealth {
            name,
            healthy: health.is_healthy,
            latency_ms: health.latency_ms,
        });
    }

    // Check Redis connectivity
    let redis_healthy = check_redis().await;

    // Check disk space
    let disk_space = check_disk_space().await;

    let all_healthy = provider_health.iter().all(|p| p.healthy)
        && redis_healthy
        && disk_space > 1.0;

    let response = HealthResponse {
        status: if all_healthy { "healthy".to_string() } else { "degraded".to_string() },
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: get_uptime(),
        providers: provider_health,
        dependencies: DependencyHealth {
            redis: redis_healthy,
            disk_space_gb: disk_space,
        },
    };

    if all_healthy {
        Ok(Json(response))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub async fn readiness_check() -> StatusCode {
    // Lightweight check for k8s readiness probe
    StatusCode::OK
}

pub async fn liveness_check() -> StatusCode {
    // Simple heartbeat for k8s liveness probe
    StatusCode::OK
}
```

#### Session Affinity Considerations

**Not Required** - The LLM Gateway is designed to be stateless:

- All request state is contained in the request itself
- Rate limiting state is stored in Redis (shared across instances)
- Provider connection pooling is per-instance (efficient reuse)
- No server-side session storage

**Benefits:**
- Simplified horizontal scaling
- Graceful pod termination without session loss
- Optimal load distribution
- Reduced operational complexity

---

### Multi-Region (Enterprise)

**Use Case:** Global deployments requiring <200ms latency worldwide with data residency compliance

#### Global Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                     Global DNS / CDN Layer                        │
│                (Route53 / CloudFlare / Cloud DNS)                 │
│          GeoDNS Routing + DDoS Protection + WAF                   │
└───────┬─────────────────────┬─────────────────────┬──────────────┘
        │                     │                     │
┌───────▼────────┐   ┌────────▼────────┐   ┌───────▼───────┐
│   US-EAST-1    │   │   EU-WEST-1     │   │  AP-SOUTH-1   │
│   (Primary)    │   │   (Secondary)   │   │  (Secondary)  │
│                │   │                 │   │               │
│  ┌──────────┐  │   │  ┌──────────┐   │   │ ┌──────────┐  │
│  │ Gateway  │  │   │  │ Gateway  │   │   │ │ Gateway  │  │
│  │ Cluster  │  │   │  │ Cluster  │   │   │ │ Cluster  │  │
│  │ (4 pods) │  │   │  │ (4 pods) │   │   │ │ (3 pods) │  │
│  └────┬─────┘  │   │  └────┬─────┘   │   │ └────┬─────┘  │
│       │        │   │       │         │   │      │        │
│  ┌────▼─────┐  │   │  ┌────▼─────┐   │   │ ┌────▼─────┐  │
│  │  Redis   │  │   │  │  Redis   │   │   │ │  Redis   │  │
│  │ Cluster  │  │   │  │ Cluster  │   │   │ │ Cluster  │  │
│  └──────────┘  │   │  └──────────┘   │   │ └──────────┘  │
└────────────────┘   └─────────────────┘   └───────────────┘
        │                     │                     │
        └─────────────────────┴─────────────────────┘
                              │
                    ┌─────────▼──────────┐
                    │  LLM Providers     │
                    │  Regional Endpoints│
                    └────────────────────┘
```

#### Geographic Distribution Strategy

**Region Selection Criteria:**

1. **US-EAST-1 (Primary)**
   - Target: North America
   - Expected Latency: 10-50ms
   - Providers: OpenAI (primary), Anthropic, Azure OpenAI

2. **EU-WEST-1 (GDPR Compliant)**
   - Target: Europe, Middle East, Africa
   - Expected Latency: 20-80ms
   - Providers: Azure OpenAI Europe, Anthropic EU
   - Data Residency: EU-only processing

3. **AP-SOUTH-1**
   - Target: Asia-Pacific
   - Expected Latency: 30-100ms
   - Providers: Azure OpenAI Asia, AWS Bedrock Asia

#### Latency-Based Routing

**AWS Route53 Geolocation Routing Policy:**

```json
{
  "Comment": "Geolocation routing for LLM Gateway",
  "Changes": [
    {
      "Action": "CREATE",
      "ResourceRecordSet": {
        "Name": "api.llmgateway.example.com",
        "Type": "A",
        "SetIdentifier": "US-East-Primary",
        "GeoLocation": {
          "ContinentCode": "NA"
        },
        "AliasTarget": {
          "HostedZoneId": "Z123456",
          "DNSName": "us-east-lb.example.com",
          "EvaluateTargetHealth": true
        }
      }
    },
    {
      "Action": "CREATE",
      "ResourceRecordSet": {
        "Name": "api.llmgateway.example.com",
        "Type": "A",
        "SetIdentifier": "EU-West-Secondary",
        "GeoLocation": {
          "ContinentCode": "EU"
        },
        "AliasTarget": {
          "HostedZoneId": "Z789012",
          "DNSName": "eu-west-lb.example.com",
          "EvaluateTargetHealth": true
        }
      }
    },
    {
      "Action": "CREATE",
      "ResourceRecordSet": {
        "Name": "api.llmgateway.example.com",
        "Type": "A",
        "SetIdentifier": "AP-South-Secondary",
        "GeoLocation": {
          "ContinentCode": "AS"
        },
        "AliasTarget": {
          "HostedZoneId": "Z345678",
          "DNSName": "ap-south-lb.example.com",
          "EvaluateTargetHealth": true
        }
      }
    }
  ]
}
```

#### Data Residency Compliance

**EU GDPR Configuration:**

```yaml
# deployment/configs/eu-config.yaml
region: eu-west-1
data_residency:
  enabled: true
  allowed_regions:
    - eu-west-1
    - eu-central-1
  deny_regions:
    - us-east-1
    - ap-south-1

providers:
  openai:
    enabled: false  # US-based, not GDPR compliant

  anthropic:
    enabled: true
    endpoint: https://api.anthropic.com
    region_override: eu

  azure_openai:
    enabled: true
    endpoint: https://eu-openai.openai.azure.com
    deployment_region: westeurope

  aws_bedrock:
    enabled: true
    region: eu-west-1
    model_whitelist:
      - anthropic.claude-3-sonnet-eu

logging:
  pii_redaction: true
  storage_location: eu-west-1
  retention_days: 90

encryption:
  at_rest: true
  in_transit: true
  key_management: aws-kms-eu
```

#### Disaster Recovery

**Recovery Time Objective (RTO):** 15 minutes
**Recovery Point Objective (RPO):** 5 minutes

**Automated Failover Strategy:**

```yaml
# deployment/disaster-recovery/failover-policy.yaml
failover:
  enabled: true

  health_checks:
    interval: 30s
    timeout: 10s
    unhealthy_threshold: 3
    healthy_threshold: 2

  automatic_failover:
    enabled: true
    primary_region: us-east-1
    failover_regions:
      - eu-west-1
      - ap-south-1

    triggers:
      - condition: health_check_failure
        consecutive_failures: 3
        action: failover_to_secondary

      - condition: latency_threshold
        threshold_ms: 2000
        duration: 5m
        action: reroute_traffic

      - condition: error_rate_threshold
        threshold_percent: 10
        duration: 2m
        action: failover_to_secondary

  manual_override:
    enabled: true
    require_approval: true
    approvers:
      - ops-team@example.com

backup:
  redis_snapshots:
    enabled: true
    interval: 5m
    retention: 24h
    cross_region_replication: true

  configuration_backup:
    enabled: true
    interval: 1h
    storage: s3://llm-gateway-config-backup
```

---

## 2. Kubernetes Architecture

### Namespace Structure

```yaml
# deployment/k8s/namespaces.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: llm-gateway
  labels:
    name: llm-gateway
    environment: production
---
apiVersion: v1
kind: Namespace
metadata:
  name: llm-gateway-monitoring
  labels:
    name: llm-gateway-monitoring
    environment: production
```

### Deployment Manifest

```yaml
# deployment/k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-gateway
  namespace: llm-gateway
  labels:
    app: llm-gateway
    version: v1.0.0
spec:
  replicas: 4
  revisionHistoryLimit: 10
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0  # Ensure zero-downtime deployments

  selector:
    matchLabels:
      app: llm-gateway

  template:
    metadata:
      labels:
        app: llm-gateway
        version: v1.0.0
      annotations:
        prometheus.io/scrape: "true"
        prometheus.io/port: "9090"
        prometheus.io/path: "/metrics"

    spec:
      serviceAccountName: llm-gateway-sa

      # Security Context
      securityContext:
        runAsNonRoot: true
        runAsUser: 1000
        fsGroup: 1000
        seccompProfile:
          type: RuntimeDefault

      # Init Containers
      initContainers:
        - name: wait-for-redis
          image: busybox:1.35
          command: ['sh', '-c', 'until nc -z redis-service 6379; do echo waiting for redis; sleep 2; done']

      containers:
        - name: gateway
          image: ghcr.io/your-org/llm-gateway:v1.0.0
          imagePullPolicy: IfNotPresent

          ports:
            - name: http
              containerPort: 8080
              protocol: TCP
            - name: metrics
              containerPort: 9090
              protocol: TCP

          env:
            - name: RUST_LOG
              value: "info,llm_gateway=debug"
            - name: SERVER_HOST
              value: "0.0.0.0"
            - name: SERVER_PORT
              value: "8080"
            - name: METRICS_PORT
              value: "9090"
            - name: REDIS_URL
              valueFrom:
                secretKeyRef:
                  name: redis-secret
                  key: url
            - name: OPENAI_API_KEY
              valueFrom:
                secretKeyRef:
                  name: llm-provider-secrets
                  key: openai-api-key
            - name: ANTHROPIC_API_KEY
              valueFrom:
                secretKeyRef:
                  name: llm-provider-secrets
                  key: anthropic-api-key

          # Resource Management
          resources:
            requests:
              cpu: 1000m      # 1 CPU core
              memory: 2Gi     # 2GB RAM
            limits:
              cpu: 2000m      # 2 CPU cores max
              memory: 4Gi     # 4GB RAM max

          # Liveness Probe
          livenessProbe:
            httpGet:
              path: /health/live
              port: 8080
            initialDelaySeconds: 30
            periodSeconds: 10
            timeoutSeconds: 5
            failureThreshold: 3
            successThreshold: 1

          # Readiness Probe
          readinessProbe:
            httpGet:
              path: /health/ready
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 5
            timeoutSeconds: 3
            failureThreshold: 3
            successThreshold: 1

          # Startup Probe
          startupProbe:
            httpGet:
              path: /health/live
              port: 8080
            initialDelaySeconds: 0
            periodSeconds: 5
            timeoutSeconds: 3
            failureThreshold: 30  # 150s max startup time

          # Security Context
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            runAsNonRoot: true
            runAsUser: 1000
            capabilities:
              drop:
                - ALL

          # Volume Mounts
          volumeMounts:
            - name: tmp
              mountPath: /tmp
            - name: cache
              mountPath: /app/cache

      # Volumes
      volumes:
        - name: tmp
          emptyDir: {}
        - name: cache
          emptyDir:
            sizeLimit: 1Gi

      # Pod Affinity
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
            - weight: 100
              podAffinityTerm:
                labelSelector:
                  matchExpressions:
                    - key: app
                      operator: In
                      values:
                        - llm-gateway
                topologyKey: kubernetes.io/hostname

      # Topology Spread Constraints
      topologySpreadConstraints:
        - maxSkew: 1
          topologyKey: topology.kubernetes.io/zone
          whenUnsatisfiable: DoNotSchedule
          labelSelector:
            matchLabels:
              app: llm-gateway
```

### Service Configuration

```yaml
# deployment/k8s/service.yaml
apiVersion: v1
kind: Service
metadata:
  name: llm-gateway-service
  namespace: llm-gateway
  labels:
    app: llm-gateway
  annotations:
    service.beta.kubernetes.io/aws-load-balancer-type: "nlb"
    service.beta.kubernetes.io/aws-load-balancer-cross-zone-load-balancing-enabled: "true"
spec:
  type: LoadBalancer
  sessionAffinity: None  # Stateless service
  ports:
    - name: http
      port: 80
      targetPort: 8080
      protocol: TCP
    - name: https
      port: 443
      targetPort: 8080
      protocol: TCP
  selector:
    app: llm-gateway
---
apiVersion: v1
kind: Service
metadata:
  name: llm-gateway-metrics
  namespace: llm-gateway
  labels:
    app: llm-gateway
spec:
  type: ClusterIP
  ports:
    - name: metrics
      port: 9090
      targetPort: 9090
      protocol: TCP
  selector:
    app: llm-gateway
```

### Ingress Configuration

```yaml
# deployment/k8s/ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: llm-gateway-ingress
  namespace: llm-gateway
  annotations:
    kubernetes.io/ingress.class: "nginx"
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
    nginx.ingress.kubernetes.io/proxy-body-size: "10m"
    nginx.ingress.kubernetes.io/proxy-connect-timeout: "300"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "300"
    nginx.ingress.kubernetes.io/proxy-read-timeout: "300"
    nginx.ingress.kubernetes.io/proxy-buffering: "off"  # For SSE streaming
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/limit-rps: "100"
spec:
  tls:
    - hosts:
        - api.llmgateway.example.com
      secretName: llm-gateway-tls
  rules:
    - host: api.llmgateway.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: llm-gateway-service
                port:
                  number: 80
```

### ConfigMap Management

```yaml
# deployment/k8s/configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: llm-gateway-config
  namespace: llm-gateway
data:
  config.yaml: |
    server:
      host: "0.0.0.0"
      port: 8080
      max_connections: 1000
      timeout_seconds: 300

    providers:
      openai:
        enabled: true
        base_url: "https://api.openai.com"
        timeout_seconds: 60
        max_retries: 3
        rate_limit:
          requests_per_minute: 500
          tokens_per_minute: 150000

      anthropic:
        enabled: true
        base_url: "https://api.anthropic.com"
        api_version: "2023-06-01"
        timeout_seconds: 300
        max_retries: 3
        rate_limit:
          requests_per_minute: 1000
          tokens_per_minute: 400000

    connection_pool:
      max_idle_per_host: 32
      idle_timeout_seconds: 90
      connect_timeout_seconds: 10
      max_connections_per_provider: 100

    cache:
      enabled: true
      ttl_seconds: 3600
      max_size_mb: 1024

    logging:
      level: "info"
      format: "json"
      output: "stdout"
```

### Secret Management

```yaml
# deployment/k8s/secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: llm-provider-secrets
  namespace: llm-gateway
type: Opaque
stringData:
  openai-api-key: "sk-..."
  anthropic-api-key: "sk-ant-..."
  azure-openai-api-key: "..."
  google-api-key: "..."
---
apiVersion: v1
kind: Secret
metadata:
  name: redis-secret
  namespace: llm-gateway
type: Opaque
stringData:
  url: "redis://redis-master:6379"
  password: "your-secure-password"
```

**Using External Secret Manager (AWS Secrets Manager):**

```yaml
# deployment/k8s/external-secret.yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: llm-provider-secrets
  namespace: llm-gateway
spec:
  refreshInterval: 1h
  secretStoreRef:
    name: aws-secrets-manager
    kind: SecretStore
  target:
    name: llm-provider-secrets
    creationPolicy: Owner
  data:
    - secretKey: openai-api-key
      remoteRef:
        key: prod/llm-gateway/openai
        property: api_key
    - secretKey: anthropic-api-key
      remoteRef:
        key: prod/llm-gateway/anthropic
        property: api_key
```

### Horizontal Pod Autoscaler (HPA)

```yaml
# deployment/k8s/hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: llm-gateway-hpa
  namespace: llm-gateway
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: llm-gateway

  minReplicas: 4
  maxReplicas: 20

  metrics:
    # CPU-based scaling
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70

    # Memory-based scaling
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80

    # Custom metric: Request per second
    - type: Pods
      pods:
        metric:
          name: http_requests_per_second
        target:
          type: AverageValue
          averageValue: "1000"

  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300  # 5 minutes
      policies:
        - type: Percent
          value: 50
          periodSeconds: 60
        - type: Pods
          value: 2
          periodSeconds: 60
      selectPolicy: Min

    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
        - type: Percent
          value: 100
          periodSeconds: 30
        - type: Pods
          value: 4
          periodSeconds: 30
      selectPolicy: Max
```

### Pod Disruption Budget (PDB)

```yaml
# deployment/k8s/pdb.yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: llm-gateway-pdb
  namespace: llm-gateway
spec:
  minAvailable: 3  # Always keep at least 3 pods running
  selector:
    matchLabels:
      app: llm-gateway
```

### Network Policies

```yaml
# deployment/k8s/network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: llm-gateway-netpol
  namespace: llm-gateway
spec:
  podSelector:
    matchLabels:
      app: llm-gateway

  policyTypes:
    - Ingress
    - Egress

  ingress:
    # Allow traffic from ingress controller
    - from:
        - namespaceSelector:
            matchLabels:
              name: ingress-nginx
      ports:
        - protocol: TCP
          port: 8080

    # Allow metrics scraping from Prometheus
    - from:
        - namespaceSelector:
            matchLabels:
              name: llm-gateway-monitoring
      ports:
        - protocol: TCP
          port: 9090

  egress:
    # Allow DNS
    - to:
        - namespaceSelector:
            matchLabels:
              name: kube-system
      ports:
        - protocol: UDP
          port: 53

    # Allow Redis
    - to:
        - podSelector:
            matchLabels:
              app: redis
      ports:
        - protocol: TCP
          port: 6379

    # Allow HTTPS to LLM providers
    - to:
        - namespaceSelector: {}
      ports:
        - protocol: TCP
          port: 443
```

---

## 3. Infrastructure Components

### Load Balancers

#### L4 vs L7 Comparison

| Feature | Layer 4 (TCP/UDP) | Layer 7 (HTTP/HTTPS) |
|---------|-------------------|----------------------|
| **Performance** | Higher throughput (2-3M RPS) | Moderate (500K-1M RPS) |
| **Latency** | <1ms | 5-10ms |
| **Protocol Support** | TCP, UDP | HTTP, HTTPS, WebSocket, gRPC |
| **SSL Termination** | No (passthrough) | Yes |
| **Content Routing** | IP/Port only | Path, Header, Cookie |
| **Health Checks** | TCP connect | HTTP endpoint |
| **Cost** | Lower | Higher |
| **Use Case** | Maximum performance | Advanced routing, SSL offload |

**Recommendation for LLM Gateway:** Use L7 (HTTP/HTTPS) for:
- SSL/TLS termination
- Path-based routing (e.g., `/v1/chat/completions`)
- WebSocket/SSE support for streaming
- Header-based provider selection
- WAF integration

### TLS Termination Options

#### Option 1: Load Balancer Termination (Recommended)

```
Client → [HTTPS] → Load Balancer → [HTTP] → Gateway Pods
```

**Pros:**
- Simplified certificate management
- Reduced pod CPU overhead
- Centralized SSL/TLS policy
- Better performance (hardware acceleration)

**Cons:**
- Internal traffic not encrypted
- Requires secure VPC/private network

**Configuration:**

```yaml
# AWS ACM Certificate
apiVersion: v1
kind: Service
metadata:
  name: llm-gateway-lb
  annotations:
    service.beta.kubernetes.io/aws-load-balancer-ssl-cert: arn:aws:acm:us-east-1:123456789:certificate/xyz
    service.beta.kubernetes.io/aws-load-balancer-ssl-ports: "443"
    service.beta.kubernetes.io/aws-load-balancer-backend-protocol: "http"
```

#### Option 2: End-to-End Encryption

```
Client → [HTTPS] → Load Balancer → [HTTPS] → Gateway Pods
```

**Pros:**
- Complete encryption in transit
- Compliance requirements (PCI-DSS, HIPAA)

**Cons:**
- Additional CPU overhead in pods
- Certificate management complexity

### DNS Configuration

```yaml
# Terraform DNS Configuration
resource "aws_route53_record" "gateway" {
  zone_id = aws_route53_zone.main.zone_id
  name    = "api.llmgateway.example.com"
  type    = "A"

  alias {
    name                   = aws_lb.gateway.dns_name
    zone_id                = aws_lb.gateway.zone_id
    evaluate_target_health = true
  }
}

# Health check record
resource "aws_route53_health_check" "gateway" {
  fqdn              = "api.llmgateway.example.com"
  port              = 443
  type              = "HTTPS"
  resource_path     = "/health"
  failure_threshold = 3
  request_interval  = 30

  tags = {
    Name = "LLM Gateway Health Check"
  }
}
```

### CDN Integration (Optional)

**Use Case:** Caching provider model lists, static assets, common responses

```yaml
# CloudFront Distribution
resource "aws_cloudfront_distribution" "gateway" {
  enabled = true

  origin {
    domain_name = "api.llmgateway.example.com"
    origin_id   = "llm-gateway-origin"

    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }

  default_cache_behavior {
    target_origin_id       = "llm-gateway-origin"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"]
    cached_methods         = ["GET", "HEAD", "OPTIONS"]

    forwarded_values {
      query_string = true
      headers      = ["Authorization", "Content-Type"]
      cookies {
        forward = "none"
      }
    }

    min_ttl     = 0
    default_ttl = 0
    max_ttl     = 0  # Don't cache LLM responses
  }

  # Cache static endpoints
  ordered_cache_behavior {
    path_pattern           = "/v1/models"
    target_origin_id       = "llm-gateway-origin"
    viewer_protocol_policy = "redirect-to-https"

    forwarded_values {
      query_string = false
      cookies {
        forward = "none"
      }
    }

    min_ttl     = 300
    default_ttl = 3600
    max_ttl     = 86400
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn = aws_acm_certificate.gateway.arn
    ssl_support_method  = "sni-only"
  }
}
```

### Redis Cluster Deployment

```yaml
# deployment/k8s/redis-cluster.yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis
  namespace: llm-gateway
spec:
  serviceName: redis
  replicas: 3
  selector:
    matchLabels:
      app: redis
  template:
    metadata:
      labels:
        app: redis
    spec:
      containers:
        - name: redis
          image: redis:7-alpine
          ports:
            - containerPort: 6379
              name: redis
          command:
            - redis-server
            - "--appendonly"
            - "yes"
            - "--requirepass"
            - "$(REDIS_PASSWORD)"
          env:
            - name: REDIS_PASSWORD
              valueFrom:
                secretKeyRef:
                  name: redis-secret
                  key: password
          volumeMounts:
            - name: redis-data
              mountPath: /data
          resources:
            requests:
              cpu: 500m
              memory: 1Gi
            limits:
              cpu: 1000m
              memory: 2Gi
  volumeClaimTemplates:
    - metadata:
        name: redis-data
      spec:
        accessModes: ["ReadWriteOnce"]
        resources:
          requests:
            storage: 20Gi
        storageClassName: fast-ssd
---
apiVersion: v1
kind: Service
metadata:
  name: redis-service
  namespace: llm-gateway
spec:
  type: ClusterIP
  ports:
    - port: 6379
      targetPort: 6379
  selector:
    app: redis
```

---

## 4. Container Architecture

### Multi-Stage Dockerfile

```dockerfile
# deployment/docker/Dockerfile
# ============================================================================
# Stage 1: Build Environment
# ============================================================================
FROM rust:1.75-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    g++ \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 -s /bin/bash appuser

# Set working directory
WORKDIR /app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Cache dependencies (layer optimization)
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build application
RUN cargo build --release --locked && \
    strip target/release/llm-gateway

# ============================================================================
# Stage 2: Runtime Environment
# ============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 -s /bin/bash appuser && \
    mkdir -p /app/cache /app/logs && \
    chown -R appuser:appuser /app

# Copy binary from builder
COPY --from=builder --chown=appuser:appuser /app/target/release/llm-gateway /usr/local/bin/llm-gateway

# Set working directory
WORKDIR /app

# Switch to non-root user
USER appuser

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run application
ENTRYPOINT ["/usr/local/bin/llm-gateway"]
```

### Security Hardening

```dockerfile
# deployment/docker/Dockerfile.hardened
# ============================================================================
# Hardened Production Dockerfile
# ============================================================================

# Use distroless base image (minimal attack surface)
FROM rust:1.75-slim-bookworm AS builder

# [Builder stage same as above...]

# ============================================================================
# Distroless Runtime (No shell, minimal packages)
# ============================================================================
FROM gcr.io/distroless/cc-debian12:nonroot

# Copy CA certificates
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy binary
COPY --from=builder /app/target/release/llm-gateway /llm-gateway

# Create cache directory
COPY --chown=nonroot:nonroot --from=builder /app/cache /app/cache

# Expose ports
EXPOSE 8080 9090

# Non-root user (built into distroless)
USER nonroot:nonroot

# Run application
ENTRYPOINT ["/llm-gateway"]
```

### Image Versioning Strategy

```bash
# deployment/scripts/build-and-tag.sh
#!/bin/bash

# Semantic versioning
VERSION="1.0.0"
GIT_COMMIT=$(git rev-parse --short HEAD)
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Build image
docker build \
  --build-arg VERSION=${VERSION} \
  --build-arg GIT_COMMIT=${GIT_COMMIT} \
  --build-arg BUILD_DATE=${BUILD_DATE} \
  -t ghcr.io/your-org/llm-gateway:${VERSION} \
  -t ghcr.io/your-org/llm-gateway:${VERSION}-${GIT_COMMIT} \
  -t ghcr.io/your-org/llm-gateway:latest \
  -f deployment/docker/Dockerfile .

# Tag conventions:
# - v1.0.0 (semantic version)
# - v1.0.0-abc1234 (version + git commit)
# - latest (most recent build)
# - stable (last production release)
# - dev (development branch)
```

### Image Scanning

```yaml
# .github/workflows/security-scan.yaml
name: Container Security Scan

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Build image
        run: docker build -t llm-gateway:test .

      - name: Run Trivy vulnerability scanner
        uses: aquasecurity/trivy-action@master
        with:
          image-ref: 'llm-gateway:test'
          format: 'sarif'
          output: 'trivy-results.sarif'
          severity: 'CRITICAL,HIGH'

      - name: Upload Trivy results to GitHub Security
        uses: github/codeql-action/upload-sarif@v2
        with:
          sarif_file: 'trivy-results.sarif'

      - name: Run Snyk scan
        uses: snyk/actions/docker@master
        env:
          SNYK_TOKEN: ${{ secrets.SNYK_TOKEN }}
        with:
          image: llm-gateway:test
          args: --severity-threshold=high
```

---

## 5. CI/CD Pipeline

### Build Stages

```yaml
# .github/workflows/ci-cd.yaml
name: CI/CD Pipeline

on:
  push:
    branches: [main, develop]
    tags: ['v*']
  pull_request:
    branches: [main]

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  # ============================================================================
  # Stage 1: Code Quality
  # ============================================================================
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy
          override: true

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Run rustfmt
        run: cargo fmt --all -- --check

      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

  # ============================================================================
  # Stage 2: Testing
  # ============================================================================
  test:
    runs-on: ubuntu-latest
    services:
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust toolchain
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Run unit tests
        run: cargo test --lib
        env:
          REDIS_URL: redis://localhost:6379

      - name: Run integration tests
        run: cargo test --test '*'
        env:
          REDIS_URL: redis://localhost:6379

      - name: Generate coverage report
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --output-dir ./coverage

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./coverage/cobertura.xml

  # ============================================================================
  # Stage 3: Security Scanning
  # ============================================================================
  security:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run cargo audit
        run: |
          cargo install cargo-audit
          cargo audit

      - name: Run SAST with Semgrep
        uses: returntocorp/semgrep-action@v1
        with:
          config: >-
            p/security-audit
            p/rust

  # ============================================================================
  # Stage 4: Build and Push
  # ============================================================================
  build:
    needs: [lint, test, security]
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write

    steps:
      - uses: actions/checkout@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v2

      - name: Log in to Container Registry
        uses: docker/login-action@v2
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v4
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=ref,event=branch
            type=ref,event=pr
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=sha,prefix={{branch}}-

      - name: Build and push
        uses: docker/build-push-action@v4
        with:
          context: .
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
          build-args: |
            VERSION=${{ github.ref_name }}
            GIT_COMMIT=${{ github.sha }}
            BUILD_DATE=${{ steps.date.outputs.date }}

  # ============================================================================
  # Stage 5: Deploy to Staging
  # ============================================================================
  deploy-staging:
    needs: build
    if: github.ref == 'refs/heads/develop'
    runs-on: ubuntu-latest
    environment:
      name: staging
      url: https://staging-api.llmgateway.example.com

    steps:
      - uses: actions/checkout@v3

      - name: Configure kubectl
        uses: azure/k8s-set-context@v3
        with:
          method: kubeconfig
          kubeconfig: ${{ secrets.KUBE_CONFIG_STAGING }}

      - name: Deploy to staging
        run: |
          kubectl set image deployment/llm-gateway \
            gateway=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:develop-${{ github.sha }} \
            -n llm-gateway-staging

          kubectl rollout status deployment/llm-gateway \
            -n llm-gateway-staging \
            --timeout=5m

      - name: Run smoke tests
        run: |
          bash deployment/scripts/smoke-tests.sh staging

  # ============================================================================
  # Stage 6: Deploy to Production
  # ============================================================================
  deploy-production:
    needs: build
    if: startsWith(github.ref, 'refs/tags/v')
    runs-on: ubuntu-latest
    environment:
      name: production
      url: https://api.llmgateway.example.com

    steps:
      - uses: actions/checkout@v3

      - name: Configure kubectl
        uses: azure/k8s-set-context@v3
        with:
          method: kubeconfig
          kubeconfig: ${{ secrets.KUBE_CONFIG_PROD }}

      - name: Deploy to production
        run: |
          kubectl set image deployment/llm-gateway \
            gateway=${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ github.ref_name }} \
            -n llm-gateway

          kubectl rollout status deployment/llm-gateway \
            -n llm-gateway \
            --timeout=10m

      - name: Run health checks
        run: |
          bash deployment/scripts/health-check.sh production

      - name: Notify Slack
        uses: 8398a7/action-slack@v3
        with:
          status: ${{ job.status }}
          text: 'LLM Gateway deployed to production: ${{ github.ref_name }}'
          webhook_url: ${{ secrets.SLACK_WEBHOOK }}
```

### Deployment Strategies

#### Blue/Green Deployment

```yaml
# deployment/k8s/blue-green/deployment-blue.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: llm-gateway-blue
  namespace: llm-gateway
spec:
  replicas: 4
  selector:
    matchLabels:
      app: llm-gateway
      version: blue
  template:
    metadata:
      labels:
        app: llm-gateway
        version: blue
    spec:
      containers:
        - name: gateway
          image: ghcr.io/your-org/llm-gateway:v1.0.0
          # [rest of config...]
---
apiVersion: v1
kind: Service
metadata:
  name: llm-gateway-service
  namespace: llm-gateway
spec:
  selector:
    app: llm-gateway
    version: blue  # Traffic goes to blue
  ports:
    - port: 80
      targetPort: 8080
```

```bash
# deployment/scripts/blue-green-switch.sh
#!/bin/bash

CURRENT_VERSION=$(kubectl get service llm-gateway-service -n llm-gateway -o jsonpath='{.spec.selector.version}')
NEW_VERSION=$([ "$CURRENT_VERSION" = "blue" ] && echo "green" || echo "blue")

echo "Current version: $CURRENT_VERSION"
echo "Switching to: $NEW_VERSION"

# Deploy new version
kubectl apply -f deployment/k8s/blue-green/deployment-${NEW_VERSION}.yaml

# Wait for rollout
kubectl rollout status deployment/llm-gateway-${NEW_VERSION} -n llm-gateway

# Run smoke tests
bash deployment/scripts/smoke-tests.sh ${NEW_VERSION}

# Switch traffic
kubectl patch service llm-gateway-service -n llm-gateway \
  -p "{\"spec\":{\"selector\":{\"version\":\"${NEW_VERSION}\"}}}"

echo "Traffic switched to $NEW_VERSION"

# Keep old version for 30 minutes for quick rollback
sleep 1800

# Scale down old version
kubectl scale deployment llm-gateway-${CURRENT_VERSION} --replicas=0 -n llm-gateway
```

#### Canary Deployment

```yaml
# deployment/k8s/canary/canary-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: llm-gateway-stable
  namespace: llm-gateway
spec:
  selector:
    app: llm-gateway
    version: stable
  ports:
    - port: 80
      targetPort: 8080
---
apiVersion: v1
kind: Service
metadata:
  name: llm-gateway-canary
  namespace: llm-gateway
spec:
  selector:
    app: llm-gateway
    version: canary
  ports:
    - port: 80
      targetPort: 8080
---
apiVersion: networking.istio.io/v1beta1
kind: VirtualService
metadata:
  name: llm-gateway-vs
  namespace: llm-gateway
spec:
  hosts:
    - api.llmgateway.example.com
  http:
    - match:
        - headers:
            x-canary:
              exact: "true"
      route:
        - destination:
            host: llm-gateway-canary
            port:
              number: 80
    - route:
        - destination:
            host: llm-gateway-stable
            port:
              number: 80
          weight: 90
        - destination:
            host: llm-gateway-canary
            port:
              number: 80
          weight: 10  # 10% canary traffic
```

### Rollback Procedures

```bash
# deployment/scripts/rollback.sh
#!/bin/bash

NAMESPACE="llm-gateway"
DEPLOYMENT="llm-gateway"

echo "Rolling back deployment in namespace: $NAMESPACE"

# Get rollout history
kubectl rollout history deployment/$DEPLOYMENT -n $NAMESPACE

# Rollback to previous version
kubectl rollout undo deployment/$DEPLOYMENT -n $NAMESPACE

# Wait for rollback to complete
kubectl rollout status deployment/$DEPLOYMENT -n $NAMESPACE --timeout=5m

# Verify health
HEALTHY=$(kubectl get deployment $DEPLOYMENT -n $NAMESPACE -o jsonpath='{.status.availableReplicas}')
DESIRED=$(kubectl get deployment $DEPLOYMENT -n $NAMESPACE -o jsonpath='{.spec.replicas}')

if [ "$HEALTHY" -eq "$DESIRED" ]; then
    echo "Rollback successful. $HEALTHY/$DESIRED pods are healthy."
    exit 0
else
    echo "Rollback failed. Only $HEALTHY/$DESIRED pods are healthy."
    exit 1
fi
```

---

## 6. Monitoring Infrastructure

### Prometheus Deployment

```yaml
# deployment/monitoring/prometheus.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-config
  namespace: llm-gateway-monitoring
data:
  prometheus.yml: |
    global:
      scrape_interval: 15s
      evaluation_interval: 15s
      external_labels:
        cluster: 'llm-gateway-prod'
        environment: 'production'

    # Alerting configuration
    alerting:
      alertmanagers:
        - static_configs:
            - targets: ['alertmanager:9093']

    # Rules
    rule_files:
      - '/etc/prometheus/rules/*.yml'

    # Scrape configurations
    scrape_configs:
      # LLM Gateway pods
      - job_name: 'llm-gateway'
        kubernetes_sd_configs:
          - role: pod
            namespaces:
              names:
                - llm-gateway
        relabel_configs:
          - source_labels: [__meta_kubernetes_pod_label_app]
            action: keep
            regex: llm-gateway
          - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
            action: keep
            regex: true
          - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_port]
            action: replace
            target_label: __address__
            regex: ([^:]+)(?::\d+)?;(\d+)
            replacement: $1:$2
          - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
            action: replace
            target_label: __metrics_path__
            regex: (.+)

      # Redis
      - job_name: 'redis'
        static_configs:
          - targets: ['redis-service:6379']

      # Node Exporter
      - job_name: 'node-exporter'
        kubernetes_sd_configs:
          - role: node
        relabel_configs:
          - source_labels: [__address__]
            regex: '(.*):10250'
            replacement: '${1}:9100'
            target_label: __address__
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: prometheus
  namespace: llm-gateway-monitoring
spec:
  replicas: 1
  selector:
    matchLabels:
      app: prometheus
  template:
    metadata:
      labels:
        app: prometheus
    spec:
      serviceAccountName: prometheus
      containers:
        - name: prometheus
          image: prom/prometheus:latest
          args:
            - '--config.file=/etc/prometheus/prometheus.yml'
            - '--storage.tsdb.path=/prometheus'
            - '--storage.tsdb.retention.time=30d'
            - '--web.enable-lifecycle'
          ports:
            - containerPort: 9090
          volumeMounts:
            - name: config
              mountPath: /etc/prometheus
            - name: storage
              mountPath: /prometheus
          resources:
            requests:
              cpu: 500m
              memory: 2Gi
            limits:
              cpu: 2000m
              memory: 4Gi
      volumes:
        - name: config
          configMap:
            name: prometheus-config
        - name: storage
          persistentVolumeClaim:
            claimName: prometheus-pvc
```

### Alert Manager Configuration

```yaml
# deployment/monitoring/alertmanager.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: alertmanager-config
  namespace: llm-gateway-monitoring
data:
  alertmanager.yml: |
    global:
      resolve_timeout: 5m
      slack_api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'

    route:
      group_by: ['alertname', 'cluster', 'service']
      group_wait: 10s
      group_interval: 10s
      repeat_interval: 12h
      receiver: 'default'

      routes:
        - match:
            severity: critical
          receiver: 'pagerduty'
          continue: true

        - match:
            severity: warning
          receiver: 'slack'

    receivers:
      - name: 'default'
        slack_configs:
          - channel: '#llm-gateway-alerts'
            title: 'LLM Gateway Alert'
            text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

      - name: 'slack'
        slack_configs:
          - channel: '#llm-gateway-alerts'
            title: '{{ .GroupLabels.alertname }}'
            text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

      - name: 'pagerduty'
        pagerduty_configs:
          - service_key: 'YOUR_PAGERDUTY_KEY'
            description: '{{ .GroupLabels.alertname }}'
```

### Prometheus Alerts

```yaml
# deployment/monitoring/alerts.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: prometheus-rules
  namespace: llm-gateway-monitoring
data:
  gateway-alerts.yml: |
    groups:
      - name: llm-gateway
        interval: 30s
        rules:
          # High Error Rate
          - alert: HighErrorRate
            expr: |
              (
                sum(rate(http_requests_total{job="llm-gateway",status=~"5.."}[5m]))
                /
                sum(rate(http_requests_total{job="llm-gateway"}[5m]))
              ) > 0.05
            for: 5m
            labels:
              severity: critical
            annotations:
              summary: "High error rate detected"
              description: "Error rate is {{ $value | humanizePercentage }} for {{ $labels.instance }}"

          # High Latency
          - alert: HighLatency
            expr: |
              histogram_quantile(0.99,
                sum(rate(http_request_duration_seconds_bucket{job="llm-gateway"}[5m])) by (le)
              ) > 10
            for: 10m
            labels:
              severity: warning
            annotations:
              summary: "High request latency"
              description: "P99 latency is {{ $value }}s"

          # Pod Down
          - alert: PodDown
            expr: |
              up{job="llm-gateway"} == 0
            for: 2m
            labels:
              severity: critical
            annotations:
              summary: "Pod is down"
              description: "Pod {{ $labels.instance }} is down"

          # High Memory Usage
          - alert: HighMemoryUsage
            expr: |
              (
                container_memory_working_set_bytes{pod=~"llm-gateway-.*"}
                /
                container_spec_memory_limit_bytes{pod=~"llm-gateway-.*"}
              ) > 0.9
            for: 5m
            labels:
              severity: warning
            annotations:
              summary: "High memory usage"
              description: "Memory usage is {{ $value | humanizePercentage }} for {{ $labels.pod }}"

          # Provider Health
          - alert: ProviderUnhealthy
            expr: |
              llm_provider_health_status{job="llm-gateway"} == 0
            for: 3m
            labels:
              severity: warning
            annotations:
              summary: "LLM Provider unhealthy"
              description: "Provider {{ $labels.provider }} is unhealthy"

          # Rate Limit Exceeded
          - alert: RateLimitExceeded
            expr: |
              rate(llm_provider_rate_limit_exceeded_total{job="llm-gateway"}[5m]) > 10
            for: 5m
            labels:
              severity: warning
            annotations:
              summary: "Rate limits frequently exceeded"
              description: "Provider {{ $labels.provider }} rate limits exceeded {{ $value }} times/sec"
```

### Grafana Dashboards

```json
{
  "dashboard": {
    "title": "LLM Gateway - Overview",
    "panels": [
      {
        "title": "Request Rate",
        "targets": [
          {
            "expr": "sum(rate(http_requests_total{job=\"llm-gateway\"}[5m])) by (status)",
            "legendFormat": "{{status}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Request Latency (P50, P95, P99)",
        "targets": [
          {
            "expr": "histogram_quantile(0.50, sum(rate(http_request_duration_seconds_bucket[5m])) by (le))",
            "legendFormat": "P50"
          },
          {
            "expr": "histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket[5m])) by (le))",
            "legendFormat": "P95"
          },
          {
            "expr": "histogram_quantile(0.99, sum(rate(http_request_duration_seconds_bucket[5m])) by (le))",
            "legendFormat": "P99"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Provider Health",
        "targets": [
          {
            "expr": "llm_provider_health_status",
            "legendFormat": "{{provider}}"
          }
        ],
        "type": "stat"
      },
      {
        "title": "Error Rate by Provider",
        "targets": [
          {
            "expr": "sum(rate(llm_provider_errors_total[5m])) by (provider, error_type)",
            "legendFormat": "{{provider}} - {{error_type}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "CPU Usage",
        "targets": [
          {
            "expr": "sum(rate(container_cpu_usage_seconds_total{pod=~\"llm-gateway-.*\"}[5m])) by (pod)",
            "legendFormat": "{{pod}}"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Memory Usage",
        "targets": [
          {
            "expr": "sum(container_memory_working_set_bytes{pod=~\"llm-gateway-.*\"}) by (pod) / 1024 / 1024 / 1024",
            "legendFormat": "{{pod}}"
          }
        ],
        "type": "graph"
      }
    ]
  }
}
```

### Log Aggregation (Loki)

```yaml
# deployment/monitoring/loki.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: loki-config
  namespace: llm-gateway-monitoring
data:
  loki.yaml: |
    auth_enabled: false

    server:
      http_listen_port: 3100

    ingester:
      lifecycler:
        ring:
          kvstore:
            store: inmemory
          replication_factor: 1
      chunk_idle_period: 5m
      chunk_retain_period: 30s

    schema_config:
      configs:
        - from: 2023-01-01
          store: boltdb-shipper
          object_store: s3
          schema: v11
          index:
            prefix: loki_index_
            period: 24h

    storage_config:
      boltdb_shipper:
        active_index_directory: /loki/index
        cache_location: /loki/cache
        shared_store: s3
      aws:
        s3: s3://us-east-1/llm-gateway-logs
        s3forcepathstyle: true

    limits_config:
      enforce_metric_name: false
      reject_old_samples: true
      reject_old_samples_max_age: 168h
      ingestion_rate_mb: 10
      ingestion_burst_size_mb: 20
```

### Distributed Tracing (Jaeger)

```yaml
# deployment/monitoring/jaeger.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: jaeger
  namespace: llm-gateway-monitoring
spec:
  replicas: 1
  selector:
    matchLabels:
      app: jaeger
  template:
    metadata:
      labels:
        app: jaeger
    spec:
      containers:
        - name: jaeger
          image: jaegertracing/all-in-one:latest
          ports:
            - containerPort: 5775
              protocol: UDP
            - containerPort: 6831
              protocol: UDP
            - containerPort: 6832
              protocol: UDP
            - containerPort: 5778
              protocol: TCP
            - containerPort: 16686
              protocol: TCP
            - containerPort: 14268
              protocol: TCP
          env:
            - name: COLLECTOR_ZIPKIN_HTTP_PORT
              value: "9411"
            - name: SPAN_STORAGE_TYPE
              value: "elasticsearch"
            - name: ES_SERVER_URLS
              value: "http://elasticsearch:9200"
          resources:
            requests:
              cpu: 500m
              memory: 1Gi
            limits:
              cpu: 1000m
              memory: 2Gi
```

---

## 7. Resource Sizing Guide

### Deployment Size Recommendations

| Deployment Size | Use Case | CPU (per pod) | Memory (per pod) | Pod Count | Total CPU | Total Memory | RPS Capacity | Monthly Cost (AWS EKS) |
|----------------|----------|---------------|------------------|-----------|-----------|--------------|--------------|----------------------|
| **Extra Small** | Dev/Testing | 0.5 cores | 1GB | 2 | 1 core | 2GB | 500 | $50-100 |
| **Small** | Startup/MVP | 1 core | 2GB | 3 | 3 cores | 6GB | 1,000 | $150-250 |
| **Medium** | Small Business | 2 cores | 4GB | 4 | 8 cores | 16GB | 5,000 | $400-600 |
| **Large** | Enterprise | 4 cores | 8GB | 6 | 24 cores | 48GB | 10,000 | $1,200-1,800 |
| **Extra Large** | High-Scale | 8 cores | 16GB | 10 | 80 cores | 160GB | 25,000+ | $3,500-5,000 |

### Sizing Calculator

```bash
# deployment/scripts/size-calculator.sh
#!/bin/bash

# Calculate required resources based on expected traffic

expected_rps=$1
avg_request_duration_ms=$2
memory_per_request_mb=$3

# Concurrency = RPS × Request Duration
concurrent_requests=$(echo "$expected_rps * $avg_request_duration_ms / 1000" | bc -l)

# CPU: 100 concurrent requests per core (rule of thumb)
required_cores=$(echo "$concurrent_requests / 100" | bc -l)
required_cores=$(printf "%.0f" "$required_cores")

# Memory: Concurrent requests × Memory per request + Base overhead (512MB)
required_memory_mb=$(echo "$concurrent_requests * $memory_per_request_mb + 512" | bc -l)
required_memory_gb=$(echo "$required_memory_mb / 1024" | bc -l)
required_memory_gb=$(printf "%.1f" "$required_memory_gb")

# Pods: At least 3 for HA, scale up if needed
min_pods=3
pods_needed=$(echo "if ($required_cores / 2 > $min_pods) $required_cores / 2 else $min_pods" | bc)
pods_needed=$(printf "%.0f" "$pods_needed")

cpu_per_pod=$(echo "$required_cores / $pods_needed" | bc -l)
cpu_per_pod=$(printf "%.1f" "$cpu_per_pod")

memory_per_pod_gb=$(echo "$required_memory_gb / $pods_needed" | bc -l)
memory_per_pod_gb=$(printf "%.1f" "$memory_per_pod_gb")

echo "==================================="
echo "LLM Gateway Resource Calculator"
echo "==================================="
echo "Expected RPS: $expected_rps"
echo "Avg Request Duration: ${avg_request_duration_ms}ms"
echo "Memory per Request: ${memory_per_request_mb}MB"
echo ""
echo "Calculated Requirements:"
echo "  Concurrent Requests: $(printf "%.0f" "$concurrent_requests")"
echo "  Total CPU Cores: $required_cores"
echo "  Total Memory: ${required_memory_gb}GB"
echo ""
echo "Recommended Deployment:"
echo "  Number of Pods: $pods_needed"
echo "  CPU per Pod: ${cpu_per_pod} cores"
echo "  Memory per Pod: ${memory_per_pod_gb}GB"
echo "==================================="
```

**Example Usage:**

```bash
# 5000 RPS, 200ms avg latency, 10MB per request
./deployment/scripts/size-calculator.sh 5000 200 10

# Output:
# Expected RPS: 5000
# Avg Request Duration: 200ms
# Concurrent Requests: 1000
# Total CPU Cores: 10
# Total Memory: 10.5GB
#
# Recommended Deployment:
#   Number of Pods: 5
#   CPU per Pod: 2.0 cores
#   Memory per Pod: 2.1GB
```

### Load Testing

```yaml
# deployment/loadtest/k6-test.js
import http from 'k6/http';
import { check, sleep } from 'k6';

export let options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 200 },   // Ramp up to 200 users
    { duration: '5m', target: 200 },   // Stay at 200 users
    { duration: '2m', target: 0 },     // Ramp down to 0 users
  ],
  thresholds: {
    http_req_duration: ['p(95)<2000'], // 95% of requests should be below 2s
    http_req_failed: ['rate<0.01'],    // Error rate should be below 1%
  },
};

export default function () {
  const url = 'https://api.llmgateway.example.com/v1/chat/completions';
  const payload = JSON.stringify({
    model: 'gpt-4',
    messages: [
      { role: 'user', content: 'Hello, how are you?' }
    ],
    temperature: 0.7,
    max_tokens: 100,
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
      'Authorization': 'Bearer YOUR_API_KEY',
    },
  };

  const response = http.post(url, payload, params);

  check(response, {
    'status is 200': (r) => r.status === 200,
    'response has content': (r) => r.json().choices.length > 0,
  });

  sleep(1);
}
```

---

## 8. Security Hardening

### Network Security

```yaml
# deployment/security/network-policies.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: llm-gateway-egress
  namespace: llm-gateway
spec:
  podSelector:
    matchLabels:
      app: llm-gateway
  policyTypes:
    - Egress
  egress:
    # Allow DNS
    - to:
        - namespaceSelector:
            matchLabels:
              name: kube-system
        - podSelector:
            matchLabels:
              k8s-app: kube-dns
      ports:
        - protocol: UDP
          port: 53

    # Allow Redis
    - to:
        - podSelector:
            matchLabels:
              app: redis
      ports:
        - protocol: TCP
          port: 6379

    # Allow HTTPS to LLM providers (OpenAI, Anthropic, etc.)
    - to:
        - namespaceSelector: {}
      ports:
        - protocol: TCP
          port: 443

    # Deny all other egress
```

### Pod Security Standards

```yaml
# deployment/security/pod-security.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: llm-gateway
  labels:
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/warn: restricted
```

### Secrets Encryption

```yaml
# Enable encryption at rest for Kubernetes secrets
apiVersion: apiserver.config.k8s.io/v1
kind: EncryptionConfiguration
resources:
  - resources:
      - secrets
    providers:
      - aescbc:
          keys:
            - name: key1
              secret: <BASE64_ENCODED_SECRET>
      - identity: {}
```

### RBAC Configuration

```yaml
# deployment/security/rbac.yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: llm-gateway-sa
  namespace: llm-gateway
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: llm-gateway-role
  namespace: llm-gateway
rules:
  - apiGroups: [""]
    resources: ["configmaps", "secrets"]
    verbs: ["get", "list"]
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["get", "list"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: llm-gateway-rolebinding
  namespace: llm-gateway
subjects:
  - kind: ServiceAccount
    name: llm-gateway-sa
    namespace: llm-gateway
roleRef:
  kind: Role
  name: llm-gateway-role
  apiGroup: rbac.authorization.k8s.io
```

---

## 9. Operational Procedures

### Deployment Checklist

```markdown
## Pre-Deployment Checklist

- [ ] All tests passing in CI/CD
- [ ] Security scans completed (no critical vulnerabilities)
- [ ] Resource limits configured
- [ ] Secrets updated in production
- [ ] Database migrations completed (if applicable)
- [ ] Monitoring dashboards ready
- [ ] Alerts configured
- [ ] Runbook updated
- [ ] Stakeholders notified

## Deployment Steps

1. [ ] Deploy to staging
2. [ ] Run smoke tests
3. [ ] Verify metrics and logs
4. [ ] Deploy to production (canary 10%)
5. [ ] Monitor for 30 minutes
6. [ ] Increase to 50%
7. [ ] Monitor for 30 minutes
8. [ ] Increase to 100%
9. [ ] Monitor for 2 hours
10. [ ] Deployment complete

## Post-Deployment

- [ ] Verify all health checks passing
- [ ] Confirm metrics baseline
- [ ] Update documentation
- [ ] Notify stakeholders of completion
```

### Incident Response

```markdown
## Incident Response Runbook

### Severity Levels

**P0 - Critical:** Service completely down
- Response Time: Immediate
- Notification: PagerDuty + Slack + Email

**P1 - High:** Major functionality impaired
- Response Time: 15 minutes
- Notification: Slack + Email

**P2 - Medium:** Minor issues, workaround available
- Response Time: 1 hour
- Notification: Slack

### Common Issues and Resolutions

#### Issue: High Error Rate

**Symptoms:**
- Error rate > 5%
- AlertManager firing HighErrorRate alert

**Investigation:**
```bash
# Check pod logs
kubectl logs -l app=llm-gateway -n llm-gateway --tail=100

# Check provider health
kubectl exec -it llm-gateway-XXX -n llm-gateway -- curl localhost:8080/health

# Check Prometheus metrics
curl -s 'http://prometheus:9090/api/v1/query?query=rate(http_requests_total{status=~"5.."}[5m])'
```

**Resolution:**
1. Identify failing provider
2. Disable unhealthy provider
3. Route traffic to healthy providers
4. Investigate provider API issues

#### Issue: High Latency

**Investigation:**
```bash
# Check P99 latency
kubectl exec -it llm-gateway-XXX -n llm-gateway -- curl localhost:9090/metrics | grep http_request_duration

# Check resource usage
kubectl top pods -n llm-gateway

# Check provider latency
curl localhost:8080/metrics | grep provider_latency
```

**Resolution:**
1. Scale up pods if CPU/memory high
2. Review provider endpoints
3. Check network connectivity
4. Enable caching if appropriate

---

## Conclusion

This deployment guide provides a comprehensive foundation for deploying the LLM Inference Gateway across development, staging, and production environments. The architecture is designed for:

- **High Availability:** Multi-pod deployments with automatic failover
- **Scalability:** Horizontal scaling with HPA based on traffic patterns
- **Security:** Network policies, RBAC, secrets management, and pod security
- **Observability:** Comprehensive monitoring with Prometheus, Grafana, and distributed tracing
- **Global Reach:** Multi-region deployments with latency-based routing

For questions or support, consult the main repository README or contact the DevOps team.
