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
| F-NAC-LOCK-009 | Watchdog auto-renew (`LockOperationEnum.RENEW`) | C↔S | batata-core/handler/lock.rs | ⚪ | | **False green corrected 2026-11-10**: `crates/batata-core/src/handler/lock.rs:94-139` handles only `ACQUIRE` and `RELEASE`; there is no `RENEW` branch anywhere in `batata-core` and no Raft renew request. Upstream `LockOperationEnum.RENEW` + `LockGrpcClient.java:279` are GA. Both the server op and the Rust client wrapper are still missing; see `F-NAC-CSDK-002` and `docs/compat/review-report.md` §8.2. |
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
| F-NAC-MAINT-004 | AiMaintainerService (MCP, A2A, prompt, skill, agentspec) | batata-maintainer-client/model/ai | 🟡 | | 2026-11-09 re-verified: only `mcp_server_*` CRUD + `agent_*` CRUD/list/search are exposed in `src/client.rs`; upstream `AiMaintainerService.mcp()/skill()/prompt()/agentSpec()/pipeline()` delegates, `listMcpServerVersions`, `submit/publish/forcePublish/redraft/online/offline` and ZIP uploads are **not** wrapped. `model/ai.rs` only holds 11 DTOs. |
| F-NAC-MAINT-005 | AgentMaintainerService (draft/publish/labels/offline/online) | batata-maintainer-client | 🟡 | | 2026-11-09 re-verified: the legacy `/v3/admin/ai/a2a` facade in `src/client.rs` has register/get/update/delete/list/list_versions/search, but no `createDraft/updateDraft/submit/publish/forcePublish/redraft/online/offline/updateLabels`; `agent_get` has no `version` parameter. Batch 2 added a separate protocol-neutral `agents_*` facade on `/v3/admin/ai/agents` (see `F-NAC-MAINT-008`); the legacy A2A facade is still partial. |
| F-NAC-MAINT-006 | MCP server version lifecycle (listVersions / getVersion / submit / publish / forcePublish / redraft / online / offline) | batata-maintainer-client | 🟢 | | Implemented 2026-11-10 in `crates/batata-maintainer-client/src/client.rs`: `mcp_server_list_versions`, `mcp_server_get_version` and the six `mcp_server_*_version` transitions, backed by the ADMIN paths in `crates/batata-maintainer-client/src/constants.rs` and the server routes in `crates/batata-ai/src/api/mcp.rs`. Covered by 4 wiremock cases in `crates/batata-maintainer-client/tests/client_test.rs` (query shape and all six paths mutant-checked). |
| F-NAC-MAINT-007 | `AiMaintainerService` delegates (`mcp()/skill()/prompt()/agentSpec()/pipeline()/a2a()`) + Skill/Prompt/AgentSpec ZIP uploads and precheck | batata-maintainer-client | ⚪ | | Registered 2026-11-10. Upstream `AiMaintainerService.java:47-73` and `.../ai/{Skill,Prompt,AgentSpec}MaintainerService.java`; console routes exist. Pure SDK wrapping gap. |
| F-NAC-MAINT-008 | `AgentMaintainerService` draft lifecycle (`createDraft/updateDraft/submit/publish/forcePublish/redraft/online/offline/updateLabels`) + `agent_get(version)` | batata-maintainer-client | 🟡 | | Registered 2026-11-10; batch 2 implemented the protocol-neutral `agents_*` facade in `crates/batata-maintainer-client/src/client.rs`: `agents_get(version)`, `agents_list`, `agents_list_versions`, `agents_get_version`, create/update/delete draft, all six version transitions, `agents_update_labels`, `agents_update_scope`, backed by `crates/batata-maintainer-client/src/constants.rs` (`/v3/admin/ai/agents*`) and the server routes in `crates/batata-ai/src/api/agent.rs`. 7 wiremock cases in `tests/client_test.rs` mutant-check the query wire. **Still partial**: `createDraft`/`updateDraft`/`updateLabels` follow batata's JSON-body wire rather than the upstream GA form wire (`F-NAC-MAINT-010`), `listAgentVersions` returns a plain array on batata instead of GA's `Page`, and `updateAgent`/`deleteAgent`/`getRuntimeEndpoints` of the GA facade are not wrapped. |
| F-NAC-MAINT-009 | `publishConfig(..., changeMessage)` + `ConfigHistoryDetailInfo.changeMessage` | batata-maintainer-client | ⚪ | | **Develop drift** (post-GA commit `4a11bd06b` / #15914), not part of the 3.3.0 GA baseline. `ConfigMaintainerService.java:203`. |
| F-NAC-MAINT-010 | AI ADMIN POST/PUT parameter channel: GA maintainer client sends a form body, batata handlers read query only (and agent draft/labels require JSON bodies) | batata-ai | ⚪ | | Registered 2026-11-10, verified against GA bytecode (`~/.m2/.../nacos-maintainer-client-3.3.0.jar`): `ClientHttpProxy.executeSync` calls `postForm`/`putForm` for POST/PUT, and `AgentMaintainerServiceImpl` / `McpMaintainerServiceImpl` use `setParamValue`. `crates/batata-ai/src/api/mcp.rs:701` (`submit_version`) and `agent.rs:512` (`submit_agent`) take `web::Query` only; `agent.rs:432,459,668` require `web::Json<AgentCard>` / `web::Json<AgentLabelsForm>` while GA sends `AgentDraftCreateRequest` / `AgentLabelsUpdateRequest` form fields. A GA Java maintainer client therefore gets 400/415 on these endpoints. Batata's Rust SDK follows the batata wire (see `F-NAC-MAINT-008`) and Spring `@RequestParam` also accepts query, so this is a **server-side** gap. The GA runtime matrix does not cover it (`maintainer-sdk-test` focuses on config/naming). |

## B.9. Client SDK gaps (`F-NAC-CSDK-`)

> Explicitly registered on 2026-11-10 so that "documented but not implemented"
> is machine-visible. All rows are `⚪ planned` until implementation lands;
> the last row is post-GA develop drift, kept separate from the GA baseline.

| ID | SDK capability | batata impl | Status | Tests | Notes |
|----|----------------|-------------|--------|-------|-------|
| F-NAC-CSDK-001 | `NamingService.subscribe(..., NamingSelector, ...)` overloads | batata-client | ⚪ | | Upstream `NamingService.java:576,589`; `crates/batata-client/src/traits.rs:239` has only the 4-arg form. Present in GA, so a true baseline gap. |
| F-NAC-CSDK-002 | `LockService::renew` + watchdog | batata-client | ⚪ | | Upstream `LockService.java:85` (`@Since 3.3.0`) and `LockOperationEnum.RENEW`; `crates/batata-client/src/lock.rs` has only lock/unlock/try_lock/lock_guard. **Both sides are missing**: the server-side `RENEW` branch claimed by `F-NAC-LOCK-009` does not exist either, so the client wrapper alone would send an unsupported op. A correct fix needs `LockService::renew` (refresh TTL without incrementing the reentrant count) plus a Raft renew request variant. |
| F-NAC-CSDK-003 | `AiService` sub-services (`mcp()/skill()/prompt()/agentSpec()/agent()`), `AgentDiscoveryService`, GA RAD capability | batata-client | ⚪ | | Upstream `AiService.java:67-120` + `api/ai/converter/`; `crates/batata-client/src/ai.rs` has 10 flat query methods only. |
| F-NAC-CSDK-004 | config 304 conditional GET (`GetConfigRequest.localMd5`) | batata-client | ⚪ | | Upstream `GetConfigRequest.java:61`, commit `e2f806777`; needs a client-side wire field plus server handling. |
| F-NAC-CSDK-005 | OIDC client auth and `configSnapshotEnabled` property switch | batata-client | ⚪ | | Upstream `client-basic/.../oidc/` (`c87f888fc`) and `PropertyKeyConst.java:96` (`0004ea910`); batata has only an internal `SnapShotSwitch` boolean. |
| F-NAC-CSDK-006 | HTTP `requestId` correlation header | batata-client | ⚪ | | Upstream client HTTP proxy; gRPC has a UUID `request_id` but `src/http.rs` sends no request-id header. |
| F-NAC-CSDK-007 | `SetupAckRequest.ability_table` GA keys `agentCardV1` / `mcpDraftRelease` / `radV1` / `radWatchV1` | batata-core | ⚪ | | Upstream `AbilityKey.java:62,76,86,92` declares 9 server keys; `crates/batata-core/src/handler/rpc.rs:838-851` advertises 5. **Deferred on purpose**: declaring `radV1`/`radWatchV1` makes GA Java clients switch to RAD gRPC requests that `crates/batata-ai/src/handler/ai_handler.rs` does not handle (`AgentPublishRpcRequest`, watch). Implement the handlers first. |
| F-NAC-CSDK-008 | GA `AgentCard` security fields in the client DTO; post-GA `SecurityRequirement`/`StringList` wrapper and `AgentSkill.securityRequirements` | batata-client | 🟡 | | GA shape implemented 2026-11-10 in `crates/batata-client/src/ai.rs` (`AgentCardInfo.security_schemes` / `security` / `security_requirements`, 2 serde tests, mutant-checked), matching `crates/batata-common/src/model/ai/a2a.rs:107-112`. **Develop drift still missing**: at GA `securityRequirements` is `List<Map<String,List<String>>>`; `ff1bcc2e9` changed it to `List<SecurityRequirement>` with a `schemes`/`list` wrapper and `7c546cfda` added `AgentSkill.securityRequirements`. That is a breaking wire change requiring a server DTO change too. |

---

# Summary (auto-updated)

| Face / module | 🟢 | 🟡 | ⚡ | ⚪ | ⛔ | Total | Impl rate |
|---------------|----|----|----|----|----|-------|-----------|
| A.1 SDK Conn | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| A.2 Config gRPC | 10 | 0 | 0 | 0 | 0 | 10 | 100% |
| A.3 Naming gRPC | 11 | 0 | 0 | 0 | 0 | 11 | 100% |
| A.4 Cluster gRPC | 4 | 0 | 0 | 0 | 1 | 5 | 80% |
| A.5 Lock gRPC | 10 | 0 | 0 | 1 | 0 | 11 | 91% |
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
| B.8 Maintainer SDK | 4 | 3 | 0 | 3 | 0 | 10 | 40% |
| B.9 Client SDK gaps | 0 | 1 | 0 | 7 | 0 | 8 | 0% |
| **Total** | 176 | 4 | 0 | 11 | 3 | 194 | 91% |

> Recomputed from the F-ID rows on 2026-11-10 (`scripts/compat_check.py` enforces
> this table). Initial verification was 2025-08-08; the maintainer SDK rows were
> re-verified against `crates/batata-maintainer-client/src/client.rs` on
> 2026-11-09, and the 2026-11-10 upstream refresh added the 12 `⚪` rows in
> B.8/B.9. Batch 2 promoted `F-NAC-MAINT-008` to `🟡` and registered the
> GA-vs-batata agent draft form wire as `F-NAC-MAINT-010` (13th row).
> Impl rate = 🟢 / Total. gRPC type names must stay exact for
> interop; HTTP rows were verified against `console-ui-next` (`src/api/*.ts`)
> and the upstream test suite. Update statuses per actual implementation and
> sync this table.
