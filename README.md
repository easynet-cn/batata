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
- **Apollo API** - Full compatibility with Apollo Config client SDK and Open API
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
| Apollo API Compatibility | ❌ | ✅ | **Extra** |
| **Observability** | | | |
| Prometheus Metrics | ✅ | ✅ | Full |
| Health Endpoint | ✅ | ✅ | Full |
| OpenTelemetry | ✅ | ✅ | Full |
| Operation Audit Logs | ✅ | ✅ | Full |

### Batata Exclusive Features

| Feature | Description |
|---------|-------------|
| **Consul API Compatibility** | Full support for Agent, Health, Catalog, KV, ACL APIs |
| **Apollo API Compatibility** | Full support for Apollo client SDK and Open API |
| **PostgreSQL Support** | In addition to MySQL |
| **Consul JSON Import/Export** | Migration support for Consul KV store |
| **Built-in Circuit Breaker** | Resilience pattern for cluster health checks |
| **OpenTelemetry Tracing** | OTLP export for distributed tracing (Jaeger, Zipkin, etc.) |
| **Multi-datacenter Support** | Locality-aware replication with local-first sync |
| **Service Mesh (xDS)** | EDS, CDS, LDS, RDS, ADS protocol support |
| **AI Integration** | MCP Server Registry and A2A Agent Registry |
| **Distributed Lock** | Raft-based distributed locking mechanism |
| **Kubernetes Sync** | Bidirectional service sync with Kubernetes |

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

Batata uses a multi-crate workspace architecture for modularity and maintainability:

```
batata/
├── crates/
│   ├── batata-common/            # Common utilities & types
│   ├── batata-api/               # API definitions & gRPC proto
│   ├── batata-persistence/       # Database entities (SeaORM)
│   ├── batata-auth/              # Authentication & authorization
│   ├── batata-consistency/       # Raft consensus protocol
│   ├── batata-core/              # Core abstractions & cluster
│   ├── batata-config/            # Configuration service
│   ├── batata-naming/            # Service discovery
│   ├── batata-plugin/            # Plugin interfaces
│   ├── batata-plugin-consul/     # Consul compatibility plugin
│   ├── batata-plugin-apollo/     # Apollo compatibility plugin
│   ├── batata-console/           # Console backend service
│   ├── batata-client/            # Client SDK
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
├── batata-mesh (xDS, Istio MCP)
├── batata-plugin-consul (Consul API)
├── batata-plugin-apollo (Apollo API)
└── batata-persistence (database)
```

## Quick Start

### Prerequisites

- Rust 1.85+ (Edition 2024)
- MySQL 5.7+ or 8.0+

### Installation

```bash
# Clone the repository
git clone https://github.com/easynet-cn/batata.git
cd batata

# Initialize database
mysql -u root -p < conf/mysql-schema.sql
mysql -u root -p < conf/consul-mysql-schema.sql
mysql -u root -p < conf/apollo-mysql-schema.sql

# Configure application
cp conf/application.yml.example conf/application.yml
# Edit conf/application.yml with your database credentials

# Build and run
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

```yaml
nacos:
  server:
    main:
      port: 8848
    context-path: /nacos
  console:
    port: 8081
  core:
    auth:
      enabled: true
      plugin:
        nacos:
          token:
            secret:
              key: "your-base64-encoded-secret-key"
      token:
        expire:
          seconds: 18000
  standalone: true  # Set to false for cluster mode

db:
  url: "mysql://user:password@localhost:3306/batata"

cluster:
  member:
    lookup:
      type: standalone  # standalone, file, or address-server
```

### Environment Variables

```bash
export NACOS_DB_URL="mysql://user:pass@localhost:3306/batata"
export NACOS_SERVER_MAIN_PORT=8848
export NACOS_CORE_AUTH_ENABLED=true
export RUST_LOG=info
```

### OpenTelemetry Configuration

Enable distributed tracing with OpenTelemetry OTLP export:

**application.yml:**
```yaml
nacos:
  otel:
    enabled: true
    endpoint: "http://localhost:4317"  # OTLP gRPC endpoint
    service_name: "batata"
    sampling_ratio: 1.0  # 0.0 to 1.0
    export_timeout_secs: 10
```

**Environment Variables (override config file):**
```bash
export OTEL_ENABLED=true
export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4317"
export OTEL_SERVICE_NAME="batata"
export OTEL_SAMPLING_RATIO=1.0
export OTEL_EXPORT_TIMEOUT_SECS=10
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
xds:
  enabled: true
  port: 15010                    # xDS gRPC server port
  server_id: "batata-xds-server"
  sync_interval_ms: 5000         # Service sync interval
  generate_listeners: true       # Generate LDS resources
  generate_routes: true          # Generate RDS resources
  default_listener_port: 15001   # Default listener port for generated resources
