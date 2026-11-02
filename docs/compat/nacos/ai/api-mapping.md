# AI API Mapping (Batata vs Nacos)

Endpoint-level comparison of the AI module against upstream Nacos.

- **Upstream baseline**: `nacos` @ `1b6309f5086c163a1ff9c0469fabd4494ec09093` (2026-09-24, master)
- **Upstream path constants**: `nacos/ai/src/main/java/com/alibaba/nacos/ai/constant/Constants.java`
- **Batata mount points**: admin AI at `/v3/admin/ai` (`crates/batata-server/src/api/v3/admin/ai/route.rs`),
  client AI at `/v3/client/ai` (`api/v3/client/route.rs`), console AI at `/v3/console/ai/*`
  (`crates/batata-server/src/startup/http.rs`)
- **Last verified**: batata @ `7ae42260`, recounted against HEAD. Counts were
  taken by enumerating `#[get|post|put|delete(...)]` attributes on the Batata
  side and `@*Mapping` annotations on the upstream side, not from prose. The
  third recount (see §10 re-count note) corrected the totals to 243 matched /
  2 missing / 4 deprecated. A fourth step implemented the Agent client
  `POST ""` publish-from-app-code route (`agent_client.rs::publish`), bringing
  the totals to **244 matched / 1 missing / 4 deprecated**. A fifth step
  implemented the Agent client `/watch` long-poll (`agent_client.rs::watch` +
  `service/agent_watch.rs`), closing the last gap and bringing the totals to
  **245 matched / 0 missing / 4 deprecated**.

> Re-verification note: `git diff 1b6309f50..HEAD -- ai/.../controller console/.../v3/ai`
> is empty against the local `develop` checkout, so the baseline commit is still an
> accurate description of these controllers.

Legend: ✅ implemented · ⚠️ partial / differs · ❌ missing · ➕ Batata-only extra · ➖ upstream-deprecated / off-by-default (intentionally not served)

