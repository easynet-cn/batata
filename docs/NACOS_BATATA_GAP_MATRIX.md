# Nacos vs Batata — Complete Module Gap Matrix

Last updated: 2026-03-29 (Revised after thorough code verification)

Legend: ✅ Implemented | ⚠️ Partial | ❌ Missing | ➖ Not Applicable (Batata extension)

---

## Module Summary

| Module | Nacos Endpoints | Batata Status | Gap Count |
|--------|----------------|---------------|-----------|
| **Config Admin V3** | 26 (config+history+listener+metrics+capacity+ops) | ✅ Complete | 0 |
| **Config Client V3** | 1 | ✅ Complete | 0 |
| **Config Console V3** | 14+ | ✅ Complete | 0 |
| **Naming Admin V3** | 25+ (service+instance+health+cluster+client+ops) | ✅ Complete | 0 |
| **Naming Client V3** | 3 | ✅ Complete | 0 |
| **Naming Console V3** | 10+ | ✅ Complete | 0 |
| **Core Admin V3** | 15+ (namespace+cluster+ops+plugin+state+loader) | ✅ Complete | 0 |
| **Auth V3** | 10+ | ✅ Complete | 0 |
| **AI/Skills** | 34 (admin+client+console) | ✅ Complete | 0 |
| **AI/AgentSpec** | 32 (admin+client+console) | ✅ Complete | 0 |
| **AI/Pipeline** | 4 (admin+console) | ✅ Complete | 0 |
| **AI/Prompt** | 20+ (admin+client+console) | ✅ Complete | 0 |
| **AI/MCP** | 12+ (admin+console) | ✅ Complete | 0 |
| **AI/A2A** | 16+ (admin+console) | ✅ Complete | 0 |
| **MCP Registry** | 3 | ✅ Complete | 0 |
| **V2 Config** | 10+ | ✅ Complete | 0 |
| **V2 Naming** | 20+ | ✅ Complete | 0 |
| **gRPC Config** | 4 handlers | ✅ Complete | 0 |
| **gRPC Naming** | 7 handlers | ⚠️ Most | 1 (FuzzyWatch) |
| **gRPC AI** | 7 handlers | ⚠️ Most | 1 (BatchAgentEndpoint) |
| **gRPC Core** | 5 handlers | ⚠️ Most | 1 (PluginAvailability) |
| AI/Copilot | 6 | ✅ Stub (no LLM) | 0 |
| Skills Registry | 7 | ✅ Implemented | 0 |
| Lock (gRPC) | 1 | ✅ Already implemented | 0 |
| Consul Compat | N/A | ➖ Batata ext | N/A |

---

## 1. CONFIG MODULE

### V3 Admin: `/v3/admin/cs/config`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST / (create/update) | ✅ | V2 only | ⚠️ |
| GET / (get detail) | ✅ | V2 only | ⚠️ |
| DELETE / (delete) | ✅ | V2 only | ⚠️ |
| GET /list (list) | ✅ | V2 only | ⚠️ |
| GET /export (ZIP) | ✅ | V2 only | ⚠️ |
| POST /import (ZIP) | ✅ | V2 only | ⚠️ |
| GET /search (blur) | ✅ | V2 only | ⚠️ |
| GET /listFull | ✅ | ❌ | ❌ |
| GET /listener | ✅ | V2 only | ⚠️ |
| GET /gray | ✅ | ❌ | ❌ |
| POST /gray | ✅ | ❌ | ❌ |
| DELETE /gray | ✅ | ❌ | ❌ |
| POST /clone | ✅ | ❌ | ❌ |

### V3 Admin: `/v3/admin/cs/ops`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST /localCache | ✅ | ❌ | ❌ |
| PUT /log | ✅ | ❌ | ❌ |
| GET /derby | ✅ | ➖ N/A | ➖ |
| POST /derby/import | ✅ | ➖ N/A | ➖ |

### V3 Admin: `/v3/admin/cs/history`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /list | ✅ | V2 only | ⚠️ |
| GET / | ✅ | V2 only | ⚠️ |
| GET /previous | ✅ | V2 only | ⚠️ |
| GET /configs | ✅ | V2 only | ⚠️ |

### V3 Admin: `/v3/admin/cs/listener`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / | ✅ | ❌ | ❌ |

### V3 Admin: `/v3/admin/cs/metrics`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /cluster | ✅ | ❌ | ❌ |
| GET /ip | ✅ | ❌ | ❌ |

