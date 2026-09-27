# AI API Mapping (Batata vs Nacos)

Endpoint-level comparison of the AI module against upstream Nacos.

- **Upstream baseline**: `nacos` @ `1b6309f5086c163a1ff9c0469fabd4494ec09093` (2026-09-24, master)
- **Upstream path constants**: `nacos/ai/src/main/java/com/alibaba/nacos/ai/constant/Constants.java`
- **Batata mount points**: admin AI is mounted at `/v3/admin/ai` (`crates/batata-server/src/api/v3/admin/route.rs`), client AI at `/v3/client/ai` (`api/v3/client/route.rs`)

Legend: ✅ implemented · ⚠️ partial / differs · ❌ missing · ❓ needs verification

> The Batata column lists the relative routes found in each module. Where a module
> scope differs from the upstream base path, the difference is called out.

---

## 1. MCP

Upstream base: `/v3/admin/ai/mcp` (`McpAdminController`), `/v3/client/ai/mcp` (`McpClientController`)

| Upstream | Method | Batata | Status |
|---|---|---|---|
| `/v3/admin/ai/mcp/list` | GET | `admin/ai/mcp.rs` list | ⚠️ |
| `/v3/admin/ai/mcp/versions` | GET | — | ❌ |
| `/v3/admin/ai/mcp/version` | GET | — | ❌ |
| `/v3/admin/ai/mcp/draft` | POST / PUT / DELETE | — | ❌ |
| `/v3/admin/ai/mcp/submit` | POST | — | ❌ |
| `/v3/admin/ai/mcp/publish` | POST | — | ❌ |
| `/v3/admin/ai/mcp/force-publish` | POST | — | ❌ |
| `/v3/admin/ai/mcp/redraft` | POST | — | ❌ |
| `/v3/admin/ai/mcp/online` | POST | — | ❌ |
| `/v3/admin/ai/mcp/offline` | POST | — | ❌ |
| `/v3/admin/ai/mcp/labels` | PUT | — | ❌ |
| `/v3/admin/ai/mcp/status` | PUT | — | ❌ |
| `/v3/admin/ai/mcp/scope` | PUT | — | ❌ |
| `/v3/client/ai/mcp/endpoints` | POST / DELETE | — | ❌ |
| `/v3/client/ai/mcp/endpoints/heartbeat` | PUT | — | ❌ |
| `/v3/client/ai/mcp/search` | GET | — | ❌ |

Batata MCP today:

- `crates/batata-server/src/api/v3/admin/ai/mcp.rs` — a **simplified form-based** API
  built on `McpForm` (`namespaceId`, `mcpName`, `mcpId`, `version`,
  `serverSpecification`, `toolSpecification`, `endpointSpecification`). No draft,
  publish or versioning lifecycle.
- `crates/batata-ai/src/registry/mcp.rs` — registry-style endpoints under a
  different path scheme: `/v3/ai/mcp/servers` (GET/POST),
  `/v3/ai/mcp/servers/{namespace}/{name}` (GET/PUT/DELETE),
  `/v3/ai/mcp/servers/import`, `/v3/ai/mcp/stats`

**Assessment**: MCP is the largest gap. Two open decisions:

1. Whether the `/v3/ai/mcp/servers` path scheme should be replaced by the upstream
   `/v3/admin/ai/mcp` + `/v3/client/ai/mcp` split, or kept as an additional
   compatibility surface.
2. Whether the simplified `McpForm` API should be retired in favour of the full
   upstream contract.

---

## 2. Skill

Upstream base: `/v3/admin/ai/skills` (`SkillAdminController`), `/v3/client/ai/skills` (`SkillClientController`)

