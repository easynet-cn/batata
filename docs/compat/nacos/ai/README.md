# AI / MCP Compatibility Plan (Batata vs Nacos)

Tracking document for aligning Batata's AI module with upstream Nacos.

- **Baseline**: upstream Nacos `master` @ `1b6309f5086c163a1ff9c0469fabd4494ec09093`
  (2026-09-24). Local checkout: `~/work/github/easynet-cn/nacos`. The commit is
  pinned so that future diffs are reproducible.
- **Endpoint-level mapping**: see [`api-mapping.md`](api-mapping.md)
- **Data-model mapping**: see [`model-mapping.md`](model-mapping.md)
- **Upstream architecture**: see [`architecture.md`](architecture.md)
- **Goal**: full capability parity, not just endpoint parity
- **Order**: architecture and core first; HTTP API is the surface, not the starting point
- **Rule**: mirror the upstream implementation logic. Adding a route that returns the
  right status code is not "done" — behaviour, validation, versioning and lifecycle
  semantics must match upstream.

---

## 1. Reference locations

| Concern | Upstream Nacos | Batata |
|---|---|---|
| AI module root | `nacos/ai/src/main/java/com/alibaba/nacos/ai/` | `crates/batata-ai/` |
| HTTP controllers | `ai/.../ai/controller/` (15 controllers) | `crates/batata-ai/src/api/`, `crates/batata-server/src/api/v3/admin/ai/`, `crates/batata-console/src/v3/ai_*` |
| Request/response forms | `ai/.../ai/form/` (91 files) | `crates/batata-ai/src/model/`, per-module structs |
| Service layer | `ai/.../ai/service/` (199 files) | `crates/batata-ai/src/service/` |
| Index / search | `ai/.../ai/index/`, `ai/.../ai/service/search/` | `crates/batata-ai/src/service/mcp_index.rs` |
| Importers | `ai/.../ai/form/importer/`, `service/McpServerImportService.java` | `crates/batata-console/src/v3/ai_import.rs` |
| Prompt / copilot | `ai/.../ai/service/prompt/` | `crates/batata-ai/src/service/prompt/`, `crates/batata-copilot/` |

---

## 2. Architecture comparison

### Upstream Nacos AI layering

```
controller  (Admin* / *Client* split: management vs. runtime/SDK access)
    |
service     a2a/  agent/  agentspecs/  mcp/  pipeline/  prompt/  skills/
            repository/  resource/  runtime/  search/  trace/  visibility/
    |
index / storage   (config-module reuse + search index)
```

Distinguishing traits:

- **Admin vs Client controllers** are separate: `McpAdminController` (management)
  and `McpClientController` (runtime discovery) expose different contracts.
- **Draft lifecycle** is a first-class concept:
  `draft (POST/PUT/DELETE) -> submit -> publish -> force-publish / redraft ->
  online / offline`, with `versions`, `version`, `version/meta`, `labels`,
  `biz-tags`, `scope` and `status`.
- **Cross-cutting layers** that Batata does not yet have as distinct modules:
  `repository/`, `runtime/`, `search/`, `trace/`, `visibility/`.
- **External resource import** is a built-in capability
  (`AiResourceImportAdminController`, importer forms, `McpServerImportService`)
  covering sources such as the official MCP registry, `skills.sh` and
  well-known endpoints.

### Batata layering (current)

```
api/         a2a.rs  agent_client.rs  agentspec.rs  mcp.rs  pipeline.rs
             prompt.rs  skill.rs  skills_registry.rs
service/     a2a_service  agentspec_service  endpoint_service  mcp_client
             mcp_index  mcp_service  pipeline_service  skill_service
             prompt/  traits  constants
registry/    a2a.rs  mcp.rs  mcp_registry.rs
model/       mod.rs
```

**Storage per domain** (verified 2026-09-26):

