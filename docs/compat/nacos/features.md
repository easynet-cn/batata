# Nacos Feature Inventory

Upstream baseline: **Nacos 3.3.0 GA (released 2026-10-29)**. The inventory was
originally labelled `3.3.0-SNAPSHOT` and later `3.3.0-RC`; see **Baseline
alignment** below for how the GA source was verified.

> Purpose & scope: track batata's Nacos **compatibility** implementation status.
> Nacos exposes **four** contract surfaces:
> 1. gRPC **data plane** (SDK / cluster, the service contract)
> 2. HTTP **console/management plane** (`v3/console/**`, used by `console-ui-next`)
> 3. HTTP **admin plane** (`v3/admin/**`, used by maintainer SDK / open platform)
> 4. HTTP **client plane** (`v3/client/**`, used by SDK v2 HTTP fallback)
>
> All four surfaces are tracked here.
>
> Granularity: each row is a **single external contract** (a gRPC request type or an HTTP resource group). Upstream implementation class (controller/WebFlux/handler) is not tracked — only the protocol contract matters.

Status: `🟢 full` | `🟡 partial` | `⚡ in-progress` | `⚪ planned` | `⛔ missing`

> Status source: verified against batata source code (`crates/`) on 2025-08-08; the
> maintainer/client SDK rows were re-verified on **2026-11-09** (see
> `docs/compat/review-report.md`). The `batata impl` column is a **source anchor**:
> for every `🟢` row it must name a crate/module that exists under `crates/`
> (enforced by `scripts/compat_check.py`). A row is `🟡` when only part of the
> upstream contract is exposed by the Rust SDK, even if the server implements it.

## Baseline alignment (2026-11-09)

**Alignment target: Nacos 3.3.0 GA** (released 2026-10-29, per the official
release history). All three version statements in the repository now point at
it:

| Where | Version | Meaning |
|-------|---------|---------|
| `sdk-tests/nacos-java-tests/pom.xml:19` | `3.3.0` | Java SDK pinned in batata's E2E matrix |
| `conf/application.yml` + `deploy/k8s/configmap.yaml` | `3.3.0` | Version batata reports as its Nacos compatibility target |
| Local `~/work/github/easynet-cn/nacos` develop `pom.xml:91` | `3.3.0` (@ `d1b97bc3e`, 2026-11-09) | **Post-GA develop snapshot**; the GA release commit `b64bc9118` is now in local history, but there are still no local tags |

The GA contract was verified against the official `-sources.jar` for
`nacos-api`, `nacos-client` and `nacos-maintainer-client` 3.3.0 from Maven
Central, because the local checkout had no tag when the alignment was done.
Diffing GA against the older RC snapshot shows the two are identical for
`ConfigService`, `NamingService`, `LockService` and the whole
`maintainer-client`; only the `api/ai` package changed (9 files modified, 4
added: RAD discovery capability, `AgentConverter` SPI, `AgentPublishRequest`
`autoSubmit`). The refreshed checkout is 11 commits past the GA release; four
of those commits touch client SDK sources and are recorded as **develop drift**
in `docs/compat/review-report.md` §8 — they are *not* part of the declared GA
baseline. The older `3.3.0-SNAPSHOT` heading was a stale 2025-08-08 label.

Consequence: capabilities introduced in 3.3.0 (config 304 conditional GET, MCP
version lifecycle, AI maintainer delegates, RAD routing) are listed here
because they exist upstream, but the Rust SDKs do not expose all of them yet.
The E2E matrix is now pinned to 3.3.0, so future runs can exercise them. See
`docs/compat/review-report.md` for commit hashes and the client-SDK gap list.

---

# Part A — gRPC data plane (core)

> gRPC via `nacos_grpc_service.proto` services `Request` + `BiRequestStream`, each transporting `Payload{metadata.type, Any.body}`. `type` is a **Java class simple name** and must match exactly for interop.

## A.1. SDK connection / management (`CONN`)

| ID | Contract (gRPC type) | Direction | batata impl | Status | Tests | Notes |
|----|----------------------|-----------|-------------|--------|-------|-------|
| F-NAC-CONN-001 | `ServerCheckRequest/Response` (connectionId, ability) | C→S | batata-core | 🟢 | | handshake probe |
| F-NAC-CONN-002 | `HealthCheckRequest` | C→S | batata-core | 🟢 | | keep-alive |
| F-NAC-CONN-003 | `ConnectionSetupRequest` (version/labels/ability/tenant) | C→S | batata-core | 🟢 | | bi-stream 1st packet |
| F-NAC-CONN-004 | `SetupAckRequest` | S→C | batata-core | 🟢 | | handshake done |
| F-NAC-CONN-005 | `ConnectResetRequest` | S→C | batata-core | 🟢 | | force switch server |
| F-NAC-CONN-006 | `ClientDetectionRequest` | S→C | batata-core | 🟢 | | server probes client |
| F-NAC-CONN-007 | `PushAckRequest` | C→S | batata-core | 🟢 | | |
| F-NAC-CONN-008 | `ServerLoaderInfoRequest/Response` | C→S | batata-core | 🟢 | | |
| F-NAC-CONN-009 | `ServerReloadRequest/Response` | C→S | batata-core | 🟢 | | |
| F-NAC-CONN-010 | TLS (SDK negotiator, cert refresh) | C↔S | batata-core | 🟢 | | optional |
| F-NAC-CONN-011 | gRPC auth via headers (`accessToken`) | C→S | batata-auth | 🟢 | | `@Secured` handlers |

## A.2. Config (`CFG`) — SDK

| ID | Contract (gRPC type) | Direction | batata impl | Status | Tests | Notes |
|----|----------------------|-----------|-------------|--------|-------|-------|
| F-NAC-CFG-001 | `ConfigQueryRequest/Response` | C→S | batata-config | 🟢 | | |
| F-NAC-CFG-002 | `ConfigPublishRequest/Response` | C→S | batata-config | 🟢 | | |
| F-NAC-CFG-003 | `ConfigRemoveRequest/Response` | C→S | batata-config | 🟢 | | |
| F-NAC-CFG-004 | `ConfigBatchListenRequest` | C→S | batata-config | 🟢 | | |
| F-NAC-CFG-005 | `ConfigChangeBatchListenResponse` | S→C | batata-config | 🟢 | | |
| F-NAC-CFG-006 | `ConfigChangeNotifyRequest/Response` | S→C | batata-config | 🟢 | | |
| F-NAC-CFG-007 | `ClientConfigMetricRequest/Response` | C→S | batata-config | 🟢 | | |
| F-NAC-CFG-008 | `ConfigFuzzyWatchRequest/Response` | C→S | batata-config | 🟢 | | |
| F-NAC-CFG-009 | `ConfigFuzzyWatchSyncRequest/Response` | S→C | batata-config | 🟢 | | |
| F-NAC-CFG-010 | `ConfigFuzzyWatchChangeNotifyRequest/Response` | S→C | batata-config | 🟢 | | |

## A.3. Naming (`NS`) — SDK

| ID | Contract (gRPC type) | Direction | batata impl | Status | Tests | Notes |
|----|----------------------|-----------|-------------|--------|-------|-------|
| F-NAC-NS-001 | `InstanceRequest` (register) | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-002 | `InstanceRequest` (deregister) | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-003 | `BatchInstanceRequest` | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-004 | `PersistentInstanceRequest` | C→S | batata-naming | 🟢 | | CP/Raft |
| F-NAC-NS-005 | `ServiceQueryRequest` | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-006 | `SubscribeServiceRequest` | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-007 | `ServiceListRequest` | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-008 | `NotifySubscriberRequest` | S→C | batata-naming | 🟢 | | |
| F-NAC-NS-009 | `NamingFuzzyWatchRequest/Response` | C→S | batata-naming | 🟢 | | |
| F-NAC-NS-010 | `NamingFuzzyWatchSyncRequest/Response` | S→C | batata-naming | 🟢 | | |
| F-NAC-NS-011 | `NamingFuzzyWatchChangeNotifyRequest/Response` | S→C | batata-naming | 🟢 | | |

## A.4. Cluster-internal (`CLUSTER`)

| ID | Contract (gRPC type) | Direction | batata impl | Status | Tests | Notes |
|----|----------------------|-----------|-------------|--------|-------|-------|
| F-NAC-CLUSTER-001 | `DistroDataRequest` | S↔S | batata-core | 🟢 | | distro sync |
| F-NAC-CLUSTER-002 | `MemberReportRequest/Response` | S→S | batata-core | 🟢 | | |
| F-NAC-CLUSTER-003 | `PluginAvailabilityRequest/Response` | S→S | batata-core/handler/cluster.rs | 🟢 | | cluster-internal plugin status query |
| F-NAC-CLUSTER-004 | `PluginConfigStorageTypeRequest` | S→S | | ⛔ | | not in upstream Nacos; batata extension not needed |
| F-NAC-CLUSTER-005 | `ConfigChangeClusterSyncRequest/Response` | S→S | batata-config | 🟢 | | |

## A.5. Lock Service (`LOCK`)

> gRPC `LockOperationRequest` via `Payload` protocol. Batata implements in-memory + Raft modes.