```

**Supported xDS Resources:**
- **EDS** (Endpoint Discovery Service) - Service endpoints
- **CDS** (Cluster Discovery Service) - Upstream clusters
- **LDS** (Listener Discovery Service) - Listeners configuration
- **RDS** (Route Discovery Service) - Route configuration
- **ADS** (Aggregated Discovery Service) - All resources via single stream

**Compatible with:** Envoy Proxy, Istio, and any xDS-compatible service mesh.

## API Reference

### Nacos Configuration API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v1/cs/configs` | GET | Get configuration |
| `/nacos/v1/cs/configs` | POST | Create/update configuration |
| `/nacos/v1/cs/configs` | DELETE | Delete configuration |
| `/nacos/v3/console/cs/config` | GET | Get config detail (console) |
| `/nacos/v3/console/cs/config/list` | GET | List configs with pagination |
| `/nacos/v3/console/cs/config/export` | GET | Export configs (ZIP) |
| `/nacos/v3/console/cs/config/import` | POST | Import configs (ZIP) |

### Nacos Naming API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v1/ns/instance` | POST | Register instance |
| `/nacos/v1/ns/instance` | PUT | Update instance |
| `/nacos/v1/ns/instance` | DELETE | Deregister instance |
| `/nacos/v1/ns/instance/list` | GET | List service instances |
| `/nacos/v1/ns/instance/beat` | PUT | Send heartbeat |

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

### Apollo API

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/configs/{appId}/{cluster}/{namespace}` | GET | Get configuration |
| `/configfiles/{appId}/{cluster}/{namespace}` | GET | Get config as text |
| `/configfiles/json/{appId}/{cluster}/{namespace}` | GET | Get config as JSON |
| `/notifications/v2` | GET | Long polling for updates |
| `/openapi/v1/apps` | GET | List all apps |
| `/openapi/v1/apps/{appId}/envclusters` | GET | Get env clusters |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces` | GET | List namespaces |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | GET | List items |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` | POST | Create item |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases` | POST | Publish release |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock` | POST | Acquire lock |
| `/openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray` | POST | Create gray release |
| `/openapi/v1/apps/{appId}/accesskeys` | POST | Create access key |
| `/openapi/v1/metrics/clients` | GET | Get client metrics |

### Console API (v3)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/nacos/v3/console/namespace/list` | GET | List namespaces |
| `/nacos/v3/console/namespace` | POST | Create namespace |
| `/nacos/v3/console/core/cluster/nodes` | GET | List cluster nodes |
| `/nacos/v3/console/core/cluster/health` | GET | Get cluster health |

## Configuration Import/Export

### Nacos Format (ZIP)

```bash
# Export configurations
curl -O "http://localhost:8848/nacos/v3/console/cs/config/export?namespaceId=public"

# Import with conflict policy (ABORT, SKIP, OVERWRITE)
curl -X POST "http://localhost:8848/nacos/v3/console/cs/config/import?namespaceId=public&policy=OVERWRITE" \
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
│  │  │    MySQL    │  │   RocksDB   │  │    Moka Cache       │  │ │
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

### Standalone Mode

```bash
# Run directly
cargo run --release -p batata-server

# Or with environment variables
NACOS_STANDALONE=true ./target/release/batata-server
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
   nacos:
     standalone: false
   cluster:
     member:
       lookup:
         type: file
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

### Generate Database Entities

```bash
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

### Project Statistics

- **~50,000+ lines** of Rust code
- **14 internal crates** in workspace
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

### Java (Apollo SDK)

```java
Config config = ConfigService.getAppConfig();
String value = config.getProperty("key", "defaultValue");

// With custom namespace
Config applicationConfig = ConfigService.getConfig("application");
```

### Go (Apollo SDK)

```go
client, _ := agollo.Start(&config.AppConfig{
    AppID:         "your-app-id",
    Cluster:       "default",
    NamespaceName: "application",
    IP:            "localhost:8848",
})
value := client.GetValue("key")
```

## Monitoring

Prometheus metrics available at `/nacos/actuator/prometheus`:

```
# HELP batata_http_requests_total Total HTTP requests
# TYPE batata_http_requests_total counter
batata_http_requests_total{method="GET",path="/nacos/v1/cs/configs"} 1234

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
- [x] Apollo Config API compatibility
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
- [Apollo](https://www.apolloconfig.com/) - For the configuration management API design
- [OpenRaft](https://github.com/datafuselabs/openraft) - For the Raft consensus implementation
- [SeaORM](https://www.sea-ql.org/SeaORM/) - For the async ORM framework
- [Actix-web](https://actix.rs/) - For the high-performance web framework
- [Tonic](https://github.com/hyperium/tonic) - For the gRPC implementation