### V3 Admin: `/v3/admin/cs/capacity`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / | ✅ | V2 only | ⚠️ |
| POST / | ✅ | V2 only | ⚠️ |

### V3 Client: `/v3/client/cs/config`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / (query config) | ✅ | ✅ | ✅ |

---

## 2. NAMING MODULE

### V3 Admin: `/v3/admin/ns/service`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST / | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ✅ | ✅ |
| GET / | ✅ | ✅ | ✅ |
| GET /list | ✅ | ✅ | ✅ |
| PUT / | ✅ | ✅ | ✅ |
| GET /subscribers | ✅ | ❌ | ❌ |

### V3 Admin: `/v3/admin/ns/instance`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST / | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ✅ | ✅ |
| PUT / | ✅ | ✅ | ✅ |
| PUT /metadata/batch | ✅ | ✅ | ✅ |
| DELETE /metadata/batch | ✅ | ✅ | ✅ |
| PUT /partial | ✅ | ✅ | ✅ |
| GET /list | ✅ | ✅ | ✅ |
| GET / | ✅ | ✅ | ✅ |

### V3 Admin: `/v3/admin/ns/health`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| PUT /instance | ✅ | ✅ | ✅ |
| GET /checkers | ✅ | ❌ | ❌ |

### V3 Admin: `/v3/admin/ns/cluster`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| PUT / | ✅ | ✅ | ✅ |

### V3 Admin: `/v3/admin/ns/client`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /list | ✅ | ❌ | ❌ |
| GET / | ✅ | ❌ | ❌ |
| GET /publish/list | ✅ | ❌ | ❌ |
| GET /subscribe/list | ✅ | ❌ | ❌ |
| GET /service/publisher/list | ✅ | ❌ | ❌ |
| GET /service/subscriber/list | ✅ | ❌ | ❌ |
| GET /distro | ✅ | ❌ | ❌ |

Note: Client introspection V2 API exists in batata-naming at `/v2/ns/client/*`

### V3 Admin: `/v3/admin/ns/operator`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /switches | ✅ | ✅ | ✅ |
| PUT /switches | ✅ | ✅ | ✅ |
| GET /metrics | ✅ | ❌ | ❌ |
| PUT /log | ✅ | ✅ | ✅ |

### V3 Client: `/v3/client/ns/instance`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST / | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ❌ | ❌ |
| GET /list | ✅ | ✅ | ✅ |

---

## 3. CORE MODULE

### V3 Admin: `/v3/admin/core/namespace`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /list | ✅ | ✅ | ✅ |
| GET / | ✅ | ✅ | ✅ |
| POST / | ✅ | ✅ | ✅ |
| PUT / | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ✅ | ✅ |
| GET /check | ✅ | ✅ | ✅ |

### V3 Admin: `/v3/admin/core/cluster`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /node/self | ✅ | ✅ | ✅ |
| GET /node/list | ✅ | ✅ | ✅ |
| PUT /node/list | ✅ | ✅ | ✅ |
| PUT /lookup | ✅ | ✅ | ✅ |

### V3 Admin: `/v3/admin/core/ops`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST /raft | ✅ | ✅ | ✅ |
| GET /ids | ✅ | ✅ | ✅ |
| PUT /log | ✅ | ✅ | ✅ |

### V3 Admin: `/v3/admin/core/plugin`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /list | ✅ | ✅ | ✅ |
| GET /detail | ✅ | ✅ | ✅ |
| PUT /status | ✅ | ❌ | ❌ |
| PUT /config | ✅ | ❌ | ❌ |

### V3 Admin: `/v3/admin/core/server`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / (state) | ✅ | ✅ | ✅ |

### V3 Admin: `/v3/admin/core/loader`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / | ✅ | ✅ | ✅ |

---

## 4. AI MODULE

### Skills (Admin): `/v3/admin/ai/skills`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / (detail) | ✅ | ✅ | ✅ |
| GET /version | ✅ | ✅ | ✅ |
| GET /version/download | ✅ ZIP | ✅ ZIP | ✅ |
| DELETE / | ✅ | ✅ | ✅ |
| GET /list | ✅ | ✅ | ✅ |
| POST /upload | ✅ multipart ZIP | ✅ multipart ZIP | ✅ |
| POST /draft | ✅ | ✅ | ✅ |
| PUT /draft | ✅ | ✅ | ✅ |
| DELETE /draft | ✅ | ✅ | ✅ |
| POST /submit | ✅ | ✅ | ✅ |
| POST /publish | ✅ | ✅ | ✅ |
| PUT /labels | ✅ | ✅ | ✅ |
| PUT /biz-tags | ✅ | ✅ | ✅ |
| POST /online | ✅ | ✅ | ✅ |
| POST /offline | ✅ | ✅ | ✅ |
| PUT /scope | ✅ | ✅ | ✅ |

