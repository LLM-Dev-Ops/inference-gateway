# LLM-Inference-Gateway - Phase 7 Cloud Run Service
# Includes RuVector validation and Phase 7 agent configuration
#
# Build: docker build -t inference-gateway-phase7 -f deploy/cloud-run/Dockerfile .
# Run:   docker run -p 8080:8080 -e RUVECTOR_API_KEY=xxx -e RUVECTOR_SERVICE_URL=xxx inference-gateway-phase7

# =============================================================================
# Build Stage
# =============================================================================
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml ./
COPY crates ./crates
COPY src ./src

# Create a modified Cargo.toml without tests/integration
RUN sed -i '/tests\/integration/d' Cargo.toml && \
    sed -i '/tests\/load/d' Cargo.toml

# Generate lockfile and build
RUN cargo build --release --bin llm-inference-gateway

# =============================================================================
# Runtime Stage
# =============================================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/llm-inference-gateway /app/llm-inference-gateway

# Copy startup and health check scripts
COPY deploy/cloud-run/scripts/startup.sh /app/startup.sh
COPY deploy/cloud-run/scripts/healthcheck.sh /app/healthcheck.sh

# Make scripts executable
RUN chmod +x /app/startup.sh /app/healthcheck.sh

# Create non-root user
RUN useradd -r -s /bin/false gateway && \
    chown -R gateway:gateway /app

USER gateway

EXPOSE 8080

# =============================================================================
# Health Check Configuration
# =============================================================================
# The healthcheck verifies:
# 1. Gateway is responding on /health
# 2. RuVector connectivity (when RUVECTOR_REQUIRED=true)
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD /app/healthcheck.sh

# =============================================================================
# Environment Variables
# =============================================================================

# Server configuration
ENV RUST_LOG=info
ENV GATEWAY_HOST=0.0.0.0
ENV GATEWAY_PORT=8080

# Phase 7 agent identity
ENV AGENT_NAME=inference-routing-agent
ENV AGENT_DOMAIN=inference-gateway
ENV AGENT_PHASE=phase7
ENV AGENT_LAYER=layer2
ENV AGENT_VERSION=1.0.0

# Performance budgets
ENV MAX_TOKENS=2500
ENV MAX_LATENCY_MS=5000
ENV MAX_CALLS_PER_RUN=5

# RuVector configuration (values must be provided at runtime)
ENV RUVECTOR_REQUIRED=true
ENV RUVECTOR_TIMEOUT_MS=5000
ENV RUVECTOR_RETRY_COUNT=3

# Phase 7 features
ENV PHASE7_ROUTING_ENABLED=true
ENV PHASE7_MODEL_SELECTION=true
ENV PHASE7_COST_OPTIMIZATION=true
ENV PHASE7_SEMANTIC_CACHE=true

# =============================================================================
# Entrypoint
# =============================================================================
# Use startup script to validate environment before starting gateway
ENTRYPOINT ["/app/startup.sh"]
