# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Rules

**IMPORTANT**: These rules MUST be followed at all times:

1. **Language**: All documentation, code comments, commit messages, and any written content MUST be in **English**.

2. **Task Tracking**: Always provide **honest and accurate** feedback on task completion status. Never mark a task as completed unless it is fully implemented and tested. Update `docs/TASK_TRACKER.md` with real progress.

3. **Code Quality**: Follow Rust best practices, run `cargo fmt` and `cargo clippy` before considering a task complete.

4. **Testing**: Write tests for new functionality. A feature is not complete without tests.

5. **API Compatibility**: Batata follows **Nacos 3.x direction**, which focuses on **V2 and V3 APIs**. **V1 API is NOT supported**. Do not implement V1 endpoints or heartbeat-based HTTP APIs. Modern clients should use V2 HTTP APIs or gRPC for service discovery and configuration management.

6. **No Frontend Work**: Batata is a backend-only implementation. No frontend/UI work is needed. The console API provides JSON responses for third-party UI integration.

7. **Reference Implementation**: The original Nacos project is located at `~/work/github/easynet-cn/nacos`. When uncertain about behavior or API contracts, always check the original Nacos source code first to ensure consistency.

## Nacos 3.x Architecture Rules

Batata follows the **Nacos 3.x architecture** where Server and Console are **separated**. All behavior MUST be consistent with the original Nacos 3.x implementation.

### Server/Console Separation

| Component | Default Port | Context Path | Role |
|-----------|-------------|--------------|------|
| Main HTTP Server | 8848 | `/nacos` | Core V2/V3 API for SDK clients |
| Console HTTP Server | 8081 | _(empty)_ | Admin console V3 API |
| SDK gRPC Server | 9848 (8848+1000) | - | Client SDK communication (config, naming) |
| Cluster gRPC Server | 9849 (8848+1001) | - | Inter-node cluster communication |

### Authentication Architecture

