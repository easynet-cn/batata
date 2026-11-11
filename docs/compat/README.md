# Batata Compatibility Tracking

This directory uses **upstream protocol feature inventories** as a single source of truth to systematically track batata's feature implementation and test porting across **Nacos / Consul / Apollo**, quantifying project maturity and quality.

- Upstream inventory source: official sources at `~/work/github/easynet-cn/{nacos,consul,apollo}` plus official docs.
- All files are Markdown, maintained by hand, but follow **fixed formats** so metrics can be recomputed by `scripts/compat_check.py`.

## Layout

```
docs/compat/
├── README.md           # This file: conventions + metric definitions
├── review-report.md    # Source-code audit report (2026-11-09, client SDK focus)
├── nacos/
│   ├── features.md     # Upstream feature × batata implementation status (197 F-IDs)
│   └── tests.md        # Upstream test inventory (401 T-IDs mapped to F-IDs)
├── consul/
│   ├── features.md     # 181 F-IDs
│   └── tests.md        # 251 T-IDs
└── apollo/
    ├── features.md     # 199 F-IDs
    └── tests.md        # 388 T-IDs
```

There is **no** `bugs.md`, `_index.md` or shell-based checker; the B-ID
scheme below is reserved for future use, and ID validation is done by
`scripts/compat_check.py`.

## Global summary (2026-11-11)

Recomputed from F-ID / T-ID rows (table header rows excluded) and enforced by
`scripts/compat_check.py`.

| Protocol | Features | 🟢 | 🟡 | ⚡ | ⚪ | ⛔ | Impl rate | Test cases | Test types |
|----------|----------|----|----|---|----|----|-----------|------------|------------|
| Nacos    | 197      | 182 | 5  | 0 | 6 | 4  | 92%       | 401        | HTTP_API, GRPC_API, SDK_CLIENT |
| Consul   | 181      | 179 | 2  | 0 | 0  | 0  | 99%       | 251        | HTTP_API, SDK_CLIENT |
| Apollo   | 199      | 169 | 3  | 0 | 27 | 0  | 85%       | 388        | HTTP_API, SDK_CLIENT, INTERNAL |
| **Total** | **577** | **530** | **10** | **0** | **33** | **4** | **92%** | **1040** | |

`Impl rate = round(100 * 🟢 / Features)`. `Test cases` counts **data T-ID rows**,
not the upstream test methods they were derived from and not batata's own E2E
cases (see below).

### Baseline alignment

**Alignment target: Nacos 3.3.0 GA** (released 2026-10-29). Keep the source
snapshot distinct from the released version:

| Where | Version | Meaning |
|-------|---------|---------|
| `sdk-tests/nacos-java-tests/pom.xml:19` | `3.3.0` | Java SDK pinned in batata's E2E matrix |
| `conf/application.yml` + `deploy/k8s/configmap.yaml` | `3.3.0` | Nacos version batata reports as its compatibility target |
| `~/work/github/easynet-cn/nacos` develop `pom.xml:91` | `3.3.0` (@ `d1b97bc3e`) | Post-GA develop snapshot; GA release commit `b64bc9118` is in local history, still zero local tags |

GA was verified against the official Maven Central `-sources.jar` for
`nacos-api` / `nacos-client` / `nacos-maintainer-client` 3.3.0. GA and the
older RC snapshot are byte-identical outside `api/ai` (9 files changed, 4
added), so the inventory and the gap list remain valid for GA. The refreshed
upstream HEAD is 11 commits past the GA release; four of those touch client
SDK sources and are recorded separately as **develop drift** in
`docs/compat/review-report.md` §8 — they do not change the declared GA target.
The full gate has since
been re-run with the 3.3.0-pinned Java SDK:
`reports/regression/20261110-072516/report.md` (all stages PASS, blockers=0)
and `reports/sdk/20261110-073645/results.tsv` (8748 total / 8556 passed /
0 failed / 0 errors / 192 skipped). All six `nacos-java` runtime logs resolve
`nacos-client-3.3.0.jar`, so these are 3.3.0 GA results. The earlier
`reports/sdk/20261109-163544` matrix was pinned to 3.2.4 and is historical
only. Older `3.3.0-SNAPSHOT` / `3.2.4` labels
were stale. Dated files such as
`sdk-tests/nacos-java-tests/TEST_REPORT.2026-03-19.md` are historical snapshots
and keep their original version numbers on purpose. `scripts/compat_check.py`
guard #10 enforces that pom, server config, k8s configmap and the inventory
headings all declare the same target. Consul tracks `2.1.0-dev`, Apollo the
2025 master snapshots.

### Registered Rust client SDK gaps (2026-11-11)

The audit in `review-report.md` compares `batata-client` /
`batata-maintainer-client` with the upstream Nacos Java client. `🟢` rows that
were falsely marked full have been downgraded. Every gap is **registered as an
F-ID** in `docs/compat/nacos/features.md` sections B.8/B.9 (client SDK) or
B.10 (server AI ADMIN), so the checker can detect a status that drifts from reality. Rows whose status has since moved to
`🟢`/`🟡` stay in the table as an audit trail, with the remaining work stated:

| Area | F-ID | Missing capability |
|------|------|--------------------|
| maintainer-client | F-NAC-MAINT-006 | MCP server version lifecycle — closed in batch 1 |
| maintainer-client | F-NAC-MAINT-007 | GA `/skills/upload` `targetVersion` / `uploadAction` / `autoPublishIfNew` — **closed in batch 11** (`🟢`): the three params are now consumed by `SkillUploadQuery` / `ConsoleSkillUploadQuery` / `SkillService::upload_skill` and the `skill_upload_from_zip` SDK; typed DTOs, the full read/write facade, Skill ADMIN `commitMsg` and Prompt draft fork metadata landed in batches 3/7/8/10. Batch 9/10 audits corrected two over-registered field lists |
| maintainer-client | F-NAC-MAINT-008 | Legacy A2A facade + GA-only agent methods (query lifecycle landed in batch 2) |
| batata-client | F-NAC-CSDK-001 | `NamingSelector` subscribe overload |
| batata-client | F-NAC-CSDK-002 | `LockService::renew` / watchdog |
| batata-client | F-NAC-CSDK-003 | `AiService` sub-services + `AgentDiscoveryService` — **closed in batch 14** (`🟢`): the five `Arc<...>` sub-service accessors (`mcp/skill/prompt/agentSpec/agent`) plus the `AgentDiscoveryService` trait (search/discover/subscribe long-poll/register/deregister/publish) over `/v3/client/ai/**`, `AgentDiscoveryCapability::Supported` for the HTTP transport; 16 wire-shape tests + 2 mutants |
| batata-client | F-NAC-CSDK-004 | config 304 conditional GET (`localMd5`) |
| batata-client | F-NAC-CSDK-005 | OIDC auth + `configSnapshotEnabled` |
| batata-client | F-NAC-CSDK-006 | HTTP `requestId` correlation |
| batata-core, batata-ai | F-NAC-CSDK-007 | GA `SetupAckRequest.ability_table` RAD keys — **closed in batch 13** (`🟢`): the four keys (`agentCardV1` / `mcpDraftRelease` / `radV1` / `radWatchV1`) are advertised in `crates/batata-core/src/handler/rpc.rs`, and the GA RAD v1 gRPC handlers (`AgentPublishRpcRequest` / `AgentSubscribeRpcRequest` / `AgentUnsubscribeRpcRequest`) are implemented in `crates/batata-ai/src/handler/ai_handler.rs` backed by `crates/batata-ai/src/service/agent_rad_watch.rs` |
| batata-ai | F-NAC-MAINT-010 | AI ADMIN POST/PUT form-vs-query channel — **closed in batch 5** (`🟢`); the ARD model divergence moved to `F-NAC-AISRV-001` |
| batata-ai (server) | F-NAC-AISRV-001 | GA ARD Agent model — **Phase A partial (🟡, 2026-11-11)**: typed `AgentVersionDetail` / `AgentCallInterface` / `AgentProviderInfo` DTOs, `callInterfaces` stored in version detail + accepted on first draft (no longer fail-closed), GA-shaped `Page<AgentSummary>` / `Page<AgentVersionSummary>` list/version responses, `online_versions` / `status` reconciliation. RAD `GET /runtime-endpoints` live source still deferred (needs Naming publication backend) |
| batata-ai (server) | F-NAC-AISRV-002 | `/v3/admin/ai/import` controller — **false gap, closed in batch 6** (`🟢`, already in `batata-console`, also `F-NAC-ADM-AI-008`) |
| batata-ai (server) | F-NAC-AISRV-003 | `/v3/admin/ai/pipelines` deprecated reads — **⛔ won't support** (GA returns 410 unless an opt-in compatibility flag is set) |

`F-NAC-CSDK-007` was **deferred on purpose** (see above narrative) because GA
Java clients gate RAD gRPC routing on `radV1`/`radWatchV1`: advertising the
keys before the handlers existed would turn a working legacy fallback into a
broken RAD call. **Batch 13 (2026-11-11) closed it** — batata's AI handlers now
cover `AgentPublishRpcRequest`, `AgentSubscribeRpcRequest` and
`AgentUnsubscribeRpcRequest` in addition to `AgentSearchRpcRequest`,
`AgentDiscoveryRpcRequest` and the two endpoint requests, so the four GA keys
are advertised and GA 3.4.0 clients negotiate RAD safely. The only remaining
RAD sub-gap is the `GET /runtime-endpoints` live source, which needs the Naming
publication backend and is tracked separately (outside `F-NAC-CSDK-007`).

**Batch 1 implemented (2026-11-10)**: `F-NAC-MAINT-006` is now `🟢` (eight MCP
version-lifecycle methods + `McpServerVersionInfo` DTO in
`crates/batata-maintainer-client`), and `F-NAC-CSDK-008` is `🟡` (GA
`AgentCard` security fields in `crates/batata-client/src/ai.rs`; the post-GA
wrapper is still pending). `F-NAC-LOCK-009` was downgraded from `🟢` to `⚪`
because the claimed server-side `RENEW` branch does not exist. Details and
mutant evidence are in `docs/compat/review-report.md` §8.5.

**Batch 2 implemented (2026-11-10)**: `F-NAC-MAINT-008` is now `🟡` — the
protocol-neutral `agents_*` facade on `/v3/admin/ai/agents` covers
version reads, the six transitions, draft create/update/delete, labels and
scope (7 mutant-checked wiremock cases). GA bytecode inspection found a
broader server-side divergence: the 3.3.0 Java maintainer client sends
POST/PUT admin AI parameters as a **form body**, while batata's MCP/Agent
ADMIN write handlers read them from the query (and the agent draft/labels
handlers additionally require JSON bodies). That is registered separately as
`F-NAC-MAINT-010` rather than being counted as full compatibility; batata's
own Rust SDK follows the batata wire and Spring `@RequestParam` accepts query,
so the new methods work against both servers. TG-03 is closed: all 10
`BatataAiService` public methods now have direct wiremock tests in
`crates/batata-client/tests/ai_service_test.rs`. See
`docs/compat/review-report.md` §9.

