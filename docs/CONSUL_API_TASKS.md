# Consul API Compatibility Tasks

This document tracks the implementation of Consul HTTP API compatibility in Batata.

**Important**: This is API compatibility only - we adapt Consul API requests to Batata's underlying Nacos functionality.

**Last Updated**: 2026-02-04

## Implementation Status Legend

- ✅ **Complete** - Fully implemented with real backend integration
- ⚠️ **Partial** - Works but with limitations (proxy/fixed data)
- 🔸 **Memory-Only** - Functional but no persistence (data lost on restart)
- ❌ **Stub** - Placeholder only (returns success/empty)
- 🔧 **TODO** - Needs implementation

---

## Quick Start

Use one of the unified route functions for easy integration:

```rust
use batata_plugin_consul::{consul_routes_full, consul_routes_persistent, consul_routes};

// For production: All features with database persistence + real cluster info
// Requires: All persistent services + Arc<ServerMemberManager>
app.service(consul_routes_full())

// For production without cluster info: All features with database persistence
// Requires: All persistent services
app.service(consul_routes_persistent())

// For development/testing: In-memory storage (data lost on restart)
// Requires: Basic services only
app.service(consul_routes())
```

---

## Implementation Status Overview

| Category | Total | ✅ | ⚠️ | 🔸 | ❌ | Notes |
|----------|-------|----|----|----|----|-------|
| Agent Service | 5 | 5 | 0 | 0 | 0 | Via NamingService |
| Agent Check | 7 | 7 | 0 | 0 | 0 | ✅ Config Service |
| Agent Core | 12 | 7 | 5 | 0 | 0 | ✅ Real cluster + metrics via ServerMemberManager |
| Catalog | 10 | 7 | 2 | 0 | 1 | Gateway stub |
| Health | 6 | 4 | 1 | 0 | 1 | ✅ Config Service |
| KV Store | 4 | 4 | 0 | 0 | 0 | ✅ Config Service |
| ACL Core | 7 | 7 | 0 | 0 | 0 | ✅ Config Service |
| ACL Roles | 5 | 5 | 0 | 0 | 0 | ✅ Config Service |
| ACL Auth Methods | 5 | 5 | 0 | 0 | 0 | ✅ Config Service |
| Session | 6 | 6 | 0 | 0 | 0 | ✅ Config Service |
| Event | 2 | 2 | 0 | 0 | 0 | ✅ Config Service |
| Prepared Query | 7 | 7 | 0 | 0 | 0 | ✅ Config Service |
| Status | 2 | 2 | 0 | 0 | 0 | ✅ Real cluster info |
| **Total** | **78** | **68** | **8** | **0** | **2** | |

**Real Implementation: 87% (68/78)**
**Functional: 97% (76/78)**

---

## Phase 1: Persistence Layer (Priority: HIGH)

These features work but lose data on restart. Need to integrate with Batata's storage.

### Task P1-1: KV Store Persistence
**Status**: ✅ COMPLETE

| Endpoint | Current | Notes |
|----------|---------|-------|
| GET /v1/kv/{key} | ✅ Config Service | Supports recursive, keys-only, raw modes |
| PUT /v1/kv/{key} | ✅ Config Service | Supports CAS operations |
| DELETE /v1/kv/{key} | ✅ Config Service | Supports recursive delete |
| PUT /v1/txn | ✅ Config Service | Full transaction support |

**Implementation Details**:
- `ConsulKVServicePersistent` class in `kv.rs` uses Batata's ConfigService
- Key format: `kv:{key}` in namespace `public`, group `consul-kv`
- Supports all Consul KV features: recursive queries, CAS operations, transactions
- In-memory cache for performance with database persistence
- Use `consul_kv_routes_persistent()` for persistent routes

### Task P1-2: ACL Persistence
**Status**: ✅ COMPLETE

| Endpoint | Current | Notes |
|----------|---------|-------|
| GET /v1/acl/tokens | ✅ Config Service | Persistent storage |
| PUT /v1/acl/token | ✅ Config Service | Persistent storage |
| GET /v1/acl/token/{id} | ✅ Config Service | Persistent storage |
| DELETE /v1/acl/token/{id} | ✅ Config Service | Persistent storage |
| GET /v1/acl/policies | ✅ Config Service | Persistent storage |
| PUT /v1/acl/policy | ✅ Config Service | Persistent storage |
| GET /v1/acl/policy/{id} | ✅ Config Service | Persistent storage |
| GET /v1/acl/roles | ✅ Config Service | Persistent storage |
| PUT /v1/acl/role | ✅ Config Service | Persistent storage |
| GET /v1/acl/role/{id} | ✅ Config Service | Persistent storage |
| PUT /v1/acl/role/{id} | ✅ Config Service | Persistent storage |
| DELETE /v1/acl/role/{id} | ✅ Config Service | Persistent storage |
| GET /v1/acl/auth-methods | ✅ Config Service | Persistent storage |
| PUT /v1/acl/auth-method | ✅ Config Service | Persistent storage |
| GET /v1/acl/auth-method/{name} | ✅ Config Service | Persistent storage |
| PUT /v1/acl/auth-method/{name} | ✅ Config Service | Persistent storage |
| DELETE /v1/acl/auth-method/{name} | ✅ Config Service | Persistent storage |

