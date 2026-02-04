# Batata vs Nacos Feature Comparison

This document provides an honest comparison between Batata and the original Nacos project, identifying features that are truly implemented vs. those that are claimed but not working, and features that are completely missing.

> Generated: 2024-02-04
> Based on: Nacos 3.x and Batata current codebase

---

## Summary

| Category | Truly Implemented | Claimed but Not Working | Missing from Nacos |
|----------|------------------|------------------------|-------------------|
| Core APIs | ~70% | ~10% | ~20% |
| Advanced Features | ~40% | ~30% | ~30% |
| AI/Cloud Native | ~5% | ~25% | ~70% |

---

## Part 1: Features Claimed as Complete but NOT Actually Working

These features are marked as complete in `docs/TASK_TRACKER.md` but are **NOT integrated** into the running server:

### 1.1 AI Features (v2.1.0 - MCP & A2A)

| Task ID | Description | Actual Status | Issue |
|---------|-------------|---------------|-------|
| MCP-001~008 | MCP Server Registry | **NOT WORKING** | Code exists in `api/ai/mcp.rs` but `configure()` is NOT called in `startup/http.rs` |
| A2A-001~005 | A2A Agent Registry | **NOT WORKING** | Code exists in `api/ai/a2a.rs` but NOT wired to HTTP server |

**Location of unintegrated code:**
- `/crates/batata-server/src/api/ai/mcp.rs` (728 lines)
- `/crates/batata-server/src/api/ai/a2a.rs` (703 lines)

**Why it doesn't work:** The `configure()` functions are defined but never called in the HTTP server setup.

### 1.2 Cloud Native Integration (v2.2.0)

| Task ID | Description | Actual Status | Issue |
|---------|-------------|---------------|-------|
| K8S-001~005 | Kubernetes Sync | **NOT WORKING** | Config structures only, no actual K8s API integration |
| PROM-001~003 | Prometheus SD | **NOT WORKING** | Endpoint defined but NOT registered in HTTP routes |

**Location of unintegrated code:**
- `/crates/batata-server/src/api/cloud/kubernetes.rs` (614 lines)
- `/crates/batata-server/src/api/cloud/prometheus.rs` (629 lines)

### 1.3 Plugin Ecosystem (v2.3.0)

| Task ID | Description | Actual Status | Issue |
|---------|-------------|---------------|-------|
| PLG-001~004 | Control Plugin | **PARTIAL** | In-memory only, not integrated with server |
| PLG-101~104 | Webhook Plugin | **NOT WORKING** | Code exists but not integrated |
| PLG-201~203 | CMDB Plugin | **NOT WORKING** | Code exists but not integrated |

### 1.4 Advanced Features (v2.4.0)

| Task ID | Description | Actual Status | Issue |
|---------|-------------|---------------|-------|
| ADV-001~005 | Distributed Lock | **PARTIAL** | `MemoryLockService` exists, Raft integration incomplete |

---

## Part 2: Features Missing Compared to Nacos

These are core Nacos features that Batata does NOT implement:

### 2.1 V1 API Endpoints (Nacos Legacy API)

Nacos provides V1 APIs that many legacy clients depend on. Batata only has V2 APIs.

| Endpoint | Nacos | Batata |
|----------|-------|--------|
| `POST /nacos/v1/ns/instance/beat` | ✓ Heartbeat | ❌ Missing |
| `POST /nacos/v1/cs/configs/listener` | ✓ Long-polling | ❌ Missing |
| `GET /nacos/v1/ns/operator/servers` | ✓ Server list | ❌ Missing |
| `GET /nacos/v1/ns/raft/leader` | ✓ Raft leader | ❌ Missing |
| `GET /nacos/v1/ns/catalog/services` | ✓ Catalog | ❌ Missing |

**Impact:** Clients using Nacos 1.x SDK will not work with Batata.

### 2.2 V3 Console API (Nacos 3.x)

