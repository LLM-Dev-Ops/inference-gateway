# ============================================================================
# LLM Inference Gateway - Multi-Stage Production Dockerfile
# ============================================================================

# ============================================================================
# Stage 1: Builder - Compile Rust Application
# ============================================================================
FROM rust:1.75-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    g++ \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency manifests first for caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source to build dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"dummy\");}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build application
RUN cargo build --release --locked && \
    strip target/release/llm-inference-gateway

# ============================================================================
# Stage 2: Runtime - Minimal Production Image
# ============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash appuser && \
    mkdir -p /app/cache /app/logs /tmp && \
    chown -R appuser:appuser /app /tmp

# Copy compiled binary from builder
COPY --from=builder --chown=appuser:appuser /app/target/release/llm-inference-gateway /usr/local/bin/llm-inference-gateway

# Set working directory
WORKDIR /app

# Switch to non-root user
USER appuser

# Expose ports
EXPOSE 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Environment variables
ENV RUST_LOG=info \
    SERVER_HOST=0.0.0.0 \
    SERVER_PORT=8080 \
    METRICS_PORT=9090

# Run application
ENTRYPOINT ["/usr/local/bin/llm-inference-gateway"]