| Domain | Storage in Batata | On the upstream `ai_resource*` path? |
|---|---|---|
| Skill | `ai_resource` + `ai_resource_version` (`skill_service.rs`) | ✅ yes |
| AgentSpec | `ai_resource` + `ai_resource_version` (`agentspec_service.rs`) | ✅ yes |
| MCP | `ai_resource` + `ai_resource_version` (`mcp_service.rs`) | ✅ migrated 2026-09-27 |
| A2A | `ai_resource` + `ai_resource_version` (`a2a_service.rs`) | ✅ migrated 2026-09-27 |

All four AI domains now use the `ai_resource*` path; the config-backed storage
is gone.

**Resource row layout** (aligned with upstream `AiResource` + MCP specifics):

| Column | Payload |
|---|---|
| `version_info` | shared `ResourceVersionInfo` — `editingVersion`, `reviewingVersion`, `onlineCnt`, `labels`. The latest published version is the server-managed `latest` label, not a dedicated field. |
| `ext` | `McpResourceExt` for MCP — `schemaVersion`, `mcpId` (upstream keeps the MCP id here, not in `version_info`). |

`McpServerVersionInfo` is **not** the storage format; upstream only uses it as a
response/compat model. Batata previously stored it, which made `labels`,
`editingVersion`, `reviewingVersion` and `onlineCount` unrepresentable — this has
been corrected. Latest selection follows upstream
`chooseLatest(onlineVersions, preferredLatest, currentLatest)`: the version being
published wins, else the existing label while still online, else the highest
remaining online version.

> Caution (bitten twice, keep it): rewriting a service file wholesale silently
> drops things that `cargo check -p batata-ai` does **not** catch.
> - A2A: lost `impl A2aAgentService for A2aServerOperationService` + unit tests
>   — only surfaced when compiling `batata-server`.
> - MCP: lost the module-level `fn default_mcp_protocol()` (a plain top-level
>   `fn`, invisible to greps for `impl`/`pub fn`).
>
> Before replacing a service file, diff it for: trait impls, `#[cfg(test)]`
> blocks, and **plain module-level helper functions**. Always finish with
> `cargo check -p batata-server`.

Batata already implements the draft lifecycle for **Skill** and **AgentSpec**
(`draft` POST/PUT/DELETE, `submit`, `publish`, `labels`, `biz-tags`, `online`,
`offline`, `scope`, `upload`, `search`, `version`, `version/download`).

Batata also has capabilities that are easy to overlook:

- Well-known registry endpoints (`.well-known/skills/index.json`, `SKILL.md`,
  `agent-skills/...`) in `api/skills_registry.rs`
- Agent client endpoints (`search`, `endpoints` POST/DELETE,
  `endpoints/heartbeat`) in `api/agent_client.rs`
- AI resource import routed via `crates/batata-console/src/v3/ai_import.rs`
- `batata-copilot` for prompt debugging/optimisation and skill generation

---

## 3. Domain / API matrix

Legend: ✅ implemented · ⚠️ partial · ❌ missing · ❓ needs verification

| Domain | Upstream controllers | Batata | Status |
|---|---|---|---|
| MCP management | `McpAdminController` (list, versions, version, draft, submit, publish, force-publish, redraft, online, offline, labels, status, scope) | `api/v3/admin/ai/mcp.rs` — simplified form-based API (`McpForm`) | ⚠️ **main gap** |
| MCP runtime / registry | `McpClientController`, `registry/mcp.rs` | `registry/mcp.rs` (`/v3/ai/mcp/servers`, `import`, `stats`) | ⚠️ |
| A2A | `A2aAdminController` (list, version/list) | `admin/ai/a2a.rs`, `api/a2a.rs`, `registry/a2a.rs` | ⚠️ verify versioning |
| Agent (entity) | `AgentAdminController` (+ `service/agent/`, ~50 files) | `api/agent_client.rs` only | ⚠️ client present, admin CRUD ❓ |
| AgentSpec | `AgentSpecAdminController` (+ Client `search`) | `api/agentspec.rs` (full lifecycle + search) | ✅ close |
| Skill | `SkillAdminController` (+ Client `search`), `SkillDownloadCountManager` | `api/skill.rs` (full lifecycle), `skills_registry.rs` | ✅ close |
| Prompt | `PromptAdminController`, `PromptClientController` | `api/prompt.rs`, `service/prompt/`, `batata-copilot` | ⚠️ verify client contract |
| Pipeline | `PipelineAdminController` | `api/pipeline.rs` | ❓ |
| Resource import | `AiResourceImportAdminController`, importer forms | `batata-console/src/v3/ai_import.rs` | ⚠️ verify parity |
| Resource search | `AiResourceSearchClientController` (+ `service/search/`, 41 files) | `api/*/search` per domain | ⚠️ verify parity |
| Capability discovery | `AiCapabilityClientController` | not found | ❌ |
| Trace / runtime | `service/trace/`, `service/runtime/` | not found | ❌ |
| Visibility | `service/visibility/`, `VisibilityHelper` | not found | ❌ |
| Repository abstraction | `service/repository/` | not found | ❌ |

