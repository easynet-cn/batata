# AI Data Model Mapping (Batata vs Nacos)

**Source of truth: the upstream Nacos SQL DDL — not inference.**

- Upstream baseline: `nacos` @ `1b6309f5086c163a1ff9c0469fabd4494ec09093` (master, 2026-09-24)
- MySQL DDL: `plugin-default-impl/.../nacos-datasource-plugin-mysql/src/main/resources/META-INF/mysql-schema.sql:198-321`
- PostgreSQL DDL: `plugin-default-impl/.../nacos-datasource-plugin-postgresql/src/main/resources/META-INF/pg-schema.sql:456-…`
- Optional vector DDL: `plugin-default-impl/nacos-default-ai-vector-plugin/src/main/resources/META-INF/pg-ai-vector-schema.sql:26-57`

Both MySQL and PostgreSQL schemas define the **same five** AI tables.

---

## 1. Upstream AI table inventory

| # | Table | Purpose (from DDL comment) | Batata |
|---|---|---|---|
| 1 | `ai_resource` | AI资源元数据表 (resource metadata) | ✅ exists |
| 2 | `ai_resource_version` | AI资源版本表 (versioned storage) | ✅ exists |
| 3 | `ai_resource_search_document` | AI资源检索文档表 (search document) | ❌ **missing** |
| 4 | `ai_resource_search_chunk` | AI资源检索分片表 (search chunk) | ❌ **missing** |
| 5 | `ai_resource_task` | AI资源持久化异步任务表 (async task queue) | ❌ **missing** |
| — | `ai_resource_search_embedding_pg` | pgvector embeddings (optional, PostgreSQL only) | ❌ missing, optional |

---

## 2. Full upstream DDL

### 2.1 `ai_resource`

| Column | Type | Default | Comment |
|---|---|---|---|
| `id` | bigint | auto increment, PK | id |
| `gmt_create` | datetime | CURRENT_TIMESTAMP | 创建时间 |
| `gmt_modified` | datetime | CURRENT_TIMESTAMP | 修改时间 |
| `name` | varchar(256) | NOT NULL | 资源名称 |
| `type` | varchar(32) | NOT NULL | 资源类型 |
| `c_desc` | varchar(2048) | NULL | 资源描述 |
| `status` | varchar(32) | NULL | 资源状态 |
| `namespace_id` | varchar(128) | `''` | 命名空间ID |
| `biz_tags` | varchar(1024) | NULL | 业务标签 |
| `ext` | longtext | NULL | 扩展信息(JSON) |
| `c_from` | varchar(256) | `local` | 来源标识(导入/同步来源) |
| `version_info` | longtext | NULL | 版本信息(JSON) |
| `meta_version` | bigint | `1` | 元数据版本(乐观锁) |
| `scope` | varchar(16) | `PRIVATE` | 可见性: PUBLIC/PRIVATE |
| `owner` | varchar(128) | `''` | 创建者用户名 |
| `download_count` | bigint | `0` | 下载次数 |

Indexes: `uk_ai_resource_ns_name_type` UNIQUE (`namespace_id`,`name`,`type`,`c_from`),
`idx_ai_resource_name`, `idx_ai_resource_type`, `idx_ai_resource_gmt_modified`.

**Batata**: identical — `crates/batata-migration/src/m20260412_000012_create_ai_resource.rs`.

### 2.2 `ai_resource_version`

| Column | Type | Default | Comment |
|---|---|---|---|
| `id` | bigint | PK | id |
| `gmt_create` / `gmt_modified` | datetime | CURRENT_TIMESTAMP | |
| `type` | varchar(32) | NOT NULL | 资源类型 |
| `author` | varchar(128) | NULL | 作者 |
| `name` | varchar(256) | NOT NULL | 资源名称 |
| `c_desc` | varchar(2048) | NULL | 版本描述 |
| `status` | varchar(32) | NOT NULL | 版本状态 |
| `version` | varchar(64) | NOT NULL | 版本号 |
| `namespace_id` | varchar(128) | `''` | 命名空间ID |
| `storage` | longtext | NULL | 存储信息(JSON) |
| `publish_pipeline_info` | longtext | NULL | 发布流水线信息(JSON) |
| `download_count` | bigint | `0` | 下载次数 |

Indexes: `uk_ai_resource_ver_ns_name_type_ver` UNIQUE
(`namespace_id`,`name`,`type`,`version`), `idx_ai_resource_ver_name`,
`idx_ai_resource_ver_status`, `idx_ai_resource_ver_gmt_modified`.

**Batata**: identical — `m20260412_000013_create_ai_resource_version.rs`.