**Batch 3 implemented (2026-11-10)**: `F-NAC-MAINT-007` is now `🟡`. The GA
`AiMaintainerService` delegates' **read surface** is exposed as 25 flat
`skill_*` / `prompt_*` / `agentspec_*` / `pipeline_*` methods on
`MaintainerClient`, plus Skill/Prompt/AgentSpec ZIP upload, precheck, download
and delete. GA bytecode (`nacos-maintainer-client-3.3.0.jar`) and batata's
route table agree on these GET/DELETE query endpoints and on the `file`
multipart part, so no server change was needed. Two HTTP helpers
(`get_bytes_with_query`, `post_multipart_with_query`) were added to
`batata-client`. Six wiremock cases (122 → **128**) and six killed mutants
cover paths, query shapes, multipart parts and raw-byte responses. The
POST/PUT lifecycle, typed DTOs and GA-style sub-service objects remain open,
so the row stays `🟡` rather than `🟢`. See `review-report.md` §10.

**Batch 4 implemented (2026-11-10)**: `F-NAC-MAINT-010` is now `🟡`. A source
re-read showed the original registration was too broad: the Skill/Prompt/
AgentSpec write endpoints in `crates/batata-ai/src/api/skill.rs`,
`crates/batata-ai/src/api/prompt.rs` and `crates/batata-ai/src/api/agentspec.rs`
already take `web::Form`, ZIP uploads keep identity fields in the query and the
payload in `Multipart`, and MCP create/draft already parse form-or-JSON
specifications. The real gap was 15 MCP/Agent POST/PUT handlers bound to
`web::Query` only, plus the MCP `create_draft` and MCP/Agent `labels`
parameters. `crates/batata-ai/src/api/admin_form.rs` adds `AdminParams` (query
+ urlencoded form + JSON object, query wins, multipart excluded) and the
`AdminForm<T>` extractor, wired through `crates/batata-ai/src/api/mcp.rs` and
`crates/batata-ai/src/api/agent.rs`. Eight unit tests and seven route-level
form-vs-query equivalence tests in
`crates/batata-server/tests/ai_admin_form_encoding.rs` guard both channels,
including real success paths; 11 mutants were killed. Four initial survivors
exposed weak guards (no duplicate key in the precedence test, no observable
`overwrite` parameter, and agent labels checked only the HTTP status), so those
assertions were strengthened. At the end of batch 4 the row stayed `🟡` because
`create_agent` / `update_agent` / `create_draft` / `update_draft` still required
a nested `AgentCard` JSON body while GA sends flat draft metadata form fields;
batch 5 closed that last channel (see below). See
`docs/compat/review-report.md` §11. No `--full` gate was run: the change is
limited to AI ADMIN request extraction.

**Batch 5 / server alignment audit (2026-11-10)**: the GA AI ADMIN controllers
at release commit `b64bc9118` were compared endpoint-by-endpoint with the
batata route tables. MCP (19/19), Agent (18/19), Skill (20/20), Prompt (24/24),
AgentSpec (18/19) and the legacy `/v3/admin/ai/a2a` facade (6/8) are all
mounted; Skill/Prompt/AgentSpec already use `web::Form`, so the remaining
`F-NAC-MAINT-007` write facade is a **client SDK** gap, not a server gap. Only
three server areas are genuinely missing: the GA ARD Agent model, the
`/v3/admin/ai/import` controller, and two pipeline read endpoints. They are
registered as `F-NAC-AISRV-001..003` in the new `features.md` section B.10.

**Batch 6 audit correction (2026-11-10)**: two of those three rows were false
gaps. The import controller is fully implemented (4/4) in
`crates/batata-console/src/v3/ai_import.rs` with 18 route tests and is already
tracked as `F-NAC-ADM-AI-008` — the batch 5 audit had grepped only
`crates/batata-ai`. The two "missing" pipeline reads are
`@Deprecated(forRemoval=true)` endpoints that GA gates behind
`nacos.core.api.compatibility.enabled` and answers with **410 Gone** by
default; batata deliberately does not ship them. So `F-NAC-AISRV-002` is now
`🟢`, `F-NAC-AISRV-003` is `⛔`, and `F-NAC-AISRV-001` is the only real
server-side gap. See `docs/compat/review-report.md` §13.

**Batch 7 implemented (2026-11-10)**: `F-NAC-MAINT-007` now covers the whole
GA AI ADMIN **write** surface, not just reads. A source re-read showed the
batch-3 note in `crates/batata-maintainer-client/src/client.rs` was stale:
`crates/batata-ai/src/api/{skill,prompt,agentspec}.rs` write handlers already
take `web::Form`, and MCP takes `AdminForm` / `AdminParams`, so GA's
`postForm` / `putForm` connect directly once `F-NAC-MAINT-010` is closed.
`crates/batata-maintainer-client` gained 35 ADMIN path constants and 42 flat
write methods (Skill/Prompt/AgentSpec draft + version transitions + labels /
biz-tags / scope / online-offline, four legacy prompt APIs,
`listAgentSpecAdminItems`, and MCP draft delete / labels / status / scope).
6 new wiremock cases (128 → **134**) assert exact urlencoded form field maps,
verbs and paths; 15 code mutants were killed. `getAgentSpecAdminDetail` was
confirmed to be a **false gap** already covered by `agentspec_get_detail`.
The row stays `🟡` only because typed AI resource DTOs are still unmodelled,
and some GA-only fields are forwarded but not consumed by batata's form
handlers. F-ID and status totals are unchanged. **Batch 9 correction**: the
`updateLatestLabel` half of that claim was false (GA ignores it on ADMIN);
see `docs/compat/review-report.md` §16. See §14 for the batch 7 evidence.

