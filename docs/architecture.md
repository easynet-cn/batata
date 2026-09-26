# Architecture

## Project Structure

Batata uses a multi-crate workspace architecture with **24 internal crates** for modularity and maintainability:

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
│   ├── batata-plugin-apollo/     # Apollo compatibility plugin (Config/Admin/OpenAPI)
│   ├── batata-plugin-cloud/      # Prometheus & Kubernetes plugin
│   ├── batata-console/           # Console backend service
│   ├── batata-client/            # Client SDK
│   ├── batata-maintainer-client/ # Maintainer client utilities
│   ├── batata-consul-client/     # Consul client utilities
│   ├── batata-ai/                # AI integration (MCP/A2A)
│   ├── batata-mesh/              # Service mesh (xDS, Istio MCP)
│   ├── batata-copilot/           # AI copilot / operations assistant
│   ├── batata-visibility/        # Metrics & observability
│   ├── batata-integration-tests/ # Cross-crate integration tests
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

## Crate Dependencies

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

## Runtime Architecture

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

## Port Separation

The runtime exposes independent port families. This separation is intentional:

| Port family | Port(s) | Serves |
|---|---|---|
| Console / UI | `8081` | Web console (frontend static assets + console APIs) |
| SDK HTTP | `8848` | Nacos-compatible REST API for SDK clients |
| SDK gRPC | `9848` (main + 1000) | gRPC for SDK clients |
| Cluster gRPC | `9849` (main + 1001) | Inter-node / Raft communication |

The console port serves the UI only; it does not expose the Nacos-compatible data APIs.
See [Configuration](configuration.md) for the relevant keys.