Nacos 3.x introduced new console APIs that replace V1:

| Feature | Nacos 3.x | Batata |
|---------|-----------|--------|
| Console independent deployment | ✓ | ✓ Partial |
| New v3 Console APIs | ✓ | ❌ Missing |
| Admin APIs for maintainers | ✓ | ❌ Missing |
| MCP Registry UI | ✓ | ❌ Missing |

### 2.3 Dynamic DNS Service

| Feature | Nacos | Batata |
|---------|-------|--------|
| DNS-based service discovery | ✓ | ❌ Missing |
| Weighted DNS routing | ✓ | ❌ Missing |
| DNS record management | ✓ | ❌ Missing |

### 2.4 Config Long-Polling / Listener

| Feature | Nacos | Batata |
|---------|-------|--------|
| HTTP Long-polling for config changes | ✓ | ❌ Missing (gRPC only) |
| `/v1/cs/configs/listener` endpoint | ✓ | ❌ Missing |
| MD5 change detection | ✓ | ❌ Unknown |

### 2.5 Instance Heartbeat Mechanism

| Feature | Nacos | Batata |
|---------|-------|--------|
| HTTP Heartbeat (`/v1/ns/instance/beat`) | ✓ | ❌ Missing |
| Heartbeat interval configuration | ✓ | ❌ Missing |
| Heartbeat-based health detection | ✓ | ❌ Partial (gRPC only) |

### 2.6 gRPC Protocol Features

| Feature | Nacos | Batata |
|---------|-------|--------|
| Bi-directional streaming | ✓ | ✓ |
| Connection management | ✓ | ✓ |
| Config batch listen | ✓ | ✓ |
| Fuzzy watch | ✓ | ✓ |
| Client detection | ✓ | ✓ |

### 2.7 Console UI Features

| Feature | Nacos | Batata |
|---------|-------|--------|
| Web UI Console | ✓ Built-in | ❌ No UI (API only) |
| Config editor with syntax highlighting | ✓ | ❌ |
| Service topology visualization | ✓ | ❌ |
| Real-time monitoring dashboard | ✓ | ❌ |
| User/Role management UI | ✓ | ❌ |
| Namespace management UI | ✓ | ❌ |
| Config comparison/diff view | ✓ | ❌ |
| Gray release management UI | ✓ | ❌ |

### 2.8 Plugin System (Nacos Native)

| Plugin Type | Nacos | Batata |
|-------------|-------|--------|
| Auth plugins | ✓ LDAP, OAuth | ✓ LDAP only |
| Config encryption plugins | ✓ Multiple | ✓ AES only |
| Config change plugins | ✓ Webhook, etc. | ❌ Incomplete |
| Instance weight plugins | ✓ | ❌ Missing |
| Selector plugins | ✓ | ❌ Missing |
| Environment plugins | ✓ | ❌ Missing |

### 2.9 Cluster Features

| Feature | Nacos | Batata |
|---------|-------|--------|
| Raft consensus | ✓ | ✓ |
| Distro protocol (AP) | ✓ | ✓ Partial |
| Cluster health reporting | ✓ | ✓ |
| Leader election | ✓ | ✓ |
| Log compaction | ✓ | ❓ Unknown |
| Snapshot management | ✓ | ❓ Unknown |

### 2.10 Nacos 3.x New Features

| Feature | Nacos 3.x | Batata |
|---------|-----------|--------|
| xDS protocol (EDS/LDS/RDS/CDS) | ✓ | ✓ Code exists, integration unclear |
| MCP Management UI | ✓ | ❌ Missing |
| A2A Registry | ✓ | ❌ Not working |
| Distributed Lock | ✓ Experimental | ❌ Partial |
| Fuzzy Listen | ✓ Experimental | ✓ Implemented |
| K8s Sync | ✓ | ❌ Not working |
| Default Auth enabled | ✓ | ✓ |
| JDK 17 requirement | ✓ | N/A (Rust) |