**Implementation Details**:
- `AclServicePersistent` class in `acl.rs` uses Batata's ConfigService
- Data format: `token:{id}`, `policy:{id}`, `role:{id}`, `auth-method:{name}` in namespace `public`, group `consul-acl`
- In-memory cache (DashMap) for performance with database persistence
- Supports all Consul ACL features: tokens, policies, roles, auth methods
- Use `consul_acl_routes_persistent()` for persistent routes

### Task P1-3: Session Persistence
**Status**: ✅ COMPLETE

| Endpoint | Current | Notes |
|----------|---------|-------|
| PUT /v1/session/create | ✅ Config Service | Persistent storage with TTL |
| PUT /v1/session/destroy/{uuid} | ✅ Config Service | Persistent storage |
| GET /v1/session/info/{uuid} | ✅ Config Service | Persistent storage with expiry check |
| GET /v1/session/list | ✅ Config Service | Persistent storage |
| GET /v1/session/node/{node} | ✅ Config Service | Persistent storage |
| PUT /v1/session/renew/{uuid} | ✅ Config Service | Persistent storage with TTL renewal |

**Implementation Details**:
- `ConsulSessionServicePersistent` class in `session.rs` uses Batata's ConfigService
- Data format: `session:{id}` in namespace `public`, group `consul-sessions`
- Stores session metadata including TTL for expiration tracking
- Automatic expired session cleanup
- In-memory cache (DashMap) for performance with database persistence
- Use `consul_session_routes_persistent()` for persistent routes

### Task P1-4: Health Check Persistence
**Status**: ✅ COMPLETE

| Endpoint | Current | Notes |
|----------|---------|-------|
| PUT /v1/agent/check/register | ✅ Config Service | Persistent storage |
| PUT /v1/agent/check/deregister/{id} | ✅ Config Service | Persistent storage |
| PUT /v1/agent/check/pass/{id} | ✅ Config Service | Updates NamingService health |
| PUT /v1/agent/check/warn/{id} | ✅ Config Service | Updates NamingService health |
| PUT /v1/agent/check/fail/{id} | ✅ Config Service | Updates NamingService health |
| PUT /v1/agent/check/update/{id} | ✅ Config Service | Updates NamingService health |
| GET /v1/agent/checks | ✅ Config Service | Persistent storage |
| GET /v1/health/checks/{service} | ✅ Config Service | Persistent storage |
| GET /v1/health/state/{state} | ✅ Config Service | Persistent storage |
| GET /v1/health/node/{node} | ✅ Config Service | Persistent storage |

**Implementation Details**:
- `ConsulHealthServicePersistent` class in `health.rs` uses Batata's ConfigService
- Check format: `check:{check_id}` in namespace `public`, group `consul-checks`
- Syncs check status to NamingService instance health
- Use `consul_agent_routes_persistent()` and `consul_health_routes_persistent()` for persistent routes

---

## Phase 2: Real Cluster Integration (Priority: MEDIUM)

### Task P2-1: Status API - Real Cluster Info
**Status**: ✅ COMPLETE

| Endpoint | Current | Notes |
|----------|---------|-------|
| GET /v1/status/leader | ✅ ServerMemberManager | Returns real cluster leader |
| GET /v1/status/peers | ✅ ServerMemberManager | Returns real healthy peers |

**Implementation Details**:
- `get_leader_real` and `get_peers_real` handlers in `status.rs`
- Queries `ServerMemberManager` for real cluster information
- Converts Batata addresses to Consul-style Raft addresses (port 8300)
- Use `consul_status_routes_real()` for real cluster routes

### Task P2-2: Agent Cluster Operations
**Status**: ⚠️ PARTIAL (Real cluster info available via `consul_agent_routes_real()`)

