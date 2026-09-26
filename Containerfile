# Batata — container image
#
# Build from the repository root:
#   podman build -f Containerfile -t batata:latest .
#   docker build -f Containerfile -t batata:latest .
#
# Multi-stage: compile with the full Rust toolchain, then ship a slim runtime
# image containing only the binary and conf/.

# ---------------------------------------------------------------------------
# Stage 1 — build
# ---------------------------------------------------------------------------
FROM rust:1.85-bookworm AS builder

# RocksDB (C++) needs a C/C++ toolchain, clang and cmake.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        clang \
        libclang-dev \
        cmake \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

RUN cargo build --release -p batata-server

# ---------------------------------------------------------------------------
# Stage 2 — runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

# curl is required by the compose healthchecks.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /src/target/release/batata-server /app/bin/batata-server
COPY conf/ /app/conf/

ENV RUST_LOG=info

# 8848 SDK HTTP | 8081 Console | 8080 Apollo | 9848 SDK gRPC
# 9849 Cluster gRPC | 8500 Consul | 9080 MCP registry
EXPOSE 8848 8081 8080 9848 9849 8500 9080

ENTRYPOINT ["/app/bin/batata-server"]