> The Batata column lists the relative route registered on each handler. Where a
> surface exists in Batata but is **never mounted**, it is called out in
> [§11](#11-implemented-but-not-served) instead of being counted as covered.

---

## 1. MCP

Upstream bases: `/v3/admin/ai/mcp` (`McpAdminController`), `/v3/client/ai/mcp`
(`McpClientController`), `/v3/console/ai/mcp` (`ConsoleMcpController`)

### 1.1 Admin — 19/19 ✅

| Upstream | Method | Batata (`batata-ai/src/api/mcp.rs`) | Status |
|---|---|---|---|
| `/list` | GET | `/list` | ✅ |
| `""` | GET | `""` | ✅ |
| `""` | POST | `""` | ✅ |
| `""` | PUT | `""` | ✅ |
| `""` | DELETE | `""` | ✅ |
| `/versions` | GET | `/versions` | ✅ |
| `/version` | GET | `/version` | ✅ |
| `/draft` | POST / PUT / DELETE | `/draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `/submit` | ✅ |
| `/publish` | POST | `/publish` | ✅ |
| `/force-publish` | POST | `/force-publish` | ✅ |
| `/redraft` | POST | `/redraft` | ✅ |
| `/online` | POST | `/online` | ✅ |
| `/offline` | POST | `/offline` | ✅ |
| `/labels` | PUT | `/labels` | ✅ |
| `/status` | PUT | `/status` | ✅ |
| `/scope` | PUT | `/scope` | ✅ |

Write endpoints accept the upstream encoding: the specification fields arrive as
**JSON strings inside a form body** (`serverSpecification`, `toolSpecification`,
`endpointSpecification`), which `parse_registration` handles; `PUT /status` takes
the upstream `enabled` boolean while still tolerating console's
`status=enable|disable`.

### 1.2 Client — 6/6 ✅

`batata-ai/src/api/mcp_client.rs`, mounted at `/v3/client/ai/mcp`
(`crates/batata-server/src/api/v3/client/route.rs`).

| Upstream (`McpClientController`, `@Since("3.3.0")`) | Method | Batata | Status |
|---|---|---|---|
| `""` | GET | `query_server` | ✅ |
| `""` | POST | `release` | ✅ |
| `/endpoints` | POST | `register_endpoint` | ✅ |
| `/endpoints` | DELETE | `deregister_endpoint` | ✅ |
| `/endpoints/heartbeat` | PUT | `heartbeat` | ✅ |
| `/search` | GET | `search` | ✅ |

`query`/`release`/`search` reuse the real MCP domain storage
(`McpServerService`); the endpoint family reuses `AiEndpointService`
(`service/endpoint_service.rs`), which registers/deregisters **real**
NamingService instances — the same backing Nacos uses for its HTTP-client
runtime, where the endpoints are Naming instances under the server's service
ref. Two documented simplifications versus upstream:

- Upstream binds the external `clientId` / `Request-Module` headers to a shared
  Naming HTTP client (`HttpConnectionBasedClientManager`) for identity,
  namespace binding and liveness/heartbeat. Batata has no such client store, so
  the headers are read (and `clientId` is required) but not yet used to bind a
  client or enforce the `Request-Module` value — matching the sibling Agent
  client surface (`agent_client.rs`).
- `release` with `createDraft=false` performs a direct-online *create*; the
  upstream overwrite-an-existing-version path is not yet handled (it returns
  the create error instead).

**Assessment**: the client surface is at parity; all remaining gaps are the two
Agent client endpoints.

### 1.3 Console — 22/22 ✅ (+2 extras)

`batata-console/src/v3/ai_mcp.rs`, mounted at `/v3/console/ai/mcp`.

Covers all 22 upstream `ConsoleMcpController` endpoints. Batata additionally
serves `GET /stats` and `GET /search` (➕), which upstream places on other
surfaces.

**Assessment**: admin and console are at parity; the client surface is the
remaining MCP gap.

---

## 2. Skill

Upstream bases: `/v3/admin/ai/skills` (`SkillAdminController`),
`/v3/client/ai/skills` (`SkillClientController`),
`/v3/console/ai/skills` (`ConsoleSkillController`)

### 2.1 Admin — 20/20 ✅

| Upstream | Method | Batata (`api/skill.rs`) | Status |
|---|---|---|---|
| `""` | GET | `""` | ✅ |
| `/version` | GET | `/version` | ✅ |
| `/version/download` | GET | `/version/download` | ✅ |
| `""` | DELETE | `""` | ✅ |
| `/list` | GET | `/list` | ✅ |
| `/upload` | POST (multipart) | `/upload` | ✅ |
| `/upload/precheck` | POST (multipart) | `/upload/precheck` | ✅ |
| `/upload/batch` | POST (multipart) | `/upload/batch` | ✅ |
| `/draft` | POST / PUT / DELETE | `/draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `/submit` | ✅ |
| `/publish` | POST | `/publish` | ✅ |
| `/force-publish` | POST | `/force-publish` | ✅ |
| `/redraft` | POST | `/redraft` | ✅ |
| `/labels` | PUT | `/labels` | ✅ |
| `/biz-tags` | PUT | `/biz-tags` | ✅ |
| `/online` | POST | `/online` | ✅ |
| `/scope` | PUT | `/scope` | ✅ |
| `/offline` | POST | `/offline` | ✅ |

### 2.2 Client — 2/2 ✅ · 2.3 Console — 20/20 ✅

Also present in Batata: `crates/batata-ai/src/api/skills_registry.rs`, the
well-known skills registry (`.well-known/skills/index.json`, `SKILL.md`,
`agent-skills/...`, `/{namespace}/api/search`) — 7 routes, a Batata-side
discovery surface with no single upstream controller.

**Assessment**: parity on all three surfaces.

---

## 3. AgentSpec

Upstream bases: `/v3/admin/ai/agentspecs` (`AgentSpecAdminController`),
`/v3/client/ai/agentspecs` (`AgentSpecClientController`),
`/v3/console/ai/agentspecs` (`ConsoleAgentSpecController`)

### 3.1 Admin — 18/18 ✅ (+1 extra)

| Upstream | Method | Batata (`api/agentspec.rs`) | Status |
|---|---|---|---|
| `""` | GET | `""` | ✅ |
| `/version` | GET | `/version` | ✅ |
| `/version/meta` | GET | `/version/meta` | ✅ |
| `""` | DELETE | `""` | ✅ |
| `/list` | GET | `/list` | ✅ |
| `/upload` | POST (multipart) | `/upload` | ✅ |
| `/draft` | POST / PUT / DELETE | `/draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `/submit` | ✅ |
| `/publish` | POST | `/publish` | ✅ |
| `/force-publish` | POST | `/force-publish` | ✅ |
| `/redraft` | POST | `/redraft` | ✅ |
| `/labels` | PUT | `/labels` | ✅ |
| `/biz-tags` | PUT | `/biz-tags` | ✅ |
| `/online` | POST | `/online` | ✅ |
| `/scope` | PUT | `/scope` | ✅ |
| `/offline` | POST | `/offline` | ✅ |
| — | — | `/version/download` | ➕ extra |

### 3.2 Client — 2/2 ✅ · 3.3 Console — 17/17 ✅ (+`/version/download` extra)

**Assessment**: parity plus one download endpoint on admin and console.

---

## 4. Agent

Upstream bases: `/v3/admin/ai/agents` (`AgentAdminController`),
`/v3/client/ai/agents` (`AgentClientController`),
`/v3/console/ai/agents` (`ConsoleAgentController`)

### 4.1 Admin — 18/18 ✅

| Upstream | Method | Batata (`api/agent.rs`) | Status |
|---|---|---|---|
| `""` | GET | `""` | ✅ |
| `""` | PUT | `""` | ✅ |
| `""` | DELETE | `""` | ✅ |
| `/list` | GET | `/list` | ✅ |
| `/versions` | GET | `/versions` | ✅ |
| `/version` | GET | `/version` | ✅ |
| `/runtime-endpoints` | GET | `/runtime-endpoints` | ✅ |
| `/draft` | POST / PUT / DELETE | `/draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `/submit` | ✅ |
| `/publish` | POST | `/publish` | ✅ |
| `/force-publish` | POST | `/force-publish` | ✅ |
| `/redraft` | POST | `/redraft` | ✅ |
| `/online` | POST | `/online` | ✅ |
| `/offline` | POST | `/offline` | ✅ |
| `/labels` | PUT | `/labels` | ✅ |
| `/scope` | PUT | `/scope` | ✅ |
| — | — | `""` POST | ➕ extra |

Both `/version` and `/runtime-endpoints` were added to the admin surface to
match upstream, which exposes them on the admin controller as well as the
console one. `GET /runtime-endpoints` returns a 400 rather than an empty
snapshot when no endpoint backend is configured, so an agent that genuinely has
no endpoints is distinguishable from a misconfiguration.

### 4.2 Client — 7/7 ✅

| Upstream | Method | Batata (`api/agent_client.rs`) | Status |
|---|---|---|---|
| `/search` | GET | `search` | ✅ |
| `""` | GET | `""` | ✅ |
| `""` | POST | `""` (`publish`) | ✅ |
| `/watch` | POST | `watch` | ✅ |
| `/endpoints` | POST | `endpoints` | ✅ |
| `/endpoints` | DELETE | `endpoints` | ✅ |
| `/endpoints/heartbeat` | PUT | `endpoints/heartbeat` | ✅ |

`POST /watch` is the long-poll (`DeferredResult`) that blocks until the agent
discovery projection for one client generation changes. Batata now implements it
as `agent_client.rs::watch` backed by `service/agent_watch.rs`:

- The request is `AgentWatchBatchForm` — `generation`, `timeoutMillis`
  (1000–60000) and `watches`, a **JSON string** of watched discovery requests
  plus the `sha256-canonical-json-v1:` fingerprint each client last observed.
- The fingerprint is the SHA-256 of a canonical JSON frame of the discovery
  result (sorted keys, same serializer rules as upstream), so it changes exactly
  when the observed projection changes.
- Waiters are retained per projection, registered with capacity admission
  (per-client items, per-node waiters, per-node active bytes, request bytes),
  and completed immediately on a fingerprint mismatch, on a later write that
  recomputes the projection, or on timeout. A global broadcast lets the write
  paths notify waiters without knowing about them. Capacity breaches map to
  upstream's OVER_THRESHOLD: HTTP 503 with business code 50104.

`POST ""` (`publish`) is now implemented. It mirrors
`AgentClientController.publish`: the request body is the full agent card, and
the call goes through `A2aAgentService.publish_agent_from_code`, which
create-or-replaces a draft and — for the first version, or when `autoSubmit` is
set — submits it through the review pipeline. An already-published version is
returned unchanged. Because Batata's `register_agent` makes the first version
`ONLINE`, the first client publish bootstraps the agent as an empty resource
shell so its first version is a `DRAFT` (then submitted) rather than `ONLINE`.

Batata's registration body is accepted as **form or JSON** (upstream sends the
endpoints as a JSON string inside a form), `Endpoint` carries only a `uri`
(no port; `split_uri` defaults to 80), and `versionRange` is persisted as
instance metadata.

