# LLM-Inference-Gateway - Unified Edge Function Service
# Simplified build without test workspace members

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

# Runtime stage
FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/llm-inference-gateway /app/llm-inference-gateway

RUN useradd -r -s /bin/false gateway && \
    chown -R gateway:gateway /app

USER gateway

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

ENV RUST_LOG=info
ENV GATEWAY_HOST=0.0.0.0
ENV GATEWAY_PORT=8080

ENTRYPOINT ["/app/llm-inference-gateway"]