- **Auth routes MUST be registered on BOTH Main Server and Console Server**. The Nacos 3.x SDK authenticates via HTTP login at the main server address (`http://server:8848/nacos/v3/auth/user/login`) before sending gRPC requests.
- **SDK authentication flow**: HTTP login (get JWT token) → gRPC requests (with token in headers). If HTTP login fails, gRPC requests will fail with `UNAUTHENTICATED`.
- Auth login endpoint: `POST /v3/auth/user/login` (under each server's context path).

### SDK Client Communication Pattern

1. SDK connects to Main Server port (8848) via HTTP for authentication
2. SDK connects to gRPC port (9848) for config/naming operations
3. All gRPC requests carry the JWT token obtained from HTTP login
4. Config operations: `ConfigPublishRequest`, `ConfigQueryRequest`, `ConfigRemoveRequest` via gRPC
5. Naming operations: `InstanceRequest`, `ServiceQueryRequest`, `SubscribeServiceRequest` via gRPC

### API Route Distribution

- **Main Server (8848)**: V2 Open API (`/nacos/v2/cs/*`, `/nacos/v2/ns/*`), V3 Admin API (`/nacos/v3/admin/*`), V3 Client API (`/nacos/v3/client/*`), Auth API (`/nacos/v3/auth/*`)
- **Console Server (8081)**: V3 Console API (`/v3/console/*`), V2 Console API (`/v2/console/*`), Auth API (`/v3/auth/*`)

## Project Overview

Batata is a Rust implementation of a Nacos-compatible service discovery, configuration management, and service management platform. It provides HTTP and gRPC APIs for configuration management, service discovery, and cluster coordination. It also supports Consul API compatibility.

## Build Commands

```bash
# Build all crates
cargo build

# Build server only (faster)
cargo build -p batata-server

# Release build (optimized, with LTO)
cargo build --release -p batata-server

# Run the application
cargo run -p batata-server

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
```

## Database Setup

Batata supports both MySQL and PostgreSQL databases.

### MySQL Setup
```bash
mysql -u user -p database < conf/mysql-schema.sql
mysql -u user -p database < conf/consul-mysql-schema.sql
mysql -u user -p database < conf/apollo-mysql-schema.sql
```

### PostgreSQL Setup
```bash
psql -U user -d batata -f conf/postgresql-schema.sql
psql -U user -d batata -f conf/consul-postgresql-schema.sql
psql -U user -d batata -f conf/apollo-postgresql-schema.sql
```

### Configuration
Set the database URL in `conf/application.yml`:
```yaml
# For MySQL:
db.url: mysql://user:password@localhost:3306/batata

# For PostgreSQL:
db.url: postgres://user:password@localhost:5432/batata
```

### Generate SeaORM Entities
```bash
# For MySQL:
sea-orm-cli generate entity -u "mysql://user:pass@localhost:3306/batata" -o ./crates/batata-persistence/src/entity --with-serde both

# For PostgreSQL:
sea-orm-cli generate entity -u "postgres://user:pass@localhost:5432/batata" -o ./crates/batata-persistence/src/entity --with-serde both
```

## Architecture

Batata uses a multi-crate workspace architecture with 15 internal crates. See `docs/ARCHITECTURE.md` for detailed architecture documentation.

### Entry Point and Servers
The application (`crates/batata-server/src/main.rs`) starts multiple concurrent servers:
- **Main HTTP Server** (port 8848): Core API endpoints under `/nacos`
- **Console HTTP Server** (port 8081): Management console endpoints
- **SDK gRPC Server** (port 9848): Client SDK communication
- **Cluster gRPC Server** (port 9849): Inter-node cluster communication

### Crate Structure
```
batata/
├── Cargo.toml                       # Workspace manifest
├── crates/
│   ├── batata-common/               # Shared types, traits, utilities
│   ├── batata-api/                  # gRPC/HTTP API definitions
│   ├── batata-persistence/          # Database entities (SeaORM)
│   ├── batata-auth/                 # Authentication and authorization
│   ├── batata-consistency/          # Raft consensus protocol
│   ├── batata-core/                 # Cluster and connection management
│   ├── batata-config/               # Configuration management service
│   ├── batata-naming/               # Service discovery service
│   ├── batata-plugin/               # Plugin SPI definitions
│   ├── batata-plugin-consul/        # Consul compatibility plugin
│   ├── batata-plugin-apollo/        # Apollo Config compatibility plugin
│   ├── batata-console/              # Management console backend
│   ├── batata-client/               # Rust SDK for clients
│   ├── batata-mesh/                 # Service mesh (xDS, Istio MCP)
│   └── batata-server/               # Main server application
│       ├── src/
│       │   ├── api/                 # API handlers (HTTP, gRPC, Consul)
│       │   ├── auth/                # Auth HTTP handlers (v1/, v3/)
│       │   ├── config/              # Config models (re-exports)
│       │   ├── console/             # Console API handlers
│       │   ├── middleware/          # HTTP middleware (auth)
│       │   ├── model/               # AppState, Configuration
│       │   ├── service/             # gRPC handlers
│       │   ├── startup/             # Server initialization
│       │   ├── lib.rs               # Library exports
│       │   └── main.rs              # Entry point
│       ├── tests/                   # Integration tests
│       └── benches/                 # Performance benchmarks
├── conf/                            # Configuration files
├── docs/                            # Documentation
└── proto/                           # Protocol buffer definitions
```

### Key Components

**AppState** (`crates/batata-server/src/model/common.rs`): Central application context holding configuration, database connection, server member manager, and console data source.

**Authentication** (`crates/batata-auth/`): JWT-based with RBAC. The `secured!` macro validates permissions. Security context built with `Secured` builder pattern.

**gRPC Handlers** (`crates/batata-api/`): Implement `PayloadHandler` trait and register in `HandlerRegistry` for message routing.

**Configuration**: Loaded from `conf/application.yml` with environment variable overrides via clap.

### API Versioning
- `v2`: Nacos V2 Open API (fully implemented)
- `v3`: Nacos V3 Console API (fully implemented)
- `v1`: **NOT SUPPORTED** - following Nacos 3.x direction

### Permission Model
Resource format: `namespace_id:group_id:resource_type/resource_name`
- Actions: `Read` ("r"), `Write` ("w")
- Supports wildcards and regex matching

## Configuration

Main config: `conf/application.yml`

Key settings:
- `nacos.server.main.port`: Main HTTP port (default 8848)
- `nacos.console.port`: Console port (default 8081)
- `db.url`: Database connection URL (MySQL or PostgreSQL)
- `nacos.core.auth.plugin.nacos.token.secret.key`: JWT secret (Base64)

## Protocol Buffers

gRPC definitions in `proto/nacos_grpc_service.proto`. Build script (`crates/batata-api/build.rs`) compiles to `crates/batata-api/src/grpc/`.

## Key Patterns

- **Error handling**: Custom `BatataError` enum with `AppError` wrapper for actix compatibility
- **Async**: Tokio runtime with `pin-project-lite` for futures
- **Database**: SeaORM for async database access (MySQL/PostgreSQL)
- **Concurrency**: `Arc<DashMap>` for thread-safe shared state
- **Logging**: Structured logging via `tracing` with Bunyan JSON formatter
- **Caching**: `moka` for async caching
- **Consensus**: OpenRaft for Raft protocol, RocksDB for log storage

## Crate Dependencies

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
├── batata-plugin-apollo (Apollo Config API)
└── batata-persistence (database)
```