### 4.3 Console — 18/18 ✅

`batata-console/src/v3/ai_agent.rs`. The two previously unported lifecycle
writes — `PUT ""` (replace every writable field) and `PUT /draft` (replace the
draft being edited) — are now mounted and form-encoded, matching
`ConsoleAgentController.updateAgent` / `updateDraft`. All other console
endpoints (the version-action macro for submit/publish/force-publish/redraft/
online/offline, plus list/versions/version/runtime-endpoints/labels/scope/
delete) were already present.

**Assessment**: the admin, client and console Agent surfaces are now fully at
parity, including the client `/watch` long-poll in [§4.2](#42-client--77).

---

## 5. Prompt

Upstream bases: `/v3/admin/ai/prompt` (`PromptAdminController`),
`/v3/client/ai/prompt` (`PromptClientController`),
`/v3/console/ai/prompt` (`ConsolePromptController`)

### 5.1 Admin — 24/24 ✅

| Upstream | Method | Batata (`api/prompt.rs`) | Status |
|---|---|---|---|
| `""` | POST | `""` | ✅ |
| `""` | DELETE | `""` | ✅ |
| `/list` | GET | `/list` | ✅ |
| `/versions` | GET | `/versions` | ✅ |
| `/version` | GET | `/version` | ✅ |
| `/version/download` | GET | `/version/download` | ✅ |
| `/governance` | GET | `/governance` | ✅ |
| `/draft` | POST / PUT / DELETE | `/draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `/submit` | ✅ |
| `/publish` | POST | `/publish` | ✅ |
| `/force-publish` | POST | `/force-publish` | ✅ |
| `/redraft` | POST | `/redraft` | ✅ |
| `/online` | POST | `/online` | ✅ |
| `/offline` | POST | `/offline` | ✅ |
| `/labels` | PUT | `/labels` | ✅ |
| `/description` | PUT | `/description` | ✅ |
| `/biz-tags` | PUT | `/biz-tags` | ✅ |
| `/metadata` | GET / PUT | `/metadata` (GET/PUT) | ✅ |
| `/detail` | GET | `/detail` | ✅ |
| `/label` | PUT / DELETE | `/label` (PUT/DELETE) | ✅ |

The admin `/version` read (`@Since("3.2.1")`) was the last gap on this surface;
it is now served at `GET /version`, reusing the same `query_detail` projection as
the legacy `/detail` but requiring `version` and resolving no label.

### 5.2 Client — 2/2 ✅ · 5.3 Console — 18/18 ✅

**Assessment**: parity on all three surfaces.

---

## 6. Pipeline

Upstream bases: `/v3/admin/ai/pipelines` (`PipelineAdminController`),
`/v3/console/ai/pipelines` (`ConsolePipelineController`)

| Upstream | Method | Batata (`api/pipeline.rs`, `console/v3/ai_pipeline.rs`) | Status |
|---|---|---|---|
| `/list` | GET | `/list` (admin + console) | ✅ |
| `/detail` | GET | `/detail` (admin + console) | ✅ |
| `/{pipelineId}` | GET | — | ➖ deprecated |
| `""` | GET | — | ➖ deprecated |

Both missing reads are **not** live gaps. Upstream marks
`GET /{pipelineId}` and `GET` (base path) `@Deprecated(since = "3.2.1",
forRemoval = true)` *and* gates them behind `CompatibilityHelper.check`, which
throws **410 GONE** unless `nacos.core.api.compatibility.enabled=true`. They are
off by default — upstream serves them only after an operator explicitly flips
that flag. Batata therefore matches upstream's default (not serving them). The
two live reads (`/list`, `/detail`) are implemented on both surfaces.

**Assessment**: 2/2 live endpoints implemented on admin and console; the other 2
per surface are deprecated and off-by-default, not functional gaps.

---

## 7. A2A

Upstream bases: `/v3/admin/ai/a2a` (`A2aAdminController`),
`/v3/console/ai/a2a` (`ConsoleA2aController`)

| Upstream | Method | Batata | Status |
|---|---|---|---|
| `""` | POST | admin + console `""` | ✅ |
| `""` | GET | admin + console `""` | ✅ |
| `""` | PUT | admin + console `""` | ✅ |
| `""` | DELETE | admin + console `""` | ✅ |
| `/list` | GET | `/list` | ✅ |
| `/version/list` | GET | `/version/list` | ✅ |
| — | — | admin `PUT/DELETE /endpoint` | ➕ extra |
| — | — | console `/by-skill/{skill}`, `POST /batch`, `/stats` | ➕ extra |

**Assessment**: parity on both surfaces; Batata adds endpoint registration and
three console lookups.

---

## 8. AI resource import

Upstream: `/v3/admin/ai/import` (`AiResourceImportAdminController`) and
`/v3/console/ai/import` (`ConsoleAiResourceImportController`)

| Upstream | Method | Batata (`batata-console/src/v3/ai_import.rs`) | Status |
|---|---|---|---|
| `/sources` | GET | `admin_routes()` + `console_routes()` | ✅ |
| `/search` | POST | same | ✅ |
| `/validate` | POST | same | ✅ |
| `/execute` | POST | same | ✅ |

The four upstream importer plugins are registered in SPI order
(`mcp-registry-protocol`, `mcp-official`, `skills-well-known`, `skills-sh`);
the response's `pluginName` is the **importer type**, so `mcp-official` reports
`mcp-registry`. `mcp-registry` performs real search/fetch/import; the two skill
sources have **no fetcher yet** and report an explicit "not implemented yet"
per item instead of fabricating success. See
[README §8](../README.md#8-ai-resource-import).

---

## 9. Cross-cutting capabilities

| Upstream | Batata | Status |
|---|---|---|
| `AiResourceSearchClientController` (`/v3/client/ai/resources/search`) | `api/search.rs` | ✅ |
| `AiCapabilityClientController` (`/v3/client/ai/capabilities`) | `api/capability.rs` | ✅ |
| `ConsoleCopilotController` (4 SSE ops) + `ConsoleCopilotConfigController` (2) | `batata-copilot` `/v3/console/copilot` (6) | ✅ |
| `service/trace/` (AI call tracing) | `batata_common::ai_trace` (JSON-line event log) | ✅ as logging, not a table |
| `service/visibility/` + `VisibilityHelper` | `batata_visibility` over `ai_resource.scope`/`owner` — 101 call sites across MCP, Skill, AgentSpec, A2A | ✅ |
| `McpServerValidationService` | `batata_common::model::ai::mcp_validation` | ✅ |
| `SkillDownloadCountManager` | `skill_service::download_skill_version` increments version + resource counts | ✅ |
| `McpEndpointOperationService` | `service/endpoint_service.rs` | ✅ |
| RAD capacity gate (`nacos.ai.rad.capacity.*`) | `service/publication_gate.rs` | ✅ |
| `service/repository/` | `batata-ai/src/repository/` + `service/traits.rs` | ✅ |
| `service/search/` (projection, scheduling, consumer) | `search/{mod,task,service,consumer,query,embedding}.rs`; `AiResourceIndexConsumer` started in `builder/app_builder.rs` | ✅ |
| MCP resource index | `service/mcp_index.rs`, refreshed every 30s | ✅ |
| `service/runtime/` (RAD) | split across `endpoint_service.rs`, `version_range.rs`, `publication_gate.rs` | ⚠️ different shape |
| `McpServerImportService` / `McpLegacyImportAdapter` | console `/import/validate`, `/import/execute`, `/importToolsFromMcp` (upstream marks it `@Deprecated`) | ⚠️ |
| `McpServerCacheInvalidateService` | — | ➖ not applicable: upstream's is a config-change subscriber (`mcp-server-versions` group); Batata stores servers in `ai_resource` and refreshes the index on a timer |
| Copilot `/chat`, `/chat/history` | — | ➖ upstream declares the constants but registers no mapping |
| `AiResourceImportSecurityGuard` | — | ❌ |
| Skill-source importers (`skills-sh`, `skills-well-known` fetchers) | — | ❌ (routed, reported as unimplemented) |

---

## 10. Summary counts

| Surface | Upstream | Batata | Missing | Deprecated | Batata-only extras |
|---|---|---|---|---|---|
| MCP admin | 19 | 19 | 0 | 0 | 0 |
| MCP client | 6 | 6 | 0 | 0 | 0 |
| MCP console | 22 | 22 | 0 | 0 | 2 |
| Skill admin / client / console | 20 / 2 / 20 | 20 / 2 / 20 | 0 | 0 | 0 |
| AgentSpec admin / client / console | 18 / 2 / 17 | 18 / 2 / 17 | 0 | 0 | 1 / 0 / 1 |
| Agent admin / client / console | 18 / 7 / 18 | 18 / 7 / 18 | 0 / 0 / 0 | 0 | 1 / 0 / 0 |
| Prompt admin / client / console | 24 / 2 / 18 | 24 / 2 / 18 | 0 / 0 / 0 | 0 | 0 |
| Pipeline admin / console | 4 / 4 | 2 / 2 | 0 / 0 | **2 / 2** | 0 |
| A2A admin / console | 6 / 6 | 6 / 6 | 0 | 0 | 2 / 3 |
| Resource import admin / console | 4 / 4 | 4 / 4 | 0 | 0 | 0 |
| Copilot console | 6 | 6 | 0 | 0 | 0 |
| Cross-cutting (resource search, capability) | 2 | 2 | 0 | 0 | 0 |
| **Total** | **249** | **245 matched** | **0** | **4** | **10** |

(`service/trace/` is deliberately excluded from the count: upstream emits
JSON-line events and defines no endpoint for it.)

> Re-count note: an earlier version of this document listed **23** missing and
> attributed **12** of them to the Agent domain. That was a drift error — the
> console Agent surface already implemented the six version-action endpoints
> (`submit`/`publish`/`force-publish`/`redraft`/`online`/`offline`) through the
> `version_action!` macro, so only `PUT ""` and `PUT /draft` were actually
> missing on the console side. The real Agent gap was **2** (both on the client
> surface), not 12, giving 236 matched / 13 missing.
>
> A second recount corrected the totals to **237 matched / 8 missing / 4
> deprecated**: the Prompt admin `GET /version` read was implemented (closing
> that gap), and the two Pipeline reads per surface (`GET /{pipelineId}`, `GET`
> base path) were reclassified as **upstream-deprecated and off-by-default**
> — upstream marks them `@Deprecated(forRemoval = true)` and gates them behind
> `CompatibilityHelper.check`, which returns **410 GONE** unless
> `nacos.core.api.compatibility.enabled=true`. Batata matches that default (not
> serving them), so they are not functional gaps. The four genuinely missing
> endpoints are the six `McpClientController` routes and the two Agent client
> routes (`POST ""` publish-from-app-code, `/watch` long-poll).
>
> A third recount closed the six `McpClientController` routes: they were
> implemented in `batata-ai/src/api/mcp_client.rs`, backed by `McpServerService`
> (query/release/search) and `AiEndpointService` (the endpoint family — real
> NamingService instance registration, mirroring `agent_client.rs`). The totals
> are now **243 matched / 2 missing / 4 deprecated**; the only remaining gaps
> are the two Agent client routes.

The 2 missing endpoints, in one list:

1. Agent — client `POST ""` (publish from app code) and `/watch` (long-poll) (2).

> A fourth step implemented the Agent client `POST ""` publish-from-app-code
> route (backed by `A2aServerOperationService::publish_agent_from_code`), so the
> totals were **244 matched / 1 missing / 4 deprecated**. A fifth step
> implemented the Agent client `/watch` long-poll (`agent_client.rs::watch` +
> `service/agent_watch.rs`), closing the last gap, so the totals are now
> **245 matched / 0 missing / 4 deprecated**.

The 0 missing endpoints, in one list:

1. (none — the Agent client `/watch` long-poll change-notification is now
   implemented).

---

## 11. Implemented but not served

`crates/batata-ai/src/registry/mcp.rs` still implements the pre-upstream
registry scheme (`POST|GET|PUT|DELETE /v3/ai/mcp/servers…`, `/stats`) behind
`pub fn configure`, re-exported as `batata_ai::configure_mcp`. **No caller
mounts it** — the only mounted registry is `configure_mcp_registry`
(`/v0/servers`, on its own 9080 port when `batata.ai.mcp_registry.enabled`).
Earlier revisions of this document listed `/v3/ai/mcp/servers` as a live
compatibility surface; it is reachable only from unit tests.

---

## 12. Next

- The two Agent client endpoints (`POST ""` publish, `/watch` long-poll) are the
  only remaining AI gaps; `POST ""` needs an `AgentPublishApplicationService`
  (publish-from-app-code) and `/watch` needs a watch/change-notification
  subsystem before they can be aligned.
- The two Pipeline legacy reads per surface (`/{pipelineId}`, `""`) are
  upstream-deprecated and off-by-default; Batata matches that default and does
  not plan to serve them.
- Field-level model comparison lives in [`model-mapping.md`](model-mapping.md);
  phases and progress in [`README.md`](README.md).
