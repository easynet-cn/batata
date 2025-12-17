# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Batata is a Rust implementation of a Nacos-compatible service discovery, configuration management, and service management platform. It provides HTTP and gRPC APIs for configuration management, service discovery, and cluster coordination.

## Build Commands

```bash
# Development build
cargo build

# Release build (optimized, with LTO)
cargo build --release

# Run the application
cargo run

# Run tests
cargo test

# Format code
cargo fmt

# Lint with Clippy
cargo clippy
```

## Database Setup

Initialize the MySQL database using the schema file:
```bash
mysql -u user -p database < conf/mysql-schema.sql
```

Generate SeaORM entities:
```bash
sea-orm-cli generate entity -u "mysql://user:pass@localhost:3306/batata" -o ./src/entity --with-serde both
```

## Architecture

### Entry Point and Servers
The application (`main.rs`) starts multiple concurrent servers:
- **Main HTTP Server** (port 8848): Core API endpoints under `/nacos`
- **Console HTTP Server** (port 8081): Management console endpoints
- **SDK gRPC Server**: Client SDK communication
- **Cluster gRPC Server**: Inter-node cluster communication

### Source Structure
```
src/
├── main.rs              # Entry point, server initialization
├── lib.rs               # Module definitions, security context
├── error.rs             # Error types with numeric codes
├── api/                 # gRPC service definitions and models
├── auth/                # Authentication (JWT, RBAC)
│   ├── v1/, v3/         # API version endpoints
│   └── service/         # User, role, permission services
├── config/              # Configuration management models
├── console/v3/          # Console API endpoints
├── core/service/        # Cluster and connection management
├── entity/              # SeaORM database entities (auto-generated)
├── middleware/          # HTTP middleware (auth)
├── model/               # Data models, AppState, Configuration
├── service/             # Business logic (config, namespace, RPC)
└── naming/              # Service discovery (placeholder)
```

### Key Components

**AppState**: Central application context holding configuration, database connection, and cluster manager.

**Authentication**: JWT-based with RBAC. The `secured!` macro validates permissions. Security context built with `Secured` builder pattern.

**gRPC Handlers**: Implement `PayloadHandler` trait and register in `HandlerRegistry` for message routing.

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
- `db.url`: MySQL connection URL
- `nacos.core.auth.plugin.nacos.token.secret.key`: JWT secret (Base64)

## Protocol Buffers

gRPC definitions in `proto/nacos_grpc_service.proto`. Build script (`build.rs`) compiles to `src/api/grpc/`.

## Key Patterns

- **Error handling**: Custom `BatataError` enum with `AppError` wrapper for actix compatibility
- **Async**: Tokio runtime with `pin-project-lite` for futures
- **Database**: SeaORM for async MySQL access
- **Concurrency**: `Arc<DashMap>` for thread-safe shared state
- **Logging**: Structured logging via `tracing` with Bunyan JSON formatter