**Batch 8 implemented (2026-11-10)**: the `F-NAC-MAINT-007` read surface now
returns typed AI resource DTOs instead of `serde_json::Value`. Re-reading the
GA 3.3.0 maintainer-client implementations and `nacos-api` model classes found
**zero post-GA drift** in the skill / prompt / agentspec / pipeline models, so
`crates/batata-maintainer-client/src/model/ai_resource.rs` types the shared
wire fields and keeps every other field in `#[serde(flatten)] extra` (the same
convention as `model/ai.rs`). 24 DTOs and 18 typed read facades (plus 2 generic
helpers) cover Skill / Prompt / AgentSpec / Pipeline reads and ZIP upload
results; both
shared read helpers are now generic over the decoded DTO. 7 serde unit tests
and 15 compile-safe mutants guard camelCase keys, `type` aliases,
null-tolerant collections, `extra` preservation and the exact DTO chosen by
each endpoint. The row stays `🟡` because a few GA-only optional fields are
still not consumed by batata's form handlers — a **server-side** semantic
divergence, not a client SDK gap. F-ID and status totals are unchanged. See
`docs/compat/review-report.md` §15. No `--full`, commit, push or publish was
performed.

**Batch 9 audit (2026-11-10)**: a GA-source re-read of the batch 8 residual
claim found three of its four fields were over-registered. GA deliberately
ignores `updateLatestLabel`: the publish forms bind it as an "ignored" flag and
the operation services say "retained for compatibility and ignored; latest is
server-managed". `basedOnVersion` / `bizTags` are already consumed by batata's Skill/AgentSpec
draft forms and the dedicated `update_biz_tags` endpoints wherever GA binds
them. The only open server-side divergences are `commitMsg` on Skill ADMIN
`POST/PUT /skills/draft` + `POST /skills/upload` (stored by GA as
`versionDesc`) and `basedOnVersion` / `bizTags` on Prompt ADMIN
`POST /prompt/draft`. The row stays partial until those are consumed; no F-ID,
status or count changed, and no code changed. See
`docs/compat/review-report.md` §16.

**Batch 10 implemented (2026-11-10)**: both batch 9 gaps are now consumed by
the server. Skill ADMIN `POST/PUT /skills/draft` and `POST /skills/upload`
accept `commitMsg` on the form, query and multipart-query channels and store it
as the version-level description (`version_description` in
`crates/batata-ai/src/service/skill_service.rs`); the public `SkillService`
trait, its single impl, the admin and console handlers and the tests were
threaded through together. Prompt ADMIN `POST /prompt/draft` now uses
`AdminForm<PromptDraftCreateForm>` plus
`PromptOperationService::create_draft_with_options`: `basedOnVersion` forks the
stored template/variables and bumps the patch version, `bizTags` initializes a
brand-new prompt's metadata, `targetVersion` wins over batata's `version`
alias, and `commitMsg` reaches both the version description and
`PromptStorage` — closing a **false claim in §16.2**, where a declared-but-never-
forwarded field was counted as "consumed". Absent fields keep the historical
behaviour. 13 new route tests (7 in `skill_admin_routes.rs`, 6 in
`prompt_admin_routes.rs`) and **19/19 mutants killed** guard both parameter
channels, the ZIP channel, exact wire keys, error paths and the absent-field
baseline. A fresh GA re-read found three further `/skills/upload` parameters
(`targetVersion`, `uploadAction`, `autoPublishIfNew`) that GA binds and consumes
but batata does not expose, so `F-NAC-MAINT-007` **stays 🟡** at the close of
batch 10; those three params are consumed in batch 11 (below), promoting the row
to `🟢`. F-ID, status and count totals are unchanged. See
`docs/compat/review-report.md` §17. No `--full`, commit, push or publish was
performed.