---

## 4. Phased task list

### Phase 0 — Baseline and inventory (read-only)

- [x] Locate upstream AI module and Batata AI crates
- [x] Extract upstream controller/method inventory
- [x] Extract Batata AI route inventory
- [x] Confirm upstream Nacos commit/tag used as the baseline
- [x] Produce a per-endpoint diff table → [`api-mapping.md`](api-mapping.md)
- [x] Produce a data-model field diff → [`model-mapping.md`](model-mapping.md)
- [ ] Record current test coverage for AI in Batata

### Phase 1 — Core architecture

Goal: introduce the upstream layering before adding endpoints.

- [x] Add the three missing upstream tables (`ai_resource_search_document`,
      `ai_resource_search_chunk`, `ai_resource_task`) — migration `m20260412_000014`
- [x] SeaORM entities for the three new tables in
      `batata-persistence/src/entity/` (registered in `entity/mod.rs` and
      `entity/prelude.rs`)
- [x] `repository/` facade in `batata-ai/src/repository/` — wraps
      `PersistenceService` and holds the upstream value vocabularies
      (resource types, meta/version status, scope, search and task constants)
      > Note: `AiResourcePersistence` with SQL, embedded and distributed
      implementations **already existed** but had no callers. The facade is the
      intended entry point for AI services, so they stop scattering raw strings.
- [x] Persistence operations for the three new tables in `batata-persistence`
      (`search_document_*`, `search_chunk_*`, `task_*`) — behaviour-tested on
      live MySQL 8.4 and PostgreSQL 18.
      Implemented for **all three** backends: SQL (behaviour-tested on live
      MySQL 8.4 and PostgreSQL 18), embedded and distributed (RocksDB column
      families `batata_ai_resource_search_document` / `_search_chunk` /
      `_task`; covered by an embedded unit test).
      > The trait default still returns an explicit "not supported by this
      storage backend" error, as a safety net for any backend added later that
      forgets to implement the index.
- [ ] `runtime/` layer (runtime endpoint resolution, heartbeat)
- [ ] `visibility/` layer via `ai_resource.scope` / `owner`
- [ ] `trace/` layer — **JSON-line trace events, not a table** (`architecture.md` §6)
- [ ] `search/` layer — async and task-driven: enqueue a `search_index` task,
      poll/lease consumer, stages `base_index` → `llm_enhancement`.
      Not query-time logic over `ai_resource`.
- [ ] Replace the config-backed storage for MCP / Skill / AgentSpec / Prompt with
      the `ai_resource*` path. No compatibility mode: Batata is unreleased and has
      no stored data to migrate.
- [ ] Rework `McpForm`-style simplified forms into structured request models

### Phase 2 — MCP parity (largest gap)

- [x] Draft lifecycle: `draft` POST/PUT/DELETE — implemented as
      `create/update/delete_mcp_server_draft` on `McpServerOperationService`,
      backed by `ai_resource_version.status = draft` and tracked through
      `ResourceVersionInfo.editingVersion`. Covered by the MCP live-database
      test (create / overwrite / update / delete, plus `editingVersion`
      clearing).