| Endpoint | Current | Notes |
|----------|---------|-------|
| GET /v1/agent/self | ✅ ServerMemberManager | Real cluster health via `consul_agent_routes_real()` |
| GET /v1/agent/members | ✅ ServerMemberManager | Real cluster members via `consul_agent_routes_real()` |
| PUT /v1/agent/join/{address} | ⚠️ Logs only | Returns success with warning log |
| PUT /v1/agent/leave | ⚠️ Logs only | Returns success with warning log |
| PUT /v1/agent/force-leave/{node} | ⚠️ Logs only | Returns success with warning log |
| PUT /v1/agent/reload | ⚠️ Logs only | Returns success with warning log |
| PUT /v1/agent/maintenance | ⚠️ Logs only | Returns success with warning log |
| PUT /v1/agent/token/{type} | ⚠️ Logs only | Returns success with warning log |

**Implementation Details**:
- `get_agent_self_real` and `get_agent_members_real` handlers in `agent.rs`
- Queries `ServerMemberManager` for real cluster information
- Returns actual member states (Up, Down, Suspicious, Starting, Isolation)
- Includes cluster health summary in `/v1/agent/self` response
- Use `consul_agent_routes_real()` for real cluster routes

**Note**: Cluster mutation operations (join/leave/etc.) don't map to Batata's architecture and remain as logged stubs.

### Task P2-3: Real Metrics
**Status**: ✅ COMPLETE

| Endpoint | Current | Notes |
|----------|---------|-------|
| GET /v1/agent/metrics | ✅ Real metrics | Full metrics via `consul_agent_routes_real()` |

**Implementation Details**:
- `get_agent_metrics_real` handler in `agent.rs` with comprehensive metrics
- Includes runtime metrics (CPU, memory, threads)
- Includes service metrics from NamingService (service count, instance count, health)
- Includes cluster metrics from ServerMemberManager (members, health summary)
- Consul-compatible metric names (`consul.runtime.*`, `consul.catalog.*`, `consul.serf.*`)
- Batata-specific metric names (`batata.runtime.*`, `batata.naming.*`, `batata.cluster.*`)
- Use `consul_agent_routes_real()` for real metrics routes

---

## Phase 3: Unsupported Features (Priority: LOW)

These features don't map to Batata's architecture:

| Endpoint | Status | Recommendation |
|----------|--------|----------------|
| GET /v1/agent/monitor | ❌ Stub | Keep as stub (log streaming) |
| GET /v1/catalog/gateway-services/{gateway} | ❌ Stub | Keep as stub (no mesh) |
| GET /v1/health/ingress/{service} | ❌ Stub | Keep as stub (no mesh) |
| GET /v1/catalog/connect/{service} | ⚠️ Proxy | Keep as proxy |
| GET /v1/health/connect/{service} | ⚠️ Proxy | Keep as proxy |

---

## Completed Features (No Changes Needed)

### Agent Service API - ✅ Complete
- PUT /v1/agent/service/register → NamingService
- PUT /v1/agent/service/deregister/{id} → NamingService
- GET /v1/agent/services → NamingService
- GET /v1/agent/service/{id} → NamingService
- PUT /v1/agent/service/maintenance/{id} → NamingService

### Agent Info API - ✅ Complete
- GET /v1/agent/self → Real system info (sysinfo) + Real cluster health via `consul_agent_routes_real()`
- GET /v1/agent/members → Real cluster members via ServerMemberManager (use `consul_agent_routes_real()`)
- GET /v1/agent/host → Real host info (sysinfo)
- GET /v1/agent/version → Batata version

### Catalog Core API - ✅ Complete
- GET /v1/catalog/services → NamingService
- GET /v1/catalog/service/{name} → NamingService
- GET /v1/catalog/nodes → NamingService
- GET /v1/catalog/node/{node} → NamingService
- PUT /v1/catalog/register → NamingService
- PUT /v1/catalog/deregister → NamingService
- GET /v1/catalog/node-services/{node} → NamingService

### Health Service API - ✅ Complete
- GET /v1/health/service/{name} → NamingService

### Prepared Query Execute - ✅ Complete
- GET /v1/query/{uuid}/execute → NamingService

### KV Store API - ✅ Complete
- GET /v1/kv/{key} → ConfigService (persistent storage)
- PUT /v1/kv/{key} → ConfigService (persistent storage)
- DELETE /v1/kv/{key} → ConfigService (persistent storage)
- PUT /v1/txn → ConfigService (persistent storage)

### Health Check API - ✅ Complete
- PUT /v1/agent/check/register → ConfigService (persistent storage)
- PUT /v1/agent/check/deregister/{id} → ConfigService (persistent storage)
- PUT /v1/agent/check/pass/{id} → ConfigService + NamingService (syncs instance health)
- PUT /v1/agent/check/warn/{id} → ConfigService + NamingService (syncs instance health)
- PUT /v1/agent/check/fail/{id} → ConfigService + NamingService (syncs instance health)
- PUT /v1/agent/check/update/{id} → ConfigService + NamingService (syncs instance health)
- GET /v1/agent/checks → ConfigService (persistent storage)
- GET /v1/health/checks/{service} → ConfigService (persistent storage)
- GET /v1/health/state/{state} → ConfigService (persistent storage)
- GET /v1/health/node/{node} → ConfigService (persistent storage)

