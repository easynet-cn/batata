# ==============================================================================
# Multi-stage build for Batata server
# ==============================================================================

# Stage 1: Build
FROM rust:1.86-bookworm AS builder

WORKDIR /build

# Install system dependencies
RUN apt-get update && apt-get install -y \
    libclang-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY proto/ proto/

# Build release binary
RUN cargo build --release -p batata-server

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r batata && useradd -r -g batata -m batata

WORKDIR /app

# Copy binary and config
COPY --from=builder /build/target/release/batata-server /app/batata-server
COPY conf/ /app/conf/
COPY scripts/ /app/scripts/

# Create data directories
RUN mkdir -p /app/data /app/logs && chown -R batata:batata /app

USER batata

# Ports: Main HTTP, Console HTTP, SDK gRPC, Cluster gRPC, Consul, MCP Registry
EXPOSE 8848 8081 9848 9849 8500 9080

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -sf http://localhost:8081/v3/console/health/liveness || exit 1

# Default: embedded mode (no external database required)
ENTRYPOINT ["/app/batata-server"]
CMD ["--batata.sql.init.platform=embedded"]
