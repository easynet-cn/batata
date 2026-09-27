# Upstream Nacos AI Module — Architecture and Processing Model

Derived by reading the upstream source, not inferred.

- Baseline: `nacos` @ `1b6309f5086c163a1ff9c0469fabd4494ec09093`
- Root: `ai/src/main/java/com/alibaba/nacos/ai/`

---

## 1. Package map

| Package | Files | Role |
|---|---|---|
| `controller/` | 15 | Admin + client controllers per domain (Mcp/Agent/AgentSpec/Skill/Prompt/A2A/Pipeline/Import/Search) |
| `form/` | 91 | Request forms per domain (`mcp/`, `agent/`, `skills/`, `prompt/`, `agentspecs/`, `a2a/`, `importer/`, `search/`) |
| `model/` | 18 | `AiResource`, `AiResourceVersion`, per-domain + `search/` models |
| `service/` | 199 | Domain services, see §2 |
| `service/repository/` | 8 | Persistence over `ai_resource*` (JDBC + Embedded) |
| `config/` | 19 | `AiResourceIndexTaskConsumer`, data bootstrap/migration tasks |
| `storage/` | — | `NacosConfigAiResourceStorage` — the **legacy config-backed** store |
| `index/` | — | `MemoryMcpCacheIndex` — in-memory MCP cache |
| `pipeline/` | — | Publish pipelines |
| `importer/`, `plugin/`, `remote/`, `web/`, `auth/`, `event/`, `enums/`, `constant/`, `param/`, `utils/` | — | supporting |

---

## 2. `service/` sub-packages

`a2a/` (41) · `agent/` (50) · `mcp/` (18) · `search/` (41) · `agentspecs/` ·
`skills/` · `prompt/` · `pipeline/` · `repository/` · `resource/` · `runtime/` ·
`trace/` · `visibility/`

The five cross-cutting layers are `repository`, `visibility`, `runtime`,
`search`, `trace`.

---

## 3. MCP: two paths and a compatibility mode

This is the single most important structural fact for Batata.

| Path | Implementation | Store |
|---|---|---|
| **Legacy** | `LegacyMcpOperationService`, `storage/NacosConfigAiResourceStorage` | config table (namespace/tenant) |
| **New** | `McpOperationService`, `McpLifecycleOperationService`, `service/mcp/storage/*` | `ai_resource` + `ai_resource_version` |

- `McpCompatibilityMode` = `SYNCING` | `LIFECYCLE_MANAGED`
  (`service/mcp/McpCompatibilityMode.java:24-29`)
- `McpCompatibilityModeResolver` selects the authority per operation
- `McpCompatibilityOperationService` bridges the two
- `McpHistoricalResourceReconciler` reconciles historical (legacy) resources

`service/mcp/storage/` holds the new-path storage helpers:
`McpVersionStorageService`, `McpVersionStorageKeyComposer`,
`McpVersionStorageDescriptorSerializer`, `McpServingManifestStorage`,
`McpVersionStorageContents`, `McpResourceExtSerializer`.

**Batata today implements the legacy path** (config groups `mcp-server-versions`,
`mcp-server`, `mcp-tools`; NamingService metadata for endpoints). Upstream did
**not** delete the legacy path when introducing `ai_resource*` — it runs both
behind a compatibility mode.

> Correction: an earlier proposal to "consolidate and delete the config-backed
> path" contradicts upstream. Keep it; add the new path behind a mode.

---

## 4. Persistence layer

`service/repository/`:

- `AiResourcePersistService` / `...Impl` — metadata CRUD
- `AiResourceVersionPersistService` / `...Impl` — version CRUD
- `EmbeddedAiResourcePersistServiceImpl` / `EmbeddedAiResourceVersionPersistServiceImpl`
  — RocksDB variants for standalone mode
- `AiResourceRowMappers`, `QueryCondition`

API surface (from `AiResourcePersistService`):

```java
long  insert(AiResource resource);
AiResource find(String namespaceId, String name, String type);
Page<AiResource> list(QueryCondition condition, int pageNo, int pageSize);
// + update with optimistic lock on meta_version
```

`QueryCondition` carries `namespaceId`, `type`, `nameLike`, `bizTagsLike`,
`orderBy`. `generateLikeArgument` converts the Nacos `*` wildcard to SQL `%`
and escapes `\` and `_` (needed for `ESCAPE '\'` clauses).

`service/resource/` holds `AiResourceManager`, `PublishPipelineInfo`,
`ResourceVersionInfo`, and cluster change propagation
(`AiResourceChangeNotifier`, `DefaultAiResourceClusterChangePublisher`).

---

## 5. Search layer — asynchronous, two stages

Trigger: publishing/versioning enqueues a task in `ai_resource_task`
(`task_type = search_index`, `task_stage = base_index`).

`config/AiResourceIndexTaskConsumer` (Spring
`ApplicationListener<ApplicationReadyEvent>`, gated by
`@ConditionalOnAiResourceSearchEnabled`):

- Poll: default **5s** (`nacos.ai.resource.search.index.consumer.interval-seconds`),
  batch **100**
- Lease **60s**, renewed every **20s** via `lease_token`
- Stage 1 `base_index` → write `ai_resource_search_document` +
  `ai_resource_search_chunk`
- Stage 2 `llm_enhancement` → LLM enrichment (representative queries,
  `ai_summary`, embeddings)
- Retry bounds: **300s** base, **1800s** enhancement
- Concurrency: `enhancementExecutor` + `Semaphore(enhancementConcurrency)`

Per-resource-type builders/handlers: `AiResourceSearchDocumentBuilder`,
`AgentSearchIndexProjector`, `AgentSpecSearchIndexProjector`,
`AgentAiResourceSearchTypeHandler`, `AgentSpecAiResourceSearchTypeHandler`.
Storage: `JdbcAiResourceSearchRepository`, `JdbcAiResourceIndexTaskRepository`.
Querying: `AiResourceSearchService` (has `PredicateOperator`, `Sort`).

---

## 6. Trace layer — logging, not persistence

`service/trace/AiResourceTraceService` (`@since 3.2.1`) publishes
`AiResourceTraceEvent`; the default subscriber emits **JSON line logs** for
ELK/Loki. No table. See `model-mapping.md` §6 for operation codes.

---

## 7. Visibility and runtime

- `service/visibility/` + `VisibilityHelper`; enforced through
  `ai_resource.scope` (`PUBLIC`/`PRIVATE`) and `ai_resource.owner`
- `service/runtime/` — endpoint resolution for MCP/A2A serving

---

## 8. Implication for Batata

1. Batata = legacy path. Target = new path. Batata is **unreleased**, so there is
   no stored data to migrate: implement the `ai_resource*` path directly and
   replace the config-backed code domain by domain. **No compatibility mode is
   needed** — upstream keeps `LegacyMcpOperationService` and
   `McpCompatibilityMode` only because it has released users to support.
2. Build `repository/` over `ai_resource` + `ai_resource_version`, with both SQL
   and embedded (RocksDB) implementations.
3. `search/` must be **task-driven and asynchronous** (poll + lease + two
   stages), not synchronous query-time logic.
4. `trace/` is JSON-line logging — no table.
5. Version lifecycle is the core state machine:
   `draft → reviewing → reviewed → online | offline`, with meta status
   `enable | disable`.