### 2.3 `ai_resource_search_document` ❌ missing in Batata

| Column | Type | Default | Comment |
|---|---|---|---|
| `id` | bigint | PK | id |
| `gmt_create` / `gmt_modified` | datetime | CURRENT_TIMESTAMP | |
| `namespace_id` | varchar(128) | `''` | 命名空间ID |
| `resource_type` | varchar(32) | NOT NULL | 资源类型 |
| `resource_name` | varchar(256) | NOT NULL | 资源名称 |
| `resource_version` | varchar(64) | NOT NULL | 资源版本 |
| `display_name` | varchar(256) | NOT NULL | 展示名称 |
| `c_desc` | varchar(2048) | NULL | 描述 |
| `tags` | longtext | NULL | 标签(JSON) |
| `capabilities` | longtext | NULL | 能力(JSON) |
| `representative_queries` | longtext | NULL | 代表性查询(JSON) |
| `metadata` | longtext | NULL | 元数据(JSON) |
| `source_digest` | varchar(64) | NOT NULL | 来源摘要 |
| `status` | varchar(32) | NOT NULL | 状态 |
| `generate_mode` | varchar(32) | NOT NULL | 生成模式 |

Indexes: `uk_search_document_resource_version` UNIQUE
(`namespace_id`,`resource_type`,`resource_name`,`resource_version`),
`idx_search_document_type_status` (`namespace_id`,`resource_type`,`status`,`resource_name`,`id`).

### 2.4 `ai_resource_search_chunk` ❌ missing in Batata

| Column | Type | Default | Comment |
|---|---|---|---|
| `id` | bigint | PK | id |
| `gmt_create` / `gmt_modified` | datetime | CURRENT_TIMESTAMP | |
| `document_id` | bigint | NOT NULL | 检索文档ID |
| `namespace_id` | varchar(128) | `''` | 命名空间ID |
| `resource_type` | varchar(32) | NOT NULL | 资源类型 |
| `resource_name` | varchar(256) | NOT NULL | 资源名称 |
| `resource_version` | varchar(64) | NOT NULL | 资源版本 |
| `chunk_type` | varchar(64) | NOT NULL | 分片类型 |
| `chunk_text` | longtext | NOT NULL | 分片文本 |
| `canonical_text` | longtext | NOT NULL | 规范化文本 |
| `language` | varchar(16) | NULL | 语言 |
| `chunk_hash` | varchar(64) | NOT NULL | 分片摘要 |
| `metadata` | longtext | NULL | 元数据(JSON) |
| `status` | varchar(32) | NOT NULL | 状态 |

Indexes: `idx_search_chunk_document` (`document_id`), `idx_search_chunk_hash`
(`chunk_hash`), `idx_search_chunk_resource`
(`namespace_id`,`resource_type`,`resource_name`,`resource_version`),
`idx_search_chunk_type_status` (`namespace_id`,`resource_type`,`status`).

### 2.5 `ai_resource_task` ❌ missing in Batata

| Column | Type | Default | Comment |
|---|---|---|---|
| `task_key` | varchar(64) | PK | 任务唯一键 |
| `namespace_id` | varchar(128) | `''` | 命名空间ID |
| `task_type` | varchar(64) | NOT NULL | 任务类型 |
| `task_stage` | varchar(32) | NOT NULL | 任务阶段 |
| `status` | varchar(16) | NOT NULL | 任务状态 |
| `task_payload` | text | NOT NULL | 任务输入，JSON文本 |
| `task_result` | text | NULL | 任务结果，JSON文本 |
| `retry_count` | int | `0` | 当前阶段重试次数 |
| `revision` | bigint | `1` | 任务修订号 |
| `lease_token` | bigint | `0` | 租约令牌 |
| `next_execute_at` | bigint | NOT NULL | 最早执行时间，Unix Epoch毫秒 |
| `lease_expire_at` | bigint | NULL | 租约到期时间，Unix Epoch毫秒 |
| `last_error` | varchar(2000) | NULL | 最近错误 |
| `gmt_create` / `gmt_modified` | datetime | CURRENT_TIMESTAMP | |

Indexes: `idx_ai_resource_task_due` (`task_type`,`status`,`next_execute_at`),
`idx_ai_resource_task_lease` (`task_type`,`status`,`lease_expire_at`).

### 2.6 `ai_resource_search_embedding_pg` (optional, PostgreSQL + pgvector)

Loaded from `pg-ai-vector-schema.sql`; requires `CREATE EXTENSION IF NOT EXISTS vector`.

