# ==============================================================================
# Multi-stage build for Batata server (Podman / Buildah)
#
# Build:
#   podman build -t batata:latest .
#   podman build -t batata:latest -f Containerfile .
#
# Run:
#   podman run -d --name batata -p 8848:8848 -p 8081:8081 -p 9848:9848 batata:latest
# ==============================================================================

# Stage 1: Build
FROM docker.io/library/rust:latest AS builder

WORKDIR /build

# Install system dependencies for RocksDB, gRPC, and TLS
RUN apt-get update && apt-get install -y \
    libclang-dev \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy source and build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY proto/ proto/

# Build release binary with LTO (configured in Cargo.toml profile.release)
RUN cargo build --release -p batata-server \
    && strip target/release/batata-server

# Stage 2: Runtime (minimal image)
FROM docker.io/library/debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r batata && useradd -r -g batata -d /app batata

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

# Health check (matches Nacos v3 admin API)
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -sf http://localhost:8848/nacos/v3/admin/core/state/liveness || exit 1

# Default: embedded mode (no external database required)
ENTRYPOINT ["/app/batata-server"]
CMD ["--batata.sql.init.platform=embedded"]
