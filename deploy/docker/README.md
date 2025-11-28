# LLM Inference Gateway - Docker Deployment

Quick start guide for running the LLM Inference Gateway using Docker.

## Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- At least one LLM provider API key (OpenAI, Anthropic, etc.)

## Quick Start

### 1. Set up environment variables

Create a `.env` file in the project root:

```bash
# Required: At least one provider API key
OPENAI_API_KEY=sk-your-openai-key
ANTHROPIC_API_KEY=sk-ant-your-anthropic-key
GOOGLE_API_KEY=your-google-api-key

# Optional: Grafana admin password
GRAFANA_PASSWORD=your-secure-password
```

### 2. Start the gateway

```bash
# Start gateway with Redis cache
docker-compose up -d

# Or start with monitoring stack
docker-compose --profile monitoring up -d

# Or start with full observability (monitoring + tracing)
docker-compose --profile monitoring --profile tracing up -d
```

### 3. Verify the deployment

```bash
# Check health
curl http://localhost:8080/health

# List available models
curl http://localhost:8080/v1/models

# Test completion
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## Services

| Service | Port | Description |
|---------|------|-------------|
| Gateway | 8080 | HTTP API endpoint |
| Gateway Metrics | 9090 | Prometheus metrics |
| Redis | 6379 | Distributed cache |
| Prometheus | 9091 | Metrics collection |
| Grafana | 3000 | Metrics visualization |
| Jaeger | 16686 | Trace visualization |

## Profiles

Docker Compose profiles allow starting subsets of services:

```bash
# Core only (gateway + redis)
docker-compose up -d

# With monitoring (+ prometheus, grafana, redis-exporter)
docker-compose --profile monitoring up -d

# With tracing (+ otel-collector, jaeger)
docker-compose --profile tracing up -d

# Everything
docker-compose --profile monitoring --profile tracing up -d
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GATEWAY_HOST` | `0.0.0.0` | Server bind address |
| `GATEWAY_PORT` | `8080` | HTTP API port |
| `GATEWAY_METRICS_PORT` | `9090` | Prometheus metrics port |
| `LOG_LEVEL` | `info` | Log level (trace, debug, info, warn, error) |
| `LOG_FORMAT` | `json` | Log format (json, pretty) |
| `REDIS_ENABLED` | `true` | Enable Redis cache |
| `REDIS_URL` | `redis://redis:6379` | Redis connection URL |
| `RATE_LIMIT_ENABLED` | `true` | Enable rate limiting |
| `OTEL_ENABLED` | `true` | Enable OpenTelemetry |

### Custom Configuration

Mount a custom configuration file:

```yaml
# docker-compose.override.yml
services:
  gateway:
    volumes:
      - ./my-config.yaml:/etc/llm-gateway/gateway.yaml:ro
```

## Building

### Build the image

```bash
# Build production image
docker build -t llm-inference-gateway:latest .

# Build development image
docker build --target development -t llm-inference-gateway:dev .

# Build with specific tag
docker build -t llm-inference-gateway:v0.1.0 .
```

### Multi-platform build

```bash
# Set up buildx
docker buildx create --name mybuilder --use

# Build for multiple platforms
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t llm-inference-gateway:latest \
  --push .
```

## Development

### Run in development mode

```bash
# Build and run development container
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up

# Or use the development target
docker build --target development -t llm-gateway:dev .
docker run -it --rm \
  -v $(pwd):/app \
  -p 8080:8080 \
  llm-gateway:dev
```

### View logs

```bash
# All services
docker-compose logs -f

# Gateway only
docker-compose logs -f gateway

# Last 100 lines
docker-compose logs --tail=100 gateway
```

## Production Considerations

### Security

1. **Use secrets management**: Never commit API keys to version control
2. **Enable TLS**: Use a reverse proxy (nginx, traefik) for HTTPS
3. **Restrict network access**: Use Docker networks and firewall rules
4. **Run as non-root**: The gateway runs as user `gateway` by default

### High Availability

For production deployments, consider:

1. **Multiple gateway instances**: Use Docker Swarm or Kubernetes
2. **Redis cluster**: For high-availability caching
3. **Load balancer**: Distribute traffic across instances

### Resource Limits

Adjust resource limits in `docker-compose.yml`:

```yaml
services:
  gateway:
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 4G
        reservations:
          cpus: '1'
          memory: 1G
```

## Troubleshooting

### Gateway won't start

```bash
# Check logs
docker-compose logs gateway

# Check if ports are in use
netstat -tlnp | grep -E '8080|9090'

# Verify environment variables
docker-compose config
```

### Redis connection issues

```bash
# Check Redis health
docker-compose exec redis redis-cli ping

# Check Redis logs
docker-compose logs redis
```

### Performance issues

```bash
# Check resource usage
docker stats

# Check gateway metrics
curl http://localhost:9090/metrics | grep llm_gateway
```

## Stopping

```bash
# Stop all services
docker-compose down

# Stop and remove volumes (data loss!)
docker-compose down -v

# Stop specific profile
docker-compose --profile monitoring down
```