### Status API - ✅ Complete
- GET /v1/status/leader → ServerMemberManager (real cluster leader)
- GET /v1/status/peers → ServerMemberManager (real healthy peers)

---

### ACL API - ✅ Complete
- GET /v1/acl/tokens → ConfigService (persistent storage)
- PUT /v1/acl/token → ConfigService (persistent storage)
- GET /v1/acl/token/{id} → ConfigService (persistent storage)
- DELETE /v1/acl/token/{id} → ConfigService (persistent storage)
- GET /v1/acl/policies → ConfigService (persistent storage)
- PUT /v1/acl/policy → ConfigService (persistent storage)
- GET /v1/acl/policy/{id} → ConfigService (persistent storage)
- GET /v1/acl/roles → ConfigService (persistent storage)
- PUT /v1/acl/role → ConfigService (persistent storage)
- GET /v1/acl/role/{id} → ConfigService (persistent storage)
- PUT /v1/acl/role/{id} → ConfigService (persistent storage)
- DELETE /v1/acl/role/{id} → ConfigService (persistent storage)
- GET /v1/acl/auth-methods → ConfigService (persistent storage)
- PUT /v1/acl/auth-method → ConfigService (persistent storage)
- GET /v1/acl/auth-method/{name} → ConfigService (persistent storage)
- PUT /v1/acl/auth-method/{name} → ConfigService (persistent storage)
- DELETE /v1/acl/auth-method/{name} → ConfigService (persistent storage)

### Session API - ✅ Complete
- PUT /v1/session/create → ConfigService (persistent storage with TTL)
- PUT /v1/session/destroy/{uuid} → ConfigService (persistent storage)
- GET /v1/session/info/{uuid} → ConfigService (persistent storage with expiry check)
- GET /v1/session/list → ConfigService (persistent storage)
- GET /v1/session/node/{node} → ConfigService (persistent storage)
- PUT /v1/session/renew/{uuid} → ConfigService (persistent storage with TTL renewal)

### Event API - ✅ Complete
- PUT /v1/event/fire/{name} → ConfigService (persistent storage)
- GET /v1/event/list → ConfigService (persistent storage)

### Prepared Query API - ✅ Complete
- POST /v1/query → ConfigService (persistent storage)
- GET /v1/query → ConfigService (persistent storage)
- GET /v1/query/{uuid} → ConfigService (persistent storage)
- PUT /v1/query/{uuid} → ConfigService (persistent storage)
- DELETE /v1/query/{uuid} → ConfigService (persistent storage)
- GET /v1/query/{uuid}/execute → ConfigService + NamingService
- GET /v1/query/{uuid}/explain → ConfigService (persistent storage)

---

## Files to Modify

### Phase 1 (Persistence)
1. ~~`crates/batata-plugin-consul/src/kv.rs` - Add ConfigService integration~~ ✅ DONE
2. ~~`crates/batata-plugin-consul/src/acl.rs` - Add database persistence~~ ✅ DONE
3. ~~`crates/batata-plugin-consul/src/session.rs` - Add distributed storage~~ ✅ DONE
4. ~~`crates/batata-plugin-consul/src/health.rs` - Map to NamingService health~~ ✅ DONE
5. ~~`crates/batata-plugin-consul/src/event.rs` - Add event persistence~~ ✅ DONE
6. ~~`crates/batata-plugin-consul/src/query.rs` - Add prepared query persistence~~ ✅ DONE

### Phase 2 (Cluster)
1. ~~`crates/batata-plugin-consul/src/status.rs` - Query ServerMemberManager~~ ✅ DONE
2. ~~`crates/batata-plugin-consul/src/agent.rs` - Real cluster info via ServerMemberManager~~ ✅ DONE

---

## Implementation Priority

1. ~~**P1-1: KV Store Persistence** - Most commonly used feature~~ ✅ COMPLETE
2. ~~**P1-4: Health Check Persistence** - Critical for service discovery~~ ✅ COMPLETE
3. ~~**P2-1: Status API** - Important for cluster monitoring~~ ✅ COMPLETE
4. ~~**P1-2: ACL Persistence** - Security feature~~ ✅ COMPLETE
5. ~~**P1-3: Session Persistence** - Distributed locking~~ ✅ COMPLETE
6. ~~**P2-2: Agent Operations** - Cluster management~~ ⚠️ PARTIAL (real cluster info available)
7. ~~**P2-3: Real Metrics** - Monitoring~~ ✅ COMPLETE