| Upstream | Method | Batata (`api/skill.rs`) | Status |
|---|---|---|---|
| `/version` | GET | `version` | ✅ |
| `/version/download` | GET | `version/download` | ✅ |
| `/list` | GET | `list` | ✅ |
| `/upload` | POST | `upload` | ✅ |
| `/upload/precheck` | POST | — | ❌ |
| `/upload/batch` | POST | — | ❌ |
| `/draft` | POST / PUT / DELETE | `draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `submit` | ✅ |
| `/publish` | POST | `publish` | ✅ |
| `/force-publish` | POST | — | ❌ |
| `/redraft` | POST | — | ❌ |
| `/labels` | PUT | `labels` | ✅ |
| `/biz-tags` | PUT | `biz-tags` | ✅ |
| `/online` | POST | `online` | ✅ |
| `/scope` | PUT | `scope` | ✅ |
| `/offline` | POST | `offline` | ✅ |
| `/v3/client/ai/skills/search` | GET | `search` | ✅ |

Also present in Batata: `crates/batata-ai/src/api/skills_registry.rs` exposing the
well-known registry (`.well-known/skills/index.json`, `SKILL.md`,
`agent-skills/...`) and `/{namespace_id}/api/search`.

**Assessment**: closest to parity. Missing `upload/precheck`, `upload/batch`,
`force-publish`, `redraft`.

---

## 3. AgentSpec

Upstream base: `/v3/admin/ai/agentspecs` (`AgentSpecAdminController`), `/v3/client/ai/agentspecs` (`AgentSpecClientController`)

| Upstream | Method | Batata (`api/agentspec.rs`) | Status |
|---|---|---|---|
| `/version` | GET | `version` | ✅ |
| `/version/meta` | GET | — | ❌ |
| `/list` | GET | `list` | ✅ |
| `/upload` | POST | `upload` | ✅ |
| `/draft` | POST / PUT / DELETE | `draft` (POST/PUT/DELETE) | ✅ |
| `/submit` | POST | `submit` | ✅ |
| `/publish` | POST | `publish` | ✅ |
| `/force-publish` | POST | — | ❌ |
| `/redraft` | POST | — | ❌ |
| `/labels` | PUT | `labels` | ✅ |
| `/biz-tags` | PUT | `biz-tags` | ✅ |
| `/online` | POST | `online` | ✅ |
| `/scope` | PUT | `scope` | ✅ |
| `/offline` | POST | `offline` | ✅ |
| `/v3/client/ai/agentspecs/search` | GET | `search` | ✅ |

**Assessment**: close to parity. Missing `version/meta`, `force-publish`, `redraft`.

---

## 4. Agent

Upstream base: `/v3/admin/ai/agents` (`AgentAdminController`), `/v3/client/ai/agents` (`AgentClientController`)

| Upstream | Method | Batata | Status |
|---|---|---|---|
| `/list` | GET | — | ❌ |
| `/versions` | GET | — | ❌ |
| `/version` | GET | — | ❌ |
| `/runtime-endpoints` | GET | — | ❌ |
| `/draft` | POST / PUT / DELETE | — | ❌ |
| `/submit` | POST | — | ❌ |
| `/publish` | POST | — | ❌ |
| `/force-publish` | POST | — | ❌ |
| `/redraft` | POST | — | ❌ |
| `/online` | POST | — | ❌ |
| `/offline` | POST | — | ❌ |
| `/labels` | PUT | — | ❌ |
| `/scope` | PUT | — | ❌ |
| `/v3/client/ai/agents/search` | GET | `agent_client.rs` `search` | ✅ |
| `/v3/client/ai/agents/watch` | POST | — | ❌ |
| `/v3/client/ai/agents/endpoints` | POST / DELETE | `endpoints` (POST/DELETE) | ✅ |
| `/v3/client/ai/agents/endpoints/heartbeat` | PUT | `endpoints/heartbeat` | ✅ |

**Assessment**: Batata covers only the **client** side. The whole admin lifecycle
(upstream `service/agent/`, ~50 files) is missing.

---

## 5. Prompt

Upstream base: `/v3/admin/ai/prompt` (`PromptAdminController`), `/v3/client/ai/prompt` (`PromptClientController`)

| Upstream | Method | Batata (`api/prompt.rs`) | Status |
|---|---|---|---|
| `/list` | GET | `list` | ✅ |
| `/versions` | GET | `versions` | ✅ |
| `/governance` | GET | — | ❌ |
| `/version` | GET | — | ❌ |
| `/version/download` | GET | — | ❌ |
| `/draft` | POST / PUT / DELETE | — | ❌ |
| `/submit` | POST | — | ❌ |
| `/publish` | POST | — | ❌ |
| `/force-publish` | POST | — | ❌ |
| `/redraft` | POST | — | ❌ |
| `/online` | POST | — | ❌ |
| `/offline` | POST | — | ❌ |
| `/labels` | PUT | — | ❌ |
| `/description` | PUT | — | ❌ |
| `/biz-tags` | PUT | — | ❌ |
| `/metadata` | GET / PUT | `metadata` (GET/PUT) | ✅ |
| `/detail` | GET | `detail` | ✅ |
| `/label` | PUT / DELETE | `label` (PUT/DELETE) | ✅ |
| `/v3/client/ai/prompt/search` | GET | — | ❌ |

**Assessment**: partial. Batata has metadata/label/detail but **no draft-publish
lifecycle**, which upstream has for prompts as well.

---

## 6. Pipeline

Upstream base: `/v3/admin/ai/pipelines` (`PipelineAdminController`)

| Upstream | Method | Batata (`api/pipeline.rs`) | Status |
|---|---|---|---|
| `/{pipelineId}` | GET | `/{pipeline_id}` | ✅ |
| — | GET | `""` (list) | ➕ extra |

**Assessment**: parity for the single upstream endpoint; Batata additionally
exposes a list endpoint.

---

## 7. AI resource import

Upstream base: `/v3/admin/ai/import` (`AiResourceImportAdminController`)

| Upstream | Method | Batata | Status |
|---|---|---|---|
| `/sources` | GET | `batata-console/src/v3/ai_import.rs` | ❓ |
| `/search` | POST | — | ❓ |
| `/validate` | POST | — | ❓ |
| `/execute` | POST | — | ❓ |

**Assessment**: Batata routes an AI import module, but the endpoint contract has
not yet been compared in detail. Needs verification.

---

## 8. Cross-cutting capabilities

| Upstream | Batata | Status |
|---|---|---|
| `AiResourceSearchClientController` (unified resource search) | per-domain `search` endpoints | ❓ |
| `AiCapabilityClientController` (capability discovery) | — | ❌ |
| `service/trace/` (AI call tracing) | — | ❌ |
| `service/runtime/` | — | ❌ |
| `service/visibility/` + `VisibilityHelper` | — | ❌ |
| `service/repository/` | — | ❌ |
| `service/search/` (41 files) | ad-hoc per domain | ❌ |
| `SkillDownloadCountManager` | — | ❌ |
| `McpServerValidationService` | — | ❌ |
| `McpServerImportService` / `McpLegacyImportAdapter` | partial (`registry/mcp.rs` import) | ⚠️ |
| `McpServerCacheInvalidateService` | — | ❌ |
| `McpEndpointOperationService` | `endpoint_service.rs` | ⚠️ |

---

## 9. Summary counts

| Domain | Upstream endpoints | Batata covered | Gap |
|---|---|---|---|
| MCP | 16 | ~2 (different scheme) | **large** |
| Skill | 17 | 13 | small |
| AgentSpec | 15 | 12 | small |
| Agent | 17 | 4 (client only) | **large** |
| Prompt | 20 | 6 | **medium** |
| Pipeline | 1 | 1 (+1 extra) | none |
| Resource import | 4 | ❓ | unknown |
| Cross-cutting | 12 | ~2 | **large** |

---

## 10. Next

Field-level model comparison is tracked separately in `model-mapping.md` (to be
produced). Implementation phases are tracked in `README.md`.
