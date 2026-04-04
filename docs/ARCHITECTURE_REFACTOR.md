# Architecture Refactor: Isolated Domain Crates

## Current State

The batata project has been refactored with a 3-layer architecture that isolates
Nacos and Consul implementations into independent domain crates.

### Layer 1: Infrastructure (shared)

| Crate | Purpose | Status |
|-------|---------|--------|
| `batata-foundation` | Storage, Consistency, Cluster, Event, Connection traits | NEW |
| `batata-common` | Application-level traits (Config, Cluster, Auth, Plugin SPIs) | EXISTING |

### Layer 2: Domain (isolated)

| Crate | Purpose | Status |
|-------|---------|--------|
| `batata-naming-nacos` | Nacos naming: namespace/group/cluster, heartbeat, protection threshold | NEW (active) |
| `batata-naming-consul` | Consul naming: agent/catalog/health, tri-state, blocking query | NEW |
| `batata-config-nacos` | Nacos config: dataId/group, gray release, MD5 cache, long-polling | NEW |
| `batata-config-consul` | Consul KV: CAS, sessions, transactions, prefix scan | NEW |

### Layer 3: API Contracts + Handlers

| Crate | Purpose | Status |
|-------|---------|--------|
| `batata-api` | gRPC proto + NamingServiceProvider trait + models | EXISTING |
| `batata-naming` | HTTP/gRPC handlers + old NamingService impl + health checks | EXISTING (to be thinned) |
| `batata-config` | Config HTTP/gRPC handlers + service logic | EXISTING |

### Layer 4: Server Integration

| Crate | Purpose | Status |
|-------|---------|--------|
| `batata-server` | Bootstrap, HTTP/gRPC servers | EXISTING |
| `batata-server-common` | Middleware, AppState, auth handlers | EXISTING (merge candidate) |
| `batata-console` | Management UI backend | EXISTING |

## What Changed

1. **main.rs** now creates `NacosNamingServiceImpl` (from batata-naming-nacos) instead of `NamingService` (from batata-naming)
2. All healthcheck components accept `Arc<dyn NamingServiceProvider>` instead of concrete type
3. `NamingInstanceDistroHandler` accepts `Arc<dyn NamingServiceProvider>`
4. `GrpcServers` stores `Arc<dyn NamingServiceProvider>`
5. Old `batata-core/src/abstraction/` removed (1,379 lines of dead code)

## Cleanup Roadmap

### Phase 1: DONE
- [x] Create foundation + 4 domain crates
- [x] Implement adapters for NamingServiceProvider trait
- [x] Switch main.rs to NacosNamingServiceImpl
- [x] Remove batata-core/src/abstraction/

### Phase 2: Migrate batata-naming internals to batata-naming-nacos
- [ ] Move `batata-naming/src/healthcheck/` to batata-naming-nacos
- [ ] Move `batata-naming/src/service/` (old NamingService) to batata-naming-nacos as legacy adapter
- [ ] Move `batata-naming/src/selector.rs` to batata-naming-nacos
- [ ] Move `batata-naming/src/persistence/` to batata-naming-nacos
- [ ] Keep only HTTP/gRPC handlers in batata-naming (thin facade)

### Phase 3: Migrate Consul plugin to batata-naming-consul
- [ ] Replace batata-plugin-consul's use of NamingServiceProvider with batata-naming-consul::ConsulNamingServiceImpl
- [ ] Remove RegisterSource::Consul from batata-api (no longer needed when domains are isolated)
- [ ] Remove source-filtered methods from NamingServiceProvider trait
- [ ] Move Consul HTTP handlers to use batata-naming-consul directly

### Phase 4: Consolidate server layer
- [ ] Merge batata-server-common into batata-server
- [ ] Update all import paths in batata-console, batata-config, batata-naming, batata-ai

### Phase 5: Cleanup
- [ ] Remove RegisterSource enum entirely (or keep only for backward-compatible API responses)
- [ ] Remove old NamingService from batata-naming (replaced by NacosNamingServiceImpl)
- [ ] Consider renaming batata-api to batata-contracts

## Design Decisions

### Why not remove RegisterSource now?
RegisterSource is used in 87 call sites across batata-plugin-consul (56), batata-naming handlers (25+),
and batata-console (7). All are production code. Removing it requires migrating the Consul plugin first.

### Why not merge batata-server-common now?
6 crates depend on it (batata-config, batata-naming, batata-ai, batata-copilot, batata-console, batata-server).
Merging requires updating import paths in all of them. Better as a dedicated task.

### Why keep batata-naming as a separate crate?
It contains HTTP/gRPC handlers that are protocol-specific (Nacos V2/V3 API format).
These should stay separate from the domain implementation. batata-naming becomes a thin
facade that delegates to NamingServiceProvider trait implementations.

### Why batata-foundation doesn't replace batata-common?
- batata-foundation: infrastructure-level traits (StorageEngine, CpProtocol, EventBus)
- batata-common: application-level traits (ConfigContext, ClusterContext, AuthPlugin)
No overlap. Different abstraction levels.
