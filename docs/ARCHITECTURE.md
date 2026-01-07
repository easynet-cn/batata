# Batata Multi-Crate Architecture Design

This document describes the recommended multi-crate architecture for Batata, a Rust implementation of Nacos-compatible service discovery and configuration management platform.

## Table of Contents

1. [Overview](#overview)
2. [Current Architecture Issues](#current-architecture-issues)
3. [Proposed Solution](#proposed-solution)
4. [Crate Structure](#crate-structure)
5. [Trait-Based Dependency Injection](#trait-based-dependency-injection)
6. [Dependency Graph](#dependency-graph)
7. [Implementation Details](#implementation-details)
8. [Migration Plan](#migration-plan)

---

## Overview

The current Batata project is a monolithic Rust application with all modules in a single crate. As the project grows, this architecture leads to:

- Long compilation times
- Circular dependency risks
- Difficulty in testing individual components
- Tight coupling between modules

This document proposes splitting the project into 13 independent crates following the architecture of Java Nacos while leveraging Rust's trait system to solve circular dependencies.

---

## Current Architecture Issues

### 1. Circular Dependency Problem

The current `AppState` structure creates potential circular dependencies:

```
console → service → AppState → console_data_source → service
                 ↓
             config ← → naming (future)
```

### 2. Tight Coupling

Multiple modules directly depend on the concrete `AppState` type, making it impossible to:
- Test modules in isolation
- Use different implementations for different deployment modes
- Split into separate crates without significant refactoring

### 3. Compilation Bottleneck

All 31,000+ lines of code must be recompiled when any file changes, leading to slow development cycles.

---

## Proposed Solution

### Trait-Based Abstraction

Instead of passing concrete `AppState` everywhere, define trait abstractions that each module requires:

```rust
// Each module only depends on the traits it needs
pub trait ConfigContext: Send + Sync {
    fn db(&self) -> &DatabaseConnection;
    fn max_content(&self) -> u64;
}

pub trait ClusterContext: Send + Sync {
    fn member_manager(&self) -> &Arc<ServerMemberManager>;
    fn is_standalone(&self) -> bool;
}
```

### Layered State Assembly

The final `AppState` is assembled in the server crate, implementing all required traits:

```rust
// In batata-server
pub struct AppState {
    pub config: Configuration,
    pub db: DbState,
    pub cluster: ClusterState,
    pub console: Arc<dyn ConsoleDataSource>,
}

impl ConfigContext for AppState { ... }
impl ClusterContext for AppState { ... }
impl DbContext for AppState { ... }
```

---

## Crate Structure

### Proposed 13-Crate Workspace

```
batata/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── batata-common/         # Shared types, traits, utilities
│   ├── batata-api/            # gRPC/HTTP API definitions
│   ├── batata-persistence/    # Database entities and migrations
│   ├── batata-auth/           # Authentication and authorization
│   ├── batata-consistency/    # Raft (CP) and Distro (AP) protocols
│   ├── batata-core/           # Cluster and connection management
│   ├── batata-config/         # Configuration management service
│   ├── batata-naming/         # Service discovery service
│   ├── batata-plugin/         # Plugin SPI definitions
│   ├── batata-plugin-consul/  # Consul compatibility plugin
│   ├── batata-console/        # Management console
│   ├── batata-client/         # Rust SDK for clients
│   └── batata-server/         # Main application entry point
└── docs/
    └── ARCHITECTURE.md        # This document
```

### Crate Descriptions

#### 1. `batata-common`
**Purpose**: Shared types, error definitions, and trait abstractions.

**Contents**:
- Error types (`BatataError`, error codes)
- Common traits (`ConfigContext`, `DbContext`, `ClusterContext`)
- Utility functions (validation, MD5, encoding)
- Common constants

**Dependencies**: Minimal (serde, thiserror, anyhow)

```rust
// crates/batata-common/src/lib.rs
pub mod error;
pub mod traits;
pub mod utils;
pub mod constants;

// Re-exports
pub use error::{BatataError, AppError};
pub use traits::*;
```

#### 2. `batata-api`
**Purpose**: gRPC and HTTP API model definitions.

**Contents**:
- Protocol buffer generated code
- API request/response models
- Payload handlers trait

**Dependencies**: batata-common, prost, tonic

```rust
// crates/batata-api/src/lib.rs
pub mod grpc;      // Generated protobuf code
pub mod config;    // Config API models
pub mod naming;    // Naming API models
pub mod remote;    // Remote communication models
pub mod consul;    // Consul compatibility models
```

#### 3. `batata-persistence`
**Purpose**: Database layer with SeaORM entities.

**Contents**:
- SeaORM entity definitions (auto-generated)
- Repository traits
- Migration scripts

**Dependencies**: batata-common, sea-orm

```rust
// crates/batata-persistence/src/lib.rs
pub mod entity;       // Auto-generated entities
pub mod repository;   // Repository traits
pub mod migration;    // Database migrations
```

#### 4. `batata-auth`
**Purpose**: Authentication and authorization services.

**Contents**:
- JWT token handling
- RBAC permission model
- User/Role/Permission services
- Auth middleware

**Dependencies**: batata-common, batata-persistence

```rust
// crates/batata-auth/src/lib.rs
pub mod jwt;          // JWT encoding/decoding
pub mod rbac;         // Role-based access control
pub mod service;      // Auth services
pub mod middleware;   // Actix middleware
pub mod model;        // Auth models
```

#### 5. `batata-consistency`
**Purpose**: Distributed consensus protocols.

**Contents**:
- Raft implementation (CP protocol for persistent data)
- Distro implementation (AP protocol for ephemeral data)
- State machine definitions
- Log storage

**Dependencies**: batata-common, openraft

```rust
// crates/batata-consistency/src/lib.rs
pub mod raft;         // Raft consensus
pub mod distro;       // Distro protocol
pub mod storage;      // Log and state storage
```

#### 6. `batata-core`
**Purpose**: Core cluster and connection management.

**Contents**:
- Server member management
- Connection management
- Health checking
- Task scheduling

**Dependencies**: batata-common, batata-consistency

```rust
// crates/batata-core/src/lib.rs
pub mod cluster;      // Cluster management
pub mod connection;   // Connection pool
pub mod health;       // Health checking
pub mod scheduler;    // Task scheduling
```

#### 7. `batata-config`
**Purpose**: Configuration management service.

**Contents**:
- Config CRUD operations
- Config listening/pushing
- Gray release (beta configs)
- Import/Export functionality
- History management

**Dependencies**: batata-common, batata-persistence, batata-consistency

```rust
// crates/batata-config/src/lib.rs
pub mod service;      // Config service
pub mod handler;      // gRPC handlers
pub mod listener;     // Config change listeners
pub mod export;       // Export functionality
pub mod import;       // Import functionality
pub mod history;      // History service
```

#### 8. `batata-naming`
**Purpose**: Service discovery service.

**Contents**:
- Service registration
- Service discovery
- Health checking
- Load balancing

**Dependencies**: batata-common, batata-persistence, batata-consistency

```rust
// crates/batata-naming/src/lib.rs
pub mod service;      // Naming service
pub mod handler;      // gRPC handlers
pub mod health;       // Instance health check
pub mod selector;     // Instance selection
```

#### 9. `batata-plugin`
**Purpose**: Plugin SPI (Service Provider Interface) definitions.

**Contents**:
- Plugin traits
- Plugin loading mechanism
- Extension points

**Dependencies**: batata-common

```rust
// crates/batata-plugin/src/lib.rs
pub mod spi;          // SPI trait definitions
pub mod loader;       // Plugin loader
pub mod registry;     // Plugin registry
```

#### 10. `batata-plugin-consul`
**Purpose**: Consul API compatibility layer.

**Contents**:
- Consul KV API
- Consul ACL API
- Consul Agent API
- Data format conversion

**Dependencies**: batata-common, batata-plugin, batata-config

```rust
// crates/batata-plugin-consul/src/lib.rs
pub mod kv;           // KV store API
pub mod acl;          // ACL API
pub mod agent;        // Agent API
pub mod convert;      // Data conversion
```

#### 11. `batata-console`
**Purpose**: Management console backend.

**Contents**:
- Console API endpoints
- Data source abstraction (local/remote)
- Cluster management UI API
- Metrics API

**Dependencies**: batata-common, batata-core, batata-config, batata-naming

```rust
// crates/batata-console/src/lib.rs
pub mod api;          // Console API handlers
pub mod datasource;   // Data source abstraction
pub mod client;       // HTTP client for remote mode
```

#### 12. `batata-client`
**Purpose**: Rust SDK for Nacos clients.

**Contents**:
- Config client
- Naming client
- gRPC transport
- Retry and failover logic

**Dependencies**: batata-common, batata-api

```rust
// crates/batata-client/src/lib.rs
pub mod config;       // Config client
pub mod naming;       // Naming client
pub mod transport;    // gRPC transport
pub mod failover;     // Failover logic
```

#### 13. `batata-server`
**Purpose**: Main application entry point that assembles all components.

**Contents**:
- Main function
- AppState assembly
- Server initialization
- Route configuration

**Dependencies**: All crates

```rust
// crates/batata-server/src/main.rs
use batata_common::*;
use batata_core::*;
use batata_config::*;
use batata_naming::*;
use batata_console::*;
use batata_auth::*;

fn main() {
    // Initialize and start servers
}
```

---

## Trait-Based Dependency Injection

### Core Traits (in batata-common)

```rust
// crates/batata-common/src/traits/mod.rs

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
    fn all_members(&self) -> Vec<Member>;
}

/// Console data source abstraction
#[async_trait::async_trait]
pub trait ConsoleDataSource: Send + Sync {
    // Namespace operations
    async fn namespace_find_all(&self) -> Vec<Namespace>;
    async fn namespace_create(&self, id: &str, name: &str, desc: &str) -> anyhow::Result<()>;

    // Config operations
    async fn config_find_one(&self, data_id: &str, group: &str, ns: &str)
        -> anyhow::Result<Option<ConfigAllInfo>>;
    async fn config_search_page(&self, ...) -> anyhow::Result<Page<ConfigBasicInfo>>;

    // History operations
    async fn history_search_page(&self, ...) -> anyhow::Result<Page<ConfigHistoryBasicInfo>>;

    // Cluster operations
    fn cluster_all_members(&self) -> Vec<Member>;
    fn cluster_is_standalone(&self) -> bool;

    // Helper methods
    fn is_remote(&self) -> bool;
}
```

### Service Layer Usage

```rust
// crates/batata-config/src/service/config.rs

use batata_common::{DbContext, ConfigContext};

/// Config service that only depends on required traits
pub struct ConfigService<C: DbContext + ConfigContext> {
    context: Arc<C>,
}

impl<C: DbContext + ConfigContext> ConfigService<C> {
    pub fn new(context: Arc<C>) -> Self {
        Self { context }
    }

    pub async fn find_one(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
    ) -> anyhow::Result<Option<ConfigAllInfo>> {
        // Use context.db() to access database
        let config = config_info::Entity::find()
            .filter(config_info::Column::DataId.eq(data_id))
            .filter(config_info::Column::GroupId.eq(group))
            .filter(config_info::Column::TenantId.eq(namespace_id))
            .one(self.context.db())
            .await?;

        Ok(config.map(ConfigAllInfo::from))
    }

    pub async fn create_or_update(
        &self,
        data_id: &str,
        group: &str,
        namespace_id: &str,
        content: &str,
        // ... other params
    ) -> anyhow::Result<()> {
        // Validate content size using config context
        if content.len() as u64 > self.context.max_content() {
            return Err(anyhow::anyhow!("Content exceeds maximum size"));
        }

        // ... implementation
        Ok(())
    }
}
```

### AppState Assembly (in batata-server)

```rust
// crates/batata-server/src/state.rs

use batata_common::*;
use batata_core::cluster::ServerMemberManager;
use batata_console::datasource::ConsoleDataSource;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Complete application state assembled from all components
pub struct AppState {
    pub configuration: Configuration,
    pub database_connection: DatabaseConnection,
    pub server_member_manager: Arc<ServerMemberManager>,
    pub console_data_source: Arc<dyn ConsoleDataSource>,
}

impl DbContext for AppState {
    fn db(&self) -> &DatabaseConnection {
        &self.database_connection
    }
}

impl ConfigContext for AppState {
    fn max_content(&self) -> u64 {
        self.configuration.max_content()
    }

    fn auth_enabled(&self) -> bool {
        self.configuration.auth_enabled()
    }

    fn token_expire_seconds(&self) -> i64 {
        self.configuration.token_expire_seconds()
    }

    fn secret_key(&self) -> &str {
        self.configuration.secret_key()
    }
}

impl ClusterContext for AppState {
    fn is_standalone(&self) -> bool {
        self.server_member_manager.is_standalone()
    }

    fn is_leader(&self) -> bool {
        self.server_member_manager.is_leader()
    }

    fn leader_address(&self) -> Option<String> {
        self.server_member_manager.leader_address()
    }

    fn all_members(&self) -> Vec<Member> {
        self.server_member_manager.all_members()
    }
}
```

---

## Dependency Graph

```
                                  batata-server
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
| L7 | batata-console | Console API, depends on L0-L6 |
| L8 | batata-server | Entry point, depends on all |

---

## Implementation Details

### Workspace Cargo.toml

```toml
# Cargo.toml (workspace root)
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
    "crates/batata-console",
    "crates/batata-client",
    "crates/batata-server",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "Apache-2.0"
repository = "https://github.com/easynet-cn/batata"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.44", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Web frameworks
actix-web = "4"
actix-rt = "2"
tonic = "0.13"
prost = "0.13"

# Database
sea-orm = { version = "1.1", features = ["sqlx-mysql", "runtime-tokio-rustls"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Auth
jsonwebtoken = "9"
bcrypt = "0.17"

# Consensus
openraft = { version = "0.10", features = ["serde"] }

# Caching
moka = { version = "0.12", features = ["future"] }
dashmap = "6"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
anyhow = "1.0"
thiserror = "2.0"

# Internal crates
batata-common = { path = "crates/batata-common" }
batata-api = { path = "crates/batata-api" }
batata-persistence = { path = "crates/batata-persistence" }
batata-auth = { path = "crates/batata-auth" }
batata-consistency = { path = "crates/batata-consistency" }
batata-core = { path = "crates/batata-core" }
batata-config = { path = "crates/batata-config" }
batata-naming = { path = "crates/batata-naming" }
batata-plugin = { path = "crates/batata-plugin" }
batata-plugin-consul = { path = "crates/batata-plugin-consul" }
batata-console = { path = "crates/batata-console" }
batata-client = { path = "crates/batata-client" }
```

### Individual Crate Cargo.toml Examples

```toml
# crates/batata-common/Cargo.toml
[package]
name = "batata-common"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
async-trait = { workspace = true }
sea-orm = { workspace = true }
```

```toml
# crates/batata-config/Cargo.toml
[package]
name = "batata-config"
version.workspace = true
edition.workspace = true

[dependencies]
batata-common = { workspace = true }
batata-persistence = { workspace = true }
batata-consistency = { workspace = true }

serde = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
sea-orm = { workspace = true }
```

```toml
# crates/batata-server/Cargo.toml
[package]
name = "batata-server"
version.workspace = true
edition.workspace = true

[[bin]]
name = "batata"
path = "src/main.rs"

[dependencies]
batata-common = { workspace = true }
batata-api = { workspace = true }
batata-persistence = { workspace = true }
batata-auth = { workspace = true }
batata-consistency = { workspace = true }
batata-core = { workspace = true }
batata-config = { workspace = true }
batata-naming = { workspace = true }
batata-console = { workspace = true }
batata-plugin-consul = { workspace = true }

actix-web = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

---

## Migration Plan

### Phase 1: Prepare (No Breaking Changes)

1. **Create workspace structure**
   - Create `crates/` directory
   - Update root `Cargo.toml` to workspace format
   - Keep existing `src/` as temporary compatibility layer

2. **Extract batata-common**
   - Move `src/error.rs` → `crates/batata-common/src/error.rs`
   - Define core traits in `crates/batata-common/src/traits/`
   - Move utility functions

3. **Extract batata-api**
   - Move `src/api/grpc/` → `crates/batata-api/src/grpc/`
   - Move API models
   - Update proto build script

### Phase 2: Core Infrastructure

4. **Extract batata-persistence**
   - Move `src/entity/` → `crates/batata-persistence/src/entity/`
   - Define repository traits
   - Update sea-orm-cli output path

5. **Extract batata-auth**
   - Move `src/auth/` → `crates/batata-auth/src/`
   - Update to use traits from batata-common

6. **Extract batata-consistency**
   - Move `src/core/raft/` → `crates/batata-consistency/src/raft/`
   - Move distro protocol code
   - Update to use traits

### Phase 3: Business Services

7. **Extract batata-core**
   - Move `src/core/service/` → `crates/batata-core/src/`
   - Cluster and connection management

8. **Extract batata-config**
   - Move `src/service/config*.rs` → `crates/batata-config/src/`
   - Move config handlers

9. **Extract batata-naming**
   - Move `src/naming/` → `crates/batata-naming/src/`
   - Service discovery implementation

### Phase 4: Console and Plugins

10. **Extract batata-console**
    - Move `src/console/` → `crates/batata-console/src/`
    - Update data source implementations

11. **Extract batata-plugin and batata-plugin-consul**
    - Define plugin SPI
    - Move Consul compatibility code

### Phase 5: Finalize

12. **Create batata-server**
    - Move `src/main.rs` → `crates/batata-server/src/main.rs`
    - Implement AppState with all traits
    - Wire up all components

13. **Extract batata-client**
    - Create Rust SDK for external clients

14. **Cleanup**
    - Remove old `src/` directory
    - Update CI/CD pipelines
    - Update documentation

### Estimated File Movements

| Source | Destination |
|--------|-------------|
| `src/error.rs` | `crates/batata-common/src/error.rs` |
| `src/model/common.rs` (types) | `crates/batata-common/src/types.rs` |
| `src/api/` | `crates/batata-api/src/` |
| `src/entity/` | `crates/batata-persistence/src/entity/` |
| `src/auth/` | `crates/batata-auth/src/` |
| `src/core/raft/` | `crates/batata-consistency/src/raft/` |
| `src/core/service/` | `crates/batata-core/src/` |
| `src/service/config*.rs` | `crates/batata-config/src/` |
| `src/service/namespace.rs` | `crates/batata-config/src/namespace.rs` |
| `src/service/history.rs` | `crates/batata-config/src/history.rs` |
| `src/naming/` | `crates/batata-naming/src/` |
| `src/console/` | `crates/batata-console/src/` |
| `src/api/consul/` | `crates/batata-plugin-consul/src/` |
| `src/main.rs` | `crates/batata-server/src/main.rs` |
| `src/lib.rs` | `crates/batata-server/src/lib.rs` |

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
| nacos-auth | batata-auth | Authentication |
| nacos-consistency | batata-consistency | Raft/Distro protocols |
| nacos-core | batata-core | Core services |
| nacos-config | batata-config | Config management |
| nacos-naming | batata-naming | Service discovery |
| nacos-plugin | batata-plugin | Plugin SPI |
| nacos-console | batata-console | Console backend |
| nacos-client | batata-client | Client SDK |
| nacos-server | batata-server | Server entry point |

---

## Conclusion

This multi-crate architecture provides a clean separation of concerns while maintaining the ability to compose all components into a single application. The trait-based dependency injection solves circular dependency issues and enables better testing and modularity.

The migration can be done incrementally without breaking existing functionality, and the resulting architecture closely mirrors Java Nacos for familiarity while taking advantage of Rust's strong type system and trait mechanisms.