- [x] `submit`, `publish`, `force-publish`, `redraft`, `online`, `offline` —
      implemented as `submit/publish/force_publish/redraft/online/offline_
      mcp_server_version`. Transitions mirror upstream statuses
      (`draft → reviewing → online | offline`) and reconcile
      `editingVersion` / `reviewingVersion` / `onlineCnt` / `latest` on each
      change. Covered by the `mcp_version_lifecycle` live-database test on
      MySQL and PostgreSQL.
      > Note: two consecutive meta updates must not reuse a stale
      > `meta_version` snapshot — `refresh_version_meta` re-reads the resource
      > so the optimistic lock cannot fail silently (this was a real bug found
      > by the test).
- [ ] `force-publish`, `redraft`
- [x] Versioning: `versions`, `version` — service methods
      `list_mcp_server_versions` / `get_mcp_server_version`
- [x] `labels`, `scope`, `status` — service methods
      `update_mcp_server_labels` (custom labels replace; the server-managed
      `latest` label is preserved; every label must point at an online
      version), `update_mcp_server_status` (enable/disable) and
      `update_mcp_server_scope` (`PUBLIC` / `PRIVATE`, validated)
- [ ] `version/meta`, `biz-tags`
- [x] **HTTP layer** — all fourteen Admin endpoints are wired on the console
      MCP scope, using the upstream paths:

      | Method | Path |
      |---|---|
      | GET | `/versions`, `/version` |
      | POST | `/draft`, `/submit`, `/publish`, `/force-publish`, `/redraft`, `/online`, `/offline` |
      | PUT | `/draft`, `/labels`, `/status`, `/scope` |
      | DELETE | `/draft` |

      The lifecycle and admin methods were added to the `McpServerService`
      trait so the console's `Arc<dyn McpServerService>` can reach them; the
      in-memory `McpServerRegistry` fallback returns an explicit
      "versioning is not supported" error instead of faking one version.

      Covered by `crates/batata-server/tests/mcp_console_routes.rs`
      (5 route tests, run on live MySQL and PostgreSQL): version listing and
      detail, draft creation, and 400 rejection of a missing `mcpName` /
      unknown `status`. The test builds a real `AppState` (stub cluster
      manager, `Configuration` with console auth disabled) on top of the real
      database and the real MCP service.

      > Note: only these five flows are covered. `submit`, `publish`,
      > `force-publish`, `redraft`, `online`, `offline`, `labels` and `scope`
      > have route tests only for the parameter-validation path; their success
      > paths are covered at the service layer instead.
- [ ] `online` / `offline`
- [ ] Admin/Client controller split (management vs runtime discovery)
- [ ] Validation service equivalent to `McpServerValidationService`
- [ ] Import service equivalent to `McpServerImportService` + legacy import adapter
- [ ] Cache invalidation equivalent to `McpServerCacheInvalidateService`

### Phase 3 — Remaining domains

- [ ] Agent admin CRUD + lifecycle (mirror `service/agent/`)
- [ ] `AiCapabilityClientController` equivalent
- [ ] Prompt client contract parity
- [ ] Pipeline parity verification
- [ ] Resource import parity (official registry, `skills.sh`, well-known)
- [ ] Resource search parity
- [ ] Skill download counting

### Phase 4 — Tests and documentation

- [ ] Port upstream AI test cases as Batata integration tests
- [ ] Add `docs/compat/nacos/ai/api-mapping.md` (endpoint-level mapping)
- [ ] Add `docs/compat/nacos/ai/model-mapping.md` (field-level mapping)
- [ ] Update `docs/feature-comparison.md` AI/module rows
- [ ] Update `conf/application.yml` with the AI configuration keys that upstream exposes
      (`nacos.plugin.ai.importer.*`, `nacos.ai.resource.import.*`, ...)

