# LLM Inference Gateway - Multi-stage Docker Build
# Optimized for minimal image size and security

# ==============================================================================
# Stage 1: Build Environment
# ==============================================================================
FROM rust:1.75-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a new empty shell project
WORKDIR /app

# Copy the Cargo files first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/gateway-core/Cargo.toml ./crates/gateway-core/
COPY crates/gateway-config/Cargo.toml ./crates/gateway-config/
COPY crates/gateway-providers/Cargo.toml ./crates/gateway-providers/
COPY crates/gateway-routing/Cargo.toml ./crates/gateway-routing/
COPY crates/gateway-resilience/Cargo.toml ./crates/gateway-resilience/
COPY crates/gateway-telemetry/Cargo.toml ./crates/gateway-telemetry/
COPY crates/gateway-server/Cargo.toml ./crates/gateway-server/

# Create dummy source files to build dependencies
RUN mkdir -p src crates/gateway-core/src crates/gateway-config/src \
    crates/gateway-providers/src crates/gateway-routing/src \
    crates/gateway-resilience/src crates/gateway-telemetry/src \
    crates/gateway-server/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn lib() {}" > crates/gateway-core/src/lib.rs && \
    echo "pub fn lib() {}" > crates/gateway-config/src/lib.rs && \
    echo "pub fn lib() {}" > crates/gateway-providers/src/lib.rs && \
    echo "pub fn lib() {}" > crates/gateway-routing/src/lib.rs && \
    echo "pub fn lib() {}" > crates/gateway-resilience/src/lib.rs && \
    echo "pub fn lib() {}" > crates/gateway-telemetry/src/lib.rs && \
    echo "pub fn lib() {}" > crates/gateway-server/src/lib.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --release && rm -rf src crates

# Copy the actual source code
COPY src ./src
COPY crates ./crates

# Touch main.rs to ensure it gets recompiled
RUN touch src/main.rs

# Build the application
RUN cargo build --release --bin llm-inference-gateway

# ==============================================================================
# Stage 2: Runtime Environment
# ==============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN groupadd -r gateway && useradd -r -g gateway gateway

# Create necessary directories
RUN mkdir -p /etc/llm-gateway /var/log/llm-gateway && \
    chown -R gateway:gateway /etc/llm-gateway /var/log/llm-gateway

# Copy the binary from builder
COPY --from=builder /app/target/release/llm-inference-gateway /usr/local/bin/

# Copy default configuration
COPY deploy/kubernetes/configmap.yaml /etc/llm-gateway/gateway.yaml.example

# Set ownership
RUN chown gateway:gateway /usr/local/bin/llm-inference-gateway

# Switch to non-root user
USER gateway

# Set working directory
WORKDIR /home/gateway

# Expose ports
# 8080 - HTTP API
# 9090 - Prometheus metrics
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Environment variables with defaults
ENV GATEWAY_HOST=0.0.0.0 \
    GATEWAY_PORT=8080 \
    GATEWAY_METRICS_PORT=9090 \
    LOG_LEVEL=info \
    LOG_FORMAT=json \
    RUST_BACKTRACE=1

# Entry point
ENTRYPOINT ["llm-inference-gateway"]

# Default command (can be overridden)
CMD []

# ==============================================================================
# Stage 3: Development Environment (optional)
# ==============================================================================
FROM rust:1.75-bookworm AS development

# Install development tools
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install cargo tools for development
RUN cargo install cargo-watch cargo-tarpaulin

WORKDIR /app

# Copy the project
COPY . .

# Build for development
RUN cargo build

# Expose ports
EXPOSE 8080 9090

# Development command with auto-reload
CMD ["cargo", "watch", "-x", "run"]
