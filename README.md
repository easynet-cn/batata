# Batata

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/easynet-cn/batata/ci.yml?branch=main)](https://github.com/easynet-cn/batata/actions)

**Batata** is a high-performance, Rust-based implementation of a dynamic service discovery, configuration management, and service management platform. It is fully compatible with [Nacos](https://nacos.io/) V2/V3 APIs and [Consul](https://www.consul.io/) APIs, making it an ideal drop-in replacement for cloud-native applications.

[中文文档](README_CN.md)

## Features

### Core Capabilities

- **Configuration Management** - Centralized, dynamic configuration with real-time updates
- **Service Discovery** - Service registration, discovery, and health checking
- **Namespace Isolation** - Multi-tenant support with namespace-based resource isolation
- **Cluster Mode** - High availability with Raft consensus protocol

### API Compatibility

- **Nacos API** - Full compatibility with Nacos V2 and V3 APIs (V1 intentionally not supported)
- **Consul API** - Compatible with Consul Agent, Health, Catalog, KV, and ACL APIs
- **gRPC Support** - High-performance bidirectional streaming for SDK clients

### Advanced Features

- **Config Import/Export** - Batch configuration migration in Nacos ZIP or Consul JSON format
- **Gray/Beta Release** - Gradual configuration rollout with gray release support
- **Authentication & Authorization** - JWT-based authentication with RBAC permission model, LDAP, OAuth2/OIDC
- **Rate Limiting** - Configurable rate limiting for API protection
- **Circuit Breaker** - Resilience patterns for fault tolerance
- **Metrics & Observability** - Prometheus-compatible metrics endpoint
- **Service Mesh** - xDS protocol support (EDS, CDS, LDS, RDS, ADS)
- **Multi-Datacenter** - Locality-aware replication with cross-DC sync
- **DNS Service** - UDP-based DNS for service discovery
- **Distributed Lock** - Raft-based distributed locking
- **AI Integration** - MCP (Model Content Protocol) and A2A (Agent-to-Agent) registry
- **Database Migration** - Automatic schema migration on startup via `batata-migration` crate

### Performance

- **Fast Startup** - ~1-2 seconds vs 30-60 seconds for Java-based Nacos
- **Low Memory** - ~50-100MB vs 1-2GB for Java-based Nacos
- **High Throughput** - Optimized async I/O with Tokio runtime
- **Efficient Storage** - RocksDB for persistent storage with tuned caching

## Feature Comparison with Nacos

Batata implements **~98% of Nacos features** and can serve as a production-ready drop-in replacement.

### Core Features

| Feature | Nacos | Batata | Status |
|---------|-------|--------|--------|
| **Configuration Management** | | | |
| Config CRUD | ✅ | ✅ | Full |
| Config Search & Pagination | ✅ | ✅ | Full |
| Config History & Rollback | ✅ | ✅ | Full |
| Config Listening (Long Polling) | ✅ | ✅ | Full |
| Gray/Beta Release | ✅ | ✅ | Full |
| Config Import/Export (ZIP) | ✅ | ✅ | Full |
| Config Tags | ✅ | ✅ | Full |
| Config Encryption | ✅ | ✅ | Full (AES-128-CBC) |
| Config Capacity Quota | ✅ | ✅ | Full |
| Aggregate Config (datumId) | ✅ | ✅ | Full |
| **Service Discovery** | | | |
| Instance Register/Deregister | ✅ | ✅ | Full |
| Service Discovery Query | ✅ | ✅ | Full |
| Health Check | ✅ | ✅ | Full |
| Heartbeat | ✅ | ✅ | Full |
| Subscribe/Push | ✅ | ✅ | Full |
| Weight Routing | ✅ | ✅ | Full |
| Ephemeral Instances | ✅ | ✅ | Full |
| Persistent Instances | ✅ | ✅ | Full |
| Service Metadata | ✅ | ✅ | Full |
| DNS-based Discovery | ✅ | ✅ | Full |
| **Namespace** | | | |
| Namespace CRUD | ✅ | ✅ | Full |
| Multi-tenant Isolation | ✅ | ✅ | Full |
| **Cluster** | | | |
| Cluster Node Discovery | ✅ | ✅ | Full |
| Node Health Check | ✅ | ✅ | Full |
| Raft Consensus (CP) | ✅ | ✅ | Full |
| Distro Protocol (AP) | ✅ | ✅ | Full |
| Multi-Datacenter Sync | ✅ | ✅ | Full |
| **Auth** | | | |
| User Management | ✅ | ✅ | Full |
| Role Management | ✅ | ✅ | Full |
| Permission (RBAC) | ✅ | ✅ | Full |
| JWT Token | ✅ | ✅ | Full |
| LDAP Integration | ✅ | ✅ | Full |
| OAuth2/OIDC | ✅ | ✅ | Full |
| **API** | | | |
| Nacos V2 API | ✅ | ✅ | Full |
| Nacos V3 API | ✅ | ✅ | Full |
| Nacos V1 API | ✅ | ❌ | **Intentionally not supported** |
| gRPC Bi-directional Streaming | ✅ | ✅ | Full |
| Consul API Compatibility | ❌ | ✅ | **Extra** |
| **Observability** | | | |
| Prometheus Metrics | ✅ | ✅ | Full |
| Health Endpoint | ✅ | ✅ | Full |
| OpenTelemetry | ✅ | ✅ | Full |
| Operation Audit Logs | ✅ | ✅ | Full |

### Batata Exclusive Features

| Feature | Description |
|---------|-------------|
| **Consul API Compatibility** | Full support for Agent, Health, Catalog, KV, ACL APIs |
| **PostgreSQL Support** | In addition to MySQL |
| **Consul JSON Import/Export** | Migration support for Consul KV store |
| **Built-in Circuit Breaker** | Resilience pattern for cluster health checks |
| **OpenTelemetry Tracing** | OTLP export for distributed tracing (Jaeger, Zipkin, etc.) |
| **Multi-datacenter Support** | Locality-aware replication with local-first sync |
| **Service Mesh (xDS)** | EDS, CDS, LDS, RDS, ADS protocol support |
| **AI Integration** | MCP Server Registry and A2A Agent Registry |
| **Distributed Lock** | Raft-based distributed locking mechanism |
| **Kubernetes Sync** | Bidirectional service sync with Kubernetes |
| **Database Migration** | Automatic schema migration via `batata-migration` crate |

### Feature Completeness Summary

| Module | Completeness | Notes |
|--------|--------------|-------|
| Configuration Management | **100%** | Full AES-GCM encryption support |
| Service Discovery | **95%** | UDP Push not implemented (gRPC push used) |
| Namespace | **100%** | Fully supported |
| Cluster Management | **95%** | Single Raft group (Multi-Raft partial) |
| Authentication | **100%** | JWT, LDAP, OAuth2/OIDC |
| API Compatibility | **100%+** | Full Nacos V2/V3 + Consul APIs |
| Cloud Native | **85%** | K8s, Prometheus, xDS (basic) |
| **Overall** | **~98%** | Production ready |

## Project Structure

Batata uses a multi-crate workspace architecture with **19 internal crates** for modularity and maintainability:

```
batata/
├── crates/
│   ├── batata-common/            # Common utilities & types
│   ├── batata-api/               # API definitions & gRPC proto
│   ├── batata-persistence/       # Database entities (SeaORM)
│   ├── batata-migration/         # Database schema migration
│   ├── batata-auth/              # Authentication & authorization
│   ├── batata-consistency/       # Raft consensus protocol
│   ├── batata-core/              # Core abstractions & cluster
│   ├── batata-server-common/     # Shared server infrastructure
│   ├── batata-config/            # Configuration service
│   ├── batata-naming/            # Service discovery
│   ├── batata-plugin/            # Plugin interfaces
│   ├── batata-plugin-consul/     # Consul compatibility plugin
│   ├── batata-plugin-cloud/      # Prometheus & Kubernetes plugin
│   ├── batata-console/           # Console backend service
│   ├── batata-client/            # Client SDK
│   ├── batata-maintainer-client/ # Maintainer client utilities
│   ├── batata-consul-client/     # Consul client utilities
│   ├── batata-ai/                # AI integration (MCP/A2A)
│   ├── batata-mesh/              # Service mesh (xDS, Istio MCP)
│   └── batata-server/            # Main server (HTTP, gRPC, Console)
│       ├── src/
│       │   ├── api/              # API handlers (HTTP, gRPC, Consul)
│       │   ├── auth/             # Auth HTTP handlers
│       │   ├── config/           # Config models (re-exports)
│       │   ├── console/          # Console API handlers
│       │   ├── middleware/       # HTTP middleware
│       │   ├── model/            # Application state & config
│       │   ├── service/          # gRPC handlers
│       │   ├── startup/          # Server initialization
│       │   ├── lib.rs            # Library exports
│       │   └── main.rs           # Entry point
│       ├── tests/                # Integration tests
│       └── benches/              # Performance benchmarks
├── conf/                         # Configuration files
├── docs/                         # Documentation
└── proto/                        # Protocol buffer definitions
```

### Crate Dependencies

```
batata-server (main binary)
├── batata-api (API types, gRPC proto)
├── batata-auth (JWT, RBAC, LDAP, OAuth2)
├── batata-config (config service)
├── batata-naming (service discovery)
├── batata-console (console backend)
├── batata-core (cluster, connections, datacenter)
├── batata-server-common (shared server infrastructure)
├── batata-mesh (xDS, Istio MCP)
├── batata-ai (MCP/A2A registry)
├── batata-plugin-consul (Consul API)
├── batata-plugin-cloud (Prometheus, K8s)
├── batata-migration (database migration)
└── batata-persistence (database)
```

## Quick Start

### Prerequisites

- Rust 1.85+ (Edition 2024)
- MySQL 5.7+ / 8.0+ or PostgreSQL 12+ (optional -- embedded mode requires no database)

### Option 1: Embedded Mode (No Database Required)

The fastest way to get started. Batata runs with an embedded RocksDB store:

```bash
# Clone the repository
git clone https://github.com/easynet-cn/batata.git
cd batata

# Build and run in embedded mode
cargo build --release -p batata-server
./target/release/batata-server --batata.sql.init.platform=embedded

# Or use the convenience script
./scripts/start-embedded.sh
```

On first startup, initialize the admin user:

```bash
curl -X POST http://localhost:8848/nacos/v3/auth/user/admin \
  -d "username=nacos&password=nacos"
```

### Option 2: With MySQL

```bash
# Clone the repository
git clone https://github.com/easynet-cn/batata.git
cd batata

# Initialize database
mysql -u root -p < conf/mysql-schema.sql
mysql -u root -p < conf/consul-mysql-schema.sql

# Edit conf/application.yml with your database credentials
# Then build and run
cargo build --release -p batata-server
./target/release/batata-server
```

Alternatively, enable auto-migration instead of manually importing schemas:

```bash
./target/release/batata-server --batata.db.migration.enabled=true
```

### Option 3: With PostgreSQL

```bash
# Initialize database
psql -U user -d batata -f conf/postgresql-schema.sql

# Edit conf/application.yml:
#   batata.sql.init.platform: postgresql
#   batata.db.url: postgres://user:password@localhost:5432/batata
cargo build --release -p batata-server
./target/release/batata-server
```

### Default Ports

| Port | Service | Description |
|------|---------|-------------|
| 8848 | Main HTTP API | Nacos-compatible API |
| 8081 | Console HTTP API | Web management console |
| 9848 | SDK gRPC | Client SDK communication |
| 9849 | Cluster gRPC | Inter-node communication |
| 15010 | xDS gRPC | Service mesh xDS/ADS protocol |

## Configuration

Main configuration file: `conf/application.yml`

All configuration keys use the flat `batata.*` prefix. Keys can be overridden via:
- **Environment variables**: `BATATA_*` (e.g., `BATATA_SERVER_MAIN_PORT=9090`)
- **CLI property overrides**: `--batata.server.main.port=9090`

```yaml
# Server
batata.server.main.port: 8848
batata.server.context_path: /nacos
batata.standalone: true
#batata.deployment.type: merged

# Console
batata.console.port: 8081
batata.console.context_path:

# Database
batata.sql.init.platform: mysql
batata.db.url: "mysql://user:password@localhost:3306/batata"
batata.db.pool.max_connections: 100
batata.db.pool.min_connections: 10
batata.db.migration.enabled: false

# Authentication
batata.core.auth.enabled: true
batata.core.auth.system.type: nacos
batata.core.auth.plugin.nacos.token.secret.key: "your-base64-encoded-secret-key"
batata.core.auth.plugin.nacos.token.expire.seconds: 18000
```

### Environment Variables

```bash
export BATATA_DB_URL="mysql://user:pass@localhost:3306/batata"
export BATATA_SERVER_MAIN_PORT=8848
export BATATA_CORE_AUTH_ENABLED=true
export RUST_LOG=info
```

### Configuration Reference

#### Server

| Key | Default | Description |
|-----|---------|-------------|
| `batata.server.main.port` | `8848` | Main HTTP server port |
| `batata.server.context_path` | `/nacos` | Server web context path |
| `batata.standalone` | `true` | Standalone mode (true=single, false=cluster) |
| `batata.deployment.type` | `merged` | Deployment type: merged/server/console/serverWithMcp |

#### Console

| Key | Default | Description |
|-----|---------|-------------|
| `batata.console.port` | `8081` | Console HTTP server port |
| `batata.console.context_path` | _(empty)_ | Console web context path |
| `batata.console.remote.server_addr` | _(empty)_ | Remote server address (console-only mode) |
| `batata.console.remote.username` | _(empty)_ | Remote server username |
| `batata.console.remote.password` | _(empty)_ | Remote server password |

#### Database

| Key | Default | Description |
|-----|---------|-------------|
| `batata.sql.init.platform` | _(empty)_ | Storage platform: `mysql`, `postgresql`, or empty (=embedded) |
| `batata.db.url` | _(empty)_ | Database connection URL |
| `batata.db.pool.max_connections` | `100` | Max pool connections |
| `batata.db.pool.min_connections` | `10` | Min pool connections |
| `batata.db.pool.connect_timeout` | `30` | Connection timeout (seconds) |
| `batata.db.pool.acquire_timeout` | `30` | Acquire timeout (seconds) |
| `batata.db.pool.idle_timeout` | `600` | Idle timeout (seconds) |
| `batata.db.pool.max_lifetime` | `1800` | Max connection lifetime (seconds) |
| `batata.db.migration.enabled` | `false` | Auto-run database migrations on startup |

#### Authentication

| Key | Default | Description |
|-----|---------|-------------|
| `batata.core.auth.enabled` | `true` | Enable authentication |
| `batata.core.auth.admin.enabled` | `true` | Enable admin API auth |
| `batata.core.auth.console.enabled` | `true` | Enable console API auth |
| `batata.core.auth.system.type` | `nacos` | Auth type: `nacos`, `ldap`, or `oauth` |
| `batata.core.auth.plugin.nacos.token.secret.key` | _(required)_ | JWT secret (Base64-encoded) |
| `batata.core.auth.plugin.nacos.token.expire.seconds` | `18000` | Token expiry (seconds) |
| `batata.core.auth.server.identity.key` | _(empty)_ | Server identity key for inter-node trust |
| `batata.core.auth.server.identity.value` | _(empty)_ | Server identity value |

#### Rate Limiting

| Key | Default | Description |
|-----|---------|-------------|
| `batata.ratelimit.enabled` | `false` | Enable API rate limiting |
| `batata.ratelimit.max_requests` | `10000` | Max requests per window |
| `batata.ratelimit.window_seconds` | `60` | Rate limit window (seconds) |
| `batata.ratelimit.auth.enabled` | `false` | Enable auth rate limiting |
| `batata.ratelimit.auth.max_attempts` | `5` | Max auth attempts per window |
| `batata.ratelimit.auth.lockout_seconds` | `300` | Auth lockout duration (seconds) |

#### Observability

| Key | Default | Description |
|-----|---------|-------------|
| `batata.otel.enabled` | `false` | Enable OpenTelemetry |
| `batata.otel.endpoint` | `http://localhost:4317` | OTLP gRPC endpoint |
| `batata.otel.service_name` | `batata` | Service name for traces |
| `batata.otel.sampling_ratio` | `0.1` | Sampling ratio (0.0 to 1.0) |
| `batata.otel.export_timeout_secs` | `10` | Export timeout (seconds) |
| `batata.logs.path` | `logs` | Log directory |
| `batata.logs.level` | `info` | Log level: trace/debug/info/warn/error |
| `batata.logs.console.enabled` | `true` | Enable console (stdout) logging |
| `batata.logs.file.enabled` | `true` | Enable file logging |

#### Cluster

| Key | Default | Description |
|-----|---------|-------------|
| `batata.member.list` | _(empty)_ | Comma-separated member addresses (ip:port) |
| `batata.core.member.lookup.type` | `file` | Member lookup type: `file` or `address-server` |

#### Config Module

| Key | Default | Description |
|-----|---------|-------------|
| `batata.config.retention.days` | `30` | Config history retention days |
| `batata.config.encryption.enabled` | `false` | Enable config encryption |
| `batata.config.encryption.plugin.type` | `aes-gcm` | Encryption algorithm |
| `batata.config.encryption.key` | _(empty)_ | Encryption key (32-byte Base64-encoded) |

#### Service Mesh (xDS)

| Key | Default | Description |
|-----|---------|-------------|
| `batata.mesh.xds.enabled` | `false` | Enable xDS protocol |
| `batata.mesh.xds.port` | `15010` | xDS gRPC server port |
| `batata.mesh.xds.server.id` | `batata-xds-server` | xDS server identifier |
| `batata.mesh.xds.sync.interval.ms` | `5000` | Service sync interval (ms) |
| `batata.mesh.xds.generate.listeners` | `true` | Generate LDS resources |
| `batata.mesh.xds.generate.routes` | `true` | Generate RDS resources |
| `batata.mesh.xds.default.listener.port` | `15001` | Default listener port |

#### Consul Compatibility

| Key | Default | Description |
|-----|---------|-------------|
| `batata.plugin.consul.enabled` | `false` | Enable Consul API compatibility |
| `batata.plugin.consul.port` | `8500` | Consul API port |
| `batata.plugin.consul.datacenter` | `dc1` | Consul datacenter name |
| `batata.plugin.consul.data_dir` | `data/consul_rocksdb` | Consul data directory |

#### AI / MCP Registry

| Key | Default | Description |
|-----|---------|-------------|
| `batata.ai.mcp.registry.enabled` | `false` | Enable MCP Registry |
| `batata.ai.mcp.registry.port` | `9080` | MCP Registry port |

### OpenTelemetry Configuration

Enable distributed tracing with OpenTelemetry OTLP export:

**application.yml:**
```yaml
batata.otel.enabled: true
batata.otel.endpoint: "http://localhost:4317"
batata.otel.service_name: "batata"
batata.otel.sampling_ratio: 1.0
batata.otel.export_timeout_secs: 10
```

**Environment Variables (override config file):**
```bash
export BATATA_OTEL_ENABLED=true
export BATATA_OTEL_ENDPOINT="http://localhost:4317"
export BATATA_OTEL_SERVICE_NAME="batata"
export BATATA_OTEL_SAMPLING_RATIO=1.0
export BATATA_OTEL_EXPORT_TIMEOUT_SECS=10
```

**Compatible backends:** Jaeger, Zipkin, Tempo, Datadog, Honeycomb, and any OTLP-compatible collector.

### Multi-datacenter Configuration

Enable locality-aware clustering with datacenter, region, and zone settings:

**Environment Variables:**
```bash
export BATATA_DATACENTER="dc1"
export BATATA_REGION="us-east"
export BATATA_ZONE="zone-a"
export BATATA_LOCALITY_WEIGHT=1.0
export BATATA_CROSS_DC_REPLICATION=true
export BATATA_CROSS_DC_SYNC_DELAY_SECS=1
export BATATA_REPLICATION_FACTOR=1
```

**Features:**
- Locality-aware member selection (local-first sync)
- Cross-datacenter replication with configurable delay
- Automatic datacenter topology discovery
- Region/zone hierarchy support

### xDS/Service Mesh Configuration

Enable xDS protocol support for Envoy proxies and Istio service mesh:

**application.yml:**
```yaml
batata.mesh.xds.enabled: true
batata.mesh.xds.port: 15010
batata.mesh.xds.server.id: "batata-xds-server"
batata.mesh.xds.sync.interval.ms: 5000
batata.mesh.xds.generate.listeners: true
batata.mesh.xds.generate.routes: true
batata.mesh.xds.default.listener.port: 15001
```

**Supported xDS Resources:**
- **EDS** (Endpoint Discovery Service) - Service endpoints
- **CDS** (Cluster Discovery Service) - Upstream clusters
- **LDS** (Listener Discovery Service) - Listeners configuration
- **RDS** (Route Discovery Service) - Route configuration
- **ADS** (Aggregated Discovery Service) - All resources via single stream

**Compatible with:** Envoy Proxy, Istio, and any xDS-compatible service mesh.

## API Reference

### Nacos Configuration API (V2)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v2/cs/config` | GET | Get configuration |
| `/nacos/v2/cs/config` | POST | Create/update configuration |
| `/nacos/v2/cs/config` | DELETE | Delete configuration |

### Nacos Configuration API (Console V3)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v3/console/cs/config` | GET | Get config detail (console) |
| `/v3/console/cs/config/list` | GET | List configs with pagination |
| `/v3/console/cs/config/export` | GET | Export configs (ZIP) |
| `/v3/console/cs/config/import` | POST | Import configs (ZIP) |

### Nacos Naming API (V2)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v2/ns/instance` | POST | Register instance |
| `/nacos/v2/ns/instance` | PUT | Update instance |
| `/nacos/v2/ns/instance` | DELETE | Deregister instance |
| `/nacos/v2/ns/instance/list` | GET | List service instances |

### Consul API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/agent/service/register` | PUT | Register service |
| `/v1/agent/service/deregister/{id}` | PUT | Deregister service |
| `/v1/health/service/{service}` | GET | Get service health |
| `/v1/catalog/services` | GET | List all services |
| `/v1/kv/{key}` | GET | Get KV value |
| `/v1/kv/{key}` | PUT | Set KV value |
| `/v1/kv/{key}` | DELETE | Delete KV value |
| `/v1/kv/export` | GET | Export KV (JSON) |
| `/v1/kv/import` | PUT | Import KV (JSON) |

### Console API (V3)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v3/console/namespace/list` | GET | List namespaces |
| `/v3/console/namespace` | POST | Create namespace |
| `/v3/console/core/cluster/nodes` | GET | List cluster nodes |
| `/v3/console/core/cluster/health` | GET | Get cluster health |

## Configuration Import/Export

### Nacos Format (ZIP)

```bash
# Export configurations
curl -O "http://localhost:8081/v3/console/cs/config/export?namespaceId=public"

# Import with conflict policy (ABORT, SKIP, OVERWRITE)
curl -X POST "http://localhost:8081/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE" \
  -F "file=@export.zip"
```

### Consul Format (JSON)

```bash
# Export KV store
curl "http://localhost:8848/v1/kv/export?namespaceId=public" > configs.json

# Import KV store
curl -X PUT "http://localhost:8848/v1/kv/import?namespaceId=public" \
  -H "Content-Type: application/json" \
  -d @configs.json
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Batata Server                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐  │
│  │   HTTP API    │  │   gRPC API    │  │    Console API      │  │
│  │   (Actix)     │  │   (Tonic)     │  │    (Actix)          │  │
│  │   Port:8848   │  │   Port:9848   │  │    Port:8081        │  │
│  └───────┬───────┘  └───────┬───────┘  └──────────┬──────────┘  │
│          │                  │                      │             │
│  ┌───────▼──────────────────▼──────────────────────▼───────────┐ │
│  │                    Service Layer                             │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌───────────────┐  │ │
│  │  │  Config  │  │  Naming  │  │  Auth  │  │   Namespace   │  │ │
│  │  │ Service  │  │ Service  │  │Service │  │   Service     │  │ │
│  │  └──────────┘  └──────────┘  └────────┘  └───────────────┘  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Storage Layer                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │ │
│  │  │ MySQL/PgSQL │  │   RocksDB   │  │    Moka Cache       │  │ │
│  │  │  (SeaORM)   │  │ (Raft Log)  │  │   (In-Memory)       │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Cluster Layer                             │ │
│  │  ┌─────────────────────────────────────────────────────────┐│ │
│  │  │               Raft Consensus (OpenRaft)                 ││ │
│  │  │                    Port: 9849                           ││ │
│  │  └─────────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Deployment

### Deployment Modes

Batata supports four deployment modes, controlled by `batata.deployment.type` or the `-d` CLI flag:

| Mode | Flag | Description |
|------|------|-------------|
| `merged` (default) | `-d merged` | Console (8081) + Main Server (8848) in same process |
| `server` | `-d server` | Main Server only (8848) + gRPC (9848/9849), no console |
| `console` | `-d console` | Console only (8081), connects to remote server |
| `serverWithMcp` | `-d serverWithMcp` | Main Server + MCP Registry (9080) |

### Startup Scripts

Convenience scripts are provided in the `scripts/` directory:

```bash
# Start in embedded mode (no database required)
./scripts/start-embedded.sh

# Start server-only (main server on 8848, no console)
./scripts/start-server.sh

# Start console-only (connects to a remote server)
./scripts/start-console.sh [server_addr]

# Start with MySQL database
./scripts/start-mysql.sh [db_url]

# Initialize admin user (required on first startup)
./scripts/init-admin.sh [username] [password] [server_url]

# Test console/server route separation
./scripts/test-separation.sh
```

### Standalone Mode

```bash
# Run directly
cargo run --release -p batata-server

# Or with explicit standalone flag
./target/release/batata-server --batata.standalone=true
```

### Cluster Mode

1. Configure `conf/cluster.conf` with member addresses:
   ```
   192.168.1.1:8848
   192.168.1.2:8848
   192.168.1.3:8848
   ```

2. Update `conf/application.yml`:
   ```yaml
   batata.standalone: false
   #batata.member.list: "192.168.1.10:8848,192.168.1.11:8848,192.168.1.12:8848"
   ```

3. Start all nodes

### Docker

```bash
# Build image
docker build -t batata:latest .

# Run container
docker run -d \
  -p 8848:8848 \
  -p 8081:8081 \
  -p 9848:9848 \
  -v $(pwd)/conf:/app/conf \
  batata:latest
```

### Docker Compose

```bash
docker-compose up -d
```

## Database Setup

Batata supports MySQL, PostgreSQL, and embedded RocksDB (no external database).

### MySQL Setup

```bash
mysql -u root -p < conf/mysql-schema.sql
mysql -u root -p < conf/consul-mysql-schema.sql
```

Configure in `conf/application.yml`:
```yaml
batata.sql.init.platform: mysql
batata.db.url: "mysql://user:password@localhost:3306/batata"
```

### PostgreSQL Setup

```bash
psql -U user -d batata -f conf/postgresql-schema.sql
```

Configure in `conf/application.yml`:
```yaml
batata.sql.init.platform: postgresql
batata.db.url: "postgres://user:password@localhost:5432/batata"
```

### Embedded Mode (No Database)

For development or single-node deployments, no external database is required:

```yaml
batata.sql.init.platform:
batata.standalone: true
```

Or start with: `--batata.sql.init.platform=embedded`

### Database Migration

Instead of manually importing SQL schema files, you can enable automatic database migration via the `batata-migration` crate:

```yaml
batata.db.migration.enabled: true
```

When enabled, Batata will automatically create or update the database schema on startup. This is the recommended approach for new deployments and upgrades.

### Generate Database Entities

```bash
# For MySQL:
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both

# For PostgreSQL:
sea-orm-cli generate entity \
  -u "postgres://user:pass@localhost:5432/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

## Development

### Build Commands

```bash
# Build all crates
cargo build

# Build server only (faster)
cargo build -p batata-server

# Release build (optimized)
cargo build --release -p batata-server

# Run tests (all crates)
cargo test --workspace

# Run specific crate tests
cargo test -p batata-server

# Run benchmarks
cargo bench -p batata-server

# Format code
cargo fmt --all

# Lint with Clippy
cargo clippy --workspace

# Generate documentation
cargo doc --workspace --open
```

### Project Statistics

- **~50,000+ lines** of Rust code
- **19 internal crates** in workspace
- **333 unit tests** with comprehensive coverage
- **3 benchmark suites** for performance testing

## Client SDKs

Batata is compatible with existing Nacos and Consul client SDKs:

### Java (Nacos SDK)

```java
Properties properties = new Properties();
properties.setProperty("serverAddr", "localhost:8848");
ConfigService configService = NacosFactory.createConfigService(properties);
String config = configService.getConfig("dataId", "group", 5000);
```

### Go (Nacos SDK)

```go
client, _ := clients.NewConfigClient(
    vo.NacosClientParam{
        ServerConfigs: []constant.ServerConfig{
            {IpAddr: "localhost", Port: 8848},
        },
    },
)
config, _ := client.GetConfig(vo.ConfigParam{DataId: "dataId", Group: "group"})
```

### Go (Consul SDK)

```go
client, _ := api.NewClient(api.DefaultConfig())
kv, _, _ := client.KV().Get("key", nil)
```

### Python (Nacos SDK)

```python
import nacos
client = nacos.NacosClient(server_addresses="localhost:8848")
config = client.get_config("dataId", "group")
```

## Monitoring

Prometheus metrics available at `/nacos/actuator/prometheus`:

```
# HELP batata_http_requests_total Total HTTP requests
# TYPE batata_http_requests_total counter
batata_http_requests_total{method="GET",path="/nacos/v2/cs/config"} 1234

# HELP batata_config_count Current configuration count
# TYPE batata_config_count gauge
batata_config_count{namespace="public"} 100

# HELP batata_service_count Current service count
# TYPE batata_service_count gauge
batata_service_count{namespace="public"} 50

# HELP batata_cluster_member_count Cluster member count
# TYPE batata_cluster_member_count gauge
batata_cluster_member_count 3
```

## Roadmap

- [x] Persistent service instances (database storage)
- [x] Distro protocol enhancement (AP mode)
- [x] Config encryption at rest (AES-128-CBC)
- [x] OpenTelemetry integration (OTLP export)
- [x] Multi-datacenter support (locality-aware sync)
- [x] Service Mesh xDS protocol (EDS, CDS, LDS, RDS, ADS)
- [x] AI Integration (MCP Server Registry, A2A Agent Registry)
- [x] Kubernetes sync (bidirectional service sync)
- [x] LDAP authentication
- [x] OAuth2/OIDC authentication (Google, GitHub, Microsoft)
- [x] Distributed locking (Raft-based)
- [x] DNS-based service discovery
- [x] Gray/Beta release API
- [x] Operation audit logs
- [x] Database auto-migration
- [ ] Kubernetes Operator
- [ ] Web UI (backend API only, use Nacos UI or custom frontend)

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the Apache-2.0 License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Nacos](https://nacos.io/) - For the original design and API specification
- [Consul](https://www.consul.io/) - For the KV store and service discovery API design
- [OpenRaft](https://github.com/datafuselabs/openraft) - For the Raft consensus implementation
- [SeaORM](https://www.sea-ql.org/SeaORM/) - For the async ORM framework
- [Actix-web](https://actix.rs/) - For the high-performance web framework
- [Tonic](https://github.com/hyperium/tonic) - For the gRPC implementation