---

## 5. Implementation principles

1. **Read the upstream implementation first.** For each task, identify the
   upstream controller → service → storage path and mirror the logic, including
   validation, error codes and side effects (events, cache invalidation, index updates).
2. **Preserve upstream semantics, not just shapes.** A draft/publish lifecycle
   that accepts the same fields but skips state transitions is not parity.
3. **Respect Batata's conventions**: Rust idioms, `batata.*` configuration keys,
   existing error model, and English-only code and documentation.
4. **Each phase ends with tests**, not just compilation.
5. **Update this document as tasks progress** — it is the single source of truth
   for AI compatibility status.
6. **Deprecation rule** (decided 2026-09-27). Batata is unreleased and carries
   no historical compatibility obligation. Before implementing any AI/MCP
   endpoint or field, check how upstream marks it:
   - `@Deprecated(..., forRemoval = true)` → **do not implement**.
   - `@Deprecated(since = "X")` → do not accept or store the field; implement
     the replacement named in its javadoc.
   - `@Since("X")` records when an API was introduced; prefer the newest
     contract when two exist.

   Upstream gates deprecated endpoints at runtime via `CompatibilityHelper`
   (`nacos.core.api.compatibility.enabled`). Batata needs no such switch because
   it never ships the deprecated paths.

   **Anything Batata already implements that matches the above must be deleted,
   not left in place.** Applied so far:

   - Pipeline `GET /{pipelineId}` and `GET` (base path), on both the admin and
     console sides: removed (`forRemoval = true`, since 3.2.1) and replaced by
     `GET /list` and `GET /detail?pipelineId=`.
   - `updateLatestLabel` on the Skill / AgentSpec publish forms: removed
     (`@Deprecated(since = "3.3.0")`; upstream states the latest label is
     managed by the server). Batata now always sets the `latest` label on
     publish. Legacy clients that still send the field are tolerated — it is
     ignored on parse.
   - MCP bulk import: `McpServerImportRequest` is `@Deprecated`
     ("use the unified AI resource import request models instead, planned for
     removal in Nacos 3.4.0"). Removed the model, the `import_mcp_servers`
     trait method and its implementations, and the
     `POST /v3/ai/mcp/servers/import` and console `POST .../mcp/import`,
     `POST .../mcp/import/execute` endpoints. Batata's implementation was a
     placeholder, which also violates the "no stubs" rule.

   Follow-up: the unified AI resource import (`AiResourceImportAdminController`)
   is a separate feature and remains unimplemented.

   Scope: this rule applies to the AI/MCP module. Config and naming v1/v2 APIs
   are **not** marked deprecated upstream and remain in scope.

---

## 6. Progress

| Phase | Status | Notes |
|---|---|---|
| 0 — Baseline | **complete** | endpoint diff (`api-mapping.md`) + model diff (`model-mapping.md`) done |
| 1 — Architecture | in progress | 3 upstream tables added (`m20260412_000014`); layers not started |
| 2 — MCP | not started | largest gap |
| 3 — Remaining domains | not started | |
| 4 — Tests & docs | not started | |

---

## 7. Open questions

1. ~~Which upstream commit/tag is the authoritative baseline for this work?~~
   → resolved: `master` @ `1b6309f5086c163a1ff9c0469fabd4494ec09093` (2026-09-24).
2. ~~Should Batata keep its simplified `McpForm` API as a compatibility shim, or
   replace it with the full upstream contract?~~
   → resolved: upstream keeps **both** MCP paths behind `McpCompatibilityMode`
   (`SYNCING` / `LIFECYCLE_MANAGED`). Mirror that instead of deleting the legacy path.
3. ~~Is AI tracing (`service/trace/`) in scope, or deferred?~~
   → resolved: tracing is **JSON-line event logging** (`@since 3.2.1`), no table.
   In scope as logging, not persistence.
4. Does `batata-copilot` have an upstream counterpart, or is it Batata-specific
   (and therefore excluded from parity work)?