| Column | Type |
|---|---|
| `id` | bigserial PK |
| `gmt_create` / `gmt_modified` | timestamp(6) |
| `namespace_id` | varchar(128) |
| `document_id` | bigint |
| `chunk_id` | bigint |
| `resource_type` / `resource_name` / `resource_version` | varchar(32/256/64) |
| `embedding_model` | varchar(128) |
| `embedding_dimension` | integer |
| `embedding` | **vector** |

Indexes: `idx_search_embedding_pg_chunk` (`chunk_id`),
`idx_search_embedding_pg_model` (`namespace_id`,`embedding_model`,`embedding_dimension`,`resource_type`),
`idx_search_embedding_pg_resource` (`namespace_id`,`resource_type`,`resource_name`,`resource_version`).

---

## 3. What the schema reveals about upstream architecture

The DDL — not assumption — shows upstream AI search is **not** per-domain ad-hoc
querying. It is:

1. **A document/chunk index**: every resource version produces one
   `ai_resource_search_document` (uniquely keyed by namespace+type+name+version),
   which is split into `ai_resource_search_chunk` rows. `source_digest` on the
   document and `chunk_hash` on each chunk imply **content-hash driven
   reindexing** (skip rebuild when unchanged).
2. **Asynchronous, lease-based processing**: `ai_resource_task` is a classic
   task queue (`next_execute_at` scheduling, `lease_token` / `lease_expire_at`
   for distributed ownership, `retry_count` + `revision` for retry/optimistic
   concurrency, `task_stage` for multi-stage pipelines such as
   `publish_pipeline_info`). This backs index building and publish pipelines.
3. **Optional vector search**: embeddings live in a separate pgvector table keyed
   by `(namespace_id, embedding_model, embedding_dimension, resource_type)` and
   tied to `chunk_id` — so semantic search is an *add-on* over the same
   document/chunk model, not a separate design.

**Consequence for Batata**: the `search/` layer cannot be "unify queries over
`ai_resource` indexes" (what I previously proposed from inference). It must be
built on dedicated `ai_resource_search_document` + `ai_resource_search_chunk`
tables driven by `ai_resource_task`.

---

## 4. Batata's additional legacy path

Separately, `crates/batata-ai/src/service/constants.rs` shows Batata also keeps
AI data in the **config table** (groups `mcp-server-versions`, `mcp-server`,
`mcp-tools`, `agent`, `agent-version`) with endpoints in NamingService metadata
(`__nacos.ai.mcp.service__`). Upstream Nacos used this approach in earlier
versions and **replaced it with the `ai_resource*` tables in 3.2.0**. Batata has
the new tables but appears to still use the config-backed path in
`mcp_index.rs`, `mcp_service.rs`, `registry/mcp.rs`.

---

## 5. Gaps and actions (DDL-grounded)

| # | Gap | Action | Phase |
|---|---|---|---|
| 1 | ~~Missing `ai_resource_search_document`~~ | **done** — migration `m20260412_000014`, verified on MySQL 8.4 + PostgreSQL 18 | 1 |
| 2 | ~~Missing `ai_resource_search_chunk`~~ | **done** — same migration | 1 |
| 3 | ~~Missing `ai_resource_task`~~ | **done** — same migration | 1 |
| 4 | `ai_resource_search_embedding_pg` | Optional — needs pgvector; decide scope | later |
| 5 | Legacy config-backed path still in use | **partial** — MCP, Skill, AgentSpec and A2A are on `ai_resource*`; **Prompt is not** (`service/prompt/mod.rs` still reads/writes configs in group `nacos-ai-prompt`, 3 call sites). No compatibility mode (Batata is unreleased). Legacy constants deleted from `service/constants.rs`.<br>⚠️ An earlier note here claimed "zero config calls remain in `batata-ai`"; that was wrong — the check grepped `publish_config\|config_info::Entity` and missed the `config_find_one` / `config_create_or_update` trait calls. | 1 |
| 6 | No `repository/` abstraction over the 5 tables | **partial** — `AiResourceRepository` wraps `ai_resource` / `ai_resource_version` (find, insert, list, CAS update). The three search tables are still reached through `PersistenceService` directly from `search/service.rs`; folding them in is cosmetic, not behavioural | 1 |
| 7 | ~~`visibility/` not enforced from `scope`/`owner`~~ | **done** — all four domains consult `batata_visibility` (MCP 14, Skill 32, AgentSpec 32, A2A 13 call sites). See `docs/compat/nacos/ai/README.md` § Phase 1 for the two real bugs this uncovered | 1 |
| 8 | ~~`search/` built on document + chunk + task~~ | **done** — projection, scheduling, consumer and keyword query. Verified on ExternalDb (MySQL 8.4 + PostgreSQL 18, standalone and cluster) and on embedded RocksDB | 1 |
| 9 | `trace/` | **Not a table** — emit JSON-line trace events (see §6) | 2 |

