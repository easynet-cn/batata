# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
```

### PostgreSQL Setup
```bash
psql -U user -d batata -f conf/postgresql-schema.sql
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

Batata uses a multi-crate workspace architecture with 13 internal crates. See `docs/ARCHITECTURE.md` for detailed architecture documentation.

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
│   ├── batata-console/              # Management console backend
│   ├── batata-client/               # Rust SDK for clients
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
- `v1`: Legacy endpoints
- `v3`: Current endpoints

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
├── batata-auth (JWT, RBAC)
├── batata-config (config service)
├── batata-naming (service discovery)
├── batata-console (console backend)
├── batata-core (cluster, connections)
├── batata-plugin-consul (Consul API)
└── batata-persistence (database)
```