---

## Part 3: Features Actually Implemented and Working

These features are genuinely implemented and should work:

### 3.1 V2 Open API (HTTP)

| Endpoint | Status | Notes |
|----------|--------|-------|
| `GET/POST/DELETE /nacos/v2/cs/config` | ✓ Working | Full config CRUD |
| `GET /nacos/v2/cs/history/*` | ✓ Working | History list, detail, previous |
| `POST/DELETE/PUT/GET /nacos/v2/ns/instance` | ✓ Working | Instance CRUD |
| `GET /nacos/v2/ns/instance/list` | ✓ Working | Instance list |
| `PUT/DELETE /nacos/v2/ns/instance/metadata/batch` | ✓ Working | Batch metadata |
| `POST/DELETE/PUT/GET /nacos/v2/ns/service` | ✓ Working | Service CRUD |
| `GET /nacos/v2/ns/service/list` | ✓ Working | Service list |
| `GET/PUT /nacos/v2/ns/operator/switches` | ✓ Working | System switches |
| `GET /nacos/v2/ns/operator/metrics` | ✓ Working | Metrics |
| `PUT /nacos/v2/ns/health/instance` | ✓ Working | Health update |
| `GET /nacos/v2/core/cluster/node/*` | ✓ Working | Cluster info |
| `GET/POST/PUT/DELETE /nacos/v2/console/namespace` | ✓ Working | Namespace CRUD |

### 3.2 gRPC Handlers

| Handler | Status |
|---------|--------|
| ConfigQueryHandler | ✓ Working |
| ConfigPublishHandler | ✓ Working |
| ConfigRemoveHandler | ✓ Working |
| ConfigBatchListenHandler | ✓ Working |
| ConfigFuzzyWatchHandler | ✓ Working |
| InstanceRequestHandler | ✓ Working |
| BatchInstanceRequestHandler | ✓ Working |
| ServiceListRequestHandler | ✓ Working |
| SubscribeServiceRequestHandler | ✓ Working |
| HealthCheckHandler | ✓ Working |
| ConnectionSetupHandler | ✓ Working |
| DistroDataSyncHandler | ✓ Working |

### 3.3 Consul API Compatibility

| API | Status | Notes |
|-----|--------|-------|
| Agent API | ✓ Working | Service registration/deregistration |
| Catalog API | ✓ Working | Service discovery |
| Health API | ✓ Working | Health checks |
| KV API | ✓ Working | Key-value store |
| ACL API | ✓ Working | Access control |

### 3.4 Authentication & Authorization

| Feature | Status |
|---------|--------|
| JWT-based auth | ✓ Working |
| RBAC | ✓ Working |
| LDAP integration | ✓ Working |
| Permission macros | ✓ Working |
| Rate limiting | ✓ Working |

### 3.5 Database Support

| Database | Status |
|----------|--------|
| MySQL | ✓ Working |
| PostgreSQL | ✓ Working |

---

## Part 4: Recommendations

### High Priority (Must Fix for Nacos Compatibility)

1. **Add V1 API endpoints** - Many clients still use V1 APIs
2. **Implement heartbeat endpoint** - Critical for service health
3. **Add config listener (long-polling)** - Required for HTTP-based clients
4. **Wire AI/Cloud features to HTTP server** - Currently orphaned code

### Medium Priority

1. **Implement DNS-based service discovery**
2. **Add console Web UI**
3. **Complete distributed lock with Raft**
4. **Add V3 Admin APIs**

### Low Priority

1. **Add more encryption plugins**
2. **Implement MCP management UI**
3. **Add config diff/comparison**

---

## References

- [Nacos GitHub](https://github.com/alibaba/nacos)
- [Nacos Open API Guide](https://nacos.io/en/docs/open-api/)
- [Nacos v2 Open API](https://nacos.io/en/docs/v2/guide/user/open-api/)
- [Nacos Releases](https://github.com/alibaba/nacos/releases)