---

## 6. Semantics resolved from upstream Java

Read from source — not inferred.

### Resource `type` (`ai_resource.type`)

`mcp` · `agent` · `skill` · `prompt` · `agentspec`

Sources: `constant/AiResourceConstants.java:29-33` (skill/prompt/mcp),
`constant/Constants.java:207,313` (agent, agentspec). There is no `pipeline`
resource type — pipelines use a separate table.

### Meta status (`ai_resource.status`)

`enable` / `disable` — `AiResourceConstants.java:41,46`

### Version status (`ai_resource_version.status`)

`draft` → `reviewing` → `reviewed` → `online` / `offline`
(`AiResourceConstants.java:51-71`)

This is the draft/publish lifecycle the version table implements.

### Search document status / generate mode

- `status`: `enabled` / `pending`
- `generate_mode`: `auto`

Source: `service/search/AiResourceSearchConstants.java:26-30`

### `chunk_type` (16 values)

`description`, `capability`, `representative_query`, `tag`, `metadata_io`,
`metadata_risk`, `not_for`, `skill_content`, `prompt_content`, `mcp_content`,
`agent_content`, `agentspec_content`, `ai_summary`, `search_intent`, `search_term`

Source: `AiResourceSearchConstants.java:32-60`

### Task (`ai_resource_task`)

- `task_type`: `search_index`
- `task_stage`: `base_index` → `llm_enhancement`

Source: `model/search/AiResourceIndexTask.java:26-30`

### Who writes the search document — **asynchronously**

`config/AiResourceIndexTaskConsumer` is a Spring
`ApplicationListener<ApplicationReadyEvent>` that polls due tasks from
`ai_resource_task` and drives index convergence:

- Poll interval: `nacos.ai.resource.search.index.consumer.interval-seconds`,
  default **5s**; batch size **100**
- Lease **60s**, renewed every **20s** (distributed ownership via `lease_token`)
- Two stages:
  1. `base_index` — builds `ai_resource_search_document` + `ai_resource_search_chunk`
  2. `llm_enhancement` — LLM enrichment (representative queries, `ai_summary`, embeddings)
- Retry bounds: **300s** for base index, **1800s** for enhancement
- Gated by `@ConditionalOnAiResourceSearchEnabled` (feature flag)

So publishing a resource enqueues a task; the search index converges shortly
after. It is **not** written synchronously on publish.

### Trace is **not** a table

`service/trace/AiResourceTraceService` (`@since 3.2.1`) publishes
`AiResourceTraceEvent`, whose default subscriber writes **JSON line logs** for
ELK/Loki. Sample record:

```json
{"timestamp":"2026-03-30T10:15:30Z","operator":"admin","resource_type":"skill",
 "resource_id":"my-skill","version":"v1.0","operation":"PUBLISH",
 "status":"SUCCESS","ip":"192.168.1.1"}
```

Operations: `CREATE_DRAFT`, `UPDATE_DRAFT`, `DELETE_DRAFT`, `UPLOAD`,
`SUBMIT_REVIEW`, `REVIEW_APPROVED`, `REVIEW_REJECTED`, `REVIEW_FORCE_SKIP`,
`REDRAFT`, `PUBLISH`, …

**Implication**: no trace table should be created; implement as event + JSON
line logging.

---

## 7. Schema change policy

Batata is not yet released: schema changes are applied by **editing the existing
migration files** in `crates/batata-migration/src/m20260412_0000NN_*.rs` rather
than adding new migration versions. All definitions must work on both **MySQL and
PostgreSQL** — use the `column_helper` helpers (e.g.
`long_text_null(.., manager.get_database_backend())`) instead of engine-specific
types. Upstream DDL above is already dual-dialect, so mirror it directly.

**Verified on live engines.** `m20260412_000014` was applied to MySQL 8.4 and
PostgreSQL 18 through `crates/batata-migration/tests/migration_smoke.rs`; all
five AI tables and their indexes are created on both. Two known, accepted
differences from the upstream DDL:

- PostgreSQL renders auto-increment primary keys as
  `bigint generated by default as identity` rather than `bigserial`
  (functionally equivalent).
- Batata tables inherit the server default collation, whereas upstream pins
  `utf8mb4_bin`. This is pre-existing across all Batata tables, not specific to
  the AI schema.
