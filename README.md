# Batata

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/easynet-cn/batata/ci.yml?branch=main)](https://github.com/easynet-cn/batata/actions)

**Batata** is a high-performance, Rust-based implementation of a dynamic service discovery, configuration management, and service management platform. It is fully compatible with [Nacos](https://nacos.io/) and [Consul](https://www.consul.io/) APIs, making it an ideal drop-in replacement for cloud-native applications.

[中文文档](README_CN.md)

## Features

### Core Capabilities

- **Configuration Management** - Centralized, dynamic configuration with real-time updates
- **Service Discovery** - Service registration, discovery, and health checking
- **Namespace Isolation** - Multi-tenant support with namespace-based resource isolation
- **Cluster Mode** - High availability with Raft consensus protocol

### API Compatibility

- **Nacos API** - Full compatibility with Nacos v1 and v3 APIs
- **Consul API** - Compatible with Consul Agent, Health, Catalog, KV, and ACL APIs
- **gRPC Support** - High-performance bidirectional streaming for SDK clients

### Advanced Features

- **Config Import/Export** - Batch configuration migration in Nacos ZIP or Consul JSON format
- **Gray/Beta Release** - Gradual configuration rollout with gray release support
- **Authentication & Authorization** - JWT-based authentication with RBAC permission model
- **Rate Limiting** - Configurable rate limiting for API protection
- **Circuit Breaker** - Resilience patterns for fault tolerance
- **Metrics & Observability** - Prometheus-compatible metrics endpoint

### Performance

- **Fast Startup** - ~1-2 seconds vs 30-60 seconds for Java-based Nacos
- **Low Memory** - ~50-100MB vs 1-2GB for Java-based Nacos
- **High Throughput** - Optimized async I/O with Tokio runtime
- **Efficient Storage** - RocksDB for persistent storage with tuned caching

## Project Structure

Batata uses a multi-crate workspace architecture for modularity and maintainability:

```
batata/
├── src/                          # Main application
│   ├── api/                      # API handlers (HTTP, gRPC, Consul)
│   ├── auth/                     # Authentication & authorization
│   ├── config/                   # Configuration models
│   ├── console/                  # Web console API
│   ├── core/                     # Core business logic & Raft
│   ├── entity/                   # Database entities (re-exports)
│   ├── middleware/               # HTTP middleware
│   ├── model/                    # Data models
│   └── service/                  # Business services
├── crates/
│   ├── batata-api/               # API definitions & gRPC proto
│   ├── batata-auth/              # Authentication module
│   ├── batata-client/            # Client SDK
│   ├── batata-common/            # Common utilities
│   ├── batata-config/            # Configuration service
│   ├── batata-console/           # Console service
│   ├── batata-consistency/       # Consistency protocols
│   ├── batata-core/              # Core abstractions & cluster
│   ├── batata-naming/            # Naming/discovery service
│   ├── batata-persistence/       # Database entities (SeaORM)
│   ├── batata-plugin/            # Plugin interfaces
│   ├── batata-plugin-consul/     # Consul compatibility plugin
│   └── batata-server/            # Server binary
├── conf/                         # Configuration files
├── proto/                        # Protocol buffer definitions
├── tests/                        # Integration tests
└── benches/                      # Benchmarks
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

# Configure application
cp conf/application.yml.example conf/application.yml
# Edit conf/application.yml with your database credentials

# Build and run
cargo build --release
./target/release/batata
```

### Default Ports

| Port | Service | Description |
|------|---------|-------------|
| 8848 | Main HTTP API | Nacos-compatible API |
| 8081 | Console HTTP API | Web management console |
| 9848 | SDK gRPC | Client SDK communication |
| 9849 | Cluster gRPC | Inter-node communication |

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
cargo run --release

# Or with environment variables
NACOS_STANDALONE=true ./target/release/batata
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
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench

# Format code
cargo fmt

# Lint with Clippy
cargo clippy

# Generate documentation
cargo doc --open
```

### Generate Database Entities

```bash
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

### Project Statistics

- **~47,500+ lines** of Rust code
- **13 internal crates** in workspace
- **178 unit tests** with comprehensive coverage
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

- [ ] Kubernetes Operator
- [ ] Web UI Console
- [ ] OpenTelemetry integration
- [ ] Config encryption at rest
- [ ] Multi-datacenter support

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
