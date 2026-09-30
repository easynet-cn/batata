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
- [x] Record current test coverage for AI in Batata — see
      [§ Test coverage](#test-coverage)

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
- [ ] `runtime/` layer — **not MCP, and blocked.** Upstream's `runtime/` is
      `service/agent/runtime/`, the Agent **RAD** subsystem:
      `AgentRuntimeRegistryService` is backed directly by Naming client
      publications (`ServiceStorage`, `EphemeralClientOperationServiceImpl`),
      with `AgentRuntimePublicationCapacityGate` and
      `RuntimeEndpointRevision` fingerprinting. It therefore depends on RAD and
      ARD (`nacos.ai.rad.capacity.*`, `nacos.ai.ard.enabled`), neither of which
      Batata has, plus Naming ephemeral registrations for liveness. Building it
      standalone would sit on missing foundations; it should follow ARD/RAD,
      not precede them.
- [x] `visibility/` layer via `ai_resource.scope` / `owner` — **done for all
      four domains** (MCP 14, Skill 32, AgentSpec 32, A2A 13 `batata_visibility`
      call sites). MCP is the documented reference case:
      `list_mcp_servers` and `get_mcp_server_detail` now take a `user`
      identity and consult `batata_visibility`; a resource the caller may not
      read is excluded from the list (and from `total_count`) or rejected on
      detail reads. `McpServerIndexData` carries `scope` / `owner`, populated
      on `refresh()`, on create (private) and on `PUT /scope`.

      Two real bugs were found and fixed while implementing this:
      1. `McpServerIndex::search_by_name` returned early from the name filter,
         which **skipped the visibility predicate entirely** whenever a name
         was supplied.
      2. `update_mcp_server_scope` updated the database but not the cached
         index, so a scope change had no effect until the next full refresh.

      Covered by `crates/batata-ai/tests/mcp_visibility.rs` (3 tests on both
      engines). That file is a **separate test binary** on purpose:
      `VisibilityPluginManager` is a process-global singleton and
      `with_visibility` only registers when none exists, so a shared binary
      would leave auth disabled and nothing would be filtered.

      gRPC (`ai_handler.rs`) passes `None` — no end-user identity is available
      there, so visibility is not enforced on that internal path.
- [x] **A2A** visibility — `list_agents` and `get_agent_card` now take a `user`
      identity. A2A's list already queried persistence through
      `AiResourceListFilter`, so the scope/owner predicate is applied **by the
      query** and shrinks `total_count` correctly — unlike MCP it has no
      in-memory index to keep in sync. Covered by
      `crates/batata-ai/tests/a2a_visibility.rs` (2 tests on both engines).

All four AI domains (MCP, A2A, Skill, AgentSpec) now enforce `ai_resource`
visibility. gRPC (`ai_handler.rs`) and the internal agent client pass `None`,
so visibility is **not** enforced on those internal paths — deliberate, since
they carry no end-user identity.
- [x] `trace/` layer — **JSON-line trace events, not a table** (`architecture.md`
      §6). `batata_common::ai_trace` mirrors upstream `AiResourceTraceService`
      (`@since 3.2.1`): 27 `OP_*` constants, `SUCCESS`/`FAILURE`/`SKIPPED`, and
      a JSON line per event logged at INFO under
      `com.alibaba.nacos.ai.resource.trace` so existing ELK/Loki pipelines keep
      working. Blank fields become `-`; blank `version` / `ext` are omitted.

      Wired into all twelve MCP write endpoints (draft create/update/delete,
      submit, publish, force-publish, redraft, online, offline, labels, status
      → `ENABLE`/`DISABLE`, scope). Emission happens in the **console
      handlers**, not inside `batata-ai`, because the operator and client IP
      are only known at the HTTP layer — and `batata-console` does not depend
      on `batata-ai`, hence the module lives in `batata-common`.

      Resource type per domain, taken from upstream (not invented):
      `skill`, `mcp`, `agentspec`, `prompt`, and **`a2a`** — note upstream
      `LegacyA2aOperationService` emits `"a2a"` on trace records even though the
      stored resource type is `agent`. Both constants exist; tracing uses
      `RESOURCE_TYPE_A2A`.

      Wiring status:

      | Domain | Resource type | Traced |
      |---|---|---|
      | MCP | `mcp` | 12/12 write endpoints |
      | Skill | `skill` | 12/12 |
      | AgentSpec | `agentspec` | 12/12 |
      | A2A | `a2a` | register, update, delete |

      Every write endpoint emits one record on success and one on failure
      (error message in `ext`).

      > Not traced: read endpoints (upstream only traces writes), the A2A
      > batch-register endpoint, and gRPC / internal client paths.
- [~] `search/` layer — **projection and scheduling done, consumer pending**.

      Implemented (mirrors `AiResourceIndexMaintenanceService` and
      `AiResourceIndexServiceImpl.rebuildAiResource`):
      - `search/mod.rs`: deterministic projection — one document plus chunks
        (`description`, `capability`, `tag`, `metadata_io`, `metadata_risk`,
        `not_for`, and `mcp_content` for source texts).
        `canonical_text` is the lowercased `type + displayName + chunkType +
        text` join; `chunk_hash` covers the resource key, chunk type and
        canonical text; `source_digest` covers every field that would change
        the projection, so an unchanged rebuild is skipped.
      - `search/task.rs`: task key is SHA-256 of
        `search_index \n namespace \n type \n name`, so re-scheduling updates
        the existing row instead of queueing a duplicate. Statuses
        `pending` / `processing` / `completed`.
      - `search/service.rs`: `schedule()` and `rebuild_mcp_version()`, the
        latter returning `false` when the stored digest already matches.
      - Publishing (and force-publishing) an MCP version now schedules a
        `base_index` task. Scheduling failures are logged, not propagated —
        indexing is asynchronous upstream.

      Covered by `crates/batata-ai/tests/mcp_search_index.rs` (both engines)
      plus 15 unit tests.

      > Deviations: digests use SHA-256 rather than upstream MD5 (they are only
      > compared against themselves, and 64 hex chars still fit `varchar(64)`).
      >
      - `search/consumer.rs`: `consume_once()` polls due tasks (batch 100),
        skips completed and still-leased ones, claims with a 60s lease, runs
        `base_index`, then completes / removes / retries. Retry backoff is
        `min(300, 5 << min(retry_count, 6))` seconds, matching upstream.
        Removal (not retry) is used when the resource is gone.
        `rebuild_latest_mcp` also prunes documents left by superseded
        versions, so only the latest version stays indexed.

      Covered by `crates/batata-ai/tests/mcp_search_index.rs` — 3 tests on
      both engines covering schedule→rebuild→skip, publish→consume→complete
      →skip, and removal for a missing resource — plus 17 unit tests.

      > Known limitation: claiming is **not** atomic. The persistence layer
      > exposes an upsert for tasks but no compare-and-set, so two nodes could
      > claim the same task. The projection is idempotent (keyed by
      > `source_digest`), so the worst case is duplicate work, not a wrong
      > index. A proper `task_claim` with a revision predicate would fix it.
      >
      Wired into server startup: `start_ai_search_index_consumer` in
      `builder/app_builder.rs` spawns a poll loop every
      `DEFAULT_INTERVAL_SECONDS` (5s, matching the upstream default), with
      `MissedTickBehavior::Skip` and errors logged rather than fatal. It is
      skipped when there is no local persistence.

      - `search/query.rs`: keyword search. Mirrors upstream recall → rank →
        paginate, minus the vector channel (with no pgvector, keyword hits are
        the ranking, which is what upstream `recallWithMaxScore` degenerates
        to). `search_chunk_search` uses the upstream CASE scoring: 1.0 for a
        `canonical_text` match, 0.8 for `chunk_text`, 0.4 otherwise, ordered by
        score desc. Hits collapse per resource keeping the best score, ties
        break by name so paging is stable.

        Two dialect details worth remembering: PostgreSQL numbers placeholders
        (`$1`) while MySQL uses `?`, so the SQL builds them per backend; and a
        bare `CASE` yields `DECIMAL` on MySQL, which does not decode into `f64`,
        hence the explicit cast to `DOUBLE` / `DOUBLE PRECISION`.

      Covered by `crates/batata-ai/tests/mcp_search_index.rs` — 4 tests on both
      engines (schedule→rebuild→skip, publish→consume→complete→skip, removal
      for a missing resource, keyword search with ranking and pagination) —
      plus 21 unit tests.

      - **HTTP endpoint**: `GET /v3/console/ai/mcp/search?namespaceId=&query=&pageNo=&pageSize=`.
        Validates that `query` is present and within upstream's 1024-char
        limit, and clamps `pageSize` to upstream's max of 100. Upstream's
        equivalent is `GET /v3/client/ai/resources/search` (`@Since 3.3.0`,
        cursor-based); this console variant is MCP-scoped and page-numbered.

        The hit type (`AiResourceSearchHit`) lives in `batata-common` because
        `batata-console` cannot depend on `batata-ai` — same constraint that
        put `ai_trace` there. It is exposed through the existing
        `McpServerService` trait, so `McpServerRegistry` returns its usual
        explicit "not supported" error.

      Covered by `crates/batata-ai/tests/mcp_search_index.rs` (4 tests) and
      `crates/batata-server/tests/mcp_console_routes.rs` (15 tests), both on
      both engines, plus 21 unit tests.

      > Still missing: the `llm_enhancement` stage and vector/embedding
      > storage. Note the consumer loop runs on **every** node; with a
      > non-atomic claim this is safe (idempotent projection) but means
      > duplicate work across nodes.

## Backend coverage (standalone vs cluster)

**Upstream and Batata differ fundamentally here, and it changes what
"supported" means.**

Nacos standalone runs on **Derby** — an embedded *SQL* database. So every
JDBC-based feature, including the `ai_resource_search_*` tables, works in
standalone. Batata has no embedded SQL engine; its embedded store is RocksDB
(key/value).

Batata's four combinations:

| Storage backend | Topology | Persistence | AI search index |
|---|---|---|---|
| `ExternalDb` | `Standalone` | SQL (shared DB) | ✅ full |
| `ExternalDb` | `Cluster` | SQL (+ RocksDB for some state) | ✅ full |
| `Embedded` | `Standalone` | RocksDB | ✅ full |
| `Embedded` | `Cluster` | RocksDB + Raft | ✅ full |

All three persistence backends (`sql`, `embedded`, `distributed`) implement the
search document / chunk / task operations. The RocksDB column families
(`CF_AI_RESOURCE_SEARCH_DOCUMENT`, `..._CHUNK`, `..._TASK`) already existed in
`batata-consistency`, so no new CFs were needed.

Keyword search is the one operation with two implementations, because RocksDB
has no `LIKE`:

- `sql/ai_resource.rs` — one SQL statement with a `CASE` for scoring.
- `search_util::keyword_hits` — the same scoring rules applied in memory, used
  by both `embedded` and `distributed` so the two paths cannot drift.

    Verified end to end on RocksDB by
    `crates/batata-ai/tests/mcp_search_embedded.rs` (no external database
    needed — it opens RocksDB in a temp dir with just the five AI column
    families): publish → schedule → consume → keyword search.

> Known gaps per mode:
> - **Embedded/Standalone**: verified working, but keyword search scans and
>   matches in memory, so it is O(chunks in namespace) rather than
>   index-backed. Fine at current scale; it will not scale like the SQL path. A
>   term index would be needed if the chunk count grows.
> - **Embedded/Cluster**: the consumer loop runs on every node; this is
>   unverified — there is no test that drives the Raft-backed path.
> - **Embedded/Cluster**: writes go through Raft; the consumer runs on every
>   node and the claim is not atomic, so nodes can duplicate work (safe but
>   wasteful). No per-node leader election for the consumer yet.
> - **Embedding computation: done.** `search::embedding` ports upstream's
>   default `HashingAiResourceEmbeddingService` — model id
>   `nacos-local-hashing-embedding-v1`, 384 dims, CRC32 bucket hashing with
>   parity sign, L2-normalized. It needs no model, no network and no pgvector,
>   so it is unit-tested outright (14 tests). CRC32 is implemented locally
>   (no new dependency) and pinned to the standard check value
>   `crc32("123456789") == 0xCBF43926`.
>
>   **Embedding storage: still not implemented.** Upstream keeps vectors in the
>   PostgreSQL-only, pgvector-backed `ai_resource_search_embedding_pg` table,
>   loaded as an opt-in plugin by the operator. Batata has no pgvector, so the
>   current behaviour — writing the document as `enabled` directly, which is
>   what upstream does when `vectorIndex.available()` is false — is already the
>   upstream-correct one. The embedder therefore has no consumer yet; adding a
>   non-pgvector vector store would be a divergence from upstream and is left
>   undone deliberately.
- [x] Replaced the config-backed storage for **MCP / Skill / AgentSpec / A2A**
      with the `ai_resource*` path. No compatibility mode: Batata is unreleased
      and has no stored data to migrate.
      **Correction:** an earlier claim here that "zero config calls remain in
      `batata-ai`" was wrong. The check grepped
      `publish_config\|config_info::Entity` and missed the trait-level
      `config_find_one` / `config_create_or_update` calls. **Prompt is still
      config-backed** (`service/prompt/mod.rs`, group `nacos-ai-prompt`,
      3 call sites) and is the only remaining domain on the legacy path.
      The legacy config-group/data-id/tag constants in
      `service/constants.rs` were deleted; only the NamingService endpoint
      constants remain (endpoint resolution is still naming-based upstream too).
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
- [x] `force-publish`, `redraft` — service methods
      `force_publish_mcp_server_version` (bypasses the review gate) and
      `redraft_mcp_server_version` (moves a version back to `draft`)
- [x] `online` / `offline` — service methods `online_mcp_server_version`
      and `offline_mcp_server_version`
- [x] Versioning: `versions`, `version` — service methods
      `list_mcp_server_versions` / `get_mcp_server_version`
- [x] `labels`, `scope`, `status` — service methods
      `update_mcp_server_labels` (custom labels replace; the server-managed
      `latest` label is preserved; every label must point at an online
      version), `update_mcp_server_status` (enable/disable) and
      `update_mcp_server_scope` (`PUBLIC` / `PRIVATE`, validated)
- [x] ~~`version/meta`, `biz-tags`~~ — **not MCP endpoints.** Both live on
      `SkillAdminController` / `AgentSpecAdminController` /
      `PromptAdminController` upstream; `McpAdminController` declares exactly
      the fourteen endpoints listed below. Tracked under those domains instead.
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

      Covered by `crates/batata-server/tests/mcp_console_routes.rs` — **13
      route tests**, run on live MySQL and PostgreSQL. Each of the fourteen
      endpoints has a success-path test asserting the state it produces
      (`submit` → `reviewing`, `publish` / `force-publish` → `online`,
      `redraft` → `draft`, `offline` → `offline`, `online` → `online`,
      `labels` keeps `latest`, …), plus 400-rejection tests for a missing
      `mcpName` and an unknown `status`.

      The test builds a real `AppState` (stub cluster manager, `Configuration`
      with console auth disabled — console auth defaults to **enabled**) on top
      of the real database and the real MCP service, so it exercises
      path → handler → service rather than the service alone.

      **MCP is endpoint-complete.** Upstream `McpAdminController` declares
      exactly these fourteen (`McpAdminController.java:92-370`) — there is no
      `biz-tags`, `version/meta`, `version/download` or `upload` on MCP; those
      belong to `SkillAdminController`, `AgentSpecAdminController` and
      `PromptAdminController`.

      Batata additionally keeps its own legacy console endpoints (`list`,
      detail, create, update, delete, `endpoint` add/remove, `stats`,
      `importToolsFromMcp`, `import/validate`). The `endpoint` pair has no
      upstream counterpart.

      > Not covered by tests: `search` (needs the async `search_index` task and
      > the document/chunk index) and the legacy console endpoints above.
- [x] ~~`online` / `offline`~~ — done (see above; also wired on
      `POST /ai/mcp/online` and `POST /ai/mcp/offline`)
- [x] ~~Admin/Client controller split (management vs runtime discovery)~~ —
      **the split already exists in a different shape:**
      - management: the console MCP endpoints (`/v3/console/ai/mcp/*`,
        `SignType::Console`)
      - client discovery: a dedicated **MCP Registry server** on its own port
        (default 9080) implementing the official MCP Registry OpenAPI
        (`GET /v0/servers`, `GET /v0/servers/{id}`) —
        `startup/http.rs:259` — plus the Skills `.well-known` registry.

      Upstream's `McpClientController` (added in 3.3.0) additionally exposes
      `GET` (latest serving version), `POST` (release), `POST /endpoints` and
      `GET /search`. Of those, `/endpoints` plus
      `AiHttpClientLifecycleService` are RAD/runtime-endpoint territory and are
      blocked as described under `runtime/` above; query, release and search
      are already reachable through the admin endpoints, the registry and the
      MCP keyword search. Nothing is missing that is not otherwise blocked.
- [x] Validation service equivalent to `McpServerValidationService` —
      `model::ai::mcp_validation`, wired into `POST /ai/mcp/import/validate`.
      The endpoint previously only parsed the JSON and answered `valid: true`
      with a server count; it now applies upstream's rules: name/protocol/
      description required, protocol in `stdio|sse|streamable|http|dubbo`,
      duplicates inside the batch (keyed on name + version), already-exists in
      the namespace, `stdio` needs `localServerConfig` or `packages`, other
      protocols need `remoteServerConfig`, and a supplied `toolSpec` must carry
      at least one tool. Upstream's `valid` is `invalidCount == 0`, so
      duplicates are counted separately and do **not** invalidate the batch —
      that is reproduced deliberately.
      Existence is resolved before the pure rules run, so the rules are
      unit-tested (16) without a database; 3 route tests cover the endpoint.
- [x] Import **execute** — `POST /ai/mcp/import/execute`. Upstream's
      `McpServerImportService` is `@Deprecated` ("use the unified AI resource
      import flow instead, planned for removal in 3.4.0"), so the rules were
      taken from it but the endpoint is Batata's own. Semantics: refuse the
      batch unless it is valid or `skipInvalid` is set; import only the
      selected (or non-invalid) entries; an existing server is `skipped` with
      `conflictType: existing` unless `overwrite` is requested, in which case
      it is updated; `success` is `failedCount == 0`, so skips do not fail the
      batch. `dubbo` is reported as failed rather than silently mapped, since
      Batata has no matching server type.
      The **external sources** half — `AiResourceImportManager` with the
      `mcp-official` / `skills-sh` / well-known / MCP-registry-protocol
      plugins, outbound fetching and `AiResourceImportSecurityGuard` — is
      **not implemented**; see below.
- [x] ~~Cache invalidation equivalent to `McpServerCacheInvalidateService`~~ —
      **not applicable, and already covered.** Upstream's service is a
      `Subscriber<LocalDataChangeEvent>`: it invalidates `McpServerIndex` when a
      **config** in group `mcp-server-versions` changes. That exists only
      because upstream stores MCP servers as configs. Batata stores them in
      `ai_resource` (zero config calls in `batata-ai`), so there is no such
      event to subscribe to.
      Batata instead refreshes the whole index from persistence every 30s
      (`start_mcp_index_refresh`, `builder/app_builder.rs:1185`), which bounds
      cross-node staleness without needing change events. Implementing the
      upstream subscriber here would be dead code.

### Phase 3 — Remaining domains

- [ ] Agent admin CRUD + lifecycle (mirror `service/agent/`) — **service layer
      done**: `A2aServerOperationService` (which is the `agent` resource type)
      now has `submit` / `publish` / `force_publish` / `redraft` / `online` /
      `offline` / `labels` / `scope`, all delegating to the shared
      `version_lifecycle`, plus `create` / `update` / `delete_agent_draft`.
      Covered by `agent_version_lifecycle` in `tests/a2a_persistence.rs` and
      by 3 route tests in `tests/mcp_console_routes.rs` (both engines).

      **HTTP**: 17 of the 18 upstream admin endpoints are wired at
      `/v3/admin/ai/agents` — `GET` (detail), `/list`, `/versions`,
      `POST`/`PUT`/`DELETE` (create/update/delete), the three `draft` verbs,
      `POST /submit`, `/publish`, `/force-publish`, `/redraft`, `/online`,
      `/offline`, `PUT /labels`, `/scope`.
      Only `GET /runtime-endpoints` is missing: it is RAD (see above).
      `GET /version` is served by `GET` with a `version` parameter, matching
      how `get_agent_card` resolves one.

      Bug found by that test: agent versions never received the server-managed
      `latest` label, because `refresh_version_meta` (which maintains `latest`
      and `online_cnt`) was MCP-only. It has been moved into
      `version_lifecycle::refresh_latest` and is now applied on every
      online/offline transition for **every** resource type, so MCP, agents and
      any future domain stay consistent.
- [ ] `AiCapabilityClientController` equivalent
- [ ] Prompt client contract parity
- [ ] Pipeline parity verification
- [ ] Resource import parity (official registry, `skills.sh`, well-known)
- [ ] Resource search parity
- [ ] Skill download counting

### Phase 4 — Tests and documentation

- [ ] Port upstream AI test cases as Batata integration tests
- [x] Add `docs/compat/nacos/ai/api-mapping.md` (endpoint-level mapping)
- [x] Add `docs/compat/nacos/ai/model-mapping.md` (field-level mapping)
- [x] Update `docs/feature-comparison.md` AI/module rows — added a 13-row AI
      Registry comparison and an AI row to the completeness summary; also
      dropped the incorrect claim that AI integration is Batata-exclusive
      (Nacos has an AI module too).
- [x] `conf/application.yml` has an **AI Module** section covering the keys
      Batata reads (`batata.ai.mcp.registry.*`, `batata.ai.skill.registry.*`,
      `batata.ai.skill.auto_publish_after_review.*`,
      `batata.ai.resource.import.*`, `batata.extension.ai.enabled`,
      `batata.core.auth.default.anonymous.ai.enabled`). AI resource **search**
      has no config knobs — the index runs whenever the tables exist — so the
      upstream `nacos.ai.resource.search.*`, `nacos.ai.ard.*` and
      `nacos.ai.rad.capacity.*` keys have no Batata equivalent and are not
      supported (documented here, not in `conf/application.yml`).

---

## Test coverage

Counts are of test functions, taken from the sources (not from memory).

### Unit (no database)

| Crate | Tests | AI-relevant content |
|---|---|---|
| `batata-ai` | 94 | projection determinism, chunk hashing, task scheduling, **embedding (14)** |
| `batata-common` | 124 | AI models, traits |
| `batata-persistence` | 47 | `ai_resource*` persistence, embedded backend |
| `batata-console` | 24 | MCP console routes |

`cargo test -p <crate> --lib`

### Live database (`#[ignore]`, need `DATABASE_URL`)

| Test binary | Tests | What it pins down |
|---|---|---|
| `batata-server` / `mcp_console_routes` | 15 | all 14 MCP Admin endpoints end to end (path → handler → service → DB) |
| `batata-ai` / `mcp_search_index` | 4 | publish → schedule → consume → keyword search |
| `batata-ai` / `mcp_persistence` | 2 | `ai_resource` round-trip and the full version lifecycle |
| `batata-ai` / `mcp_visibility` | 3 | MCP scope/owner enforcement |
| `batata-ai` / `a2a_visibility` | 2 | A2A scope/owner enforcement |
| `batata-ai` / `a2a_persistence` | 1 | A2A `ai_resource` round-trip |
| `batata-persistence` / `ai_search_persistence` | 1 | document/chunk/task persistence |
| `batata-migration` / `migration_smoke` | 1 | the AI tables exist after `Migrator::up` |

```bash
DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
  cargo test -p batata-ai --test mcp_search_index -- --ignored --test-threads=1
```

Run on **MySQL 8.4** and **PostgreSQL 18**; both are expected to pass.

Two notes that cost real time and are easy to trip over:

- **The Podman containers stop.** They do not come back after a machine
  restart or a `podman machine stop`, because Podman is daemonless and the
  `unless-stopped` policy is not enforced across a VM restart. The symptom is
  a ~30s-per-test pool timeout. The live tests now use a 3s connect timeout and
  panic with the URL plus a hint, so a stopped container costs seconds instead
  of minutes and says what is wrong.
- **Some binaries must stay separate.** `mcp_visibility` and `a2a_visibility`
  are separate test binaries on purpose: `VisibilityPluginManager` is a
  process-global singleton, so sharing a binary would leave auth disabled and
  nothing would be filtered.

### Embedded RocksDB (no external database)

| Test binary | Tests | What it pins down |
|---|---|---|
| `batata-ai` / `mcp_search_embedded` | 1 | the whole search loop on RocksDB: publish → schedule → consume → keyword search |

`cargo test -p batata-ai --test mcp_search_embedded` — runs by default, needs
nothing external.

### Not covered

- Embedded + **Cluster** (Raft) search path — no test drives it.
- Skill / AgentSpec / Prompt live paths (MCP and A2A are the covered domains).
- Legacy console endpoints (`stats`, `importToolsFromMcp`, `import/validate`,
  `endpoint` add/remove).

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