**Batch 11 implemented (2026-11-10)**: `F-NAC-MAINT-007` is now `🟢`. The three
remaining GA `/skills/upload` parameters — `targetVersion`, `uploadAction`,
`autoPublishIfNew` — are consumed end to end. `SkillUploadQuery` (ADMIN) and
`ConsoleSkillUploadQuery` (console) gained the fields; `SkillService::upload_skill`
gained `target_version` / `upload_action` / `auto_publish_if_new` and mirrors GA
`doUploadSingleSkill` exactly: a blank action falls back to
`overwrite ? overwriteUploadedSkill : createUploadedSkillDraft`, an explicit
`CREATE_DRAFT` / `OVERWRITE_DRAFT` / `DELETE_DRAFT_AND_CREATE` selects the
matching branch (any other value → `PARAMETER_VALIDATE_ERROR` "Unsupported
uploadAction"), and `autoPublishIfNew` force-publishes only the first upload of a
brand-new skill (`meta == null`). The maintainer-client SDK `skill_upload_from_zip`
exposes the same wire via `AiUploadQuery` with `skip_serializing_if`, so the GA
`/upload/batch` endpoint (which binds only `namespaceId` / `overwrite` / `file`)
stays faithful and `skill_batch_upload` / `agentspec_upload_from_zip` are
unchanged. Six new route tests in `crates/batata-server/tests/skill_admin_routes.rs`
(target-version override, `OVERWRITE_DRAFT` success, `CREATE_DRAFT` rejection,
unsupported action, `autoPublishIfNew` publish / keep-draft) and **4/4
compile-safe mutants killed** guard the new branches and the absent-field
baseline. `compat_check.py`, `cargo fmt`, `git diff --check`, clippy `-D warnings`
and the route-test suite all pass. No `--full`, commit, push or publish was
performed.

**Batch 12 implemented (2026-11-11)**: `F-NAC-AISRV-001` is now `🟡` (partial). Phase A closes the GA ARD Agent model gap against Nacos 3.3.0 GA (`b64bc9118`): typed ARD DTOs (`AgentVersionDetail` / `AgentCallInterface` / `AgentProviderInfo` / `AgentVersionSummary` / `AgentSummary`) in `crates/batata-common/src/model/ai/a2a.rs`; `callInterfaces` stored in the version detail (`AgentCardDetailInfo.call_interfaces`, written by `build_detail`, read by `version_row`) and accepted on first draft via `parse_call_interfaces` → `resolve_agent_write` → `create_agent_draft(ns, &card, false, Some(cis))` (no longer fail-closed); `listAgents` / `listVersions` return GA-shaped `Page<AgentSummary>` / `Page<AgentVersionSummary>`; `AgentSummary.online_versions` surfaces the legacy `AgentCardVersionInfo.latest_published_version` so the catalog `latestVersion` resolves, and `AgentVersionSummary.status` uses the `online` / `draft` / `reviewing` lifecycle resolved via `Page.page_items`. All `batata-ai` + `batata-server` route-level callers were migrated to the new shapes (`.page_items`, no `is_latest`). `GET /runtime-endpoints` live support stays deferred to `F-NAC-CSDK-007` (needs Naming publication backend + RAD gRPC handlers), so the row is partial. Counts: B.10 `1🟢/1🟡/0⚪/1⛔` (rate 67%), Nacos Total `179🟢/5🟡/9⚪/4⛔/197` (rate 91% unchanged). `compat_check.py`, `cargo fmt` and `cargo check -p batata-ai --tests` pass; `batata-ai` lib + integration and `batata-server` route suites green. No `--full`, commit, push or publish was performed (commit/push handled separately).

**Batch 14 implemented (2026-11-11)**: `F-NAC-CSDK-003` is now `🟢`. The GA `AiService` facade and `AgentDiscoveryService` are implemented in `crates/batata-client/src/ai.rs`: `BatataAiService` exposes `mcp()` / `skill()` / `prompt()` / `agent_spec()` / `agent()` returning `Arc<...>` sub-services that share the `Arc<BatataHttpClient>`, the 10 legacy flat query methods are preserved as delegates (TG-03 `crates/batata-client/tests/ai_service_test.rs` unchanged), and `AgentAiService` implements the `AgentDiscoveryService` trait over the server's `/v3/client/ai/**` endpoints — `search_agents` (`GET /agents/search`), `discover_agent` (`GET /agents`), `subscribe_agent` (HTTP long-poll `POST /agents/watch` with the `clientId` header, fingerprint-diff notification through `AgentDiscoveryListener`, cancellable `AgentSubscription`), `register_agent_endpoints` / `deregister_agent_endpoints` (`POST` / `DELETE /agents/endpoints`, `clientId` header) and `publish_agent` (`POST /agents` with a `ClientAgentCard` mapped from the existing `AgentCardInfo`). All client DTOs are source-verified against `crates/batata-ai` handlers and the GA release commit `b64bc9118`; `AgentDiscoveryCapability` mirrors GA `AbilityStatus` and `get_agent_discovery_capability()` returns `Supported` for the HTTP transport (the BATATA server exposes `/v3/client/ai/agents/*` unconditionally over HTTP), deliberately not claiming the gRPC `radV1`/`radWatchV1` negotiation (covered by `F-NAC-CSDK-007`). 16 wire-shape unit tests + 2 mutants guard the contract; `cargo fmt`, `cargo check -p batata-client --tests` and both the lib + `ai_service_test` suites pass. Counts: B.9 `2🟢/1🟡/5⚪/0⛔` (rate 13%→25%); Nacos Total `180🟢/5🟡/8⚪/4⛔/197` → `181🟢/5🟡/7⚪/4⛔/197` (rate 91%→92%). `features.md` B.9 + Summary Total, `README.md` global table and F-ID narrative updated. See `review-report.md` §21. No `--full`, commit, push or publish was performed (commit/push handled separately).

**Batch 15 implemented (2026-11-11)**: `F-NAC-CSDK-006` is now `🟢`. `crates/batata-client/src/http.rs` now attaches a fresh v4 UUID under the `Nacos-Request-Id` header to every outgoing client HTTP request, mirroring Nacos 3.3.0 GA `RequestIdHttpClientRequestInterceptor` (which adds `HttpHeaderConsts.NACOS_REQUEST_ID` per request via an interceptor). All `request_with_retry` closures receive a per-request `request_id`, and the retry-bypassing paths (`authenticate()`, `post_json_with_headers()`, `get_bytes*` / `post_bytes` / `post_multipart*`) route through the `with_request_id()` helper. gRPC already set a UUID `request_id`, so the HTTP side is now at parity. 3 wiremock cases in `crates/batata-client/tests/request_id_test.rs` (GET / POST / JWT `authenticate()` login) assert the header is present and a valid v4 UUID; all 3 are mutant-checked (removing the header fails the test). `cargo fmt` and `cargo test -p batata-client` pass (372 lib + 2 ai + 11 tls + 25 unit + 3 request_id + 2 doctests). Counts: B.9 `3🟢/1🟡/4⚪/0⛔` (rate 25%→38%); Nacos Total `181🟢/5🟡/7⚪/4⛔/197` → `182🟢/5🟡/6⚪/4⛔/197` (rate 92% unchanged). `features.md` B.9 + Summary Total, `README.md` global table and F-ID narrative updated. See `review-report.md` §22. No `--full`, commit, push or publish was performed (commit/push handled separately).

Batch 5 then made the four remaining Agent write handlers dual-wire:
`crates/batata-ai/src/api/agent.rs` (`resolve_agent_write`) decodes either batata's
nested `AgentCard` JSON or the GA flat `AgentDraftCreateForm` /
`AgentUpdateForm` parameters (query still wins), maps the overlapping fields,
preserves GA-only governance values in batata `metadata`, and rejects ARD
`callInterfaces` fail-closed. `F-NAC-MAINT-010` is now `🟢`. Six unit tests and
seven new route tests (14 total in
`crates/batata-server/tests/ai_admin_form_encoding.rs`) guard the mapping,
overwrite, validation and query precedence; **16/16 mutants killed**. These
are batata-native guards, so the 401 / 1040 T-ID counts are unchanged. See
`docs/compat/review-report.md` §12. No `--full` gate, commit, push or publish
was performed.

**Batch 6 implemented (2026-11-10)**: `create_agent_draft` now initializes
the Agent metadata row on a first draft, matching GA
`AgentOperationService.createValidatedDraft` (`b64bc9118:368-378`) and the
existing `mcp_service.rs::create_initial_draft_resource`. Before this, a first
`POST /v3/admin/ai/agents/draft` on a fresh Agent returned an internal
"agent not found" error. Three route tests in
`crates/batata-server/tests/ai_admin_form_encoding.rs` (14 → 17) cover the GA
flat form, the nested JSON wire and the persisted version index;
**9/9 code mutants killed**. `F-NAC-AISRV-001` stays `⚪` because typed ARD
DTOs, `Page` responses, RAD endpoints and GA first-draft `callInterfaces`
remain. See `docs/compat/review-report.md` §13. No `--full`, commit, push or
publish was performed.

Two earlier "gaps" were corrected on re-reading upstream source:
cross-namespace `cloneConfig` is only a `default` interface method that throws
`CLIENT_INVALID_PARAM` (`ConfigMaintainerService.java:548-560`), and
`configSnapshotEnabled` **does** exist upstream in `PropertyKeyConst.java:96`
— it is a real client-side switch gap, not an upstream-only feature.

### Post-GA develop drift (not part of the GA baseline)

The refreshed upstream HEAD `d1b97bc3e` is 11 commits past `b64bc9118`.
Four client-facing commits are recorded in `review-report.md` §8 and as
`F-NAC-MAINT-009` / `F-NAC-CSDK-008`: `publishConfig(changeMessage)` and
`ConfigHistoryDetailInfo.changeMessage` (`4a11bd06b`), A2A 1.0
`securityRequirements` on `AgentCard` / `AgentSkill` (`ff1bcc2e9`,
`7c546cfda`), and offline `AgentRequestBuilder` helpers (`a55137012`).
The ARD protocol change (#16014) touched markdown/spec files only — **zero Java
files** — so `getAgentDiscoveryCapability`, `AgentConverter` and synchronous
`Watch` are GA features, not new drift.

### Test-coverage debt

Registered in `review-report.md` §8 rather than as F-IDs (they are test gaps,
not feature gaps): five upstream scenario families with no batata counterpart
(`BinaryCompatibility`, `UpgradeMigration`, `RadRouting`, `JsonAdapter`,
`AgentDiscovery`); `crates/batata-maintainer-client/src/traits.rs` declares three
traits with no `impl ... for MaintainerClient` anywhere; and 14 `#[ignore]`
cases in `crates/batata-client/tests/integration_test.rs` do not count as tests
under project convention. **TG-03 is resolved** for `BatataAiService` itself:
all 10 of its public methods are exercised by
`crates/batata-client/tests/ai_service_test.rs`; the ~42 other untested
`batata-client` public methods remain open.

### Nacos 3.3.0 GA runtime validation (2026-11-10)

The GA-pinned full gate is green (see **Baseline alignment** above). It exposed
one **server-side wire change**: the 3.3.0 `nacos-maintainer-client` sends the
naming ADMIN create/update service and cluster parameters as URL query
parameters instead of a form body. batata's ADMIN handlers now accept both, so
the Java E2E suite passes on 3.3.0; the fix, its regression tests and the gate
repair are documented in `review-report.md` §7. One residual limitation is
recorded there too: nested `healthChecker` / `metadata` fields in GA cluster
query parameters are not yet deserializable from `serde_urlencoded`, and the
Java suite does not exercise them.

### Test inventory (`tests.md`)

Each protocol's `tests.md` maps **upstream API contract tests** to batata F-IDs.
A T-ID tracks whether that *specific upstream IT case* was ported; it is **not**
a coverage measurement.

batata's real compatibility evidence is the E2E matrix:
`reports/sdk/<timestamp>/results.tsv` (produced by `scripts/test.sh sdk --matrix`)
and `reports/regression/<timestamp>/report.md`. The Nacos `tests.md` keeps a
**Coverage evidence** section linking those results; neither file stores a
"Latest result" column.

- **T-ID format**: `T-<feature-id>-<NN>` (e.g. `T-NAC-CFG-012-01`)
- **Upstream source**: each T-ID references the upstream test file + method name
- **Test type**: `HTTP_API` (HTTP integration), `GRPC_API` (gRPC integration, Nacos only), `SDK_CLIENT` (SDK client API), `INTERNAL` (Mockito unit test, not portable to Rust)
- **Port status**: `⬜ not ported` (upstream IT case not copied) | `🔄 in progress` | `✅ ported` | `⏳ pending` | `⏭️ deferred` (scenario intentionally deferred; **not** a missing feature)
- **Strategy**: adapt upstream tests to point at batata server; fix batata bugs when tests fail; keep the port-status column and the Summary table in sync
- **Scope exclusions**:
  - Nacos: internal unit tests (`@RunWith(MockitoJUnitRunner.class)`) excluded
  - Consul: internal RPC tests (`agent/consul/*_test.go`) and gRPC tests (`agent/grpc-external/*`) excluded — gRPC/8502 out of scope
  - Apollo: pure Mockito unit tests and AOP aspect tests listed as INTERNAL (not ported)

The per-module Summary tables at the bottom of `consul/tests.md` (255 cases)
and `apollo/tests.md` (382 cases) are **historical upstream estimates**; the
authoritative count is the number of T-ID data rows (251 / 388). Their totals
are not enforced by the checker.

## Status enums (grep-able; use no other markers)

### Feature status (`features.md` "Status" column)

| Enum     | Meaning                                     |
|----------|---------------------------------------------|
| 🟢 full  | Fully implemented and verified in batata    |
| 🟡 partial | Partially implemented / known gaps         |
| ⚡ in-progress | Under active development              |
| ⚪ planned | Planned, not started                       |
| ⛔ missing | Explicitly absent / won't support (with reason) |

### Bug status (reserved; `bugs.md` does not exist)

`open` | `fixing` | `done` | `regressed` — no bug inventory is maintained today.

### Test result (`tests.md` "Status" column)

`✅ pass` | `⏳ pending` | `❌ fail` | `⏭️ deferred` | `⬜ not ported` | `🔄 in progress`

## Metrics (countable via grep)

| Metric      | Definition                                  |
|-------------|---------------------------------------------|
| Total features | Number of `F-` data rows in `features.md` (table header rows excluded) |
| Implemented % | `round(100 * 🟢 / total)`; same formula for the global summary |
| Gap list       | ⚪ + ⛔ rows = todo roadmap                  |
| Test cases     | Number of `T-` **data** rows in `tests.md` (exclude `| T-ID |` headers) |
| Literal port % | Features with a ✅ in `tests.md` / 🟢+🟡 features — measures upstream test porting only, **not** compatibility coverage |
| Source anchors | 🟢 rows whose `batata impl` names an existing crate under `crates/` |

Example — count implemented features in a file:

```bash
grep -c "🟢" docs/compat/nacos/features.md
```

### Automated validation (`scripts/compat_check.py`)

Validates every `features.md` (ID format, global uniqueness, per-module
contiguous numbering, valid status enum), recomputes each file's Summary **Total**
row and the global summary in this README, verifies the per-module Summary rows
add up to that Total, checks that every Nacos `🟢` row has a resolvable
`batata impl` source anchor, rejects dangling F-ID references and dead file
links, and keeps the F-ID counts in the **Layout** block in sync with the
recomputed inventories (check 11). The status of a row is read from its own
cell, so a status marker quoted in a Notes cell cannot reclassify the row
(check 4, hardened in batch 6). Run from repo root:

```bash
python3 scripts/compat_check.py
```

Exit code 1 means the inventory has drifted from the rows — fix the docs, not
the checker.

## Maintenance conventions

1. **Feature ID** is the primary key and the single anchor for all tracking. Every feature and test carries a globally-unique ID; tests reference the feature they belong to (see **ID scheme** below).
2. New feature → add as `⚪ planned` → set `⚡` while developing → set `🟢` only after source-verifying the implementation and filling a resolvable `batata impl` anchor.
3. **Do not infer implementation status from a `⬜` T-ID.** Conversely, do not assume a `🟢` is correct because the checker passed: `Grep` + `git log -S` against upstream and `crates/` before making a claim.
4. After running `scripts/test.sh sdk --matrix`, refresh the **Coverage evidence** section of `nacos/tests.md` and this README's global summary; results live in `reports/`, never in a `tests.md` status column.
5. When a PR touches a module, review must confirm status columns, Summary rows and source anchors match reality, and re-run `scripts/compat_check.py`.
6. Record newly discovered upstream capabilities as `⚪` F-ID rows before implementing them; do not silently widen the scope of an existing `🟢` row.

---

# Global ID scheme (Feature / Test / Bug)

IDs are **globally unique, self-describing, and cross-linkable**. Feature is the master entity; tests and bugs anchor to it.

## 1. Feature ID

```
F-<PROTO>-<MODULE>-<NNN>
```

| Segment | Rule | Example |
|---------|------|---------|
| `F` | fixed prefix = Feature | |
| `PROTO` | `NAC` Nacos / `CON` Consul / `APO` Apollo / `SYS` system-level (console, auth, cluster, mesh...) | `NAC` |
| `MODULE` | module short-name (uppercase letters; shorter preferred, a few exceptions allowed) | `CFG` |
| `NNN` | 3-digit sequence, **increments within the module**, never reused on delete | `012` |

Examples: `F-NAC-CFG-012` · `F-CON-KV-003` · `F-APO-CFGSVC-001`

## 2. Test ID

```
T-<feature-id>-<NN>
```

Anchored to a feature; `NN` counts the feature's tests (01, 02, …). Example: `T-NAC-CFG-012-01`.

## 3. Bug ID

```
B-<feature-id>-<NN>
```

Same anchor; `NN` increments independently of tests. Example: `B-NAC-CFG-012-01`. Reserved for a future bug inventory; `B-...` is the stable in-repo code, the GitHub Issue number would live in the (not yet created) `bugs.md` `Issue` column.

## Relationship

```
F-NAC-CFG-012  ──┬─ T-NAC-CFG-012-01 (smoke)
                 ├─ T-NAC-CFG-012-02 (regression)
                 └─ B-NAC-CFG-012-01 (defect)
```

`grep F-NAC-CFG-012 docs/compat -r` returns implementation + all tests + all bugs for one feature.

## Global registry enforcement

- Uniqueness is defined on the **`PROTO + MODULE`** pair: the same acronym may appear under different protocols/SYS, since the full ID (e.g. `F-NAC-AUTH-001` vs `F-SYS-AUTH-<NNN>`) is always distinct.
- Module short-names are fixed in the **registry below** (one abbreviation per module → no collisions within a protocol).
- `NNN` increments within a module and tags are never reused.
- There is **no separate registration ledger**: `features.md` is the registry, and `scripts/compat_check.py` enforces ID format regex, global uniqueness, contiguous numbering, referenced `F-...` existence and legal status enums. Run it after every inventory edit.

## Module acronym registry

A module = a composable feature domain from the upstream project (not a server process). Acronyms are stable and never clash across protocols.

### Nacos (nacos)

| Acronym | Module | Notes |
|---------|--------|-------|
| `CFG` | Config | publish/query/listen config |
| `NS` | Naming | instance register/discover |
| `CORE` | Core | namespace / cluster state / ops |
| `AUTH` | Auth | users/roles/permissions/login |
| `CLUSTER` | Cluster | raft election / distro / members |
| `CONN` | SDK connection | gRPC connection setup/auth/heartbeat/TLS |
| `LOCK` | Lock | distributed lock (nacos-lock) |
| `AISDK` | AI gRPC SDK | MCP/A2A agent/prompt gRPC client |
| `CLIENT` | Client HTTP API | `v3/client/**` SDK v2 HTTP fallback |
| `CMDB` | CMDB | datacenter/region metadata |
| `ADDR` | Address | address server |
| `ISTIO` | Istio | mesh sync |
| `K8S` | k8s-sync | Kubernetes sync |
| `PROM` | Prometheus | metrics export |
| `COP` | Copilot | Copilot (3.x) |
| `AI` | AI console | AI console surface (`v3/console/ai/**`) |
| `MCP` | AI Registry (ai-registry-adaptor) | AI registry adapter |
| `MAINT` | Maintainer | maintainer client SDK |
| `PLUG` | Plugin SPI | auth/ai/config/control/datasource/encryption/trace/visibility/environment |
| `ADM` | Admin HTTP API | `v3/admin/**` — sub-modules: `ADM-CS` (config), `ADM-NS` (naming), `ADM-CORE` (core), `ADM-AUTH` (auth), `ADM-AI` (AI), `ADM-LEGACY` (legacy/misc) |
| `OTH` | Other/exclusive | istio/cmdb/address/prometheus/k8s-sync/audit |

> **Nacos HTTP path families**: most `console-ui-next` calls use `v3/console/**` (config `v3/console/cs/**`, naming `v3/console/ns/**`, core/namespace/plugin `v3/console/{core,server,plugin}/**`, auth `v3/auth/**`), plus a dedicated AI console surface `v3/console/ai/**` and copilot `v3/console/copilot/**`. Admin API (`v3/admin/**`) is used by the maintainer SDK and open platform. Client API (`v3/client/**`) is the SDK v2 HTTP fallback. All three surfaces are tracked in features.md.

> **Consul surface**: external contract is HTTP `/v1/**` + **blocking queries** (`?index=&wait=` long-poll, `?stale/consistent/cached`) and headers `X-Consul-Index` / `X-Consul-ContentHash`. Agent-local registration (`/v1/agent/service/register`) syncs to catalog (`/v1/catalog/*`) and is aggregated into `/v1/health/*`. gRPC (8502, connect/xDS) + gossip are out of core scope. ACL via `X-Consul-Token`.

### Consul (consul)

| Acronym | Module | Notes |
|---------|--------|-------|
| `AGNT` | Agent | local services/checks/register/reload/token |
| `CAT` | Catalog | service/node directory |
| `HEALTH` | Health | health queries / `passing` / state |
| `KV` | KV | get/put/list/CAS/session lock |
| `SES` | Session | session create/renew + TTL |
| `ACL` | ACL | token/policy/role/bootstrap/oidc |
| `OTH` | Status/Coord/Event/Snap/Query/Txn | misc core endpoints |
| `CFGE` | Config entry | config entries (extension) |
| `OP` | Operator | cluster ops: raft/autopilot/usage (extension) |
| `CONN` | Connect mesh | CA/intentions (extension) |
| `PEER` | Peering/Partition/Namespace | isolation (extension) |
| `CORE` | Blocking query contract | index/hash/consistency/token cross-cutting |

### Apollo (apollo)

> **Apollo surfaces**: three services in one contract — **configservice** (client fetch `/configs/...` + long-poll `/notifications/v2`, optional AccessKey HMAC signature), **adminservice** (write ops, `/apps`, `/items`, `/releases`, `/branches`, admin token whitelist), **portal** (**OpenAPI v1** `/openapi/v1/**` used by the Portal UI itself; `ConsumerToken`/session/user-token + `@PreAuthorize`). Legacy WebAPI is `@Deprecated`.

| Acronym | Module | Notes |
|---------|--------|-------|
| `CFGSVC` | ConfigService | client fetch `/configs` `/configfiles` |
| `LONGPOL` | Long polling | `/notifications/v2` (304 on timeout) |
| `META` | Metaservice | `/services/config`,`/services/admin` |
| `ADM` | AdminService | apps/clusters/namespaces |
| `ITEM` | AdminService items | items CRUD / batch |
| `RELEASE` | AdminService release | publish/rollback/history |
| `BRANCH` | AdminService gray | branches/rules |
| `ADMSVC` | AdminService misc | instances/accesskey/appnamespace/server-config |
| `PORT` | Portal OpenAPI apps | apps/envs/clusters/namespaces |
| `PITEM` | Portal items | items/diff/sync/validation |
| `PREL` | Portal releases | publish/gray/rollback/history |
| `PMISC` | Portal misc | instances/accesskey/perms/users/tokens |
| `LEGACY` | Portal legacy WebAPI | deprecated compat surface |

### System-level (SYS)

SHARED domains implemented independently of a single protocol: `CONSOLE` · `AUTH`(server) · `CLUSTER` · `MESH`(xDS) · `AI`(server) · `ENCRYPT` · `AUDIT`