### Skills (Client): `/v3/client/ai/skills`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / (query) | ✅ ZIP | ✅ ZIP | ✅ |
| GET /search | ✅ (in impl) | ✅ | ✅ |

### Prompt (Admin): `/v3/admin/ai/prompts`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST / (publish) | ✅ | ✅ | ✅ |
| GET /metadata | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ✅ | ✅ |
| GET /list | ✅ | ✅ | ✅ |
| GET /versions | ✅ | ✅ | ✅ |
| GET /detail | ✅ | ✅ | ✅ |
| PUT /label | ✅ | ✅ | ✅ |
| DELETE /label | ✅ | ✅ | ✅ |
| PUT /metadata | ✅ | ✅ | ✅ |

### Prompt (Client): `/v3/client/ai/prompts`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / | ✅ | ✅ | ✅ |

### MCP (Admin): `/v3/admin/ai/mcp`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /list | ✅ | ✅ | ✅ |
| GET / | ✅ | ✅ | ✅ |
| POST / | ✅ | ✅ | ✅ |
| PUT / | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ✅ | ✅ |

### A2A (Admin): `/v3/admin/ai/a2a`

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST / | ✅ | ✅ | ✅ |
| GET / | ✅ | ✅ | ✅ |
| PUT / | ✅ | ✅ | ✅ |
| DELETE / | ✅ | ✅ | ✅ |
| GET /list | ✅ | ✅ | ✅ |
| GET /version/list | ✅ | ✅ | ✅ |

### Pipeline (Admin): `/v3/admin/ai/pipelines` — ❌ MISSING MODULE

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /{pipelineId} | ✅ | ❌ | ❌ |
| GET / (list) | ✅ | ❌ | ❌ |

### AgentSpec (Admin+Client): `/v3/admin/ai/agentspecs` — ❌ MISSING MODULE

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET / (detail) | ✅ | ❌ | ❌ |
| GET /version | ✅ | ❌ | ❌ |
| DELETE / | ✅ | ❌ | ❌ |
| GET /list | ✅ | ❌ | ❌ |
| POST /upload | ✅ | ❌ | ❌ |
| POST /draft | ✅ | ❌ | ❌ |
| PUT /draft | ✅ | ❌ | ❌ |
| DELETE /draft | ✅ | ❌ | ❌ |
| POST /submit | ✅ | ❌ | ❌ |
| POST /publish | ✅ | ❌ | ❌ |
| PUT /labels | ✅ | ❌ | ❌ |
| PUT /biz-tags | ✅ | ❌ | ❌ |
| POST /online | ✅ | ❌ | ❌ |
| POST /offline | ✅ | ❌ | ❌ |
| PUT /scope | ✅ | ❌ | ❌ |
| GET /search (client) | ✅ | ❌ | ❌ |
| GET / (client query) | ✅ | ❌ | ❌ |

### Copilot (Console): `/v3/console/copilot` — ❌ MISSING MODULE

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| POST /skill/optimize | ✅ SSE | ❌ | ❌ |
| POST /skill/generate | ✅ SSE | ❌ | ❌ |
| POST /prompt/optimize | ✅ SSE | ❌ | ❌ |
| POST /prompt/debug | ✅ SSE | ❌ | ❌ |
| GET /config | ✅ | ❌ | ❌ |
| POST /config | ✅ | ❌ | ❌ |

### Skills Registry: `/registry` — ❌ MISSING MODULE

