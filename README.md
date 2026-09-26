# Batata

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/easynet-cn/batata/ci.yml?branch=main)](https://github.com/easynet-cn/batata/actions)

**Batata** is a high-performance service discovery, configuration management and
service management platform written in Rust. It is a drop-in replacement for
[Nacos](https://nacos.io/) (V2/V3), with additional compatibility for
[Consul](https://www.consul.io/) and [Apollo](https://github.com/apolloconfig/apollo).

[中文文档](README_CN.md)

## Why Batata

- **Rust-native** — starts in ~1-2s and runs in ~50-100MB, versus 30-60s and 1-2GB for Java Nacos
- **Drop-in compatible** — Nacos V2/V3 APIs, plus Consul and Apollo compatibility layers
- **Zero-dependency start** — embedded RocksDB mode needs no external database
- **Production ready** — implements ~98% of Nacos features; Raft-based clustering, RBAC, rate limiting

## Features

**Core**

- Configuration management with real-time updates, history/rollback, gray release
- Service discovery with registration, health checks, subscribe/push
- Namespace isolation for multi-tenancy
- Cluster mode with Raft consensus (CP) and distro protocol (AP)

**Protocol compatibility**

- **Nacos** — full V2 and V3 API compatibility (V1 intentionally unsupported)
- **Consul** — Agent, Health, Catalog, KV, ACL APIs
- **Apollo** — Config Service (`/configs`, `/configfiles`, `/notifications/v2`, `/releaseMessage`), Admin Service and OpenAPI; existing Apollo SDKs work unchanged, and writes are Raft-replicated in cluster mode. See [`docs/compat/apollo/`](docs/compat/apollo/)
- **gRPC** — bidirectional streaming for SDK clients

**Operations**

- JWT authentication with RBAC, LDAP and OAuth2/OIDC
- Rate limiting, circuit breaker, audit logs, distributed locking
- Config import/export (Nacos ZIP, Consul JSON)
- Service mesh xDS (EDS/CDS/LDS/RDS/ADS), multi-datacenter replication, DNS discovery
- Prometheus metrics and OpenTelemetry tracing

Full comparison: [`docs/compat/nacos/feature-comparison.md`](docs/compat/nacos/feature-comparison.md)

## Web Console (batata-ui)

The web console is a separate frontend project:
[**easynet-cn/batata-ui**](https://github.com/easynet-cn/batata-ui)

- Build the UI and place its output in `console-ui/`; the console server serves it
  (controlled by `batata.console.ui.enabled`, enabled by default)
- Console UI is available in `merged` (default) and `console` deployment modes
- Access it at **`http://localhost:8081/`** after startup

Ports are separated by role — the console port serves the UI only and does **not**
expose the Nacos-compatible data APIs:

| Port | Role | Serves |
|------|------|--------|
| `8081` | Console | Web console UI + console APIs |
| `8848` | SDK HTTP | Nacos-compatible REST API |
| `9848` | SDK gRPC | Client SDK communication |
| `9849` | Cluster gRPC | Inter-node / Raft communication |

## Quick Start

Embedded RocksDB mode — no database required:

```bash
git clone https://github.com/easynet-cn/batata.git
cd batata

cargo build --release -p batata-server
./target/release/batata-server --batata.sql.init.platform=embedded

# Or use the convenience script
./scripts/start-embedded.sh
```

Initialize the admin user on first startup:

```bash
curl -X POST http://localhost:8848/nacos/v3/auth/user/admin \
  -d "username=nacos&password=nacos"
```

Then open the console at **http://localhost:8081/**.

MySQL / PostgreSQL setups: [`docs/database.md`](docs/database.md)

### Default Ports

| Port | Service | Description |
|------|---------|-------------|
| 8848 | Main HTTP API | Nacos-compatible API |
| 8081 | Console HTTP API | Web management console |
| 8080 | Apollo HTTP API | Apollo Config/Admin/OpenAPI (use per-node ports in cluster) |
| 9848 | SDK gRPC | Client SDK communication |
| 9849 | Cluster gRPC | Inter-node communication |
| 15010 | xDS gRPC | Service mesh xDS/ADS protocol |

## Compatibility

| Protocol | Support | Details |
|----------|---------|---------|
| Nacos V2 / V3 | Full | V1 intentionally unsupported |
| Nacos gRPC | Full | Bidirectional streaming |
| Consul | Full | Agent, Health, Catalog, KV, ACL |
| Apollo | ~81% | Config/Admin/OpenAPI ported — [`docs/compat/apollo/`](docs/compat/apollo/) |

Client SDK examples: [`docs/api.md`](docs/api.md#client-sdk-examples)

## Configuration

Main file: `conf/application.yml` (fully commented — single source of truth).
Keys use the flat `batata.*` prefix and can be overridden by
`BATATA_*` environment variables or `--batata.<key>=<value>` CLI flags.

Most-used keys:

| Key | Default | Description |
|-----|---------|-------------|
| `batata.server.main.port` | `8848` | SDK HTTP port |
| `batata.console.port` | `8081` | Console port |
| `batata.deployment.type` | `merged` | `merged` / `server` / `console` / `serverWithMcp` |
| `batata.sql.init.platform` | _(empty)_ | `mysql`, `postgresql`, or empty (= embedded) |
| `batata.core.auth.enabled` | `true` | Enable authentication |

Complete reference: [`docs/configuration.md`](docs/configuration.md)

## Deployment

| Mode | Flag | Description |
|------|------|-------------|
| `merged` (default) | `-d merged` | Console (8081) + Main Server (8848) in one process |
| `server` | `-d server` | Main Server only, no console |
| `console` | `-d console` | Console only, connects to a remote server |
| `serverWithMcp` | `-d serverWithMcp` | Main Server + MCP Registry (9080) |

Container, Compose and Kubernetes instructions: [`docs/deployment.md`](docs/deployment.md)

## Documentation

| Topic | Location |
|-------|----------|
| Architecture & crates | [`docs/architecture.md`](docs/architecture.md) |
| Configuration | [`docs/configuration.md`](docs/configuration.md) |
| API reference | [`docs/api.md`](docs/api.md) |
| Deployment | [`docs/deployment.md`](docs/deployment.md) |
| Database & migration | [`docs/database.md`](docs/database.md) |
| Client SDKs | [`docs/api.md`](docs/api.md#client-sdk-examples) |
| Monitoring (metrics) | [`docs/configuration.md`](docs/configuration.md#metrics) |
| Development & release | [`docs/development.md`](docs/development.md) |
| Nacos feature comparison | [`docs/compat/nacos/feature-comparison.md`](docs/compat/nacos/feature-comparison.md) |
| Protocol compatibility | [`docs/compat/`](docs/compat/) |
| Roadmap | [`docs/roadmap.md`](docs/roadmap.md) |

## Security

Please do **not** open public issues for security vulnerabilities.
Reporting instructions and response timelines are in [`SECURITY.md`](SECURITY.md).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Licensed under the Apache-2.0 License — see [LICENSE](LICENSE).

## Acknowledgments

- [Nacos](https://nacos.io/) - Original design and API specification
- [Consul](https://www.consul.io/) - KV store and service discovery API design
- [Apollo](https://github.com/apolloconfig/apollo) - Config Service / Admin Service / OpenAPI design
- [batata-ui](https://github.com/easynet-cn/batata-ui) - Web console frontend
- [OpenRaft](https://github.com/datafuselabs/openraft) - Raft consensus
- [SeaORM](https://www.sea-ql.org/SeaORM/) - Async ORM
- [Actix-web](https://actix.rs/) - Web framework
- [Tonic](https://github.com/hyperium/tonic) - gRPC implementation