| ID | Contract (gRPC type) | Direction | batata impl | Status | Tests | Notes |
|----|----------------------|-----------|-------------|--------|-------|-------|
| F-NAC-LOCK-001 | `LockOperationRequest` (ACQUIRE) | C→S | batata-core/lock.rs | 🟢 | | basic lock |
| F-NAC-LOCK-002 | `LockOperationRequest` (RELEASE) | C→S | batata-core/lock.rs | 🟢 | | basic unlock |
| F-NAC-LOCK-003 | Lock auto-expiration (TTL) | C↔S | batata-core/lock.rs | 🟢 | | background expiry task |
| F-NAC-LOCK-004 | Lock renewal (lease renew) | C→S | batata-consistency/raft | 🟢 | | RaftRequest::LockRenew |
| F-NAC-LOCK-005 | Reentrant lock (same owner re-acquire) | C→S | batata-core/lock.rs | 🟢 | | DashMap + fencing tokens |
| F-NAC-LOCK-006 | Non-reentrant lock | C→S | batata-core/lock.rs | 🟢 | | |
| F-NAC-LOCK-007 | Wait queue (FIFO, timeout, cancel) | C↔S | batata-consistency/raft | 🟢 | | server-side waiter queue |
| F-NAC-LOCK-008 | Connection cleanup releases locks | S→C | batata-core | 🟢 | | disconnect → force release |
| F-NAC-LOCK-009 | Watchdog auto-renew (`LockOperationEnum.RENEW`) | C↔S | batata-core/handler/lock.rs | 🟢 | | Closed 2026-11-11 (batch 20). GA `b64bc9118` source-verified: `LockRequestHandler.java:95-101` dispatches `RENEW`, rejects `expiredTime == 0` and returns a raw Boolean; `LockOperationServiceImpl.java:487-492` treats `expiredTime` as a duration; `AbstractAtomicLock.renew()` refreshes only the expiry timestamp. `crates/batata-core/src/handler/lock.rs` now has the `RENEW` arm, a shared `resolve_owner()` (payload `owner` → connectionId fallback) used by ACQUIRE/RELEASE/**RENEW**, and `raft_renew()` — the `RaftRequest::LockRenew` variant / `apply_lock_renew` state machine already existed but had **no producer**, so the earlier "no Raft renew request" claim was overstated and no new variant was added. `crates/batata-core/src/service/lock.rs::renew()` covers standalone mode and never changes `lock_count`. Client `renew()` + watchdog + owner wire are in `F-NAC-CSDK-002`. |
| F-NAC-LOCK-010 | Backward compat (old client without owner) | C→S | batata-core | 🟢 | | uses connectionId fallback |
| F-NAC-LOCK-011 | HTTP admin `GET /v3/admin/core/lock/list` | — | batata-server/api/v3/admin/core/lock.rs | 🟢 | | lock list endpoint |

## A.6. AI gRPC SDK (`AISDK`)

> gRPC via `Payload` protocol. AI request types are defined in `batata-api/src/remote/model.rs`.

| ID | Contract (gRPC type) | Direction | batata impl | Status | Tests | Notes |
|----|----------------------|-----------|-------------|--------|-------|-------|
| F-NAC-AISDK-001 | `ReleaseMcpServerRequest` | C→S | batata-ai/handler | 🟢 | | MCP server release/unregister |
| F-NAC-AISDK-002 | `QueryMcpServerRequest` | C→S | batata-ai/handler | 🟢 | | MCP server query |
| F-NAC-AISDK-003 | `McpServerEndpointRequest` | C→S | batata-ai/handler | 🟢 | | MCP endpoint register/deregister |
| F-NAC-AISDK-004 | `ReleaseAgentCardRequest` | C→S | batata-ai/handler | 🟢 | | A2A agent card release |
| F-NAC-AISDK-005 | `QueryAgentCardRequest` | C→S | batata-ai/handler | 🟢 | | A2A agent card query |
| F-NAC-AISDK-006 | `AgentEndpointRequest` | C→S | batata-ai/handler | 🟢 | | A2A endpoint register/deregister |
| F-NAC-AISDK-007 | `QueryPromptRequest` | C→S | batata-ai/handler | 🟢 | | prompt query |
| F-NAC-AISDK-008 | Agent search/discover/subscribe | C→S | batata-ai/handler | 🟢 | | RAD protocol: AgentSearchRpcRequest + AgentDiscoveryRpcRequest + EndpointRegister/Deregister gRPC handlers |
| F-NAC-AISDK-009 | Skill download (gRPC) | C→S | | ⛔ | | HTTP-only; upstream gRPC throws SERVER_NOT_IMPLEMENTED |
| F-NAC-AISDK-010 | AgentSpec query (gRPC) | C→S | | ⛔ | | HTTP-only; upstream gRPC throws SERVER_NOT_IMPLEMENTED |

---

# Part B — HTTP management plane

## B.1. Core / cluster / state / ops — console (`F-NAC-CORE-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-NAC-CORE-001 | GET `v3/console/core/namespace/list` | batata-console | 🟢 | | namespace list |
| F-NAC-CORE-002 | GET `v3/console/core/namespace` (namespaceId) | batata-console | 🟢 | | detail |
| F-NAC-CORE-003 | POST/PUT/DELETE `v3/console/core/namespace` | batata-console | 🟢 | | create/update/delete |
| F-NAC-CORE-004 | GET `v3/console/core/cluster/nodes?keyword=` | batata-console | 🟢 | | nodes |
| F-NAC-CORE-005 | POST `v3/console/core/cluster/server/leave` | batata-console/v3/cluster.rs | 🟢 | | member leave + self-protection |
| F-NAC-CORE-006 | GET `v3/console/server/state` | batata-console | 🟢 | | server/version |
| F-NAC-CORE-007 | GET `v3/console/server/announcement?language=` | batata-console | 🟢 | | |
| F-NAC-CORE-008 | GET `v3/console/server/guide` | batata-console | 🟢 | | |
| F-NAC-CORE-009 | GET `v3/console/plugin/list?pluginType=` | batata-console | 🟢 | | |
| F-NAC-CORE-010 | GET `v3/console/plugin` (pluginType/pluginName) | batata-console | 🟢 | | detail |
| F-NAC-CORE-011 | PUT `v3/console/plugin/status` | batata-console | 🟢 | | enable/disable |
| F-NAC-CORE-012 | GET `v3/console/plugin/availability` | batata-console | 🟢 | | |
| F-NAC-CORE-013 | PUT `v3/console/plugin/config` | batata-console | 🟢 | | |
| F-NAC-CORE-014 | GET `v3/console/health/liveness` | batata-console | 🟢 | | k8s probe |
| F-NAC-CORE-015 | GET `v3/console/health/readiness` | batata-console | 🟢 | | k8s probe |

## B.2. Config admin — console (`F-NAC-CFG-`)

> All under `v3/console/cs/config`.

| ID | HTTP action (base + path) | batata impl | Status | Tests | Notes |
|----|----------------------------|-------------|--------|-------|-------|
| F-NAC-CFG-011 | GET `.../config/list` | batata-config | 🟢 | | paged |
| F-NAC-CFG-012 | GET `.../config/searchDetail` | batata-config | 🟢 | | full-text search |
| F-NAC-CFG-013 | GET `.../config` (dataId/group/namespaceId) | batata-config | 🟢 | | detail |
| F-NAC-CFG-014 | POST `.../config` | batata-config | 🟢 | | publish/update |
| F-NAC-CFG-015 | DELETE `.../config` (dataId,group,namespace) | batata-config | 🟢 | | delete single |
| F-NAC-CFG-016 | DELETE `.../config/batchDelete` (ids,namespace) | batata-config | 🟢 | | batch delete |
| F-NAC-CFG-017 | GET `.../history/list` | batata-config | 🟢 | | |
| F-NAC-CFG-018 | GET `.../history` (nid,dataId,group,namespace) | batata-config | 🟢 | | detail |
| F-NAC-CFG-019 | GET `.../history/previous` | batata-config | 🟢 | | prev diff |
| F-NAC-CFG-020 | GET `.../config/beta` | batata-config | 🟢 | | query beta |
| F-NAC-CFG-021 | POST `.../config` (header `betaIps`) | batata-config | 🟢 | | publish beta |
| F-NAC-CFG-022 | DELETE `.../config/beta` | batata-config | 🟢 | | stop beta |
| F-NAC-CFG-023 | GET `.../config/listener` | batata-config | 🟢 | | by config |
| F-NAC-CFG-024 | GET `.../config/listener/ip?ip=` | batata-config | 🟢 | | by IP |
| F-NAC-CFG-025 | POST `.../config/import` (multipart ZIP) | batata-config | 🟢 | | import |
| F-NAC-CFG-026 | GET `.../config/export2` | batata-config | 🟢 | | export |
| F-NAC-CFG-027 | POST `.../config/clone` | batata-config | 🟢 | | clone config |
| F-NAC-CFG-028 | POST/DELETE `.../config/gray` | batata-config | 🟢 | | gray publish (tag-based) |
| F-NAC-CFG-029 | GET `.../config/gray/info` | batata-config | 🟢 | | gray info |

## B.3. Naming admin — console (`F-NAC-NS-`)

> All under `v3/console/ns`.

| ID | HTTP action (base + path) | batata impl | Status | Tests | Notes |
|----|----------------------------|-------------|--------|-------|-------|
| F-NAC-NS-012 | GET `ns/service/list` | batata-naming | 🟢 | | |
| F-NAC-NS-013 | GET `ns/service` (ns,service,group) | batata-naming | 🟢 | | detail |
| F-NAC-NS-014 | POST/PUT/DELETE `ns/service` | batata-naming | 🟢 | | create/update/delete |
| F-NAC-NS-015 | GET `ns/service/selector/types` | batata-naming | 🟢 | | |
| F-NAC-NS-016 | PUT `ns/service/cluster` | batata-naming | 🟢 | | update cluster |
| F-NAC-NS-017 | GET `ns/instance/list` | batata-naming | 🟢 | | |
| F-NAC-NS-018 | PUT `ns/instance` (incl. health) | batata-naming | 🟢 | | |
| F-NAC-NS-019 | DELETE `ns/instance` | batata-naming | 🟢 | | |
| F-NAC-NS-020 | GET `ns/service/subscribers` | batata-naming | 🟢 | | |

## B.4. Auth — console (`F-NAC-AUTH-`)

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-AUTH-001 | POST `v3/auth/user/login` (JWT) | batata-auth | 🟢 | | |
| F-NAC-AUTH-002 | POST `v3/auth/user/admin` | batata-auth | 🟢 | | |
| F-NAC-AUTH-003 | GET/POST/PUT/DELETE `v3/auth/user(/list)` | batata-auth | 🟢 | | list/create/reset/delete |
| F-NAC-AUTH-004 | GET/POST/DELETE `v3/auth/role(/list)` | batata-auth | 🟢 | | |
| F-NAC-AUTH-005 | GET/POST/DELETE `v3/auth/permission(/list)` | batata-auth | 🟢 | | grant/revoke |
| F-NAC-AUTH-006 | GET `v1/auth/oidc/login` | batata-server-common/auth/v3/oidc.rs | 🟢 | | OIDC redirect + callback + cookie |
| F-NAC-AUTH-007 | GET `v1/auth/oidc/logout?redirect=` | batata-server-common/auth/v3/oidc.rs | 🟢 | | OIDC logout + optional IdP redirect |

## B.5. AI management — console (`F-NAC-AI-`)

> Console-ui dedicated AI surface. All verified against batata source on 2025-08-08.

| ID | AI area | batata impl | Status | Tests | Notes |
|----|---------|-------------|--------|-------|-------|
| F-NAC-AI-001 | Agent CRUD/draft/publish/online/offline `v3/console/ai/agents` | batata-console/v3/ai_agent.rs | 🟢 | | list/detail/draft/force-publish/labels/offline/online/delete |
| F-NAC-AI-002 | AgentSpec CRUD + upload `v3/console/ai/agentspecs` | batata-console/v3/ai_agentspec.rs | 🟢 | | draft/publish/upload/list/versions |
| F-NAC-AI-003 | Skill CRUD + upload `v3/console/ai/skills` | batata-console/v3/ai_skill.rs | 🟢 | | upload/precheck/batch/draft/publish |
| F-NAC-AI-004 | Prompt CRUD `v3/console/ai/prompt` | batata-console/v3/ai_prompt.rs | 🟢 | | draft/publish/governance/download/labels |
| F-NAC-AI-005 | MCP server CRUD `v3/console/ai/mcp` | batata-console/v3/ai_mcp.rs | 🟢 | | create/update/list/get/delete + import tools |
| F-NAC-AI-006 | AI resource import `v3/console/ai/import` | batata-console/v3/ai_import.rs | 🟢 | | generic importer: sources/search/validate/execute |
| F-NAC-AI-007 | A2A agent card `v3/console/ai/a2a` | batata-console/v3/ai_a2a.rs | 🟢 | | register/update/list/version-list/delete |
| F-NAC-AI-008 | Pipeline `v3/console/ai/pipelines` | batata-console/v3/ai_pipeline.rs | 🟢 | | list/detail |
| F-NAC-AI-009 | Copilot `v3/console/copilot` | batata-copilot | 🟢 | | config + SSE (skill optimize/generate, prompt optimize/debug) |
| F-NAC-AI-010 | AgentSpec upload (single + seed archive) `v3/console/ai/agentspecs/upload` | batata-console/v3/ai_agentspec.rs | 🟢 | | multipart upload |
| F-NAC-AI-011 | Skill upload (single + batch + precheck) `v3/console/ai/skills/upload` | batata-console/v3/ai_skill.rs | 🟢 | | multipart upload |

## B.6. Client HTTP API (`F-NAC-CLIENT-`)

> `v3/client/**` — SDK v2 HTTP fallback endpoints. Verified against batata source.

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-CLIENT-001 | GET `/v3/client/cs/config` | batata-config/api/v3/client | 🟢 | | config fetch |
| F-NAC-CLIENT-002 | POST `/v3/client/ns/instance` | batata-naming/api/v3/client | 🟢 | | instance register |
| F-NAC-CLIENT-003 | DELETE `/v3/client/ns/instance` | batata-naming/api/v3/client | 🟢 | | instance deregister |
| F-NAC-CLIENT-004 | GET `/v3/client/ns/instance/list` | batata-naming/api/v3/client | 🟢 | | instance list |
| F-NAC-CLIENT-005 | PUT `/v3/client/ns/instance/beat` | batata-naming/api/v3/client | 🟢 | | heartbeat |
| F-NAC-CLIENT-006 | GET `/v3/client/ai/prompt` | batata-ai (prompt_client_routes) | 🟢 | | prompt query |
| F-NAC-CLIENT-007 | GET `/v3/client/ai/skills` | batata-ai (skill_client_routes) | 🟢 | | skill download |
| F-NAC-CLIENT-008 | GET `/v3/client/ai/agentspecs` | batata-ai (agentspec_client_routes) | 🟢 | | agentspec query |
| F-NAC-CLIENT-009 | GET `/v3/client/ai/agentspecs/search` | batata-ai | 🟢 | | agentspec search |
| F-NAC-CLIENT-010 | GET `/v3/client/ai/agents/search` | batata-ai/api/agent_client.rs | 🟢 | | agent discovery |
| F-NAC-CLIENT-011 | POST/DELETE/PUT `/v3/client/ai/agents/endpoints` | batata-ai/api/agent_client.rs | 🟢 | | agent endpoint publisher + heartbeat |

## B.7. Admin HTTP API (`F-NAC-ADM-`)

> `v3/admin/**` — backend/open-platform endpoints. Used by maintainer SDK. Verified against batata source.

### B.7.1. Config admin (`ADM-CS`)

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-ADM-CS-001 | POST/GET/PUT/DELETE `/v3/admin/cs/config` | batata-config/api/v3/admin | 🟢 | | CRUD + metadata update |
| F-NAC-ADM-CS-002 | DELETE `/v3/admin/cs/config/batchDelete` | batata-config | 🟢 | | batch delete |
| F-NAC-ADM-CS-003 | GET/POST/DELETE `/v3/admin/cs/config/beta` | batata-config | 🟢 | | beta management |
| F-NAC-ADM-CS-004 | POST/DELETE `/v3/admin/cs/config/gray` | batata-config | 🟢 | | gray (tag-based) management |
| F-NAC-ADM-CS-005 | GET `/v3/admin/cs/config/gray/info` | batata-config | 🟢 | | gray info |
| F-NAC-ADM-CS-006 | POST `/v3/admin/cs/config/clone` | batata-config | 🟢 | | clone config |
| F-NAC-ADM-CS-007 | GET `/v3/admin/cs/config/export` | batata-config | 🟢 | | export ZIP |
| F-NAC-ADM-CS-008 | POST `/v3/admin/cs/config/import` | batata-config | 🟢 | | import ZIP |
| F-NAC-ADM-CS-009 | GET `/v3/admin/cs/history/list` | batata-config | 🟢 | | history list |
| F-NAC-ADM-CS-010 | GET `/v3/admin/cs/history` | batata-config | 🟢 | | history detail |
| F-NAC-ADM-CS-011 | GET `/v3/admin/cs/history/previous` | batata-config | 🟢 | | previous version |
| F-NAC-ADM-CS-012 | GET `/v3/admin/cs/listener` + `/listener/ip` | batata-config | 🟢 | | listener diagnostics |
| F-NAC-ADM-CS-013 | GET/POST `/v3/admin/cs/capacity` | batata-config/api/v3/admin/capacity | 🟢 | | capacity/quota |
| F-NAC-ADM-CS-014 | GET `/v3/admin/cs/metrics/ip` | batata-config | 🟢 | | config metrics |
| F-NAC-ADM-CS-015 | POST `/v3/admin/cs/ops/localCache` + PUT `/ops/log` | batata-config | 🟢 | | ops maintenance |
| F-NAC-ADM-CS-016 | GET `/v3/admin/cs/history/configs` | batata-config | 🟢 | | history per namespace |

### B.7.2. Naming admin (`ADM-NS`)

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-ADM-NS-001 | POST/GET/PUT/DELETE `/v3/admin/ns/service` + GET `/list` | batata-naming/api/v3/admin | 🟢 | | service CRUD |
| F-NAC-ADM-NS-002 | POST/GET/PUT/DELETE `/v3/admin/ns/instance` + GET `/list` | batata-naming | 🟢 | | instance CRUD |
| F-NAC-ADM-NS-003 | PUT/DELETE `/v3/admin/ns/instance/metadata/batch` | batata-naming | 🟢 | | batch metadata update/delete |
| F-NAC-ADM-NS-004 | PUT `/v3/admin/ns/cluster` | batata-naming | 🟢 | | cluster metadata update |
| F-NAC-ADM-NS-005 | GET `/v3/admin/ns/health/checkers` | batata-naming | 🟢 | | health checker types (static) |
| F-NAC-ADM-NS-006 | PUT `/v3/admin/ns/health/instance` | batata-naming | 🟢 | | instance health update |
| F-NAC-ADM-NS-007 | GET `/v3/admin/ns/client` + `/client/list` + publish/subscribe lists | batata-naming | 🟢 | | client diagnostics |
| F-NAC-ADM-NS-008 | GET/PUT `/v3/admin/ns/ops/switches` | batata-naming | 🟢 | | switches |
| F-NAC-ADM-NS-009 | GET `/v3/admin/ns/ops/metrics` | batata-naming | 🟢 | | naming metrics |
| F-NAC-ADM-NS-010 | PUT `/v3/admin/ns/ops/log` | batata-naming | 🟢 | | log level |
| F-NAC-ADM-NS-011 | GET `/v3/admin/ns/operator/metrics` | batata-naming | 🟢 | | operator metrics |

### B.7.3. Core admin (`ADM-CORE`)

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-ADM-CORE-001 | POST/GET/PUT/DELETE `/v3/admin/core/namespace` + `/list` + `/check` | batata-server/api/v3/admin/core | 🟢 | | namespace CRUD + check |
| F-NAC-ADM-CORE-002 | GET `/v3/admin/core/cluster/node/self` + `/node/list` | batata-server | 🟢 | | cluster node info |
| F-NAC-ADM-CORE-003 | GET `/v3/admin/core/ops/ids` + PUT `/ops/log` + POST `/ops/raft` | batata-server | 🟢 | | ops maintenance |
| F-NAC-ADM-CORE-004 | GET `/v3/admin/core/state` + `/liveness` + `/readiness` + `/servers` | batata-server/api/v3/admin/core/state | 🟢 | | server state + k8s probes |
| F-NAC-ADM-CORE-005 | GET `/v3/admin/core/loader/current` + `/clusterMetrics` | batata-server | 🟢 | | loader diagnostics |
| F-NAC-ADM-CORE-006 | GET `/v3/admin/core/plugin/list` + `/detail` + PUT `/status` + `/config` | batata-server | 🟢 | | plugin management |
| F-NAC-ADM-CORE-007 | GET `/v3/admin/core/lock/list` | batata-server/api/v3/admin/core/lock | 🟢 | | lock list |

### B.7.4. Auth admin (`ADM-AUTH`)

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-ADM-AUTH-001 | POST/DELETE `/v3/auth/visibility` | batata-auth/service/visibility_grant.rs | 🟢 | | visibility grant/revoke via RBAC |

### B.7.5. AI admin (`ADM-AI`)

| ID | HTTP action | batata impl | Status | Tests | Notes |
|----|-------------|-------------|--------|-------|-------|
| F-NAC-ADM-AI-001 | POST/GET/PUT/DELETE `/v3/admin/ai/mcp` + GET `/list` | batata-server/api/v3/admin/ai + batata-ai | 🟢 | | MCP server admin |
| F-NAC-ADM-AI-002 | POST/GET/PUT/DELETE `/v3/admin/ai/a2a` + `/list` + `/version/list` | batata-server/api/v3/admin/ai/a2a | 🟢 | | A2A agent admin |
| F-NAC-ADM-AI-003 | POST/PUT/DELETE `/v3/admin/ai/agents/draft` + `/force-publish` + `/online` + `/offline` + `/submit` + `/labels` + GET `/list` + `/versions` | batata-console/v3/ai_agent | 🟢 | | Agent admin |
| F-NAC-ADM-AI-004 | POST/PUT/DELETE `/v3/admin/ai/agentspecs/draft` + `/force-publish` + `/online` + `/offline` + `/submit` + `/labels` + GET `/list` + `/versions` + POST `/upload` | batata-console/v3/ai_agentspec | 🟢 | | AgentSpec admin |
| F-NAC-ADM-AI-005 | POST/PUT/DELETE `/v3/admin/ai/skills/draft` + `/force-publish` + `/online` + `/offline` + `/submit` + `/labels` + GET `/list` + `/versions` + POST `/upload` + `/upload/batch` + `/upload/precheck` | batata-console/v3/ai_skill | 🟢 | | Skill admin |
| F-NAC-ADM-AI-006 | POST/PUT/DELETE `/v3/admin/ai/prompt/draft` + `/force-publish` + `/online` + `/offline` + `/submit` + `/labels` + GET `/list` + `/versions` + `/governance` + `/version/download` | batata-console/v3/ai_prompt | 🟢 | | Prompt admin |
| F-NAC-ADM-AI-007 | GET `/v3/admin/ai/pipelines` + `/list` + `/detail` + `/{pipelineId}` | batata-ai/api/pipeline | 🟢 | | Pipeline admin |
| F-NAC-ADM-AI-008 | GET `/v3/admin/ai/import/sources` + POST `/search` + `/validate` + `/execute` | batata-console/v3/ai_import | 🟢 | | generic AI importer: sources/search/validate/execute |

### B.7.6. Legacy / misc (`ADM-LEGACY`)

| ID | Feature | upstream | batata impl | Status | Tests | Notes |
|----|---------|----------|-------------|--------|-------|-------|
| F-NAC-ADM-LEGACY-001 | Prometheus metrics | `/metrics` | batata-plugin-cloud | 🟢 | | |
| F-NAC-ADM-LEGACY-002 | Distro / Raft consistency | consistency/* | batata-consistency | 🟢 | | |
| F-NAC-ADM-LEGACY-003 | v2 capacity API | `/nacos/v2/cs/capacity` | batata-config/api/v2 | 🟢 | | backward compat |

## B.8. Maintainer SDK (`F-NAC-MAINT-`)

> Rust maintainer client SDK (`batata-maintainer-client`). Wraps the admin HTTP API. Also used by console in remote mode.

| ID | SDK service | batata impl | Status | Tests | Notes |
|----|------------|-------------|--------|-------|-------|
| F-NAC-MAINT-001 | ConfigMaintainerService (CRUD, history, beta, clone, listener, ops) | batata-maintainer-client | 🟢 | | wiremock tests in tests/client_test.rs |
| F-NAC-MAINT-002 | NamingMaintainerService (service/instance/subscriber mgmt) | batata-maintainer-client | 🟢 | | |
| F-NAC-MAINT-003 | CoreMaintainerService (server state, cluster, namespace, plugin) | batata-maintainer-client | 🟢 | | |
| F-NAC-MAINT-004 | AiMaintainerService (MCP, A2A, prompt, skill, agentspec) | batata-maintainer-client/model/ai | 🟢 | | Closed 2026-11-11 (batch 20) after a GA source re-read. Both residuals were **over-registered**: the 24 typed AI resource DTOs already landed in batch 8 (`crates/batata-maintainer-client/src/model/ai_resource.rs`), and GA `A2aMaintainerService` declares exactly seven methods (register / get / update / delete / listAllVersionOfAgent / search / list), all already wrapped by `agent_*` in `crates/batata-maintainer-client/src/client.rs`. `AiMaintainerService.a2a()`'s seven default methods merely forward to that delegate. The only real GA gap was `getAgentCard(..., version)`: `agent_get_with_version()` now forwards it (`agent_get` delegates with `None`), matching `A2aAdminController.java:96-101` where the parameter is bound **and** consumed. |
| F-NAC-MAINT-005 | AgentMaintainerService (draft/publish/labels/offline/online) | batata-maintainer-client | 🟢 | | Closed 2026-11-11 (batch 20) with a mis-attribution correction: GA `A2aMaintainerService` has **no** `createDraft/updateDraft/submit/publish/forcePublish/redraft/online/offline/updateLabels`; those belong to the protocol-neutral `AgentMaintainerService`, which batch 2 already implemented as `agents_*` on `/v3/admin/ai/agents`. The legacy `/v3/admin/ai/a2a` facade is therefore complete (7/7 methods, `List` versions, `Page` list/search). Its only missing GA parameter — `version` on `getAgentCard` — is now exposed via `agent_get_with_version()` and consumed by `crates/batata-server/src/api/v3/admin/ai/a2a.rs` (`q.version.as_deref()`), guarded by `crates/batata-server/tests/a2a_admin_version.rs`. |
| F-NAC-MAINT-006 | MCP server version lifecycle (listVersions / getVersion / submit / publish / forcePublish / redraft / online / offline) | batata-maintainer-client | 🟢 | | Implemented 2026-11-10 in `crates/batata-maintainer-client/src/client.rs`: `mcp_server_list_versions`, `mcp_server_get_version` and the six `mcp_server_*_version` transitions, backed by the ADMIN paths in `crates/batata-maintainer-client/src/constants.rs` and the server routes in `crates/batata-ai/src/api/mcp.rs`. Covered by 4 wiremock cases in `crates/batata-maintainer-client/tests/client_test.rs` (query shape and all six paths mutant-checked). |
| F-NAC-MAINT-007 | `AiMaintainerService` delegates (full read/write surface) + Skill/Prompt/AgentSpec ZIP I/O and MCP resource updates | batata-maintainer-client | 🟢 | | Implemented 2026-11-10 (batches 3 + 7). Batch 3: 25 flat `skill_*` / `prompt_*` / `agentspec_*` / `pipeline_*` read methods, 23 ADMIN constants, ZIP upload/precheck/download/delete via the `file` multipart part. Batch 7: 35 more ADMIN constants and 42 flat write methods in `crates/batata-maintainer-client/src/{constants.rs,client.rs}` — the complete Skill / Prompt / AgentSpec draft, submit, publish, force-publish, redraft, labels, biz-tags, scope and online/offline lifecycle, the four legacy prompt APIs (label bind/unbind, `publishPrompt`, `updatePromptMetadata`), `listAgentSpecAdminItems` with `orderBy`/`owner`/`scope`, and the MCP draft delete / labels / status / scope updates left by batch 1. GA sends POST/PUT as urlencoded form bodies and DELETE identity in the query; batata's `web::Form` / `AdminForm` handlers accept the same wire after `F-NAC-MAINT-010` closed. GA delegate objects stay represented as flat methods, as in batches 1/2. 6 new wiremock cases (128 → **134**) guard exact form field maps, verbs, paths and error paths; 15 mutants killed. **Also corrected**: `getAgentSpecAdminDetail` was a false gap — `agentspec_get_detail` has covered GA's `GET /v3/admin/ai/agentspecs` since batch 3. **Still partial (batch 8)**: typed AI resource DTOs are now modelled (`crates/batata-maintainer-client/src/model/ai_resource.rs`, 24 DTOs / 18 typed read facades + 2 generic helpers / 7 serde unit tests / 15 mutants killed); `#[serde(flatten)] extra` preserves unmodelled GA and batata fields. **Batch 9 source correction**: the batch 8 residual claim was overbroad. GA deliberately **ignores** `updateLatestLabel`: the publish forms bind it as an "ignored update latest label flag" (`SkillPublishForm.java:45`, `PromptVersionPublishForm.java:45`, `AgentSpecPublishForm.java:45`) and the operation services say "retained for compatibility and ignored; latest is server-managed" (`SkillOperationService.java:264`, `PromptOperationService.java:107`, `AgentSpecOperationService.java:262`), so batata not acting on it is correct; Skill/AgentSpec `basedOnVersion` and the dedicated `bizTags` update endpoints are already consumed by batata. **Batch 10 implemented (2026-11-10)**: both batch 9 server-side gaps are closed — `commit_msg` now flows from `SkillDraftCreateForm`, `SkillUpdateForm` and `SkillUploadQuery` through `SkillService` into `AiResourceVersionInfo.description` (GA `versionDesc`), and Prompt ADMIN `POST /prompt/draft` now binds `targetVersion` / `basedOnVersion` / `commitMsg` / `bizTags`, forks the base template, bumps the patch version and initializes brand-new metadata. Batch 10 also corrected a **false claim in §16.2**: `PromptDraftCreateForm.commit_msg` was declared but never forwarded on create-draft; it now reaches both the version description and `PromptStorage`. 13 new route tests (7 Skill + 6 Prompt) and **19/19 compile-safe mutants killed** cover the form, query and multipart channels plus the absent-field baseline. **Remaining** (newly source-verified, keeps the row 🟡): GA `/skills/upload` also binds and consumes `targetVersion`, `uploadAction` and `autoPublishIfNew` (`SkillAdminController.java:200-204`, `SkillOperationServiceImpl.java:200-211,399-435`), which batata's `SkillUploadQuery` does not expose. No client SDK work remains. See `docs/compat/review-report.md` §16–§17. **Batch 11 implemented (2026-11-10)**: GA `/skills/upload` `targetVersion` / `uploadAction` / `autoPublishIfNew` are now consumed. `SkillUploadQuery` (ADMIN) and `ConsoleSkillUploadQuery` (console) gained the three fields; `SkillService::upload_skill` gained `target_version` / `upload_action` / `auto_publish_if_new` and mirrors GA `doUploadSingleSkill`: blank action → overwrite?overwrite:create-draft, `CREATE_DRAFT` / `OVERWRITE_DRAFT` / `DELETE_DRAFT_AND_CREATE` branches (unsupported → `PARAMETER_VALIDATE_ERROR`), and `autoPublishIfNew` force-publishes only the first upload of a brand-new skill. The maintainer-client SDK `skill_upload_from_zip` exposes the same wire (`AiUploadQuery` with `skip_serializing_if` so the GA `/upload/batch` path stays faithful). Six new route tests in `skill_admin_routes.rs` and **4/4 mutants killed**; `skill_batch_upload` / `agentspec_upload_from_zip` are unchanged (the GA batch endpoint does not bind these params). F-NAC-MAINT-007 is now `🟢`; the F-ID count total is unchanged. |
| F-NAC-MAINT-008 | `AgentMaintainerService` draft lifecycle (`createDraft/updateDraft/submit/publish/forcePublish/redraft/online/offline/updateLabels`) + `agent_get(version)` | batata-maintainer-client | 🟢 | | Closed 2026-11-11 (batch 20). Batch 2's `agents_*` facade is now GA-wire complete: `agents_create_draft` / `agents_update_draft` / `agents_update_labels` send **urlencoded form** (`post_form`/`put_form`; nested `labels` becomes one JSON string form value per GA `putJsonIfNotNull`), with `*_json` aliases keeping batata's dual-channel JSON body; `agents_list_versions` now decodes `Page<AgentVersionInfo>` — the server has returned `Page` since batch 12, so the earlier "batata returns a plain array" note had the direction reversed; `agents_update_agent` (PUT), `agents_delete_agent` (DELETE) and `agents_get_runtime_endpoints` (GET, new `AI_AGENTS_RUNTIME_ENDPOINTS` constant) are wrapped with tolerant camelCase DTOs. No server wire change was needed (`F-NAC-MAINT-010` `AdminParams` already accepts both channels). 11 new wiremock cases assert exact form field maps, verbs, paths, query and error paths; 7 serde tests; **13/13 compile-safe mutants killed**. The GA `RuntimeEndpointSnapshot` live source / response shape remains the separate server-side `F-NAC-AISRV-001`, not a facade gap. |
| F-NAC-MAINT-009 | `publishConfig(..., changeMessage)` + `ConfigHistoryDetailInfo.changeMessage` | batata-maintainer-client | ⚪ | | **Develop drift** (post-GA commit `4a11bd06b` / #15914), not part of the 3.3.0 GA baseline. `ConfigMaintainerService.java:203`. |
| F-NAC-MAINT-010 | AI ADMIN POST/PUT parameter channel: GA form body vs batata query/JSON | batata-ai | 🟢 | | Closed 2026-11-10 (batches 4 + 5). GA bytecode (`~/.m2/.../nacos-maintainer-client-3.3.0.jar`) shows `ClientHttpProxy.executeSync` uses `postForm`/`putForm` for POST/PUT and `AgentMaintainerServiceImpl` / `McpMaintainerServiceImpl` fill `setParamValue`; Spring `@RequestParam` also accepts query. A source re-read corrected the original overbroad registration: `skill.rs` / `prompt.rs` / `agentspec.rs` write endpoints already use `web::Form`, ZIP uploads use query + `Multipart`, and MCP create/draft already parse form-or-JSON specifications. Batch 4 added `crates/batata-ai/src/api/admin_form.rs` (`AdminParams` merges urlencoded form / JSON object / query with query precedence and multipart exclusion, plus the `AdminForm<T>` extractor) and rewired the 15 query-only MCP/Agent POST/PUT handlers, MCP `create_draft` and MCP/Agent `labels`. Batch 5 rewired the last four Agent write handlers (`POST/PUT /agents`, `POST/PUT /agents/draft`) through `resolve_agent_write`, which accepts either the GA flat form (`AgentDraftCreateForm` / `AgentUpdateForm` fields) or batata's nested `AgentCard` JSON body. 8 + 6 unit tests and 14 route-level tests in `crates/batata-server/tests/ai_admin_form_encoding.rs` guard both channels, including real success paths; 11 + 16 mutants killed. ARD-only `callInterfaces` is rejected fail-closed, not dropped. The remaining GA ARD **model** divergence (typed `AgentVersionDetail` / `AgentCallInterface`, `createDraft` auto-creating Agent metadata, `Page` responses) is registered separately as `F-NAC-AISRV-001`. The GA runtime matrix does not cover AI ADMIN writes (`maintainer-sdk-test` focuses on config/naming). |

## B.9. Client SDK gaps (`F-NAC-CSDK-`)

> Explicitly registered on 2026-11-10 so that "documented but not implemented"
> is machine-visible. All rows are `⚪ planned` until implementation lands;
> the last row is post-GA develop drift, kept separate from the GA baseline.

| ID | SDK capability | batata impl | Status | Tests | Notes |
|----|----------------|-------------|--------|-------|-------|
| F-NAC-CSDK-001 | `NamingService.subscribe(..., NamingSelector, ...)` overloads | batata-client | 🟢 | | Closed 2026-11-11. GA `b64bc9118` source-verified: `NamingService.java:524,530` declare both `subscribe(serviceName, NamingSelector, listener)` and `subscribe(serviceName, groupName, NamingSelector, listener)`; `NamingSelector` is a **client-side** filter (`select(NamingContext) -> NamingResult.getResult()`), and `NamingSelectorWrapper.buildListenerEvent` applies it to the current list plus the added/removed/modified diff buckets before invoking the listener — nothing is sent to the server. `crates/batata-client/src/naming/selector.rs` now ports the whole contract: `NamingContext`, the `NamingSelector` trait, `DefaultNamingSelector` (predicate-backed), `NamingSelectorFactory` (empty / healthy / cluster / IP regex whole-match / metadata all-or-any, plus `getUniqueClusterString`), and `SelectorEventListener` (the wrapper). `crates/batata-client/src/naming/mod.rs` adds `subscribe_with_selector` / `subscribe_with_selector_default_group`, which register the wrapper and filter the returned initial snapshot through the same `filter_event` path; `crates/batata-client/src/traits.rs` adds the two contract methods with default implementations so any `NamingService` implementor inherits them. The existing 4-arg `subscribe` is unchanged. 12 tests (11 selector unit + 1 real `handle_notify_subscriber` push-dispatch path) with **4/4 compile-safe mutants killed**; `cargo fmt` and `cargo test -p batata-client` pass (384 lib tests). |
| F-NAC-CSDK-002 | `LockService::renew` + watchdog | batata-client | 🟢 | | Closed 2026-11-11 (batch 20). GA `b64bc9118` `LockService.java:77-85` (`@Since 3.3.0`), `LockGrpcClient.java:260-287`, `NacosLockWatchdog.java:43,82-96` (interval `max(1000, min(ttl/3, 10000))`, `false`/error unregisters) and `NacosLock.java:108-110,154-156,304-307` (`clientId:threadId` owner, register only on first acquire) source-verified. `crates/batata-api/src/remote/model.rs` adds `LockInstance.owner` (`#[serde(default)]`); `crates/batata-client/src/lock.rs` adds `renew()` / `renew_default()` / `renew_with_params()`, a cached stable owner shared by lock/unlock/renew, and `LockWatchdog` (idempotent register, cancel-on-unregister, `shutdown()` on drop, unregister **before** RELEASE). Server `RENEW` branch, `raft_renew()` producer and standalone `renew()` are in `F-NAC-LOCK-009`; **no new Raft variant** was added (the earlier note overestimated the work). 16/16 compile-safe mutants killed. |
| F-NAC-CSDK-003 | `AiService` sub-services (`mcp()/skill()/prompt()/agentSpec()/agent()`), `AgentDiscoveryService`, GA RAD capability | batata-client | 🟢 | | Closed 2026-11-11. `crates/batata-client/src/ai.rs` now exposes the GA `AiService` facade: `BatataAiService` returns `Arc<McpAiService>` / `SkillAiService` / `AgentSpecAiService` / `PromptAiService` / `AgentAiService` accessors (all sharing the `Arc<BatataHttpClient>`), and the legacy 10 flat query methods are preserved as delegates (TG-03 integration test `crates/batata-client/tests/ai_service_test.rs` unchanged). The five sub-services are source-verified against the server's `/v3/client/ai/**` handlers in `crates/batata-ai/src/api/{agent_client,mcp_client,skill,prompt,agentspec}.rs` and the GA Java `AgentDiscoveryService.java` / `AbilityStatus.java` at release commit `b64bc9118`: `McpAiService` (client `/mcp` + `/mcp/search`), `SkillAiService` (`/skills` ZIP bytes + `/skills/search`), `AgentSpecAiService` (`/agentspecs` + `/agentspecs/search`), `PromptAiService` (`/prompt` + `/prompt/search`), and `AgentAiService` implementing the `AgentDiscoveryService` trait — `search_agents` (`GET /agents/search`), `discover_agent` (`GET /agents`), a `subscribe_agent` HTTP long-poll watch loop (`POST /agents/watch`, `clientId` header, fingerprint-diff notification via `AgentDiscoveryListener`, `AgentSubscription` cancellable via `JoinHandle`/`AtomicBool`), `register_agent_endpoints` / `deregister_agent_endpoints` (`POST`/`DELETE /agents/endpoints`, `clientId` header), and `publish_agent` (`POST /agents` with a `ClientAgentCard` mapped from the existing `AgentCardInfo`). All client-side DTOs (`AgentSummary` = server `AgentCatalogEntry` shape, `AgentDiscoveryResult`, `AgentReference`, `AgentSearchRequest`, `AgentEndpointRegistrationBatch`, `Endpoint`, `AgentVersionDetail`, …) are defined locally because `batata-client` does not depend on `batata-common`. RAD capability is exposed as `AgentDiscoveryCapability` mirroring GA `AbilityStatus` (`Supported`/`NotSupported`/`Unknown`); `get_agent_discovery_capability()` returns `Supported` for the HTTP discovery transport (the BATATA server exposes `/v3/client/ai/agents/*` unconditionally over HTTP), and deliberately does **not** claim the gRPC `radV1`/`radWatchV1` negotiation (handled by `F-NAC-CSDK-007`). 16 serde/round-trip + 2 mutant-checked (`code==0` guard, capability mapping) unit tests in `ai.rs` guard the wire shapes; `cargo fmt`, `cargo check -p batata-client --tests` and both the lib + `ai_service_test` suites pass. |
| F-NAC-CSDK-004 | config 304 conditional GET (`GetConfigRequest.localMd5`) | batata-client | 🟢 | | Closed 2026-11-11. GA `b64bc9118` source-verified: `GetConfigRequest.java:61` / `ConfigQueryRequest.java:40` carry the optional `localMd5`; `ConfigQueryRequestHandler.buildNotModifiedResponse()` returns `ConfigQueryResponse.CONFIG_NOT_MODIFIED (304)` with md5/contentType/lastModified and no content when it matches the server MD5; `ClientWorker.queryConfigInner()` reuses the local content captured **before** the request (and retries unconditionally when none is available). `DefaultChainRequestExtractor.java:71` explicitly excludes HTTP, so this is a **gRPC-only** contract. Batata: `crates/batata-api/src/config/model.rs` adds `ConfigQueryRequest.local_md5` (wire `localMd5`, `skip_serializing_if` for backward compatibility) and `ConfigQueryResponse::CONFIG_NOT_MODIFIED`; `crates/batata-config/src/handler/config_handler.rs` applies the 304 guard to both the gray and formal branches through the single exit point `finalize_config_query_response`; `crates/batata-client/src/config/mod.rs` adds `GetConfigRequest` + `get_config_with_request()`, keeps the server-returned MD5 in `CacheData.server_md5`, sends `localMd5` only when a restorable local representation exists, and restores the retained content on 304 (`resolve_local_content` / `restore_not_modified`). `GrpcClient::request_typed_allowing` lets exactly 304 reach the caller as a decoded response while preserving the 403 reLogin path. `ConfigQueryResult.md5` now uses the authoritative server MD5 (valid for CAS and for the next conditional GET). HTTP is untouched. 3 API serde tests + 4 server tests + 5 client tests + 1 integration wire round-trip; **13/13 compile-safe mutants killed** (including bound-but-ignored `localMd5` and both handler call sites). `cargo check --workspace --exclude batata-integration-tests --tests`, `cargo test -p batata-api -p batata-client -p batata-config` and clippy on all changed files pass. |
| F-NAC-CSDK-005 | OIDC client auth and `configSnapshotEnabled` property switch | batata-client | 🟢 | | Closed 2026-11-11 (batch 19). GA `b64bc9118` (introduced by `c87f888fc` / `0004ea910`): `client-basic/.../META-INF/services/...AbstractClientAuthService` registers `OidcClientAuthServiceImpl` as the third auth provider; `OidcClientConstants` defines `nacos.client.auth.oidc.{issuer-uri,client-id,client-secret,scope,token-endpoint}` (scope defaults to `openid`, a direct token endpoint skips Discovery); `OidcClientContext.discover()` GETs `{issuer}/.well-known/openid-configuration` and reads `token_endpoint`; `OidcTokenHolder.fetchToken()` POSTs a `grant_type=client_credentials` form with `client_id`/`client_secret`/`scope`, parses `access_token` and `expires_in` (default 300s), and refreshes inside a random `[ttl/15, ttl/10)` window before expiry; `getLoginIdentityContext()` emits **both** `Authorization: Bearer <token>` and `accessToken` (`OidcProtocolConstants`). `PropertyKeyConst.java:96` defines `configSnapshotEnabled`; `SnapShotSwitch` reads it once with default `true` and cleans all snapshots when disabled, while `LocalConfigInfoProcessor` and `LocalEncryptedDataKeyProcessor` both gate reads and writes on the switch. **Doc correction**: the earlier note said `SnapShotSwitch`; batata's type is `crates/batata-client/src/local_config.rs::SnapshotSwitch`, previously hard-coded to `new(true)` on both processors and unreachable from `ClientConfig`. **Implementation**: `crates/batata-client/src/oidc.rs` ports `OidcConfig` (the five GA property keys via `from_properties`), `discovery_url()`, the Client Credentials `token_form()`, OIDC Discovery, `access_token`/`expires_in` parsing (default 300s) and the random `[ttl/15, ttl/10)` refresh window; `OidcAuthProvider` implements `ClientAuthService` and emits the GA dual identity headers. `ClientConfig` gains `oidc` + `with_oidc()` + `with_client_properties()`; `BatataHttpClient::from_config` and `GrpcClient::from_config` consume it (`with_auth_headers` applies every identity header, and gRPC metadata also carries `Authorization: Bearer …`), so the provider is a real call site, not a declaration. 401/403 refresh uses `OidcAuthProvider::refresh`. For the snapshot switch `ClientConfig::config_snapshot_enabled` (default `true`, GA property key `configSnapshotEnabled` plus the `BATATA_CONFIG_SNAPSHOT_ENABLED` env alias) flows through `SnapshotSwitch::from_property` in both processors; `BatataClient::config_service()` now builds the processor with the explicit value and cleans persisted snapshots when disabled (best effort, as in GA). 8 wiremock integration cases + 17 unit/wiring cases guard the contract; **20/20 compile-safe mutants killed**. |
| F-NAC-CSDK-006 | HTTP `requestId` correlation header | batata-client | 🟢 | | Closed 2026-11-11. `crates/batata-client/src/http.rs` now attaches a fresh v4 UUID under the `Nacos-Request-Id` header (`HttpHeaderConsts.NACOS_REQUEST_ID`, mirroring Nacos 3.3.0 GA `RequestIdHttpClientRequestInterceptor`) to every outgoing client HTTP request: all `request_with_retry` closures receive a per-request `request_id`, and the bypass paths (`authenticate()`, `post_json_with_headers()`, `get_bytes*`, `post_bytes`, `post_multipart*`) route through the `with_request_id()` helper. gRPC already sets a UUID `request_id`; this closes the HTTP-side gap. 3 wiremock cases in `crates/batata-client/tests/request_id_test.rs` (GET / POST / JWT login) assert the header is present and a valid v4 UUID; all 3 are mutant-checked (dropping the header fails the test). `cargo fmt` and `cargo test -p batata-client` pass. |
| F-NAC-CSDK-007 | `SetupAckRequest.ability_table` GA keys `agentCardV1` / `mcpDraftRelease` / `radV1` / `radWatchV1` | batata-core, batata-ai | 🟢 | | Upstream `AbilityKey.java:62,76,86,92` declares 9 server keys; `crates/batata-core/src/handler/rpc.rs:838-851` now advertises all 9 (added `agentCardV1` / `mcpDraftRelease` / `radV1` / `radWatchV1`, capacity 5→9, 2026-11-11). Declaring `radV1`/`radWatchV1` makes GA 3.4.0 Java clients switch to the RAD v1 gRPC contract, so the matching handlers are implemented in `crates/batata-ai/src/handler/ai_handler.rs`: `AgentPublishRpcHandler` (`AgentPublishRpcRequest` → `AgentVersionDetail`), `AgentSubscribeRpcHandler` (`AgentSubscribeRpcRequest` → watch key + observed fingerprint) and `AgentUnsubscribeRpcHandler` (`AgentUnsubscribeRpcRequest`), backed by `crates/batata-ai/src/service/agent_rad_watch.rs` (`AgentRadWatchService` + `WatchPusher`). GA clients that negotiate RAD no longer hit unhandled request types. (The `GET /runtime-endpoints` live-support sub-capability remains a separate deferred item — it needs the Naming publication backend — and is tracked outside this row.) |
| F-NAC-CSDK-008 | GA `AgentCard` security fields in the client DTO; post-GA `SecurityRequirement`/`StringList` wrapper and `AgentSkill.securityRequirements` | batata-client | ⚪ | | **GA baseline is complete** (batch 1, 2026-11-10): `crates/batata-client/src/ai.rs` has `AgentCardInfo.security_schemes` / `security` / `security_requirements` as GA `List<Map<String,List<String>>>`, matching `crates/batata-common/src/model/ai/a2a.rs:107-112` (2 serde tests, mutant-checked). The rest is **post-GA develop drift, outside the 3.3.0 GA alignment target and deliberately not planned**: source verification shows `ff1bcc2e9` (#15976, 2026-11-03) and `7c546cfda` (#15982, 2026-11-03) are not ancestors of `b64bc9118`; they change the type to `List<SecurityRequirement>` with a `schemes`/`list` wrapper and add `AgentSkill.securityRequirements`. Porting them is a **breaking wire change** on both client and server and would regress GA compatibility (see `docs/compat/review-report.md` §27). Status `⚪` here means "baseline-out, not tracked", not a GA gap. |

## B.10. Server AI ADMIN alignment (`F-NAC-AISRV-`)

> Registered 2026-11-10 from a GA-controller-vs-batata-route audit. These are
> **server-side** gaps, deliberately separated from the Rust SDK gaps in
> B.8/B.9. Verified against GA release commit `b64bc9118`
> (`ai/src/main/java/com/alibaba/nacos/ai/controller/`); the AI controllers
> have **zero** post-GA drift at local develop `d1b97bc3e`.

| ID | Server capability | batata impl | Status | Tests | Notes |
|----|-------------------|-------------|--------|-------|-------|
| F-NAC-AISRV-001 | GA ARD Agent resource model (`AgentVersionDetail` / `AgentCallInterface` / `AgentProvider`, `Page` responses, RAD runtime endpoints) | batata-ai | 🟢 | | **Source re-verified 2026-11-12 against `b64bc9118`** (`AgentAdminController.java:181-193`, `AgentRuntimeEndpointForm.java:27-42`, `AgentAdminForm.java:36-45`, `RuntimeEndpointSnapshot.java:26-69`, `AgentRuntimeRegistryService.java:154-174`, `ServiceStorage.java:78-106`). GA `GET /v3/admin/ai/agents/runtime-endpoints` binds `namespaceId` (optional, default `public`) / `agentName` (required) / `protocol` (required) / `version` (optional), answers `Result<RuntimeEndpointSnapshot>` whose `data` is **flat** `{namespaceId, agentName, version?, callInterface}` — not `ConsoleRuntimeEndpointView`; `callInterface` holds exactly one `endpointSets` entry with `source=RUNTIME`, an epoch-millis `lastUpdatedTime`, and `Endpoint` values (`uri`, `transport`, `priority`, `weight`, `metadata`, `healthy`, `bindings`, `enabled`). The live source is Naming `ServiceStorage` (group `agent-endpoints`); a missing agent/service is a **200 empty snapshot**, not 400. **Documentation correction**: the old note claiming "no Naming publication backend / live source deferred to `F-NAC-CSDK-007`" is wrong — batata has had a Naming-backed reader since `crates/batata-ai/src/service/endpoint_service.rs:351-377`, injected in production by `crates/batata-server/src/startup/http.rs:365-388`. The real residual is the **GA response/admission shape**: `crates/batata-ai/src/api/agent.rs:547-594` returns `ConsoleRuntimeEndpointView`, requires `version`, leaves `protocol` optional, and answers 400 when no backend is wired; the maintainer-client DTO in `crates/batata-maintainer-client/src/model/ai.rs:453-469` keeps `callInterface` as a raw value. Phase A (batch 12) already delivered typed ARD DTOs, `callInterfaces` storage, `Page` list/version responses and `online_versions` / `status` reconciliation. **Closed 2026-11-12 (batch 21)**: `crates/batata-common/src/model/ai/a2a.rs::GaRuntimeEndpointSnapshot` models the flat GA `data`, `crates/batata-common/src/traits/ai.rs::get_runtime_endpoint_snapshot` provides a default empty snapshot, and `crates/batata-ai/src/service/a2a_service.rs` overrides it with live Naming instances mapped to GA `Endpoint` values (range-aware version filter, one `RUNTIME` set, node-local `lastUpdatedTime`). `crates/batata-ai/src/api/agent.rs` now makes `version` optional, requires `protocol`, and returns 200 for an unknown agent; the console keeps `ConsoleRuntimeEndpointView`. `crates/batata-maintainer-client/src/model/ai.rs` decodes the typed `endpointSets` tree. See `docs/compat/review-report.md` §28. |
| F-NAC-AISRV-002 | `/v3/admin/ai/import` resource-import controller | crates/batata-console/src/v3/ai_import.rs | 🟢 | | **Batch 6 audit correction**: the batch 5 audit grepped only `crates/batata-ai` and wrongly reported 0/4. All four GA endpoints (`GET /sources`, `POST /search`, `POST /validate`, `POST /execute`) are implemented in `crates/batata-console/src/v3/ai_import.rs::admin_routes()` and mounted by `crates/batata-server/src/api/v3/admin/ai/route.rs`; `crates/batata-server/tests/ai_import_routes.rs` has 18 route tests covering source filtering, form validation, unknown-source 404, registry-backed search/execute and the console mount `/v3/console/ai/import`. The capability is also registered as `F-NAC-ADM-AI-008` (B.7.5); this row is kept as an audit trail. |
| F-NAC-AISRV-003 | `/v3/admin/ai/pipelines` by-id and legacy collection reads | batata-ai | ⛔ | | **Batch 6 audit correction — won't support.** GA `PipelineAdminController.java` marks both `GET /{pipelineId}` (`@Deprecated(since = "3.2.1", forRemoval = true)`) and the parameterless `GET` as deprecated; both call `CompatibilityHelper.check`, which throws HTTP **410** / `ErrorCode.API_DEPRECATED` (40000) unless `nacos.core.api.compatibility.enabled=true` — **disabled by default** (`core/.../CompatibilityHelper.java:29-46`). The two canonical reads (`GET /list`, `GET /detail?pipelineId=`) are already 🟢. Batata is unreleased and intentionally does not ship endpoints GA refuses to serve by default (documented at the top of `crates/batata-ai/src/api/pipeline.rs`). |

---

# Summary (auto-updated)

| Face / module | 🟢 | 🟡 | ⚡ | ⚪ | ⛔ | Total | Impl rate |
|---------------|----|----|----|----|----|-------|-----------|
| A.1 SDK Conn | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| A.2 Config gRPC | 10 | 0 | 0 | 0 | 0 | 10 | 100% |
| A.3 Naming gRPC | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| A.4 Cluster gRPC | 4 | 0 | 0 | 0 | 1 | 5 | 80% |
| A.5 Lock gRPC | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| A.6 AI gRPC SDK | 8 | 0 | 0 | 0 | 2 | 10 | 80% |
| B.1 Console core | 15 | 0 | 0 | 0 | 0 | 15 | 100% |
| B.2 Config console | 19 | 0 | 0 | 0 | 0 | 19 | 100% |
| B.3 Naming console | 9 | 0 | 0 | 0 | 0 | 9 | 100% |
| B.4 Auth | 7 | 0 | 0 | 0 | 0 | 7 | 100% |
| B.5 AI console | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| B.6 Client HTTP | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| B.7.1 Admin Config | 16 | 0 | 0 | 0 | 0 | 16 | 100% |
| B.7.2 Admin Naming | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| B.7.3 Admin Core | 7 | 0 | 0 | 0 | 0 | 7 | 100% |
| B.7.4 Admin Auth | 1 | 0 | 0 | 0 | 0 | 1 | 100% |
| B.7.5 Admin AI | 8 | 0 | 0 | 0 | 0 | 8 | 100% |
| B.7.6 Admin Legacy | 3 | 0 | 0 | 0 | 0 | 3 | 100% |
| B.8 Maintainer SDK | 9 | 0 | 0 | 1 | 0 | 10 | 90% |
| B.9 Client SDK gaps | 7 | 0 | 0 | 1 | 0 | 8 | 88% |
| B.10 Server AI ADMIN | 2 | 0 | 0 | 0 | 1 | 3 | 67% |
| **Total** | 191 | 0 | 0 | 2 | 4 | 197 | 97% |

> Recomputed from the F-ID rows on 2026-11-10 (`scripts/compat_check.py` enforces
> this table). Initial verification was 2025-08-08; the maintainer SDK rows were
> re-verified against `crates/batata-maintainer-client/src/client.rs` on
> 2026-11-09, and the 2026-11-10 upstream refresh added the 12 `⚪` rows in
> B.8/B.9. Batch 2 promoted `F-NAC-MAINT-008` to `🟡` and registered the
> GA-vs-batata agent draft form wire as `F-NAC-MAINT-010` (13th row).
> Batch 3 promoted `F-NAC-MAINT-007` from `⚪` to `🟡`; the overall F-ID
> count is unchanged. Batch 4 promoted `F-NAC-MAINT-010` from `⚪` to `🟡` and
> narrowed its description after re-reading the batata AI handlers: only the
> MCP/Agent query-only POST/PUT handlers needed the dual-channel resolver. The
> F-ID count is still unchanged. Batch 5 closed the parameter channel
> (`F-NAC-MAINT-010` → `🟢`) by resolving the last four Agent write handlers,
> and added section B.10 with 3 newly audited **server-side** GA gaps
> (`F-NAC-AISRV-001..003`), taking the Nacos total from 194 to 197.
> Batch 6 re-audited those three rows against the whole workspace:
> `F-NAC-AISRV-002` is 🟢 (the importer lives in `batata-console`, not
> `batata-ai`), `F-NAC-AISRV-003` is ⛔ (GA-deprecated 410 endpoints, not
> served by default), and `F-NAC-AISRV-001` lost its draft-created metadata
> sub-gap. The F-ID total is unchanged at 197. Batch 7 completed the
> client-side `F-NAC-MAINT-007` write facade (42 methods / 35 path constants)
> without changing any row status: only typed AI resource DTOs remain, so the
> row stays 🟡 and the F-ID total is unchanged at 197. Batch 8 modelled the
> typed AI resource DTOs (24 DTOs, 18 typed read facades + 2 generic helpers, 7
> serde unit tests and 15 compile-safe mutants) but changed no row status: the remaining
> `F-NAC-MAINT-007` divergence is server-side handling of GA-only optional
> fields, so the F-ID total stays 197 and the Nacos status split is unchanged.
> Batch 9 re-audited that claim against the GA ADMIN controllers and forms:
> `updateLatestLabel` is GA-ignored everywhere, Skill/AgentSpec
> `basedOnVersion` and the dedicated `bizTags` endpoints are already consumed,
> so only Skill `commitMsg` and Prompt draft `basedOnVersion`/`bizTags` remain
> (`docs/compat/review-report.md` §16). No row status or count changed.
> Batch 10 implemented both gaps plus the missed Prompt `commitMsg` wiring,
> added 13 route tests and killed 19/19 mutants; a fresh GA re-read registered
> the remaining `/skills/upload` `targetVersion` / `uploadAction` /
> `autoPublishIfNew` divergence, so the row stays 🟡 and no row status, F-ID
> count or Nacos status split changed (`docs/compat/review-report.md` §17).
> Batch 12 promoted `F-NAC-AISRV-001` from `⚪` to `🟡` (Phase A): typed ARD
> DTOs + version-detail storage of `callInterfaces` + GA-shaped
> `Page<AgentSummary>` / `Page<AgentVersionSummary>` list/version responses +
> `AgentSummary.online_versions` / `AgentVersionSummary.status` reconciliation +
> GA first-draft `callInterfaces` accepted (no longer fail-closed). `GET
> /runtime-endpoints` GA shape was left for later (**corrected 2026-11-12**:
> the live Naming backend already existed; see batch 21), so the row
> is partial (`🟡`) not `🟢`. Counts: B.10 `1🟢/1🟡/0⚪/1⛔` (rate 67%),
> Nacos Total `179🟢/5🟡/9⚪/4⛔/197` (rate 91% unchanged).
> Batch 13 (2026-11-11) promoted `F-NAC-CSDK-007` from `⚪` to `🟢`: the four deferred GA server ability keys (`agentCardV1`, `mcpDraftRelease`, `radV1`, `radWatchV1`) are now advertised in `crates/batata-core/src/handler/rpc.rs` (`SERVER_ABILITIES` capacity 5→9), and the GA RAD v1 gRPC handlers they enable are implemented in `crates/batata-ai/src/handler/ai_handler.rs` — `AgentPublishRpcHandler` (`AgentPublishRpcRequest` → `AgentVersionDetail`), `AgentSubscribeRpcHandler` (`AgentSubscribeRpcRequest` → watch key + observed fingerprint) and `AgentUnsubscribeRpcHandler` (`AgentUnsubscribeRpcRequest`) — backed by `crates/batata-ai/src/service/agent_rad_watch.rs` (`AgentRadWatchService` + `WatchPusher`). GA 3.4.0 Java clients that negotiate RAD no longer hit unhandled request types. The `GET /runtime-endpoints` GA response shape remained outstanding (**corrected 2026-11-12**: the Naming publication backend was already wired, so only the response/admission shape was missing; closed in batch 21) and is tracked separately. Counts: B.9 `1🟢/1🟡/6⚪/0⛔` (rate 13%), Nacos Total `180🟢/5🟡/8⚪/4⛔/197` (rate 91% unchanged).
> Batch 15 (2026-11-11) promoted `F-NAC-CSDK-006` from `⚪` to `🟢`: `crates/batata-client/src/http.rs` now attaches a fresh v4 UUID under the `Nacos-Request-Id` header to every outgoing client HTTP request, mirroring Nacos 3.3.0 GA `RequestIdHttpClientRequestInterceptor` (GA Java clients add the same header via an interceptor on every request). Counts: B.9 `3🟢/1🟡/4⚪/0⛔` (rate 25%→38%), Nacos Total `181🟢/5🟡/7⚪/4⛔/197` → `182🟢/5🟡/6⚪/4⛔/197` (rate 92% unchanged). 3 wiremock cases in `crates/batata-client/tests/request_id_test.rs` (GET / POST / JWT login) assert the header is present and a valid v4 UUID, and are mutant-checked.
> Batch 16 (2026-11-11) promoted `F-NAC-CSDK-001` from `⚪` to `🟢`: the GA
> `NamingSelector` client-side instance filter is ported in
> `crates/batata-client/src/naming/selector.rs` (selector trait,
> `NamingSelectorFactory` with all five GA constructors, and the
> `SelectorEventListener` wrapper that filters current instances and all three
> diff buckets). `subscribe_with_selector` /
> `subscribe_with_selector_default_group` land in `naming/mod.rs`, with default
> methods on the `NamingService` trait; the existing 4-arg `subscribe` is
> unchanged. 12 tests (11 unit + 1 real `handle_notify_subscriber` dispatch
> path) with **4/4 compile-safe mutants killed**. Counts: B.9
> `4🟢/1🟡/3⚪/0⛔` (rate 38%→50%), Nacos Total `182🟢/5🟡/6⚪/4⛔/197` →
> `183🟢/5🟡/5⚪/4⛔/197` (rate 92%→93%).
> Batch 17 (2026-11-11) promoted `F-NAC-CSDK-004` from `⚪` to `🟢`: the GA 3.3.0
> config 304 conditional GET is now end-to-end. `ConfigQueryRequest.localMd5`
> is on the gRPC wire (empty values omitted for old-server compatibility),
> `ConfigQueryResponse::CONFIG_NOT_MODIFIED` is 304, and the server returns a
> metadata-only 304 from both the gray and formal query branches when the MD5
> matches. The client retains a (content, server-MD5) pair before the request,
> restores it on 304, and degrades to an unconditional query when no usable
> local representation exists; a bounded `request_typed_allowing` variant keeps
> the 403 reLogin path intact. GA explicitly excludes HTTP conditional GET, so
> the HTTP config API is unchanged. 13/13 compile-safe mutants killed across
> `batata-api`, `batata-config` and `batata-client`. Counts: B.9
> `5🟢/1🟡/2⚪/0⛔` (rate 50%→63%), Nacos Total `183🟢/5🟡/5⚪/4⛔/197` →
> `184🟢/5🟡/4⚪/4⛔/197` (rate 93% unchanged).
> Batch 19 (2026-11-11) promoted `F-NAC-CSDK-005` from `⚪` to `🟢`: the GA
> OIDC Client Credentials provider (Discovery / token endpoint / dual identity
> headers) and the `configSnapshotEnabled` property switch are now implemented
> and wired into both the HTTP and gRPC transports. Counts: B.9
> `5🟢/1🟡/2⚪/0⛔` → `6🟢/1🟡/1⚪/0⛔` (rate 63%→75%), Nacos Total
> `184🟢/5🟡/4⚪/4⛔/197` → `185🟢/5🟡/3⚪/4⛔/197` (rate 93%→94%).
> Batch 20 (2026-11-11) closed the remaining GA-baseline client gaps.
> `F-NAC-CSDK-002` and `F-NAC-LOCK-009` ⚪→🟢: GA `RENEW` op, owner-aware
> lock wire, standalone/Raft renew, and the client watchdog. `F-NAC-MAINT-004`
> / `F-NAC-MAINT-005` / `F-NAC-MAINT-008` 🟡→🟢: a source re-read showed two
> residuals were over-registered (typed DTOs already landed in batch 8; GA's
> A2A delegate is 7/7 wrapped) and one was mis-attributed (the draft lifecycle
> belongs to `AgentMaintainerService`, not `A2aMaintainerService`); the real
> residuals — GA form wire, `Page` decode, three wrappers and
> `getAgentCard(version)` — are now implemented. `F-NAC-CSDK-008` 🟡→⚪ is a
> **documentation-only correction**: the GA shape is fully delivered and the
> remaining `SecurityRequirement` / `AgentSkill` work is post-GA develop drift
> with a breaking wire change, so it is explicitly baseline-out alongside
> `F-NAC-MAINT-009`; no GA wire was modified. Counts: A.5 `10🟢/1⚪` →
> `11🟢` (91%→100%), B.8 `6🟢/3🟡/1⚪` → `9🟢/1⚪` (60%→90%), B.9
> `6🟢/1🟡/1⚪` → `7🟢/1⚪` (75%→88%), Nacos Total
> `185🟢/5🟡/3⚪/4⛔/197` → `190🟢/1🟡/2⚪/4⛔/197` (94%→96%).
> Batch 21 (2026-11-12) promoted `F-NAC-AISRV-001` from `🟡` to `🟢`. A GA
> source re-read corrected the old "no live Naming backend" note: batata already
> reads `AiEndpointService` from Naming (`crates/batata-ai/src/service/endpoint_service.rs:351`,
> wired by `crates/batata-server/src/startup/http.rs:365`). The real gap was the
> admin shape — flat `RuntimeEndpointSnapshot` with one `RUNTIME` `endpointSets`
> entry + `lastUpdatedTime`, optional `version`, required `protocol`, and a 200
> empty snapshot for an unknown agent. Implemented via a new GA-specific trait
> method with an empty-snapshot default, an `A2aServerOperationService` override
> that maps live instances to GA `Endpoint` values, a range-aware optional-version
> reader, and a typed maintainer-client DTO. The console wrapper is unchanged.
> Counts: B.10 `1🟢/1🟡/1⛔` → `2🟢/1⛔` (67%), Nacos Total
> `190🟢/1🟡/2⚪/4⛔/197` → `191🟢/0🟡/2⚪/4⛔/197` (96%→97%).
> Impl rate = 🟢 / Total. gRPC type names must stay exact for
> interop; HTTP rows were verified against `console-ui-next` (`src/api/*.ts`)
> and the upstream test suite. Update statuses per actual implementation and
> sync this table.