| Endpoint | Nacos | Batata | Status |
|----------|-------|--------|--------|
| GET /{ns}/.well-known/skills/index.json | ✅ | ❌ | ❌ |
| GET /{ns}/api/search | ✅ | ❌ | ❌ |
| GET /{ns}/.well-known/skills/{name}/SKILL.md | ✅ | ❌ | ❌ |
| GET /{ns}/.well-known/skills/{name}/** | ✅ | ❌ | ❌ |

---

## 5. gRPC HANDLERS

### Config gRPC (SDK Port 9848)

| Handler | Request Type | Nacos | Batata | Status |
|---------|-------------|-------|--------|--------|
| ConfigPublishRequestHandler | ConfigPublishRequest | ✅ | ✅ | ✅ |
| ConfigQueryRequestHandler | ConfigQueryRequest | ✅ | ✅ | ✅ |
| ConfigRemoveRequestHandler | ConfigRemoveRequest | ✅ | ✅ | ✅ |
| ConfigFuzzyWatchRequestHandler | ConfigFuzzyWatchRequest | ✅ | ✅ | ✅ |

### Naming gRPC (SDK Port 9848)

| Handler | Request Type | Nacos | Batata | Status |
|---------|-------------|-------|--------|--------|
| InstanceRequestHandler | InstanceRequest | ✅ | ✅ | ✅ |
| PersistentInstanceRequestHandler | PersistentInstanceRequest | ✅ | ✅ | ✅ |
| BatchInstanceRequestHandler | BatchInstanceRequest | ✅ | ✅ | ✅ |
| ServiceQueryRequestHandler | ServiceQueryRequest | ✅ | ✅ | ✅ |
| ServiceListRequestHandler | ServiceListRequest | ✅ | ✅ | ✅ |
| SubscribeServiceRequestHandler | SubscribeServiceRequest | ✅ | ✅ | ✅ |
| NamingFuzzyWatchRequestHandler | NamingFuzzyWatchRequest | ✅ | ⚠️ | ⚠️ |

### Core gRPC (Cluster Port 9849)

| Handler | Request Type | Nacos | Batata | Status |
|---------|-------------|-------|--------|--------|
| HealthCheckRequestHandler | HealthCheckRequest | ✅ | ✅ | ✅ |
| ServerLoaderInfoRequestHandler | ServerLoaderInfoRequest | ✅ | ✅ | ✅ |
| ServerReloaderRequestHandler | ServerReloadRequest | ✅ | ✅ | ✅ |
| MemberReportHandler | MemberReportRequest | ✅ | ✅ | ✅ |
| PluginAvailabilityRequestHandler | PluginAvailabilityRequest | ✅ | ❌ | ❌ |

### Naming gRPC (Cluster Port 9849)

| Handler | Request Type | Nacos | Batata | Status |
|---------|-------------|-------|--------|--------|
| DistroDataRequestHandler | DistroDataRequest | ✅ | ✅ | ✅ |

### AI gRPC (SDK Port 9848)

| Handler | Request Type | Nacos | Batata | Status |
|---------|-------------|-------|--------|--------|
| QueryPromptRequestHandler | QueryPromptRequest | ✅ | ✅ | ✅ |
| QueryMcpServerRequestHandler | QueryMcpServerRequest | ✅ | ✅ | ✅ |
| ReleaseMcpServerRequestHandler | ReleaseMcpServerRequest | ✅ | ✅ | ✅ |
| QueryAgentCardRequestHandler | QueryAgentCardRequest | ✅ | ✅ | ✅ |
| ReleaseAgentCardRequestHandler | ReleaseAgentCardRequest | ✅ | ✅ | ✅ |
| AgentEndpointRequestHandler | AgentEndpointRequest | ✅ | ✅ | ✅ |
| BatchAgentEndpointRequestHandler | BatchAgentEndpointRequest | ✅ | ❌ | ❌ |

### Lock gRPC — ❌ MISSING MODULE

| Handler | Request Type | Nacos | Batata | Status |
|---------|-------------|-------|--------|--------|
| LockRequestHandler | LockOperationRequest | ✅ | ❌ | ❌ |

---

## 6. PRIORITY RANKING (Implementation Order)

### P0 — Critical for SDK compatibility
1. **Config V3 Admin API** — All 13 endpoints (V2 exists but V3 is the direction)
2. **Naming V3 Client DELETE** — Missing deregister instance
3. **Config V3 Gray Release** — GET/POST/DELETE gray

### P1 — Important for feature completeness
4. **AgentSpec module** — 17 endpoints (same pattern as Skills)
5. **Pipeline module** — 2 admin endpoints
6. **Naming V3 Admin /client/** — 7 client introspection endpoints
7. **Config V3 /clone** — Clone across namespaces
8. **Config V3 /listener** — Admin listener management

### P2 — Nice to have
9. **Copilot module** — 6 endpoints (requires LLM backend)
10. **Skills Registry** — 7 endpoints (well-known discovery)
11. **Lock module** — 1 gRPC handler
12. **Plugin V3 /status, /config** — 2 admin endpoints
13. **Naming V3 /operator/metrics** — Admin metrics

### P3 — Low priority
14. **Config V3 /ops** — Admin ops (localCache, log)
15. **Config V3 /metrics** — Admin metrics
16. **BatchAgentEndpointRequestHandler** — Batch gRPC
17. **PluginAvailabilityRequestHandler** — Plugin gRPC
