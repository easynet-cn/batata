# Batata Multi-Crate Architecture

This document describes the multi-crate architecture of Batata, a Rust implementation of Nacos-compatible service discovery and configuration management platform.

## Table of Contents

1. [Overview](#overview)
2. [Crate Structure](#crate-structure)
3. [Dependency Graph](#dependency-graph)
4. [Implementation Details](#implementation-details)
5. [Trait-Based Dependency Injection](#trait-based-dependency-injection)

---

## Overview

Batata uses a multi-crate workspace architecture for modularity and maintainability. This architecture provides:

- Faster compilation times through parallel crate compilation
- Clear dependency boundaries between modules
- Better testability with isolated unit tests
- Flexible deployment options

The project consists of **15 internal crates** following a layered architecture similar to Java Nacos while leveraging Rust's trait system for dependency injection.

---

## Crate Structure

```
batata/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── batata-common/         # Shared types, traits, utilities
│   ├── batata-api/            # gRPC/HTTP API definitions
│   ├── batata-persistence/    # Database entities (SeaORM)
│   ├── batata-auth/           # Authentication and authorization
│   ├── batata-consistency/    # Raft consensus protocol
│   ├── batata-core/           # Cluster and connection management
│   ├── batata-config/         # Configuration management service
│   ├── batata-naming/         # Service discovery service
│   ├── batata-plugin/         # Plugin SPI definitions
│   ├── batata-plugin-consul/  # Consul compatibility plugin
│   ├── batata-plugin-apollo/  # Apollo Config compatibility plugin
│   ├── batata-console/        # Management console backend
│   ├── batata-client/         # Rust SDK for clients
│   ├── batata-mesh/           # Service mesh (xDS, Istio MCP)
│   └── batata-server/         # Main application entry point
│       ├── src/
│       │   ├── api/           # API handlers (HTTP, gRPC, Consul)
│       │   ├── auth/          # Auth HTTP handlers
│       │   ├── config/        # Config models (re-exports)
│       │   ├── console/       # Console API handlers
│       │   ├── middleware/    # HTTP middleware
│       │   ├── model/         # Application state & config
│       │   ├── service/       # gRPC handlers
│       │   ├── startup/       # Server initialization
│       │   ├── lib.rs         # Library exports
│       │   └── main.rs        # Entry point
│       ├── tests/             # Integration tests
│       └── benches/           # Performance benchmarks
├── conf/                      # Configuration files
├── docs/                      # Documentation
└── proto/                     # Protocol buffer definitions
```

### Crate Descriptions

#### 1. `batata-common`
**Purpose**: Shared types, error definitions, and trait abstractions.

**Contents**:
- Error types (`BatataError`, error codes)
- Common traits (`DbContext`, `ConfigContext`, `ClusterContext`)
- Utility functions (validation, MD5, encoding)
- Common constants and re-exports

**Dependencies**: Minimal (serde, thiserror, anyhow, sea-orm)

#### 2. `batata-api`
**Purpose**: gRPC and HTTP API model definitions.

**Contents**:
- Protocol buffer generated code (`grpc/`)
- API request/response models
- Payload handlers trait and registry
- Raft protocol definitions

**Dependencies**: batata-common, prost, tonic

#### 3. `batata-persistence`
**Purpose**: Database layer with SeaORM entities.

**Contents**:
- SeaORM entity definitions (auto-generated)
- Database migrations
- Repository patterns

**Dependencies**: batata-common, sea-orm

#### 4. `batata-auth`
**Purpose**: Authentication and authorization services.

**Contents**:
- JWT token handling
- RBAC permission model
- User/Role/Permission services
- Auth middleware and macros (`secured!`)

**Dependencies**: batata-common, batata-persistence, jsonwebtoken, bcrypt

#### 5. `batata-consistency`
**Purpose**: Distributed consensus protocols.

**Contents**:
- Raft implementation (CP protocol for persistent data)
- State machine definitions
- Log storage (RocksDB)

**Dependencies**: batata-common, openraft, rocksdb

#### 6. `batata-core`
**Purpose**: Core cluster and connection management.

**Contents**:
- Server member management
- Connection management
- Health checking
- Circuit breaker
- Task scheduling
- Multi-datacenter topology management (`DatacenterManager`)
- Locality-aware replication

**Dependencies**: batata-common, batata-consistency, batata-persistence

#### 7. `batata-config`
**Purpose**: Configuration management service.

**Contents**:
- Config CRUD operations
- Config listening/pushing
- Gray release (beta configs)
- Import/Export functionality (ZIP format)
- History management
- Namespace service

**Dependencies**: batata-common, batata-persistence, batata-consistency

#### 8. `batata-naming`
**Purpose**: Service discovery service.

**Contents**:
- Service registration models
- Service discovery models
- Health checking
- Instance management

**Dependencies**: batata-common, batata-persistence, batata-consistency

#### 9. `batata-plugin`
**Purpose**: Plugin SPI (Service Provider Interface) definitions.

**Contents**:
- Plugin traits
- Extension points

**Dependencies**: batata-common

#### 10. `batata-plugin-consul`
**Purpose**: Consul API compatibility layer.

**Contents**:
- Consul KV API handlers
- Consul Agent API (service registration)
- Consul Health API
- Consul Catalog API
- Consul ACL API
- Data format conversion

**Dependencies**: batata-common, batata-plugin, batata-naming

#### 11. `batata-plugin-apollo`
**Purpose**: Apollo Config API compatibility layer.

**Contents**:
- Apollo Config Service API (`/configs`, `/configfiles`)
- Apollo Notification API (`/notifications/v2`)
- Apollo to Nacos concept mapping
- Long-polling notification service
- Release key generation

**Dependencies**: batata-common, batata-plugin, batata-config

#### 12. `batata-console`
**Purpose**: Management console backend.

**Contents**:
- Console API endpoints (v3)
- Data source abstraction (local/remote)
- Cluster management UI API
- Metrics API

**Dependencies**: batata-common, batata-core, batata-config, batata-naming

#### 13. `batata-client`
**Purpose**: Rust SDK for Nacos clients.

**Contents**:
- Config client
- HTTP transport
- API abstractions

**Dependencies**: batata-common, batata-api, reqwest

#### 14. `batata-mesh`
**Purpose**: Service mesh integration with xDS and Istio support.

**Contents**:
- xDS protocol implementation (EDS, CDS, LDS, RDS, ADS)
- Istio MCP (Mesh Configuration Protocol) server
- Service to Envoy resource conversion
- Incremental/Delta discovery support

**Dependencies**: batata-common, batata-naming, tonic, prost

#### 15. `batata-server`
**Purpose**: Main application entry point that assembles all components.

**Contents**:
- Main function and server initialization
- AppState assembly
- HTTP route configuration (actix-web)
- gRPC service initialization (tonic)
- Metrics and observability setup
- **OpenTelemetry integration** (OTLP export, distributed tracing)
- Integration tests and benchmarks

**Dependencies**: All crates

---

## Dependency Graph

```
                              batata-server (main binary)
                                     │
         ┌───────────────────────────┼───────────────────────────┐
         │                           │                           │
         ▼                           ▼                           ▼
  batata-console              batata-config               batata-naming
         │                           │                           │
         │                           │                           │
         ▼                           ▼                           ▼
  batata-core ◄──────────── batata-consistency ─────────► batata-core
         │                           │                           │
         │                           │                           │
         └───────────┬───────────────┴───────────────────────────┘
                     │
                     ▼
            batata-persistence
                     │
                     ▼
              batata-auth
                     │
                     ▼
               batata-api
                     │
                     ▼
             batata-common

Plugins (optional):
┌─────────────────────┐
│ batata-plugin-consul│ ──► batata-plugin ──► batata-common
└─────────────────────┘
┌─────────────────────┐
│ batata-plugin-apollo│ ──► batata-plugin ──► batata-config
└─────────────────────┘

Client SDK:
┌─────────────────┐
│  batata-client  │ ──► batata-api ──► batata-common
└─────────────────┘
```

### Layer Description

| Layer | Crates | Description |
|-------|--------|-------------|
| L0 | batata-common | Zero dependencies, core types and traits |
| L1 | batata-api | API definitions, depends on L0 |
| L2 | batata-persistence | Database layer, depends on L0-L1 |
| L3 | batata-auth | Auth services, depends on L0-L2 |
| L4 | batata-consistency | Consensus protocols, depends on L0-L2 |
| L5 | batata-core | Cluster management, depends on L0-L4 |
| L6 | batata-config, batata-naming | Business services, depends on L0-L5 |
| L6.5 | batata-mesh | Service mesh (xDS), depends on L0-L6 |
| L7 | batata-console | Console API, depends on L0-L6 |
| L8 | batata-server | Entry point, depends on all |

---

## Implementation Details

### Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/batata-common",
    "crates/batata-api",
    "crates/batata-persistence",
    "crates/batata-auth",
    "crates/batata-consistency",
    "crates/batata-core",
    "crates/batata-config",
    "crates/batata-naming",
    "crates/batata-plugin",
    "crates/batata-plugin-consul",
    "crates/batata-plugin-apollo",
    "crates/batata-console",
    "crates/batata-client",
    "crates/batata-mesh",
    "crates/batata-server",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
repository = "https://github.com/easynet-cn/batata"

[workspace.dependencies]
# Internal crates
batata-common = { path = "crates/batata-common" }
batata-api = { path = "crates/batata-api" }
# ... etc
```

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
```

---

## Trait-Based Dependency Injection

### Core Traits (in batata-common)

The architecture uses trait-based abstractions to avoid circular dependencies and enable better testing:

```rust
// crates/batata-common/src/traits.rs

use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Database access context
pub trait DbContext: Send + Sync {
    fn db(&self) -> &DatabaseConnection;
}

/// Configuration access context
pub trait ConfigContext: Send + Sync {
    fn max_content(&self) -> u64;
    fn auth_enabled(&self) -> bool;
    fn token_expire_seconds(&self) -> i64;
    fn secret_key(&self) -> &str;
}

/// Cluster management context
pub trait ClusterContext: Send + Sync {
    fn is_standalone(&self) -> bool;
    fn is_leader(&self) -> bool;
    fn leader_address(&self) -> Option<String>;
}
```

### Service Layer Usage

Services depend only on the traits they need, not concrete types:

```rust
// crates/batata-config/src/service/config.rs

use batata_common::{DbContext, ConfigContext};

pub struct ConfigService<C: DbContext + ConfigContext> {
    context: Arc<C>,
}

impl<C: DbContext + ConfigContext> ConfigService<C> {
    pub fn new(context: Arc<C>) -> Self {
        Self { context }
    }

    pub async fn find_one(&self, data_id: &str, group: &str, namespace_id: &str)
        -> anyhow::Result<Option<ConfigAllInfo>>
    {
        // Use context.db() to access database
        // ...
    }
}
```

### AppState Assembly (in batata-server)

The final `AppState` is assembled in the server crate, implementing all required traits:

```rust
// crates/batata-server/src/model/common.rs

pub struct AppState {
    pub configuration: Configuration,
    pub database_connection: Option<DatabaseConnection>,
    pub server_member_manager: Option<Arc<ServerMemberManager>>,
    pub console_datasource: Arc<dyn ConsoleDataSource>,
}

impl DbContext for AppState {
    fn db(&self) -> &DatabaseConnection {
        self.database_connection.as_ref().unwrap()
    }
}

impl ConfigContext for AppState {
    fn max_content(&self) -> u64 {
        self.configuration.max_content()
    }
    // ...
}
```

---

## Benefits

### 1. Faster Compilation
- Only changed crates need recompilation
- Parallel crate compilation
- Better incremental compilation

### 2. Clear Dependencies
- Explicit dependency declarations
- No hidden circular dependencies
- Easy to understand module boundaries

### 3. Better Testing
- Unit test individual crates in isolation
- Mock trait implementations for testing
- Cleaner integration tests

### 4. Flexible Deployment
- Build only needed components
- Smaller binary sizes for specific use cases
- Feature flags per crate

### 5. Team Scalability
- Clear ownership boundaries
- Independent crate development
- Easier code reviews

---

## Comparison with Java Nacos

| Java Nacos Module | Batata Crate | Notes |
|-------------------|--------------|-------|
| nacos-common | batata-common | Shared utilities and types |
| nacos-api | batata-api | API definitions |
| nacos-persistence | batata-persistence | Database layer |
| nacos-auth | batata-auth | Authentication (JWT, LDAP, OAuth2) |
| nacos-consistency | batata-consistency | Raft protocols |
| nacos-core | batata-core | Core services, datacenter |
| nacos-config | batata-config | Config management |
| nacos-naming | batata-naming | Service discovery |
| nacos-plugin | batata-plugin | Plugin SPI |
| nacos-console | batata-console | Console backend |
| nacos-client | batata-client | Client SDK |
| N/A | batata-mesh | Service mesh (xDS, Istio) |
| N/A | batata-plugin-apollo | Apollo Config compatibility |
| nacos-server | batata-server | Server entry point |

---

## Observability Architecture

### OpenTelemetry Integration

Batata provides comprehensive distributed tracing support through OpenTelemetry, enabling seamless integration with observability platforms like Jaeger, Zipkin, and Grafana Tempo.

#### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Batata Server                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │   HTTP API   │  │   gRPC API   │  │   Cluster Sync       │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘  │
│         │                 │                      │              │
│         └─────────────────┼──────────────────────┘              │
│                           │                                     │
│                  ┌────────▼────────┐                            │
│                  │ tracing-subscriber │                         │
│                  │   + otel layer   │                           │
│                  └────────┬────────┘                            │
│                           │                                     │
│                  ┌────────▼────────┐                            │
│                  │  OTLP Exporter  │                            │
│                  │  (gRPC/HTTP)    │                            │
│                  └────────┬────────┘                            │
└───────────────────────────┼─────────────────────────────────────┘
                            │
                            ▼
              ┌─────────────────────────┐
              │   OTLP Collector        │
              │  (Jaeger/Tempo/etc)     │
              └─────────────────────────┘
```

#### Key Components

| Component | Crate | Description |
|-----------|-------|-------------|
| `OtelConfig` | batata-server | Configuration for OpenTelemetry settings |
| `OtelGuard` | batata-server | RAII guard for proper shutdown |
| `init_tracing_with_otel` | batata-server | Initialization function |

#### Implementation Details

```rust
// crates/batata-server/src/startup/telemetry.rs

pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub service_name: String,
    pub sampling_ratio: f64,
    pub export_timeout_secs: u64,
}

pub struct OtelGuard {
    _tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(ref provider) = self._tracer_provider {
            if let Err(e) = provider.shutdown() {
                eprintln!("Failed to shutdown tracer provider: {e}");
            }
        }
    }
}
```

---

## Multi-Datacenter Architecture

Batata supports multi-datacenter deployments with locality-aware data synchronization and cross-datacenter replication.

### Topology Model

```
                    ┌─────────────────────────────────────┐
                    │            Global Cluster           │
                    └─────────────────────────────────────┘
                                     │
        ┌────────────────────────────┼────────────────────────────┐
        │                            │                            │
        ▼                            ▼                            ▼
┌───────────────┐          ┌───────────────┐          ┌───────────────┐
│   DC: us-east │          │   DC: us-west │          │   DC: eu-west │
│   Region: NA  │          │   Region: NA  │          │   Region: EU  │
├───────────────┤          ├───────────────┤          ├───────────────┤
│ Zone: zone-a  │          │ Zone: zone-a  │          │ Zone: zone-a  │
│  └─ Member 1  │          │  └─ Member 3  │          │  └─ Member 5  │
│ Zone: zone-b  │          │ Zone: zone-b  │          │ Zone: zone-b  │
│  └─ Member 2  │          │  └─ Member 4  │          │  └─ Member 6  │
└───────────────┘          └───────────────┘          └───────────────┘
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| **Datacenter** | Physical or logical grouping of nodes (e.g., `us-east-1`) |
| **Region** | Geographic area containing datacenters (e.g., `us-east`) |
| **Zone** | Availability zone within a datacenter (e.g., `zone-a`) |
| **Locality Weight** | Priority for local-first data access (higher = prefer local) |

### Core Components

#### DatacenterConfig

Configuration for datacenter-aware behavior:

```rust
// crates/batata-core/src/service/datacenter.rs

pub struct DatacenterConfig {
    pub local_datacenter: String,     // e.g., "us-east-1"
    pub local_region: String,         // e.g., "us-east"
    pub local_zone: String,           // e.g., "zone-a"
    pub locality_weight: f64,         // Priority weight (default: 1.0)
    pub cross_dc_replication_enabled: bool,
    pub cross_dc_sync_delay_secs: u64,
    pub replication_factor: usize,
}
```

#### DatacenterManager

Manages datacenter topology and provides locality-aware operations:

```rust
pub struct DatacenterManager {
    config: DatacenterConfig,
    members_by_dc: DashMap<String, Vec<Arc<Member>>>,
    datacenter_info: DashMap<String, DatacenterInfo>,
}

impl DatacenterManager {
    // Get members in the local datacenter
    pub fn get_local_members(&self) -> Vec<Arc<Member>>;

    // Get members in remote datacenters
    pub fn get_remote_members(&self) -> Vec<Arc<Member>>;

    // Select targets for replication (local-first)
    pub fn select_replication_targets(&self, exclude_self: Option<&str>, max_count: usize) -> Vec<Arc<Member>>;

    // Select one member per remote DC for cross-DC replication
    pub fn select_cross_dc_replication_targets(&self, exclude_self: Option<&str>) -> Vec<Arc<Member>>;
}
```

#### Member Extensions

The `Member` struct includes datacenter metadata:

```rust
// crates/batata-api/src/model.rs

impl Member {
    pub fn datacenter(&self) -> String;
    pub fn region(&self) -> String;
    pub fn zone(&self) -> String;
    pub fn locality_weight(&self) -> f64;
    pub fn is_same_datacenter(&self, other: &Member) -> bool;
    pub fn is_same_region(&self, other: &Member) -> bool;
    pub fn is_same_zone(&self, other: &Member) -> bool;
}
```

### Replication Strategies

| Strategy | Description |
|----------|-------------|
| **Local-First** | Prefer members in the same datacenter for reads/writes |
| **Cross-DC Replication** | Asynchronously replicate to one member per remote DC |
| **Locality-Weighted** | Higher locality_weight members are preferred |

### Data Flow

```
Write Request
     │
     ▼
┌────────────────┐
│ Local DC Write │ ◄─── Synchronous (low latency)
└───────┬────────┘
        │
        ├──────────────────────┬──────────────────────┐
        ▼                      ▼                      ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│ Cross-DC Sync │    │ Cross-DC Sync │    │ Cross-DC Sync │
│   (us-west)   │    │   (eu-west)   │    │   (ap-east)   │
└───────────────┘    └───────────────┘    └───────────────┘
        │                    │                    │
        └──────── Asynchronous (configurable delay) ────┘
```

---

## Project Statistics

- **~50,000+ lines** of Rust code
- **15 internal crates** in workspace
- **350+ unit tests** with comprehensive coverage
- **3 benchmark suites** for performance testing
