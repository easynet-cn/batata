# Client SDK compatibility review — 2026-11-09 (updated 2026-11-10)

**Scope**: `crates/batata-client` and `crates/batata-maintainer-client`
against the upstream **Nacos Java client SDK** (`api/`, `client/`,
`client-basic/`, `maintainer-client/`) and its test suites. Server-side
controllers, console UI, Consul and Apollo are **out of scope** for sections
1–10; section 12 extends the audit to the **server-side AI ADMIN surface** in
response to a later request.

**Method**: read-only source audit. Every claim below was checked with `Grep`
in both repositories plus `git log -S` in the upstream checkout. The original
audit used upstream `develop` = `1511b5a85` (2026-10-14), `pom.xml:91`
revision `3.3.0-RC`. **Alignment target was later set to Nacos 3.3.0 GA**
(released 2026-10-29); section 1 was updated accordingly. The upstream
checkout was refreshed on 2026-11-10 to HEAD `d1b97bc3e` (`<revision>3.3.0`,
GA release commit `b64bc9118` + 11 post-GA commits); section 8 records that
delta. The audit itself was read-only; the 2026-11-10 runtime re-run did
change production code (section 7), and the follow-up implementation changed
more (section 8).

## 1. Baseline alignment

**Decision (2026-11-09): align with Nacos 3.3.0 GA.** The official release
history lists 3.3.0 as the current GA (2026-10-29); `nacos-client:3.3.0` and
`nacos-maintainer-client:3.3.0` are on Maven Central. The earlier `3.3.0-RC`
label described a pre-release candidate, not a release.

| Where | Version | Meaning |
|-------|---------|---------|
| Alignment target | `3.3.0` GA | Declared in `docs/compat/{README.md,nacos/features.md,nacos/tests.md}` |
| `sdk-tests/nacos-java-tests/pom.xml:19` | `3.3.0` | Java SDK pinned in batata's E2E matrix (bumped from 3.2.4) |
| `conf/application.yml` + `deploy/k8s/configmap.yaml` | `3.3.0` | Version batata reports as its Nacos compatibility target (bumped from 3.2.3) |
| `~/work/github/easynet-cn/nacos` develop | `3.3.0` @ `d1b97bc3e` (2026-11-09) | Post-GA develop snapshot; GA release commit `b64bc9118` is now in local history |

The GA contract was originally verified from the official `-sources.jar` of
`nacos-api`, `nacos-client` and `nacos-maintainer-client` 3.3.0 on Maven
Central, because the local checkout then had no tag and its `origin` was
blocked by the proxy. Byte-level diff GA vs the older RC snapshot: **0 changed
files** in `maintainer-client`; **0** in `ConfigService`, `NamingService`,
`LockService`; `api` has 9 modified + 4 added files, all under `api/ai` (RAD
discovery capability via `getAgentDiscoveryCapability()`, `AgentConverter`
SPI, `AgentPublishRequest.autoSubmit`, synchronous `Watch` admission in
`AgentDiscoveryService`). The refreshed checkout still has **zero local tags**,
but `git show b64bc9118` confirms the GA release commit; HEAD is 11 commits
past it, 4 of which touch client SDK sources (section 8). Therefore the
comparison tables below are valid for 3.3.0 GA, while post-GA drift is
tracked separately rather than folded into the declared baseline.

The full matrix has since been re-run with the 3.3.0-pinned Java SDK
(2026-11-10): `reports/sdk/20261110-073645/results.tsv` and
`reports/regression/20261110-072516/report.md`. All six `nacos-java` runtime
logs resolve `nacos-client-3.3.0.jar`, so sections 3 and 7 quote 3.3.0 GA
coverage; the earlier 3.2.4-pinned run is historical only.

Representative capability-introducing commits (`git log -S` on upstream):

| Capability | Commit | Date |
|------------|--------|------|
| config 304 conditional GET / `GetConfigRequest.localMd5` | `e2f806777` | 2026-09-09 |
| MCP maintainer version lifecycle (`submitMcpServerVersion` …) | `33f416768` | 2026-08-28 |
| `configSnapshotEnabled` property switch | `0004ea910` | 2026-10-11 |
| OIDC client-side auth | `c87f888fc` | 2026-03-23 |
| `NamingSelector` subscribe overloads | `cc34b6ca0` | 2023-08-17 |

`NamingSelector` predates 3.2.4 and is present unchanged in the 3.3.0 GA
sources, so it is a **true gap** at any alignment target, not a version
difference. `LockService.renew` carries `@Since("3.3.0")` in
`api/src/main/java/com/alibaba/nacos/api/lock/LockService.java`.

## 2. Feature comparison

### 2.1 `batata-client` ↔ upstream Java client

| Upstream capability | Upstream evidence | batata evidence | Verdict |
|---------------------|-------------------|-----------------|---------|
| Config get/publish/remove/listen, `getConfigWithResult`, `publishConfigCas` | `api/.../config/ConfigService.java` | `crates/batata-client/src/traits.rs`, `config/*` | ✅ aligned |
| Batch register/deregister, fuzzy watch, config filter, change parser | `client/.../config/impl/` | `crates/batata-client/src/config/*` | ✅ aligned |
| Failover snapshot, redo, protect mode, service info cache | `client/.../{config,naming}/` | `crates/batata-client/src/config/*`, `naming/*` | ✅ aligned |
| gRPC connection / reconnect / auth / heartbeat / labels / TLS | `client/.../remote/` | `crates/batata-client/src/grpc/*`, `src/tls.rs` | ✅ aligned (TLS wired in 1.0.0) |
| RAM signature, address server, gRPC port offset | `client/.../security/`, `.../address/` | `src/signing.rs`, `src/grpc/connection.rs` | ✅ aligned |
| `NamingService.subscribe(..., NamingSelector, ...)` | `api/.../naming/NamingService.java:576,589` | `src/traits.rs:239` only `(service, group, clusters, listener)` | ❌ true gap |
| `LockService.renew(LockInstance)` + watchdog | `api/.../lock/LockService.java` (`@Since 3.3.0`) | `src/lock.rs` only `lock/lock_with_params/unlock/try_lock/lock_guard/release/key` | ❌ 3.3.0 gap |
| `AiService.mcp()/skill()/prompt()/agentSpec()/agent()` + `AgentDiscoveryService`; GA also adds `getAgentDiscoveryCapability()` (RAD) and `AgentConverter` SPI | `api/.../ai/AiService.java`, `api/.../ai/converter/` (3.3.0 GA sources) | `src/ai.rs` = `BatataAiService` with 10 methods (MCP/A2A query + cache) | ❌ 3.3.0 gap |
| config 304 conditional GET (`localMd5`) | `e2f806777` | `batata-api/src` has no `local_md5` field | ❌ 3.3.0 gap |
| OIDC client auth, `configSnapshotEnabled` | `c87f888fc`, `0004ea910` | no `oidc` module; snapshot is an internal switch | ❌ 3.3.0 gap |
| HTTP `requestId` correlation header | upstream client HTTP proxy | gRPC has a UUID `request_id`; `src/http.rs` has no request-id header | ❌ 3.3.0 gap |

Note on `F-NAC-LOCK-009` (`🟢`): that row is the **server-side** gRPC renewal
contract (`RENEW` op + owner field), implemented in
`crates/batata-core/src/handler/lock.rs`. It is correctly `🟢`; the missing piece is the
Rust **client API** wrapper, not the wire contract.

### 2.2 `batata-maintainer-client` ↔ upstream maintainer client

`crates/batata-maintainer-client/src/client.rs` has **106 public methods**
(98 before batch 1 added the eight MCP version-lifecycle methods);
`src/model/ai.rs` has **12 public DTOs** (11 + `McpServerVersionInfo`).

| Upstream capability | Upstream evidence | batata evidence | Verdict |
|---------------------|-------------------|-----------------|---------|
| Config / Naming / Core maintainer services, import+export | `maintainer-client/.../{config,naming,core}/` | `src/client.rs` | ✅ aligned (import/export is an extra vs upstream SDK) |
| `overrideExisting`, `registrationType`, plugin `localOnly` params | `.../ai/AiMaintainerService.java`, `.../core/` | `src/client.rs:3350/3448/3770` | ✅ aligned |
| MCP CRUD (`list/search/detail/create/update/delete`) | `.../ai/McpMaintainerService.java` | `src/client.rs:3153-3437` | ✅ aligned |
| A2A agent register/get/update/delete/list/list_versions/search | `.../ai/A2aMaintainerService.java` | `src/client.rs:3444-3730` | ✅ aligned (basic CRUD only) |
| MCP version lifecycle: `listMcpServerVersions`, `getMcpServerVersion`, submit/publish/forcePublish/redraft/online/offline | `.../ai/McpMaintainerService.java`, `AiMaintainerService.java:172-249` | eight `mcp_server_*` wrappers added in batch 1 (section 8.5) | ✅ closed (`F-NAC-MAINT-006`); `deleteMcpServerDraft` + labels/scope/status are still in `F-NAC-MAINT-007` |
| AI delegates `skill()`, `prompt()`, `agentSpec()`, `pipeline()`, `a2a()`, `mcp()` | `AiMaintainerService.java:47-73` | no delegate API; `model/ai.rs` holds only DTOs | ❌ gap |
| Skill/Prompt/AgentSpec ZIP uploads, precheck, batch upload, version detail | `.../ai/{Skill,Prompt,AgentSpec}MaintainerService.java` | none | ❌ gap |
| `AgentMaintainerService` draft lifecycle (`createDraft/updateDraft/submit/publish/forcePublish/redraft/online/offline/updateLabels`) | `.../ai/AgentMaintainerService.java:281-498` | none | ❌ gap |
| `agent_get(version)` / `getAgentVersion` / runtime endpoints | `AgentMaintainerService.java:227,256` | `src/client.rs:3478` has no `version` param | ❌ small gap |
| cross-namespace `cloneConfig` overload | `ConfigMaintainerService.java:548-560` | `src/client.rs:1137` takes a single `namespace_id` | ❎ **not a gap**: upstream's 6-arg overload is a `default` method that throws `CLIENT_INVALID_PARAM ("Cross-namespace clone is not supported by this implementation.")` whenever the two namespaces differ, and otherwise delegates to the 4-arg form batata already wraps |

**Key finding**: these are **pure SDK wrapping gaps**. The batata server and
console already expose the routes — `crates/batata-console/src/v3/ai_mcp.rs`
(`/versions`, `/draft`, submit/publish/redraft/online/offline), plus
`ai_skill.rs`, `ai_prompt.rs`, `ai_agent.rs`, `ai_agentspec.rs`,
`ai_pipeline.rs`, `ai_a2a.rs`, `ai_import.rs`. No wire-protocol change is
needed to close them.

## 3. Test comparison

| Upstream suite | Cases | batata counterpart | Cases | Verdict |
|----------------|-------|--------------------|-------|---------|
| `client/src/test/java` | 1479 `@Test` | `batata-client/src` unit tests | 359 | quantitative gap, but different granularity |
| `client-basic/src/test/java` | 286 `@Test` | `batata-client/tests/` | 50 | same |
| `api/src/test/java` | 1322 `@Test` | — | — | mostly Java DTO/serialization tests, not portable |
| `maintainer-client/src/test/java` | 342 `@Test` | maintainer crate: 37 `src` + 115 `tests/` | 152 | same |
| `test/java-sdk-test` | 123 `@Test` (120 at GA release) | `sdk-tests/nacos-java-tests` | **539 `@Test`** methods | ✅ batata suite is larger (the old `588` was annotation tokens) |
| `test/maintainer-sdk-test` | 46 `@Test` | `sdk-tests/batata-maintainer-tests` | 77 Rust cases | ✅ |
| `test/lock-test` | 62 `@Test` | lock coverage inside Rust suites | — | partial |

Matrix evidence (**3.3.0 GA-pinned run**;
`reports/sdk/20261110-073645/results.tsv`, 6 combos, 36 rows): `nacos-java`
3210 total / 3162 passed / 0 errors, `batata-client` 750/750,
`batata-maintainer` 462/462; grand total **8748 / 8556 passed / 0 failed /
0 errors / 192 skipped**. Full gate
`reports/regression/20261110-072516/report.md`: all stages PASS, blockers=0,
3155 unit tests passed, 65m44s. Case-count anchors are unchanged from the
3.2.4 run — the GA bump added no new cases. The first GA-pinned pass had 7
errors per `nacos-java` combo (42 across the matrix); the fix in section 7
brought all of them back to green without changing any case count.

**But**: every one of the ❌ feature gaps above has **no client test**, because
the API does not exist yet. Upstream scenario families
`BinaryCompatibility`, `UpgradeMigration`, `RadRouting`, `JsonAdapter` and
`AgentDiscovery` have **0 matches** in `sdk-tests/nacos-java-tests`. Test
coverage is therefore complete only for the API surface batata already has.

## 4. Documentation drift found (fixed in this pass)

| Drift | Evidence | Fix |
|-------|----------|-----|
| `F-NAC-MAINT-004` / `F-NAC-MAINT-005` marked `🟢` while only part of the upstream contract is wrapped | `features.md:341-342` vs `src/client.rs` | downgraded to `🟡` with explicit missing-method notes |
| Nacos feature Summary (`B.5`, `B.8`, `Total`) did not match the F-ID rows | Summary table vs recomputed rows | recomputed: `176/2/0/0/3 = 181`, 97% |
| `tests.md` claimed 304 mapped T-IDs / 283 to port; actual rows are 401 / 380 / 21 | recomputed from data rows (18 table-header rows had been counted as data) | Summary rewritten; ⬜ semantics clarified |
| 21 `⏭️` rows said "skip because F-ID is ⚪"; all five referenced F-IDs are `🟢` and there are zero `⚪` Nacos rows | `F-NAC-ADM-AUTH-001`, `F-NAC-ADM-AI-008`, `F-NAC-CLIENT-010/011`, `F-NAC-AISDK-008` | redefined `⏭️` as deferred scenario |
| README global summary frozen at 2025-08-08 (541 / 89%) while inventories hold 561 F-IDs and 1040 T-IDs | `README.md` | recomputed to 561 / 93% / 1040 |
| README referenced `review-report.md`, `bugs.md`, `_index.md`, `scripts/compat_check.sh`; none existed | filesystem check | this report created; others removed; enforcement moved to `compat_check.py` |
| README convention #4 told maintainers to refresh a non-existent "Latest result" column | `tests.md` has 6 columns only | replaced with the Coverage-evidence workflow |
| Heading claimed "three contract surfaces" and listed four | `nacos/features.md:6-12` | corrected to four |
| Apollo Summary impl rate `86%` vs computed `169/199 = 85%` | `apollo/features.md` | corrected to 85% |
| 5 `T-APO-SDK-001-*` rows use a synthetic `APO-SDK-001` anchor that is not registered as a feature | `apollo/tests.md:696-700` | **not changed** (outside this audit's scope); the `Feature` column already points at real F-IDs |
| Nacos version baseline was split: E2E pom `3.2.4`, server config `3.2.3`, inventory headings `3.3.0-SNAPSHOT` / `3.3.0-RC` | `pom.xml`, `conf/application.yml`, `deploy/k8s/configmap.yaml`, `docs/compat/nacos/*.md` | unified on **3.3.0 GA** (official release 2026-10-29); GA contract verified from Maven Central `-sources.jar`; `compat_check.py` guard #10 now enforces it |

`scripts/compat_check.py` was extended to make the numeric drift, missing
source anchors and version-baseline drift fail the check instead of silently
passing. The version guard was mutant-checked together with the earlier guards
(25/25 mutants killed).

## 5. Gap register (implementation backlog, not done)

| Priority | Gap | Classification |
|----------|-----|----------------|
| ✅ | maintainer-client: MCP version lifecycle (8 methods) | **done** in batch 1, `F-NAC-MAINT-006` now `🟢`; see section 8.5 |
| P1 | maintainer-client: `AiMaintainerService` delegates (skill/prompt/agentSpec/pipeline) + ZIP uploads | SDK wrapping only |
| P1 | maintainer-client: `AgentMaintainerService` draft/publish lifecycle | SDK wrapping only |
| P2 | batata-client: `NamingSelector` subscribe overload | true baseline gap (present in GA too) |
| P2 | batata-client: `AiService` sub-services + `AgentDiscoveryService` + GA RAD capability | 3.3.0; server routes exist |
| P2 | batata-client: `LockService::renew` public API | 3.3.0; server contract already `🟢` |
| P2 | server `SetupAckRequest.ability_table` missing GA keys `agentCardV1`, `mcpDraftRelease`, `radV1`, `radWatchV1` | **new GA wire gap**: 3.2.4 declares 6 server keys, GA declares 9; batata's `SERVER_ABILITIES` (`crates/batata-core/src/handler/rpc.rs:838-851`) has 5 (`fuzzyWatch`, `lock`, `mcp`, `agent`, `supportPersistentInstanceByGrpc`). GA clients gate RAD routing / MCP draft release on these keys, so they fall back to the legacy path. `F-NAC-CONN-004` is still `🟢` because the `SetupAckRequest` message itself exists |
| P3 | config 304 conditional GET, OIDC, `configSnapshotEnabled`, HTTP `requestId` | 3.3.0 client-side wire work |
| P3 | E2E tests for the five missing upstream scenario families | test gap |
| P3 | GA cluster query parameters carrying nested `healthChecker` / `metadata` JSON | residual ADMIN wire gap found by the 3.3.0 re-run: `serde_urlencoded` cannot decode nested structures, and the Java suite does not exercise them (section 7) |
| ✅ | re-run the full matrix with the 3.3.0-pinned Java SDK | **done** (2026-11-10): all 6 combos green, 8748 / 8556 passed / 0 errors |
| ✅ | refresh the debug server binary in the regression gate | **done**: `ensure_server_binary()` now always runs `cargo build -p batata-server` |
| ✅ | declare one alignment target | **done**: Nacos 3.3.0 GA; pom/config/docs unified |

Per project convention, new guards/tests must pass mutant checking
(removing the guard must turn the test red) and success paths must be tested.

## 6. Conclusion

**Version baseline aligned and runtime-verified; feature/test coverage not yet
complete.** The shortfall is well-bounded: the Nacos wire protocol, the batata
server and the console are ahead of the Rust SDKs, and the compatibility
tracking documents had drifted from both. After this pass the declared target
is a single, verified **Nacos 3.3.0 GA**, the E2E pom and server compat version
match it, the inventories are internally consistent and machine-validated, and
the full gate has been re-run against 3.3.0 GA with all stages PASS. That re-run
also closed one real ADMIN wire incompatibility (section 7). The remaining work
is SDK API wrapping plus tests, not protocol rework, plus the GA ability keys
and the nested cluster query decode noted in section 5.

## 7. Nacos 3.3.0 GA runtime re-run (2026-11-10)

**Preflight**: MySQL/PostgreSQL containers healthy, all 22 SDK ports free,
`conf/cluster.conf` backed up, pom / server config / k8s configmap all `3.3.0`.

**First pass (red)**: `reports/regression/20261109-225713` — fmt/check/unit
green (3144 passed), integration FAIL, all 6 matrix combos FAIL. Only
`nacos-java` was red: 7 errors per combo (42 total), 0 failures, case counts
unchanged (8748 total / 8514 passed / 42 errors / 192 skipped). Errors were
`Parse error: missing field serviceName` from `NacosMaintainServiceTest` and
`NacosNamingMaintainerServiceTest`, both on the `createService` setup call.

**Root cause (source-verified with official `-sources.jar`, 3.2.4 vs 3.3.0
GA)**: upstream `NacosNamingMaintainerServiceImpl` replaced 9
`.setParamValue(params)` calls (form body) with `appendQuery(path, params)`
(URL query). `instance.rs` already had a dual-channel `merge_form` helper;
`service.rs` (create / update / update_service_cluster) and `cluster.rs`
(create / update) still used `web::Form` only, so an empty body produced
`missing field serviceName` → a plain-text 400 that the Java client could not
parse. `ClientHttpProxy` also switched Jackson → `JsonUtils` for error parsing.
`Service` DTO and `RequestUtil.toParameters` are unchanged between 3.2.4 and
3.3.0; `AbstractCoreMaintainerService` also moved readiness from
`/ops/readiness` to `/state/readiness` (not exercised by this suite).

**Fix**: shared helper
`crates/batata-naming/src/api/v3/admin/form.rs::merge_form` (query first, body
fallback) extracted from `instance.rs`; the five ADMIN handlers now take
`Option<Query<T>> + Option<Form<T>>` and return a structured 400 when both are
absent. New tests: one unit test pinning the GA query wire shape
(`ga_330_admin_service_query_shape_deserializes`, covering metadata/selector
as JSON text) and `crates/batata-server/tests/admin_naming_form_encoding.rs`
(5 route-level cases: GA query create, legacy body create, GA query update,
query precedence, empty-form structured 400). Mutant check: replacing the three
`merge_form(query, body)` calls with `merge_form(None, body)` makes 3 of the 5
route tests fail; restored version is 5/5 green.

**Gate defect found and fixed**: `scripts/regression.sh::ensure_server_binary()`
returned early whenever either release or debug binary existed, but
`test.sh sdk --quick` uses the **debug** binary. `cargo check` does not relink,
so after any source change the integration stage silently ran the stale
pre-fix executable. It now always runs `cargo build -p batata-server`
(incremental, near-zero cost when unchanged). The matrix stage builds its own
release binary, which is why its first combo was already green.

**Final pass (green)**: `reports/regression/20261110-072516/report.md` —
fmt PASS, check PASS (0 errors / 0 warnings), unit PASS (3155 passed,
0 failed), integration PASS, matrix PASS (6 combos), total 65m44s,
blockers=0. `reports/sdk/20261110-073645/results.tsv`: 36 rows,
**8748 total / 8556 passed / 0 failed / 0 errors / 192 skipped**, with
`nacos-java` 3210 / 3162 passed / 0 errors / 48 skipped and every per-combo
anchor from `scripts/regression_matrix_baseline.txt` met (531 standalone /
539 cluster, 552 consul-go, 125 client, 129 consul-client, 77 maintainer,
40 apollo-java). All six runtime logs resolve `nacos-client-3.3.0.jar`,
confirming GA at execution time, not just in the pom.

**Residual limitation**: `CreateClusterForm` / `UpdateClusterForm` carry nested
`healthChecker` / `metadata` structures. GA sends them as JSON text inside a
URL query parameter, which `serde_urlencoded` cannot decode; the Java suite
does not exercise that path. Both form-body callers and the flat query field
set work. Tracked as P3 in section 5.

## 8. Upstream refresh and gap register (2026-11-10)

**Decision**: keep **Nacos 3.3.0 GA** as the declared alignment target (guard
#10 stays unchanged). Post-GA develop drift is registered separately so it is
visible but does not silently redefine the release baseline.

### 8.1 What actually changed upstream

| Repo | HEAD | Client-facing delta |
|------|------|---------------------|
| nacos | `d1b97bc3e` (2026-11-09), `<revision>3.3.0</revision>` | GA release commit `b64bc9118` is now in local history; **11 post-GA commits**, still zero local tags |
| consul | `8439acc468` (2026-11-10) | HEAD only touches `ui/pnpm-lock.yaml` / `pnpm-workspace.yaml`; no wire change |
| apollo | `5b663c5f` (2026-11-07) | CI / Portal fixes; `apollo-java` `2f4b4a4` adds OpenAPI `extendInfo` / `parentAppId` (affects `batata-plugin-apollo`, **not** the two SDK crates) |

`grep -rilE 'consul|apollo'` over `crates/batata-client/src` and
`crates/batata-maintainer-client/src` returns zero files — both crates are pure
Nacos SDKs, so the Consul/Apollo updates create no gap here.

Four of the 11 post-GA commits touch client SDK sources:

| Commit | Subject | Client-facing change |
|--------|---------|----------------------|
| `4a11bd06b` | #15914 Add publish operation messages to config history | `publishConfig(..., changeMessage)` + `ConfigHistoryDetailInfo.changeMessage` |
| `ff1bcc2e9` | #15976 Support standard AgentCard security requirements JSON | A2A 1.0 `AgentCard.securityRequirements` |
| `7c546cfda` | #15982 Preserve A2A 1.0 AgentSkill security requirements | `AgentSkill.securityRequirements` |
| `a55137012` | #15984 Add offline Agent request builders | `AgentRequestBuilder` / `A2aRequestUtils` |

`#16014` (Simplify ARD protocol specifications, HEAD) changed **markdown/spec
files only, zero Java files**. `getAgentDiscoveryCapability()`,
`AgentConverter`, `AgentWatch*` and `AgentPublishRequest.autoSubmit` already
exist at `b64bc9118`, i.e. they are GA features, not new drift. Upstream
`test/java-sdk-test` grew from 120 to **123 `@Test`**; `maintainer-sdk-test`
(46) and `lock-test` (62) are unchanged.

### 8.2 Corrections to this report and the inventories

| Earlier claim | Verified value | Evidence |
|---------------|----------------|----------|
| `nacos-java-tests` has 588 `@Test` methods | **539** `@Test` methods | annotation breakdown: 539 `@Test` + 48 `@TestMethodOrder` + 1 `@TestInstance`; 588 was all annotation tokens |
| upstream `test/java-sdk-test` has 120 `@Test` | **123** at HEAD, 120 at GA release | `grep -rhoE '@Test([[:space:]]\|$)'` |
| local upstream is a `3.3.0-RC` pre-release snapshot | `3.3.0` @ `d1b97bc3e`, post-GA develop | `pom.xml:91`, `git log b64bc9118..HEAD` |
| `configSnapshotEnabled` is missing upstream | Exists in `PropertyKeyConst.java:96` | commit `0004ea910`; the gap is batata's client-side switch |
| cross-namespace `cloneConfig` is a gap | Not a gap | the 6-arg overload is a `default` method that rejects differing namespaces |
| 14 `#[ignore]` cases count as tests | They do **not** (project convention) | `crates/batata-client/tests/integration_test.rs` |
| `F-NAC-CONN-004` ability keys are a simple missing-map-entry gap | Deliberately deferred | declaring `radV1`/`radWatchV1` routes GA clients to unimplemented gRPC handlers (`AgentPublishRpcRequest`, watch); batata only handles AgentSearch/AgentDiscovery/Endpoint(De)Register |
| `F-NAC-LOCK-009` "Watchdog auto-renew" is `🟢` | **False green**, downgraded to `⚪` | `crates/batata-core/src/handler/lock.rs:94-139` has only `ACQUIRE`/`RELEASE` branches; `grep -rn 'RENEW' crates/batata-core/src` returns nothing. Upstream GA has `LockOperationEnum.RENEW` and `LockGrpcClient.java:279`. Both the server op and the client wrapper are missing; a correct fix also needs a Raft renew variant and must not reuse `acquire()` (it increments the reentrant count) |
| A2A `securityRequirements` is one post-GA drift item | Two different wire contracts | At GA release `b64bc9118`, `AgentCard.securityRequirements` is `List<Map<String,List<String>>>` (matching `crates/batata-common/src/model/ai/a2a.rs:112`) and `AgentSkill` has no security field. Post-GA `ff1bcc2e9` changes it to `List<SecurityRequirement>` with a `schemes` / `StringList.list` wrapper, and `7c546cfda` adds `AgentSkill.securityRequirements`. Implementing the develop shape would be a breaking wire change on both client and server |

### 8.3 Registered feature gaps

The 12 rows below were added as `⚪ planned` F-IDs in
`docs/compat/nacos/features.md` (B.8 `F-NAC-MAINT-006..009`, new B.9
`F-NAC-CSDK-001..008`). The README global Summary was recomputed to
193 Nacos / 573 total features / 91% at that point. Batch 2 later added a
13th row, `F-NAC-MAINT-010` (GA-vs-batata agent draft form wire), and
promoted `F-NAC-MAINT-008` to `🟡`; the recomputed totals are now
194 Nacos / 574 total / 91%.

| F-ID | Gap | Priority | Classification |
|------|-----|----------|----------------|
| F-NAC-MAINT-006 | MCP server version lifecycle (list/get/submit/publish/forcePublish/redraft/online/offline) | P1 | Pure SDK wrapping; ADMIN routes exist in `crates/batata-ai/src/api/mcp.rs` |
| F-NAC-MAINT-007 | `AiMaintainerService` delegates + Skill/Prompt/AgentSpec ZIP uploads | P1 | Pure SDK wrapping |
| F-NAC-MAINT-008 | `AgentMaintainerService` draft lifecycle + `agent_get(version)` | P1 | Pure SDK wrapping |
| F-NAC-MAINT-009 | `publishConfig(changeMessage)` + history DTO | P3 | **develop drift**, not GA |
| F-NAC-CSDK-001 | `NamingSelector` subscribe overload | P2 | True GA gap |
| F-NAC-CSDK-002 | `LockService::renew` / watchdog | P2 | GA; **both sides missing** — the server `RENEW` branch claimed by `F-NAC-LOCK-009` does not exist (see 8.2) |
| F-NAC-CSDK-003 | `AiService` sub-services + `AgentDiscoveryService` + RAD capability | P2 | GA |
| F-NAC-CSDK-004 | config 304 conditional GET (`localMd5`) | P3 | Client-side wire work |
| F-NAC-CSDK-005 | OIDC auth + `configSnapshotEnabled` | P3 | Client-side wire work |
| F-NAC-CSDK-006 | HTTP `requestId` correlation header | P3 | Client-side wire work |
| F-NAC-CSDK-007 | GA `SetupAckRequest.ability_table` RAD keys | **deferred** | Needs RAD gRPC handlers first |
| F-NAC-CSDK-008 | A2A 1.0 `securityRequirements` client DTOs | P3 | **develop drift**, not GA; server DTOs already carry the fields |

### 8.4 Registered test gaps

| ID | Gap | Evidence |
|----|-----|----------|
| TG-01 | Five upstream scenario families unported: `BinaryCompatibility`, `UpgradeMigration`, `RadRouting`, `JsonAdapter`, `AgentDiscovery` | 0 matching files in `sdk-tests/nacos-java-tests`; upstream post-GA adds `A2aRadRoutingJavaSdkITCase` (179 lines) and `AgentRequestBuilderJavaSdkITCase` (240 lines) |
| TG-02 | Three `batata-maintainer-client` traits (`CoreMaintainerService`, `ConfigMaintainerService`, `NamingMaintainerService`) have **no `impl ... for MaintainerClient`** anywhere | `crates/batata-maintainer-client/src/traits.rs:26,155,236`; only 3 object-safety tests at `:393-405` |
| TG-03 | ~~9 of 10 `BatataAiService` public methods have no direct test call~~ **CLOSED in batch 2**; ~42 other `batata-client` public methods still have no direct test call | New `crates/batata-client/tests/ai_service_test.rs` calls all 10 public methods (4 reads, 4 cache accessors, `clear_cache`, sizes) on both success and non-zero-code paths; the 6 inline AI tests still only cover DTO serialization |
| TG-04 | 14 `#[ignore]` cases in `crates/batata-client/tests/integration_test.rs` are not executable tests | project convention: ignored = does not exist |
| TG-05 | `apollo-java` OpenAPI `extendInfo` / `parentAppId` not represented in `batata-plugin-apollo` | separate crate, out of scope for these two SDKs; follow up independently |

Per project convention, any new guard or test must be mutant-checked
(removing it must turn the assertion red) and success paths must be tested.

### 8.5 Implementation batch 1 (2026-11-10)

Two registered gaps were closed in this pass; both are pure client-side
changes with **no wire-protocol change**:

| F-ID | Status change | What landed |
|------|---------------|-------------|
| F-NAC-MAINT-006 | `⚪` → `🟢` | `MaintainerClient::mcp_server_list_versions`, `mcp_server_get_version`, `mcp_server_submit_version`, `mcp_server_publish_version`, `mcp_server_force_publish_version`, `mcp_server_redraft_version`, `mcp_server_online_version`, `mcp_server_offline_version`; new `McpServerVersionInfo` DTO (typed common fields + `flatten` for the optional spec sections); 10 ADMIN path constants. Server routes in `crates/batata-ai/src/api/mcp.rs` were already present. |
| F-NAC-CSDK-008 | `⚪` → `🟡` | `AgentCardInfo.security_schemes` / `security` / `security_requirements` added with the **GA** map shape and `skip_serializing_if` emptiness rules, matching `crates/batata-common/src/model/ai/a2a.rs`. The post-GA `SecurityRequirement` wrapper and `AgentSkill.securityRequirements` remain unimplemented, so the row stays partial. |

Also corrected: `F-NAC-LOCK-009` `🟢` → `⚪` (false green, section 8.2), and
`F-NAC-CSDK-002`'s note now records that the server `RENEW` branch is missing
too. Recomputed totals: Nacos **193 F-IDs / 176 🟢 / 3 🟡 / 11 ⚪ / 3 ⛔ /
91%**, global **573 / 1040 T-ID rows / 91%**.

Tests: `crates/batata-maintainer-client/tests/client_test.rs` 111 → **115**
cases (list versions with query shape, empty-status omission, get version, and
a table-driven case covering all six transition paths);
`crates/batata-client/src/ai.rs` 359 → **361** inline cases (GA security wire
shape round-trip + empty-section omission). Both new guards were
mutant-checked: skipping the `version` query parameter in
`mcp_server_transition` turns the transition case red with a 404 (RC=101), and
`#[serde(skip)]` on `security_requirements` turns the wire-shape case red
(RC=101). Restored versions are green; `cargo check --workspace
--all-targets` and `cargo clippy -p batata-client -p
batata-maintainer-client --all-targets -- -D warnings` both pass.

**Deliberately not implemented in this batch**: the four GA `ability_table`
keys (`F-NAC-CSDK-007`, needs RAD gRPC handlers first), `LockService::renew`
and the server `RENEW` branch (`F-NAC-CSDK-002` / `F-NAC-LOCK-009`, needs a
Raft variant), `F-NAC-MAINT-007/008` delegates and draft lifecycle,
`F-NAC-CSDK-001/003/004/005/006`, `F-NAC-MAINT-009`, and test gaps TG-01..TG-05.
These are the next batches.

## 9. Implementation batch 2 (2026-11-10)

### 9.1 Source verification before changing anything

The target was the protocol-neutral GA `AgentMaintainerService`
(`F-NAC-MAINT-008`). Upstream `Constants.java:67` declares
`/v3/admin/ai/agents`; `AgentMaintainerServiceImpl.java` builds the six
version commands, `deleteDraft`, `getAgentVersion` and `listAgentVersions`
with `setParamValue`, and `AgentDraftCreateRequest` /
`AgentLabelsUpdateRequest` with the same form parameter map.
`crates/batata-ai/src/api/agent.rs::admin_routes()` registers all 19 of those
handlers and `crates/batata-server/src/api/v3/admin/ai/route.rs:15` mounts
them, so no new server route was needed.

A **real wire divergence** was found and registered rather than papered over.
Decompiling the GA jar
`~/.m2/repository/com/alibaba/nacos/nacos-maintainer-client/3.3.0/nacos-maintainer-client-3.3.0.jar`
shows `ClientHttpProxy.executeSync` routing POST to
`NacosRestTemplate.postForm` and PUT to `putForm` (GET/DELETE use `Query`);
`AgentMaintainerServiceImpl` and `McpMaintainerServiceImpl` both call
`setParamValue`. Batata's AI ADMIN handlers take `web::Query` only —
e.g. `crates/batata-ai/src/api/mcp.rs:701` (`submit_version`) and
`agent.rs:512` (`submit_agent`) — and `agent.rs:432,459,668` additionally
require `web::Json<AgentCard>` / `web::Json<AgentLabelsForm>` bodies. A GA
Java maintainer client therefore gets 400/415 on those POST/PUT endpoints.
This is `F-NAC-MAINT-010`, a **server-side** gap. It is not caught by the GA
runtime matrix because `maintainer-sdk-test` covers config/naming only.

The new Rust SDK methods deliberately follow the batata server wire (query
for transitions, JSON bodies for draft/labels). Spring `@RequestParam`
accepts query parameters as well, so the same methods also work against a
real Nacos server. That is why the SDK can land while the server-side
divergence stays registered as a separate `⚪` row.

### 9.2 What landed

| F-ID / TG | Change | Files |
|-----------|--------|-------|
| `F-NAC-MAINT-008` `⚪` → `🟡` | 15 `agents_*` methods: `agents_get` (with optional `version`), `agents_list`, `agents_list_versions`, `agents_get_version`, create/update/delete draft, six version transitions via one shared helper, `agents_update_labels`, `agents_update_scope` | `crates/batata-maintainer-client/src/client.rs` |
| `F-NAC-MAINT-008` | 13 ADMIN `/v3/admin/ai/agents*` path constants; `AgentVersionInfo` DTO (typed shared fields + `#[serde(flatten)] extra`) | `src/constants.rs`, `src/model/ai.rs`, `src/model/mod.rs` |
| support | `BatataHttpClient::put_json_with_query` — the AI admin endpoints combine `@RequestParam`s with an `@RequestBody`, which neither `put_form` nor `put_json` can express | `crates/batata-client/src/http.rs` |
| `F-NAC-MAINT-010` | new `⚪` row for the AI ADMIN POST/PUT form-vs-query channel divergence (GA bytecode evidence) | `docs/compat/nacos/features.md` |
| TG-03 **CLOSED** | all 10 `BatataAiService` public methods covered by direct wiremock tests (constructor, 4 reads, 2 cache getters, `clear_cache`, 2 size accessors), success paths and non-zero-code error paths | `crates/batata-client/tests/ai_service_test.rs` |
| tests | 7 wiremock cases for the agent facade; `client_test.rs` 115 → **122** | `crates/batata-maintainer-client/tests/client_test.rs` |

`F-NAC-MAINT-008` stays `🟡`, not `🟢`: `createDraft`/`updateDraft`/
`updateLabels` use batata's JSON wire, `listAgentVersions` returns a plain
array on batata instead of GA's `Page`, and the GA facade's
`updateAgent`/`deleteAgent`/`getRuntimeEndpoints` are not wrapped.

Also corrected a stale claim in §8.3: `F-NAC-CSDK-002` said the server
`RENEW` contract was implemented, contradicting §8.2's verified false-green
finding. It now says both sides are missing.

### 9.3 Mutant evidence

Four guards were removed one at a time and each turned its test red; all were
restored and re-run green:

| Mutant | Result |
|--------|--------|
| `agent_version_transition` uses `post_form` instead of `post_with_query` | `test_agents_version_transitions` 404, FAILED |
| `agents_update_labels` sends `{}` instead of `LabelsBody` | `test_agents_update_labels_and_scope` 404, FAILED |
| `get_mcp_server` no longer inserts into `mcp_cache` | `test_ai_service_read_paths_and_cache` `0 != 1`, FAILED |
| `get_mcp_server` ignores `resp.code == 0` | `test_ai_service_error_paths` `expect_err` FAILED |

A fifth docs guard was checked by reverting `F-NAC-MAINT-008` to `⚪`:
`scripts/compat_check.py` reports three mismatches —
`features.md` Summary **Total**, README Nacos row and README global Total
row (RC=1). Restored version is RC=0. The checker enforces the **Total**
row, not the per-module rows, so B.8 must still be updated by hand.

### 9.4 Validation

`python3 scripts/compat_check.py` RC=0 — **574 feature rows / 1040 T-ID
rows**, Nacos **194 F-IDs / 176 🟢 / 4 🟡 / 11 ⚪ / 3 ⛔ / 91%**, global
**574 / 91%**, `nacos 176/176 anchored`, alignment target 3.3.0 GA.
`cargo fmt --all -- --check`, `py_compile`, `git diff --check`,
`cargo check --workspace --all-targets`,
`cargo clippy -p batata-client -p batata-maintainer-client --all-targets --
-D warnings`, `cargo test -p batata-client --lib` (**361**),
`--test ai_service_test` (**2**), `cargo test -p batata-maintainer-client
--test client_test` (**122**) and `cargo check --tests` in all three Rust
`sdk-tests` workspaces are green.

Release-prep impact: `batata-client` gained `wiremock` as a dev-dependency and
one test file, so `cargo package -p batata-client --no-verify` now packs
**58 files** (was 57); `batata-maintainer-client` stays at 26. The packaged
client source tree was extracted and `cargo check --all-targets` passed with a
local `batata-api` patch, so the published tree still builds. Re-run the full
release recipe in `docs/development.md` before publishing.

No full `--full` regression gate was run: this batch changes only the two
client SDK crates and documentation, not server behaviour or any wire
contract. No commit, push, publish, yank, archive or tag was performed.

### 9.5 Remaining backlog

`F-NAC-MAINT-007` (`AiMaintainerService` delegates + ZIP uploads),
`F-NAC-MAINT-009`, `F-NAC-MAINT-010` (server form wire),
`F-NAC-CSDK-001/002/003/004/005/006/007`, `F-NAC-LOCK-009` (server `RENEW`
+ Raft variant + client watchdog), and test gaps TG-01, TG-02, TG-04, TG-05.

## 10. Implementation batch 3 (2026-11-10) — `F-NAC-MAINT-007`

### 10.1 Source verification before changing anything

The GA baseline was taken from `~/.m2/repository/com/alibaba/nacos/nacos-maintainer-client/3.3.0/nacos-maintainer-client-3.3.0.jar`
(bytecode, not the post-GA local develop snapshot). `javap` confirms
`AiMaintainerService` declares `skill()`, `agentSpec()`, `agent()`, `mcp()`,
`a2a()`, `prompt()` and `pipeline()`; the four sub-service interfaces expose
roughly 90 methods once default overloads are counted. Extracting the path
constants from `SkillMaintainerServiceImpl`, `PromptMaintainerServiceImpl`,
`AgentSpecMaintainerServiceImpl` and `PipelineMaintainerServiceImpl` yields
`/v3/admin/ai/skills*`, `/v3/admin/ai/prompt*`,
`/v3/admin/ai/agentspecs*` and `/v3/admin/ai/pipelines*`.

`crates/batata-server/src/api/v3/admin/ai/route.rs:13-20` already mounts all
four `*_admin_routes()` scopes; the route tables in
`crates/batata-ai/src/api/{skill,prompt,agentspec,pipeline}.rs` cover every
path used below. No new server route was added.

Three wire facts determined the scope:

1. **GET/DELETE are query-only on both sides.** Batata handlers use
   `web::Query<SkillForm>` / `PromptQueryForm` / `AgentSpecForm` /
   `PipelineListForm`; GA `ClientHttpProxy` builds a `Query` for GET/DELETE.
   Field names (`namespaceId`, `skillName`, `promptKey`, `agentSpecName`,
   `version`, `pageNo`, `pageSize`, ...) agree.
2. **ZIP endpoints are multipart on both sides.** GA
   `ClientHttpProxy.executeMultipartPost` writes a part named `file`
   (`SkillMaintainerServiceImpl` bytecode: `// String file`); batata's
   `read_zip_field` reads exactly that part and keeps `namespaceId` /
   `overwrite` in the query. Batata's download endpoints return raw ZIP bytes,
   not a JSON envelope.
3. **POST/PUT lifecycle endpoints are still divergent** (`F-NAC-MAINT-010`):
   GA sends form bodies, batata reads query parameters. They were deliberately
   left out of this batch rather than wrapped against a wire that a GA Java
   client cannot use.

### 10.2 What landed

| Item | Change | Files |
|------|--------|-------|
| `F-NAC-MAINT-007` `⚪` → `🟡` | 25 flat methods: 8 `skill_*`, 8 `prompt_*`, 7 `agentspec_*`, 2 `pipeline_*`. Java delegate objects are represented as flat methods, consistent with the batch 1 `mcp_server_*` and batch 2 `agents_*` facades. | `crates/batata-maintainer-client/src/client.rs` |
| paths | 23 ADMIN constants for skills / prompt / agentspecs / pipelines | `crates/batata-maintainer-client/src/constants.rs` |
| HTTP helpers | `get_bytes_with_query` (raw ZIP + identity query) and `post_multipart_with_query` (multipart `file` part + query), both with the existing auth/failover behaviour | `crates/batata-client/src/http.rs` |
| tests | 6 wiremock cases; `client_test.rs` 122 → **128**. `ZipFilePart` is a custom `Match` that inspects the raw body for `name="file"` and the exact ZIP bytes. | `crates/batata-maintainer-client/tests/client_test.rs` |
| docs | status + summary (`🟡` 4→5, `⚪` 11→10 in Nacos; global `🟡` 9→10, `⚪` 38→37), `F-NAC-MAINT-004` note refreshed, batch 3 paragraph, this section | `docs/compat/nacos/features.md`, `docs/compat/README.md` |

Return values are `serde_json::Value` / `Page<Value>` / `Vec<u8>` / `String` /
`bool`. That is why the row stays `🟡`: typed `SkillMeta`, `PromptMetaInfo`,
`AgentSpec`, `PipelineExecution` DTOs are not modelled yet. Also still open:
the POST/PUT lifecycle, `getAgentSpecAdminDetail`,
`listAgentSpecAdminItems`, prompt label bind/unbind, `publishPrompt`,
`updatePromptMetadata`, GA-shaped sub-service objects, and the MCP
`deleteMcpServerDraft` / labels / status / scope wrappers noted in §8.

### 10.3 Mutant evidence

Six guards were removed one at a time; each turned its test red and all were
restored:

| Mutant | Result |
|--------|--------|
| multipart part renamed `file` → `archive` | `test_skill_zip_uploads` FAILED |
| `post_multipart_with_query` drops `.query(query)` | `test_skill_zip_uploads` FAILED (404) |
| `get_bytes_with_query` drops `.query(query)` | `test_skill_reads_and_download` FAILED (404) |
| `SkillIdentityQuery.version` becomes `#[serde(skip)]` | `test_skill_reads_and_download` FAILED |
| `pipeline_get_detail` uses the list path | `test_skill_delete_and_pipeline_reads` FAILED |
| shared `prompt_get` ignores its `path` argument | `test_prompt_reads_and_delete` FAILED |

`test_ai_resource_error_paths` independently asserts that a 500 becomes an
`Err` for JSON reads, byte downloads and multipart uploads.

### 10.4 Validation and release impact

Validation commands: `python3 scripts/compat_check.py`, `cargo fmt --all --
--check`, `python3 -m py_compile scripts/compat_check.py`, `git diff --check`,
`cargo check --workspace --all-targets`, `cargo clippy -p batata-client -p
batata-maintainer-client --all-targets -- -D warnings`, `cargo test -p
batata-client --lib`, `cargo test -p batata-client --test ai_service_test`,
`cargo test -p batata-maintainer-client --test client_test`, `cargo check
--tests` in all Rust `sdk-tests` workspaces, and `cargo package
--no-verify` for both SDK crates with local registry patches. No `--full`
regression gate was run: this batch changes only the two client SDK crates and
documentation, with no server behaviour or wire-contract change. No commit,
push, publish, yank, archive or tag was performed in this batch.

All checks passed: `compat_check.py` RC=0 (**574 F-ID / 1040 T-ID rows**,
Nacos **194 = 176 🟢 / 5 🟡 / 10 ⚪ / 3 ⛔ / 91%**, global **574 / 91%**,
`nacos 176/176 anchored`, target 3.3.0 GA); `cargo test -p batata-client
--lib` (**361**), `--test ai_service_test` (**2**),
`-p batata-maintainer-client --test client_test` (**128**); clippy
`-D warnings`, `cargo check --workspace --all-targets` and `cargo check
--tests` in all three Rust `sdk-tests` workspaces are green. Both packaged
source trees were extracted and `cargo check --all-targets` passed with local
registry patches.

The package file counts stay **20 / 58 / 26**: batch 3 only edits existing
files in `batata-client` and `batata-maintainer-client`. The full release
recipe in `docs/development.md` still has to be re-run before publishing.

### 10.5 Remaining backlog

`F-NAC-MAINT-009`, `F-NAC-MAINT-010` (server form wire), the unfinished half
of `F-NAC-MAINT-007`, `F-NAC-CSDK-001/002/003/004/005/006/007`,
`F-NAC-LOCK-009` (server `RENEW` + Raft variant + client watchdog), and test
gaps TG-01, TG-02, TG-04, TG-05.

## 11. Implementation batch 4 (2026-11-10) — `F-NAC-MAINT-010`

### 11.1 Source verification and scope correction

GA evidence is the released bytecode in
`~/.m2/repository/com/alibaba/nacos/nacos-maintainer-client/3.3.0/nacos-maintainer-client-3.3.0.jar`.
`javap -c -p` on `ClientHttpProxy.executeSync` shows POST → `postForm`, PUT →
`putForm`, GET/DELETE → query. `AgentMaintainerServiceImpl.executeFormRequest`
and the equivalent MCP service fill a `setParamValue` map, so every
`namespaceId` / `mcpName` / `agentName` / `version` / `scope` / `labels`
arrives in an `application/x-www-form-urlencoded` body. The GA `-sources.jar`
was not present locally, so GA field names were cross-checked against the
local upstream `develop` snapshot (the declared 3.3.0 GA target is unchanged).
Spring `@RequestParam` binds query and form bodies alike, which is why
batata's Rust SDK (query) and the GA Java client (form) both work upstream.

Re-reading batata's handlers **narrowed** the batch 2 registration, which had
been too broad:

| Surface | Pre-batch-4 batata extractor | Affected by the gap? |
|---------|------------------------------|----------------------|
| `crates/batata-ai/src/api/skill.rs`, `prompt.rs`, `agentspec.rs` POST/PUT | `web::Form<T>` | No — already GA-compatible |
| ZIP upload endpoints | query identity + `Multipart` `file` part | No — multipart is a separate contract |
| MCP create/draft specifications | `parse_registration` (form JSON string or JSON body) | No |
| 8 MCP + 7 Agent POST/PUT handlers | `web::Query<T>` only | **Yes** — 400 "is required" for GA |
| MCP `create_draft`, MCP/Agent `labels` | query-only params, JSON-only labels | **Yes** |
| Agent `create_agent` / `update_agent` / `create_draft` / `update_draft` | `web::Json<AgentCard>` | **Yes, but a model gap, not a channel gap** |

### 11.2 What landed

- New `crates/batata-ai/src/api/admin_form.rs`:
  - `AdminParams::from_request(req, body)` parses an urlencoded form or a JSON
    object body (nested values flattened to their JSON text), then the query
    string; **query wins** on duplicate keys, matching
    `crates/batata-naming/src/api/v3/admin/form.rs::merge_form`.
    `multipart/form-data` is skipped so binary ZIP payloads can never inject
    parameters. Malformed bodies are ignored rather than rejected, leaving the
    handler's own structured 400 in place.
  - `decode<T>()` re-encodes the flat map through `serde_urlencoded`, keeping
    `Option<bool>`, `Option<u64>` and JSON-string fields working exactly as
    with `web::Query<T>`.
  - `labels(body)` accepts the GA `labels` JSON-string form parameter, the
    console's `{"labels": {...}}` object and a bare JSON map.
  - `AdminForm<T>` is a drop-in `FromRequest` replacement with `Deref` and
    `into_inner()`, so handler bodies are unchanged. It consumes the payload
    once and must not be combined with another body extractor.
- `crates/batata-ai/src/api/mcp.rs`: 8 handlers switched from `web::Query` to
  `AdminForm`; `create_draft` and `labels` now resolve `web::Bytes` through
  `AdminParams`. The dead `parse_labels` helper and `McpLabelsQuery.labels`
  were removed; their three unit tests were migrated to `AdminParams`.
- `crates/batata-ai/src/api/agent.rs`: 7 handlers switched to `AdminForm`;
  `labels` now resolves `web::Bytes` through `AdminParams`; the local
  `AgentLabelsForm` was removed.
- `crates/batata-ai/src/api/mod.rs`: registers `pub(crate) mod admin_form`.
- New `crates/batata-server/tests/ai_admin_form_encoding.rs`: mounts the real
  MCP and agent admin scopes under `/v3/admin/ai` (same as the production
  route table) on the embedded `TestStore`, and asserts form-body and
  query-string responses are byte-identical per endpoint. Real success paths
  (draft creation, submit/publish, status, scope, labels, scope) prove the
  equivalence is not two identical failures. A never-existing
  `MISSING_VERSION = "9.9.9"` makes the remaining transitions deterministic
  without state mutation between the two calls.

The four `AgentCard` handlers are deliberately untouched: GA sends flat draft
metadata form fields while batata requires a nested `AgentCard` JSON body.
That is a DTO/semantic difference, so `F-NAC-MAINT-010` stays `🟡`.

### 11.3 Mutant evidence

A throwaway harness backed up and restored each source file in a `finally`
block. A mutant counts as **KILLED** only when the code still compiles and a
test assertion turns red; a compile break is rejected as an invalid mutant.
Final result: **11 / 11 KILLED**.

| Mutant | Guard it attacks | Result |
|--------|------------------|--------|
| M1 drop urlencoded/JSON body params | GA form channel | unit + route FAILED |
| M2 drop query params | batata SDK channel | unit + route FAILED |
| M3 query precedence reversed (body wins) | `merge_form` parity | unit + route FAILED |
| M4 ignore JSON object bodies | console channel | unit + route FAILED |
| M5 ignore the GA `labels` form parameter | labels resolver | unit + route FAILED |
| M6 remove the multipart exclusion | multipart injection guard | unit FAILED |
| M7 MCP `submit_version` back to `web::Query` | MCP wiring | route FAILED |
| M8 Agent `submit_agent` back to `web::Query` | agent wiring | route FAILED |
| M9 ignore the form body in MCP `create_draft` | draft parameter wiring | route FAILED |
| M10 drop resolved labels in the MCP handler | MCP labels wiring | route FAILED |
| M11 drop resolved labels in the Agent handler | agent labels wiring | route FAILED |

The first pass killed only 7/11. The four survivors exposed real guard
weaknesses and were fixed rather than suppressed:

- **M3**: `query_wins_and_body_fills_the_gaps` had no duplicate key, so it
  tested merging, not precedence. A body-side `agentName=from-body` was added.
- **M9**: `create_draft`'s `namespaceId` defaulted to the same value as the
  specification, so dropping the body was unobservable. New route test
  `mcp_create_draft_reads_ga_form_parameters` asserts `overwrite=false` is
  rejected with the domain error and `overwrite=true` succeeds with the draft
  still persisted.
- **M10/M11**: the original mutations were *equivalent* — `labels()` reads
  the merged `labels` parameter first, so passing an empty body changes
  nothing. They were redesigned to drop the resolved labels entirely; the
  agent labels test now asserts the `stable` label is echoed, not just 200.
- The multipart unit test gained a realistic body containing
  `namespaceId=attacker` so removing the exclusion cannot survive by parsing
  binary bytes as an unparseable form.

**Documentation guard (D1–D7)**: reverting the `F-NAC-MAINT-010` status, the
B.8 Summary row, the `features.md` Total row or either README global row must
make `scripts/compat_check.py` return 1. The first pass killed only 4/5: D2 (a
stale per-module Summary row) survived because check 5 recomputed just the
`**Total**` row from F-ID rows. `scripts/compat_check.py` now has **check 5b**
— per-module Summary rows must add up to the Total row — verified clean on all
three inventories. Two extra mutants (a stale Consul `Agent` row and a stale
Apollo `13 legacy webapi` row) confirm the guard is generic, not Nacos-specific.
Final docs result: **7 / 7 KILLED**, restored baseline RC=0. The checker's
docstring and `docs/compat/README.md` were updated to describe check 5b.

### 11.4 Validation

`python3 scripts/compat_check.py` RC=0 (**574 F-ID / 1040 T-ID rows**, Nacos
**194 = 176 🟢 / 6 🟡 / 9 ⚪ / 3 ⛔ / 91%**, global **574 / 91%**,
`nacos 176/176 anchored`, target 3.3.0 GA); `cargo fmt --all -- --check`,
`python3 -m py_compile scripts/compat_check.py` and `git diff --check` RC=0;
`cargo check --workspace --all-targets` RC=0; `cargo clippy -p batata-ai -p
batata-server -p batata-client -p batata-maintainer-client --all-targets --
-D warnings` RC=0.

Targeted tests: `cargo test -p batata-ai --lib` **181**; `cargo test -p
batata-ai --tests` (11 targets, all green); `cargo test -p batata-server
--test ai_admin_form_encoding --test mcp_admin_routes --test
agent_admin_routes --test admin_naming_form_encoding` **7 / 7 / 3 / 5**;
`cargo test -p batata-maintainer-client --tests` **37 + 128**;
`cargo test -p batata-client --tests` **361 + 2 + 11 + 25** (14 upstream E2E
cases remain `#[ignore]`). `cargo check --tests` in all three independent
`sdk-tests` workspaces returned RC=0.

No `--full` regression gate was run: the standing rule is to run it only on
explicit request, and batch 4 changes only AI ADMIN request extraction. The
3.3.0 GA runtime matrix does not exercise AI ADMIN writes at all
(`maintainer-sdk-test` covers config/naming), so the new route-level guards
are the only coverage for this channel. No commit, push, publish, yank,
archive or tag was performed in this batch.

### 11.5 Remaining backlog

The flat-GA-form ↔ nested `AgentCard` **parameter** mapping was completed in
batch 5 (section 12), closing `F-NAC-MAINT-010`; the remaining ARD **model**
semantics moved to `F-NAC-AISRV-001`. Also open: `F-NAC-MAINT-009`, the
unfinished half of `F-NAC-MAINT-007` (POST/PUT lifecycle, typed AI resource
DTOs, GA-shaped sub-service objects, `getAgentSpecAdminDetail` /
`listAgentSpecAdminItems`, prompt label bind/unbind, `publishPrompt`,
`updatePromptMetadata`), `F-NAC-CSDK-001/002/003/004/005/006/007`,
`F-NAC-LOCK-009` (server `RENEW` + Raft variant + client watchdog), the RAD
ability keys and their gRPC handlers, and test gaps TG-01, TG-02, TG-04,
TG-05.

## 12. Server AI ADMIN alignment audit + implementation batch 5 (2026-11-10)

### 12.1 Question: does the **server** also need alignment?

**Yes — but far less than the client SDK backlog suggested.** Batches 1–4 were
mostly SDK work, so the GA AI ADMIN controller surface was re-audited
endpoint-by-endpoint. Source of truth is the GA release commit `b64bc9118`
(`ai/src/main/java/com/alibaba/nacos/ai/controller/`);
`git diff --stat b64bc9118 HEAD -- ai/src/main/java/com/alibaba/nacos/ai/controller/`
is **empty**, so the local post-GA `develop` snapshot `d1b97bc3e` has no AI
controller drift. Batata counts come from each `admin_routes()` scope.

| GA ADMIN controller | Path | GA | batata | Verdict |
|---------------------|------|----|--------|---------|
| `McpAdminController` | `/v3/admin/ai/mcp` | 19 | 19 | paths aligned; batch 4 fixed the parameter channel |
| `AgentAdminController` | `/v3/admin/ai/agents` | 18 | 19 | paths aligned (batata adds `POST ""`), **model diverges** |
| `SkillAdminController` | `/v3/admin/ai/skills` | 20 | 20 | aligned; already uses `web::Form` |
| `PromptAdminController` | `/v3/admin/ai/prompt` | 24 | 24 | aligned; already uses `web::Form` |
| `AgentSpecAdminController` | `/v3/admin/ai/agentspecs` | 18 | 19 | aligned (batata adds `/version/download`) |
| `A2aAdminController` | `/v3/admin/ai/a2a` | 6 | 8 | legacy facade exists in `crates/batata-server/src/api/v3/admin/ai/a2a.rs` |
| `PipelineAdminController` | `/v3/admin/ai/pipelines` | 4 | 2 (+2 deprecated) | **corrected in batch 6**: the two missing reads are 410-gated deprecated endpoints |
| `AiResourceImportAdminController` | `/v3/admin/ai/import` | 4 | 4 | **corrected in batch 6**: implemented in `batata-console`, not `batata-ai` |

**Consequence**: the "remaining half of `F-NAC-MAINT-007`" (POST/PUT resource
lifecycle, labels, biz-tags, scope, online/offline) is a **client SDK facade**
gap, not a server gap — all of those endpoints are already mounted and accept
`web::Form`. Three genuine server gaps are registered as
`F-NAC-AISRV-001..003` in the new `features.md` section B.10.

> **Correction (batch 6, 2026-11-10)**: the counts above for the import and
> pipeline controllers were wrong. The batch 5 grep was limited to
> `crates/batata-ai`, while the importer lives in `crates/batata-console/src/v3/
> ai_import.rs` (4/4 endpoints, 18 route tests, mounted from
> `crates/batata-server/src/api/v3/admin/ai/route.rs`). The two pipeline
> reads are deprecated and disabled by default upstream. Consequently only
> `F-NAC-AISRV-001` is a genuine server gap; see §13.

### 12.2 Source evidence for the Agent model divergence

- GA `AgentMaintainerServiceImpl` (`b64bc9118`) sends `createDraft` as a form
  with `namespaceId`, `agentName`, `version`, `displayName`, `description`,
  `iconUrl`, JSON-encoded `provider` / `tags` / `extensions` /
  `callInterfaces`, `author`, `changeDescription`, `basedOnVersion`;
  `updateAgent` additionally sends `status` and **no** `version`.
- GA `AbstractAgentDraftForm.fillRequest` parses the JSON-valued fields;
  `AgentAdminForm.validate` requires `agentName`, `AgentVersionForm.validate`
  adds `version` for the draft endpoints.
- GA `AgentProvider` is `{name, url}` while batata's A2A
  `AgentCard.provider` is `{organization, url}`. GA `AgentCallInterface`
  carries `protocol`, `protocolVersion`, `descriptorMediaType`,
  `nativeDescriptor`, `endpointSourceOrder` and `endpointSets` — none of which
  has an `AgentCard` counterpart.
- GA `AgentAdminController.createDraft` documents that it "may initialize
  Agent metadata when the Agent is absent". Batata's
  `crates/batata-ai/src/service/a2a_service.rs:884` (`create_agent_draft`)
  calls `existing_resource` and fails with "agent not found"; the new route
  test reproduced this as HTTP 500 before the fixture was changed to register
  the Agent first. This is now part of `F-NAC-AISRV-001`, not a batch-5 fix.

### 12.3 What landed (server-side, batch 5)

`crates/batata-ai/src/api/agent.rs`:

- `AgentWriteForm` models the GA flat admin parameters, `GaAgentProvider`
  handles the `name`/`url` shape, and `AgentWrite` is wire-independent.
- `resolve_agent_write(req, body, require_version)` uses the batch-4
  `AdminParams` merge (urlencoded form / JSON object / query, query wins) and
  chooses between batata's nested `AgentCard` JSON body (`is_json_card`) and
  the flat GA form. All four write handlers (`POST`/`PUT /agents`,
  `POST`/`PUT /agents/draft`) now take `web::Bytes` instead of
  `web::Json<AgentCard>`, so a GA Java client no longer gets 415.
- `agent_card_from_form` maps `agentName` → `name`, `version`,
  `displayName`, `description`, `iconUrl`, JSON `provider` →
  `AgentProvider.organization/url`, JSON `tags` → `Vec<String>`, JSON
  `extensions` → `AgentCard.metadata` (non-string values JSON-encoded), and
  preserves `author` / `changeDescription` / `basedOnVersion` / `status` as
  batata-internal `ga.*` metadata keys. Draft endpoints require `version`;
  `PUT /agents` deliberately does not.
- ARD `callInterfaces` is **rejected fail-closed** with a structured 400
  rather than silently dropped; accepting it would register an agent with no
  usable endpoint. `[]` / `null` are accepted.
- `crates/batata-ai/src/api/admin_form.rs` doc comment now points at the
  separate ARD model gap `F-NAC-AISRV-001`.

`crates/batata-server/tests/ai_admin_form_encoding.rs`: 7 new route tests
(7 → 14) with a real MCP + Agent admin scope and embedded `TestStore`:
mapped-field persistence, nested JSON wire preserved, `PUT /draft` update,
`PUT /agents` without version, `callInterfaces` fail-closed (and not
persisted), identity/version validation + query precedence, and form
`overwrite=false`/`true`. Six unit tests were added to `agent.rs`.

### 12.4 Mutant evidence

`/tmp/batata_mutants_b5.py` — **16/16 KILLED**, all sources restored and both
suites green afterwards. A "kill" requires the mutation to still compile and
make an assertion fail; compile failures are reported separately.

| Mutant | Guard killed |
|--------|--------------|
| M1/M2 `is_json_card` always false / true | dual-wire detection (unit) |
| M3 ignore the form body in `resolve_agent_write` | GA form parameters (unit) |
| M4/M5/M6 drop `displayName` / `provider.name` / `tags` | field mapping (unit) |
| M7/M8 drop `extensions` / `ga.*` metadata | preservation (unit) |
| M9 remove `callInterfaces` fail-closed guard | ARD rejection (unit) |
| M10 remove the version requirement | draft validation (unit) |
| M11 drop `iconUrl` | optional mapping (unit) |
| M12 ignore `overwrite` | route `agent_flat_form_overwrite_is_honoured` |
| M13/M14 draft handlers stop requiring a version | route validation tests |
| M15 query stops winning over the form body | shared `AdminParams` precedence (unit) |
| M16 `json_field` always returns `None` | all JSON-valued fields (unit) |

`/tmp/batata_docs_mutants_b5.py` — **10/10 KILLED**: reverting the
`F-NAC-MAINT-010` status, B.8/B.10 Summary rows, features Total, either README
global row, the README Layout F-ID count, or falsely promoting `AISRV-001` to
`🟢` / `AISRV-003` to `⛔` all make `scripts/compat_check.py` return 1. A
checker-on-checker mutant (disabling check 11's regex) also turns the baseline
red. The first docs run exposed a **new false-green class**: the README Layout
block is prose, so checks 5/6 never saw its `(NNN F-IDs)` annotation. Check 11
now compares those counts with the recomputed inventories; `README.md` and the
checker docstring were updated accordingly.

### 12.5 Validation

`python3 scripts/compat_check.py` RC=0: **577 F-ID / 1040 T-ID rows**, Nacos
**197 = 177 🟢 / 5 🟡 / 12 ⚪ / 3 ⛔ / 90%**, global **577 / 91%**,
`nacos 177/177 anchored`, target 3.3.0 GA. `cargo fmt --all -- --check`,
`python3 -m py_compile scripts/compat_check.py`, `git diff --check`,
`cargo check --workspace --all-targets` and clippy (`-D warnings` on
`batata-ai`, `batata-server`, `batata-client`, `batata-maintainer-client`) all
RC=0. One clippy lint (`useless use of format!` in the new route test) was
fixed before the final run.

Tests: `cargo test -p batata-ai --lib` **187**; `cargo test -p batata-ai
--tests` 7 targets green; `cargo test -p batata-server --test
ai_admin_form_encoding --test mcp_admin_routes --test agent_admin_routes
--test admin_naming_form_encoding` **14 / 7 / 3 / 5**; `cargo test -p
batata-maintainer-client --tests` **37 + 128**; `cargo test -p batata-client
--tests` **361 + 2 + 11 + 25** (14 upstream E2E cases remain `#[ignore]`);
`cargo check --tests` in all three independent `sdk-tests` workspaces RC=0.

No `--full` regression gate was run (standing rule: explicit request only).
The 3.3.0 GA runtime matrix does not exercise AI ADMIN writes, so these
route-level guards are the only coverage of the new channel. No commit, push,
publish, yank, archive or tag was performed.

### 12.6 Remaining backlog

`F-NAC-AISRV-001` (typed ARD DTOs/storage, draft-created metadata, `Page`
responses, RAD runtime endpoints), `F-NAC-AISRV-002` (import controller),
`F-NAC-AISRV-003` (two pipeline reads), the client-side
`F-NAC-MAINT-007` write facade + typed AI resource DTOs,
`F-NAC-MAINT-009`, `F-NAC-CSDK-001/002/003/004/005/006/007`,
`F-NAC-LOCK-009` (server `RENEW` + Raft variant + client watchdog), the RAD
ability keys/handlers, and test gaps TG-01, TG-02, TG-04, TG-05.

## 13. Batch 6 — server audit correction + first-draft Agent initialization (2026-11-10)

### 13.1 The batch 5 server audit was too narrow

Re-reading the whole workspace (not just `crates/batata-ai`) showed two of the
three `F-NAC-AISRV-*` rows were false gaps:

| Row | Batch 5 claim | Source-verified reality |
|-----|---------------|-------------------------|
| `F-NAC-AISRV-002` | `/v3/admin/ai/import` absent (0/4) | `crates/batata-console/src/v3/ai_import.rs::admin_routes()` mounts all four GA endpoints (`/sources`, `/search`, `/validate`, `/execute`); `crates/batata-server/src/api/v3/admin/ai/route.rs` registers it under `/v3/admin/ai`, and `console_routes()` mounts `/v3/console/ai/import`. `crates/batata-server/tests/ai_import_routes.rs` has **18** route tests. The capability was already `F-NAC-ADM-AI-008` (B.7.5). → `⚪` → `🟢` |
| `F-NAC-AISRV-003` | two pipeline reads missing (2/4) | GA `PipelineAdminController.java` marks `GET /{pipelineId}` and the parameterless `GET` `@Deprecated(since = "3.2.1", forRemoval = true)`. Both call `CompatibilityHelper.check`, which throws HTTP **410** with `ErrorCode.API_DEPRECATED` (40000) unless `nacos.core.api.compatibility.enabled=true`; `EnvUtil.getProperty(..., false)` makes that **off by default** (`core/.../CompatibilityHelper.java:29-46`). The two canonical reads are already 🟢 and batata's `pipeline.rs` header documents the deliberate omission. → `⚪` → `⛔` (won't support) |

GA `AiResourceImportProperties` defaults `enabled=true` but the GA distribution
ships **no** `AiResourceImportServiceBuilder` implementations, and the GA
maintainer-client has **no** import methods — this is a console/plugin-facing
surface, which batata already covers. Implementing either false gap would have
duplicated an existing controller or added endpoints upstream refuses to serve.

`F-NAC-AISRV-001` is therefore the only genuine server-side AI gap. The GA
source also refined one of its sub-gaps: `createValidatedDraft`
(`b64bc9118:368-378`) calls `createInitialDraft(toInitialAgent(...), draft)`
when `findMeta` returns null, but `requireInitialDraftContent`
(`b64bc9118:1171-1177`) requires a non-null `callInterfaces` for that first
draft. Batch 5 rejects non-empty `callInterfaces` fail-closed, so full GA Java
first-draft compatibility still needs typed ARD DTOs.

### 13.2 What landed (server-side)

`crates/batata-ai/src/service/a2a_service.rs`:

- `resource_parts` factors the existing resource-row splitting so both the
  existing and the newly created row go through the same decoder.
- `create_initial_draft_resource(namespace, card, "manual")` inserts the
  `ai_resource` metadata row with `meta_status::ENABLE`, `scope::PRIVATE`, the
  card description, a UUID agent id and `manual` registration type in `ext`,
  and a dual-readable `version_info` (`AgentCardVersionInfo` for the legacy A2A
  reader, empty `ResourceVersionInfo` for the lifecycle reader). It re-reads the
  row so the CAS uses the persisted `meta_version`.
- `create_agent_draft` now does find-or-create instead of
  `existing_resource(...)` (which failed with "Agent not found"). This mirrors
  both GA `createValidatedDraft` and the existing
  `mcp_service.rs::create_initial_draft_resource`.

`crates/batata-server/tests/ai_admin_form_encoding.rs` gained 3 route tests
(14 → **17**): first GA flat-form draft with no pre-registered Agent (asserts
the metadata row, `ext`, editing-version index, stored card id and
registration type), first nested-JSON draft, and `overwrite=false`/`true` on an
initialized Agent. The stale macro comment about "batata requires an existing
Agent" was corrected.

### 13.3 Mutant evidence

`/tmp/batata_mutants_b6.py` — **9/9 KILLED**, all sources restored and the
suite green afterwards. A kill requires the mutation to still compile and make
an assertion fail; compile failures are reported separately.

| Mutant | Guard that failed |
|--------|-------------------|
| M1 first draft no longer initializes metadata | all three new route tests |
| M2 initialized resource has no `ext` | id / registration-type assertions |
| M3 initialized resource is `disable` | resource status assertion |
| M4 initialized resource is `PUBLIC` | resource scope assertion |
| M5 initialized resource drops the description | resource description assertion |
| M6 first draft uses an empty registration type | `ext` / stored-card assertions |
| M7 initial version index claims an editing version | first draft returns a domain error |
| M8 initialized resource has an empty agent id | stored card `id` assertion |
| M9 existing-resource branch stops reading the persisted index | batch 5 existing-resource tests |

Documentation mutants (`/tmp/batata_docs_mutants_b6.py`) — **9/9 KILLED**:
reverting either B.10 status, falsely promoting `AISRV-001` to `🟢`, reverting
the B.10 / features Total / README global / README Layout rows, or reverting
the checker's new status-cell parsing all make `scripts/compat_check.py` return
1. The checker fix matters: `row_status` used to scan the whole line, so the
`🟢` quoted inside the `⛔` row's Notes reclassified it as implemented. Check 4
now reads the status from its own table cell (allowing suffixes such as
`🟢 [D]`); all 577 rows still parse. This is the third false-green class closed
in the compat checker (after check 5b and check 11).

### 13.4 Validation

`python3 scripts/compat_check.py` RC=0: **577 F-ID / 1040 T-ID rows**, Nacos
**197 = 178 🟢 / 5 🟡 / 10 ⚪ / 4 ⛔ / 90%**, global
**526 🟢 / 10 🟡 / 37 ⚪ / 4 ⛔ / 91%**, target 3.3.0 GA.
`cargo fmt --all -- --check`, `python3 -m py_compile scripts/compat_check.py`,
`git diff --check`, `cargo check --workspace --all-targets` and clippy
(`-D warnings` on `batata-ai`, `batata-server`, `batata-client`,
`batata-maintainer-client`) all RC=0. Targeted suites: `batata-ai --lib`,
`ai_admin_form_encoding` **17**, `mcp_admin_routes` **7**,
`agent_admin_routes` **3**, `admin_naming_form_encoding` **5**,
`ai_import_routes` **18**, `batata-maintainer-client --tests` and
`batata-client --tests`; all three independent `sdk-tests` workspaces
`cargo check --tests` RC=0.

No `--full` regression gate was run (standing rule: explicit request only).
No commit, push, publish, yank, archive or tag was performed.

### 13.5 Remaining backlog

`F-NAC-AISRV-001` (typed ARD DTOs/storage, GA first-draft `callInterfaces`,
`Page` responses, RAD runtime endpoints), the client-side `F-NAC-MAINT-007`
write facade + typed AI resource DTOs, `F-NAC-MAINT-009`,
`F-NAC-CSDK-001/002/003/004/005/006/007`, `F-NAC-LOCK-009` (server `RENEW` +
Raft variant + client watchdog), RAD ability keys/handlers, and test gaps
TG-01, TG-02, TG-04, TG-05.

## 14. Implementation batch 7 (2026-11-10) — `F-NAC-MAINT-007` write facade

### 14.1 Source verification before changing anything

The GA baseline is `~/.m2/repository/com/alibaba/nacos/nacos-maintainer-client/3.3.0/nacos-maintainer-client-3.3.0.jar`
(by bytecode via `javap`), cross-checked against the local develop checkout.
`git diff --stat b64bc9118..HEAD -- maintainer-client/src/main/java/com/alibaba/nacos/maintainer/client/ai/`
shows **exactly one** changed AI client file, `A2aMaintainerServiceImpl.java`
(plus two config files); `SkillMaintainerServiceImpl`,
`PromptMaintainerServiceImpl`, `AgentSpecMaintainerServiceImpl`,
`PipelineMaintainerServiceImpl`, `McpMaintainerServiceImpl` and their
interfaces are GA-identical in the local snapshot. MCP method names were
re-verified against the released jar (`javap
com.alibaba.nacos.maintainer.client.ai.McpMaintainerService`), which declares
`deleteMcpServerDraft`, `updateMcpServerLabels`, `updateMcpServerStatus` and
`updateMcpServerScope`.

Three wire facts determined the scope:

1. **The batch-3 comment was stale.** `crates/batata-maintainer-client/src/client.rs`
   still said the AI POST/PUT lifecycle was unwrappable because "GA sends them
   as form bodies while batata reads query parameters". Reading
   `crates/batata-ai/src/api/{skill,prompt,agentspec}.rs` shows every write
   handler uses `web::Form<...>`; `mcp.rs` uses `AdminForm<T>` /
   `AdminParams`. Those handlers have always accepted GA's `postForm` /
   `putForm`. `F-NAC-MAINT-010` (batches 4 + 5) removed the last query-only
   MCP/Agent handlers, so the wire is now aligned end-to-end.
2. **DELETE is query, POST/PUT is form.** GA `ClientHttpProxy.executeSync`
   builds a `Query` for GET/DELETE and a form body for POST/PUT. Batata's
   `delete_draft` / `unbind_label` handlers use `web::Query`, matching it.
   The new `ai_write_ok` helper therefore dispatches on `AiWriteVerb`.
3. **Two backlog items were not what they seemed.** GA
   `getAgentSpecAdminDetail` is `GET /v3/admin/ai/agentspecs` with
   `namespaceId` + `agentSpecName` — batch 3's `agentspec_get_detail` is
   exactly that call, so it was a **false gap**. `listAgentSpecAdminItems`
   reuses `GET /v3/admin/ai/agentspecs/list` but adds `orderBy`, `owner` and
   `scope`; `agentspec_list` did not forward them, so that one is real.

GA-only optional parameters (`commitMsg`, `updateLatestLabel`, prompt
`basedOnVersion` / `targetVersion` / `bizTags`) are forwarded on the wire but
are not honoured by batata's handlers (some are absent from the form structs,
others are declared but never consumed). Serde ignores unknown fields, so the
calls still work; the semantic divergence is recorded in the
`F-NAC-MAINT-007` row rather than claimed as parity. For the prompt first
draft the client sends **both** `targetVersion` (GA) and `version` (batata),
so one call works against either server.

> **Batch 9 correction (§16)**: this paragraph was too broad. GA itself
> ignores `updateLatestLabel` on ADMIN, Skill/AgentSpec `basedOnVersion` and
> the dedicated `bizTags` endpoints are already consumed by batata, and
> `targetVersion` is covered by the dual alias above. Only Skill `commitMsg`
> and Prompt draft `basedOnVersion`/`bizTags` remain real server-side gaps.

### 14.2 What landed

| Item | Change | Files |
|------|--------|-------|
| paths | 35 ADMIN constants: 10 skill, 11 prompt, 10 agentspec, 4 MCP (draft, submit/publish/force-publish/redraft, labels, biz-tags, scope, online/offline, legacy `/label`, `/labels`, `/description`, `/status`) | `crates/batata-maintainer-client/src/constants.rs` |
| write facade | 42 flat methods: 11 `skill_*`, 15 `prompt_*`, 12 `agentspec_*`, 4 `mcp_server_*`; four shared write-form structs with `skip_serializing_if = "Option::is_none"`, plus `ai_write_ok` / `ai_write_version` helpers | `crates/batata-maintainer-client/src/client.rs` |
| stale comment | the AI facade header now documents the four wire shapes (query / form / multipart / raw bytes) instead of claiming POST/PUT divergence | `crates/batata-maintainer-client/src/client.rs` |
| tests | 6 new wiremock cases (128 → **134**): four lifecycle suites with exact `FormFields` equality on every urlencoded body, a failure-code case and a POST/PUT/DELETE error-path case; `FormFields` decodes percent-encoded fields into a `HashMap` and compares the whole map, so a missing, extra or renamed field fails the match | `crates/batata-maintainer-client/tests/client_test.rs` |
| docs | `F-NAC-MAINT-004/007` notes, Summary note, README gap row + batch 7 paragraph, this section | `docs/compat/nacos/features.md`, `docs/compat/README.md` |

`F-NAC-MAINT-007` stays `🟡`: the only remaining client-side item is typed
AI resource DTOs (`SkillMeta`, `SkillSummary`, `PromptMetaInfo`,
`PromptVersionInfo`, `AgentSpec*`, `PipelineExecution`). GA delegate objects
remain represented by flat methods, consistent with batches 1–3. No F-ID or
status total changed. No server code was touched in this batch.

### 14.3 Mutant evidence

Fifteen single-point code mutants were applied one at a time; each had to
compile and turn the wiremock suite red, then was restored:

| Mutant | Result |
|--------|--------|
| `AiWriteVerb::Post` uses `put_form` / `Put` uses `post_form` / `Delete` uses `get_with_query` | 3 × FAILED |
| `ai_write_ok` returns `true` regardless of the envelope code | `test_ai_write_failure_code_is_false` FAILED |
| `ai_write_version` uses `post_json` instead of `post_form` | FAILED |
| prompt first draft drops the batata `version` alias | FAILED |
| skill / prompt / agentspec online↔offline paths swapped | 3 × FAILED |
| `SkillWriteForm` / `McpWriteForm` / `AgentSpecAdminListQuery` switch to `snake_case` | 3 × FAILED |
| `AI_SKILLS_FORCE_PUBLISH` points at `/publish` | FAILED |
| MCP labels use `put_json` | FAILED |
| MCP status drops `enabled` | FAILED |

Two first-round survivors exposed weak guards and were fixed rather than
suppressed: the prompt online/offline mocks had **identical** expected form
fields, so a path swap was unobservable (the two cases now use distinct
versions `1.1.0` / `1.0.0`); and removing `skip_serializing_if` from an
`Option` is an equivalent mutant because `serde_urlencoded` already omits
`None` (replaced with a mutant that drops the MCP `enabled` parameter).
Final result: **15/15 KILLED**. Eight documentation mutants were also applied
(the Summary Total row, the B.10 module row, a false `🟡` → `🟢` promotion of
`F-NAC-MAINT-007`, the README global Nacos row, the README Layout F-ID count,
a dangling F-ID reference, the alignment target, and a dead path link); all
were caught by `scripts/compat_check.py` (**8/8 KILLED**).

### 14.4 Validation and release impact

Validation commands: `cargo fmt --all`, `python3 scripts/compat_check.py`,
`python3 -m py_compile scripts/compat_check.py`, `git diff --check`,
`cargo check --workspace --all-targets`, clippy `-D warnings` on
`batata-client` / `batata-maintainer-client`, the full
`batata-maintainer-client --test client_test` suite (**134**), and
`cargo check --tests` in all three independent Rust `sdk-tests` workspaces.
No `--full` regression gate was run: this batch changes only the maintainer
client crate and compatibility documentation, with no server behaviour or
wire-contract change. Baseline `compat_check.py` output: **577 F-ID / 1040
T-ID rows**, Nacos **178/178 anchored**, target **3.3.0 GA**, RC=0 — no
Summary, global table or Layout count changed in this batch. No commit, push,
publish, yank, archive or tag was performed.

Package file counts stay **20 / 58 / 26**: batch 7 edits existing files in
`batata-maintainer-client` and `docs/compat` only. The full release recipe in
`docs/development.md` still has to be re-run before publishing.

### 14.5 Remaining backlog

`F-NAC-MAINT-007` GA `/skills/upload` `targetVersion` / `uploadAction` /
`autoPublishIfNew` only — the Skill ADMIN `commitMsg` and Prompt draft
`basedOnVersion` / `bizTags` items were closed by batch 10 (see §17),
`F-NAC-AISRV-001` (typed ARD DTOs /
storage, GA first-draft `callInterfaces`, `Page` responses, RAD runtime
endpoints), `F-NAC-MAINT-008` (legacy A2A facade + GA-only agent methods),
`F-NAC-MAINT-009`, `F-NAC-CSDK-001/002/003/004/005/006/007`,
`F-NAC-LOCK-009` (server `RENEW` + Raft variant + client watchdog), RAD
ability keys/handlers, and test gaps TG-01, TG-02, TG-04, TG-05.

## 15. Implementation batch 8 — typed AI resource DTOs (`F-NAC-MAINT-007`)

### 15.1 GA source verification

`b64bc9118..HEAD` in the local Nacos checkout changes only four files under
`api/src/main/java/com/alibaba/nacos/api/ai/model/` (`a2a/AgentCard`,
`a2a/AgentSkill`, `a2a/SecurityRequirement`, `a2a/StringList`) and one
maintainer-client file (`ai/A2aMaintainerServiceImpl`). The Skill, Prompt,
AgentSpec and Pipeline models — and all four maintainer-client AI service
implementations — are **GA-stable**, so the local snapshot is a valid 3.3.0 GA
reference for the DTO shapes.

The GA read methods map one-to-one onto batata's flat facades:
`getSkillMeta` → `SkillMeta`, `getSkillVersionDetail` → `Skill`,
`listSkills` → `Page<SkillSummary>`, `precheckUploadSkillFromZip` →
`List<SkillUploadPrecheckResult>`, `batchUploadSkillsFromZip` →
`BatchUploadResult`, `listPrompts` → `Page<PromptMetaSummary>`,
`listPromptVersions` → `Page<PromptVersionSummary>`, `getPromptMeta` /
`getPromptGovernanceDetail` → `PromptMetaInfo`, `getVersionDetail` /
`queryPromptDetail` → `PromptVersionInfo`, `getAgentSpecAdminDetail` →
`AgentSpecMeta`, `getAgentSpecVersionDetail` / `getAgentSpecVersionMeta` →
`AgentSpec`, `listAgentSpecs` → `Page<AgentSpecBasicInfo>`,
`listAgentSpecAdminItems` → `Page<AgentSpecSummary>`, and
`getPipelineDetail` / `listPipelineExecutions` → `PipelineExecution` /
`Page<PipelineExecution>`. `PipelineMaintainerService` exposes `JsonNode` to
callers, but its implementation already decodes typed
`Result<PipelineExecution>` / `Result<Page<PipelineExecution>>`.

Batch 7's false-gap correction is confirmed: GA `getAgentSpecDetail` is a
client-side composite (`getAgentSpecAdminDetail` then a version lookup), while
`getAgentSpecAdminDetail` is the single `GET /v3/admin/ai/agentspecs` already
wrapped by `agentspec_get_detail`.

### 15.2 Implementation (client crate only, zero server changes)

- New `crates/batata-maintainer-client/src/model/ai_resource.rs`: **24**
  public DTOs covering Skill, Prompt, AgentSpec and Pipeline reads plus ZIP
  upload results, re-exported from `model/mod.rs`. Each DTO declares
  `#[serde(rename_all = "camelCase", default)]`, types the fields shared by GA
  and batata, and keeps every other wire field in
  `#[serde(flatten)] extra: AiExtra`. GA-only and batata-only fields are both
  typed (for example `commitMsg` plus `description` on version summaries,
  `title` plus `name` on pipeline checkpoints), so either response decodes.
- Collection fields use a `null_to_default` deserializer: Jackson may emit a
  `null` list/map where batata omits it, and both now decode to an empty
  collection.
- `client.rs`: **18** public read facades and **2** shared helpers (20
  signatures) retyped from
  `serde_json::Value` / `Page<Value>` / `Vec<Value>` to the GA DTOs. The shared
  `prompt_get` / `agentspec_get` helpers are now generic over
  `T: DeserializeOwned`, with an explicit turbofish at each of the 7 call
  sites so metadata and version endpoints keep their distinct DTOs.
- No server endpoint, route, form or constant changed. Package layout gains
  one source file; the published crate's file count changes from 26 to 27.

### 15.3 Guards

- **7** serde unit tests cover GA-shaped and batata-shaped payloads, camelCase
  keys, the `type` alias on resource DTOs, null/absent collections, both
  prompt version shapes, nested version/resource/checkpoint DTOs and `extra`
  preservation.
- The existing **134** wiremock cases were updated to assert typed fields plus
  `extra`. Type-discriminating assertions check that a typed wire key must not
  leak into `extra`, which catches a wrong DTO being chosen by a facade.
- **15/15 code mutants KILLED, all with a red assertion (none merely fails to
  compile)**: camelCase removal, both `flatten` removals, `type` alias removal,
  null-tolerance removal/weakening, five wire-key renames, two DTO swaps on
  shared helpers and two `Page<T>` swaps. Two first-round mutants were compile
  only (return type changed without the helper turbofish) and were redesigned
  to compile and fail an assertion.

### 15.4 Status

`F-NAC-MAINT-007` **stays 🟡**. The client-side typed DTO gap is closed, but
some GA-only optional fields are still not consumed by batata's `web::Form`
handlers — a server-side semantic divergence, deliberately not claimed as
parity. **Batch 9 narrowed this claim**: `updateLatestLabel` is GA-ignored and
`basedOnVersion` / `bizTags` are consumed where GA binds them; the open items
are Skill `commitMsg` and Prompt draft `basedOnVersion` / `bizTags` (§16).
No F-ID row was added or promoted, so the global totals
(577 / 526🟢 / 10🟡 / 37⚪ / 4⛔ / 91%) and the Nacos split
(197 = 178🟢 / 5🟡 / 10⚪ / 4⛔ / 90%) are unchanged.

### 15.5 Validation and no-ops

`cargo fmt --all`, `cargo test -p batata-maintainer-client` (lib 44 +
134 wiremock), `cargo check --workspace --all-targets`, clippy `-D warnings`,
`scripts/compat_check.py`, `git diff --check` and `python3 -m py_compile`
are green, as are `cargo check --tests` in all three independent
`sdk-tests/*` workspaces. `cargo package --list` now reports **20 / 58 / 27**
files (the new `model/ai_resource.rs` adds one file to
`batata-maintainer-client`). **No** `--full` matrix, commit, push, publish,
yank, archive or tag was performed.

## 16. Implementation batch 9 — source audit of the batch 8 residual claim (`F-NAC-MAINT-007`)

### 16.1 Method

Batch 8 closed the client-side typed DTO gap but recorded the remaining
divergence as "GA-only optional fields (`commitMsg` / `updateLatestLabel` /
`basedOnVersion` / `bizTags`) are forwarded but ignored by batata's form
handlers". Batch 9 re-read the GA sources instead of trusting that summary.

The local Nacos checkout (`d1b97bc3e`) has **zero** post-GA drift in
`ai/src/main/java/com/alibaba/nacos/ai/controller/` and `.../form/`
(`git diff --stat b64bc9118..HEAD`); the only AI drift in
`maintainer-client/.../ai/` is `A2aMaintainerServiceImpl.java`. Those trees
are therefore valid GA truth here. Both GA **form binding** and GA **service
consumption** were checked, because a bound-but-ignored parameter is not a
gap.

### 16.2 Field-by-field verdict

| Field | GA ADMIN binding + consumption | batata | Verdict |
|---|---|---|---|
| `updateLatestLabel` | Bound on `SkillPublishForm.java:45`, `PromptVersionPublishForm.java:45`, `AgentSpecPublishForm.java:45` but documented as an "ignored update latest label flag"; `SkillOperationService.java:264`, `PromptOperationService.java:107`, `AgentSpecOperationService.java:262` say "retained for compatibility and ignored; latest is server-managed". Maintainer-client javadocs (`SkillMaintainerService.java:476`, `PromptMaintainerService.java:214`, `AgentSpecMaintainerService.java:332`) say "ignored by server" | Not bound, not acted on | **False gap** — ignoring it is correct alignment |
| `basedOnVersion` | Bound on `SkillDraftCreateForm.java:39`, `AgentSpecDraftCreateForm.java:36`, `PromptDraftCreateForm.java:39`; drives the fork path | Skill/AgentSpec bound in `crates/batata-common/src/model/ai/skill.rs:334` / `agentspec.rs:304` and consumed by `skill_service.rs:752-758` / `agentspec_service.rs:573-583`. **Absent** from `crates/batata-ai/src/api/prompt.rs::PromptDraftCreateForm` (744-762) | **Aligned for Skill/AgentSpec; real gap for Prompt `POST /prompt/draft`** |
| `bizTags` | Bound on `PromptDraftCreateForm.java:51`, `PromptPublishForm.java:60`, `PromptMetadataForm.java:41`, `PromptListForm.java:54` and the dedicated `SkillBizTagsUpdateForm` / `AgentSpecBizTagsUpdateForm`; **not** bound on Skill/AgentSpec draft forms | Prompt bound + consumed (`api/prompt.rs:38,42,829`; `service/prompt/mod.rs:282-288`), except on `PromptDraftCreateForm`. Dedicated update endpoints exist for all three resources (`api/skill.rs:831`, `api/agentspec.rs:818`, `api/prompt.rs:1205`) | **Aligned except Prompt `POST /prompt/draft`** |
| `commitMsg` | Bound and stored as the version description (`versionDesc`) on `SkillDraftCreateForm.java:43`, `SkillUpdateForm.java:36`, `SkillAdminController.java:201` (ZIP upload), `PromptDraftCreateForm.java:47`, `PromptDraftUpdateForm.java:40`, `PromptPublishForm.java:50`; consumed by `SkillOperationServiceImpl.java:1246,1306` and `PromptOperationServiceImpl.java:160,261`. GA AgentSpec has no `commitMsg` at all | Prompt bound + consumed (`api/prompt.rs:38,759,777`). Skill **absent** from `SkillDraftCreateForm` / `SkillUpdateForm` (`crates/batata-common/src/model/ai/skill.rs:327-362`) and `SkillUploadQuery` (`crates/batata-ai/src/api/skill.rs:1069-1076`) | **Real gap for Skill ADMIN `POST/PUT /skills/draft` and `POST /skills/upload`** |

**Batch 10 correction**: the `commitMsg` row above says "Prompt bound +
consumed (`api/prompt.rs:38,759,777`)", but those were *declarations*, not
call sites. `PromptDraftCreateForm.commit_msg` (line 759) was never forwarded
by `POST /prompt/draft`; only `PromptDraftUpdateForm` (777) and
`PromptPublishForm` (38) were consumed. Batch 10 wired create-draft's
`commitMsg` into both `AiResourceVersionInfo.description` and `PromptStorage`
and added a route assertion for it. The declaration-vs-consumption check is
the mirror image of the batch 5 lesson (an unchecked box is not proof of a
missing feature): a checked box is not proof of a wired field.

### 16.3 Corrected residual gap

`F-NAC-MAINT-007` stays 🟡, but the open items are now exactly:

1. **Skill ADMIN `commitMsg`** — `POST /v3/admin/ai/skills/draft`,
   `PUT /v3/admin/ai/skills/draft`, `POST /v3/admin/ai/skills/upload`. GA
   writes it into the version record description; batata's three form structs
   have no `commit_msg`, so serde silently drops it.
2. **Prompt ADMIN draft fork metadata** — `POST /v3/admin/ai/prompt/draft`
   ignores `basedOnVersion` (fork from an existing version) and `bizTags`.
   `targetVersion` is **not** a gap: batch 7 sends both GA's `targetVersion`
   and batata's `version` for the first draft.

Everything else in the batch 8 claim was over-registered. `F-NAC-MAINT-007`
is now a **server-side consumption** item; no client SDK work remains. It was
not moved into B.10 in this batch because that would change the
machine-checked F-ID / Summary totals without any implementation evidence.

### 16.4 Docs changes and status

Corrected `docs/compat/nacos/features.md` (MAINT-007 row + batch 8 Summary
note), `docs/compat/README.md` (gap row, batch 7/8 narratives, new batch 9
paragraph) and this report (§14 note, §14.5 backlog, §15.4 status). **No row
status, F-ID count, T-ID count or Summary total changed**: 577 F / 1040 T-ID,
Nacos 197 = 178🟢 / 5🟡 / 10⚪ / 4⛔, global 526🟢 / 10🟡 / 37⚪ / 4⛔.
`scripts/compat_check.py` stays green.

### 16.5 Proposed implementation (not executed — needs confirmation)

Closing the two gaps is a **server-side semantic change**, not a client fix:

1. Add `commit_msg: Option<String>` to `SkillDraftCreateForm`,
   `SkillUpdateForm` and `SkillUploadQuery`; thread it through
   `SkillService::create_draft` / `update_draft` (trait in
   `crates/batata-common/src/traits/ai.rs`, impl in
   `crates/batata-ai/src/service/skill_service.rs`) and the ZIP upload path;
   write it to `AiResourceVersionInfo.description`, mirroring GA
   `versionDesc`. Behaviour when the field is absent must stay unchanged.
2. Add `based_on_version` / `biz_tags` to `PromptDraftCreateForm` and thread
   them into `PromptOperationService::create_draft` — fork storage from the
   base version and call `ai_resource_update_biz_tags` when tags are present.
3. Route-level tests for the form and query/multipart channels, exact wire
   keys, success and error paths; mutant-check every new guard; re-run the
   baseline gates.

Because step 1 changes a public trait signature and the meaning of the version
`description` column, it is held for explicit confirmation. No `--full`
matrix, commit, push, publish, yank, archive or tag was performed in batch 9.

## 17. Implementation batch 10 — GA ADMIN field consumption (`F-NAC-MAINT-007`)

### 17.1 Method and GA sources

Batch 9 registered exactly two server-side gaps. Batch 10 re-read the GA
sources before implementing, including both **form binding** and **service
consumption**, because §16.2 had made the opposite mistake for `commitMsg`
(a declaration counted as a call site).

* `SkillAdminController.java:196-217,277-295` binds `commitMsg` on
  `POST /skills/upload`, `POST /skills/draft` and `PUT /skills/draft`.
* `AiResourceManager.insertVersionRow(..., String description, ...)`
  (line 647-666) and `SkillOperationServiceImpl.java:1246,1668` write it into
  the version row's `desc` column (`versionDesc`).
* `PromptAdminController.java:185-191` forwards `basedOnVersion`,
  `targetVersion`, `template`, `variables`, `commitMsg`, `description` and
  `bizTags`; `PromptOperationServiceImpl.java:131-232` forks the base
  template/variables, bumps the base patch version and initializes a brand-new
  prompt's metadata; `updateDraft` (235-261) writes `commitMsg` as the version
  description.
* `ClientHttpProxy.executeMultipartPost` (line 264) puts every scalar upload
  parameter in the **query string** and only the file in the multipart body, so
  `commitMsg` belongs in `SkillUploadQuery`, not in a multipart text field.
* `git diff --stat b64bc9118..HEAD` still shows zero post-GA drift in
  `ai/.../controller/`, `ai/.../form/` and the AI maintainer client except
  `A2aMaintainerServiceImpl.java`.

### 17.2 Implementation

| Area | Change |
|---|---|
| Skill forms | `SkillDraftCreateForm` / `SkillUpdateForm` (`crates/batata-common/src/model/ai/skill.rs`) gain `commit_msg`; `SkillUploadQuery` (`crates/batata-ai/src/api/skill.rs`) gains `commit_msg`. All use camelCase → `commitMsg`. |
| Skill service | `SkillService::{upload_skill,create_draft,update_draft}` (`crates/batata-common/src/traits/ai.rs`) take `commit_msg: Option<&str>`; `SkillOperationService` forwards it to `AiResourceVersionInfo.description` via `version_description()` and to `ai_resource_version_update_storage`. `batch_upload_from_zip` passes `None`; the console handlers pass their own form value. Absent field → skill-card / ZIP description, exactly as before. |
| Skill handlers | `POST/PUT /skills/draft` move from `web::Form` to `AdminForm<T>` (query + form + JSON object, query precedence); the ZIP upload reads `commitMsg` from the query. |
| Prompt ADMIN | `PromptDraftCreateForm` gains `targetVersion`, `basedOnVersion`, `bizTags`; `create_draft` moves to `AdminForm` and calls the new `PromptOperationService::create_draft_with_options`. `targetVersion` wins over batata's `version`; `basedOnVersion` forks the stored template/variables and bumps the patch; `bizTags` initializes brand-new metadata via `ai_resource_update_biz_tags`; `commitMsg` reaches both the version description and `PromptStorage.commit_msg`. |
| Prompt facade | The historical `create_draft` and the `PromptService` trait are unchanged: they delegate with `None` GA-only fields, so the console route and `tests/prompt_persistence.rs` keep their contract. |

This closes a **third, previously unregistered gap**: §16.2 counted
`api/prompt.rs:759` as "Prompt bound + consumed", but `PromptDraftCreateForm`
only declared `commit_msg`; the create-draft handler never forwarded it. The
§16.2 correction note records that.

### 17.3 Guards

13 new route tests on the real mounted `/v3/admin/ai` scope:

* `crates/batata-server/tests/skill_admin_routes.rs` (7) — form channel,
  query channel, absent-field baseline, update form + query channels, ZIP
  upload query channel and its absent-field baseline. Each asserts the stored
  `ai_resource_version.description`, not just the HTTP status.
* `crates/batata-server/tests/prompt_admin_routes.rs` (+6, now 12) — GA form
  and query channels, `targetVersion` alias precedence, fork copies the base
  template and creates `1.0.1`, unknown base version 400 naming the missing
  version, and the absent-field baseline.

`/tmp/b10_mutants.py` **19/19 KILLED**, all compile-safe and assertion-red:
`version_description`, all three handler forwards, the `commitMsg` wire key,
both dual-channel extractors, the version-row description override, all four
Prompt handler forwards, `targetVersion` precedence, the fork template and
patch bump, `bizTags` serialization, the missing-base guard,
`PromptStorage.commit_msg` and the Prompt version description. Two initially
weak designs were fixed rather than accepted: an ambiguous anchor (three
`form.commit_msg` call sites) and a mutant whose fallback error message still
satisfied a generic 400 assertion — the test now asserts the error names the
missing base version.

### 17.4 Status and newly registered residual

`F-NAC-MAINT-007` **stays 🟡**; no row status, F-ID, T-ID or Summary total
changed (577 F / 1040 T-ID, Nacos 197 = 178🟢 / 5🟡 / 10⚪ / 4⛔, global
526🟢 / 10🟡 / 37⚪ / 4⛔). Promoting it would be a false green: a further GA
re-read of `SkillAdminController.java:196-205` and
`SkillOperationServiceImpl.java:200-211,399-435` found three upload parameters
that GA binds **and consumes** but batata's `SkillUploadQuery` does not expose:

* `targetVersion` — explicit upload version override;
* `uploadAction` — selects create-draft / overwrite-draft / delete-and-create;
* `autoPublishIfNew` — force-publishes the first uploaded version.

These are registered as the remaining `F-NAC-MAINT-007` item, not silently
dropped. `setAsLatest` was checked and is **not** a gap: GA binds it on
`SkillUpdateForm.java` but never passes it to `updateDraft`.

### 17.5 Validation and no-ops

`cargo fmt --all -- --check`, `git diff --check`, `py_compile`,
`scripts/compat_check.py`, `cargo check --workspace --all-targets`, workspace
clippy `-D warnings` and `cargo test --workspace --exclude
batata-integration-tests` (**130 green result lines**) all pass; three separate
`sdk-tests/*` workspaces `cargo check --tests` pass; `cargo package --list`
still reports **20 / 58 / 27** files. The one red crate in an unfiltered
`cargo test --workspace` is `batata-integration-tests`, which requires a live
server on `127.0.0.1:8081` (all 145 failures are `error sending request`); it
is an environmental gate, not a regression, and is intentionally excluded from
the unit tier. `/tmp/b10_docs_mutants.py` guards the docs evidence. No
`--full` matrix, commit, push, publish, yank, archive or tag was performed.

## 18. Implementation batch 11 — GA `/skills/upload` action + auto-publish (`F-NAC-MAINT-007` → 🟢)

### 18.1 Method and GA sources

Batch 10 (§17.4) registered three upload params as the final `F-NAC-MAINT-007`
residual. Batch 11 re-read the GA sources to pin the exact contract before
implementing:

* `SkillAdminController.java:192-219` `POST /upload` binds `@RequestParam`
  `namespaceId`, `overwrite` (default false), `targetVersion`, `commitMsg` (done
  in batch 10), `uploadAction`, `autoPublishIfNew` (default false), `file`.
  `POST /upload/batch` (lines 250-275) binds only `namespaceId` / `overwrite` /
  `file` — so the batch path does **not** accept the three new params, and
  batata's `skill_batch_upload` / `agentspec_upload_from_zip` are intentionally
  left untouched.
* `SkillOperationServiceImpl.uploadSkillFromZip` (lines 200-211) forwards the
  params to `doUploadSingleSkill` (lines 399-435).
* `doUploadSingleSkill` semantics: blank `uploadAction` →
  `overwrite ? overwriteUploadedSkill(requireEditingDraft=false) :
  createUploadedSkillDraft`; `CREATE_DRAFT` → `createUploadedSkillDraft`;
  `OVERWRITE_DRAFT` / `DELETE_DRAFT_AND_CREATE` →
  `overwriteUploadedSkill(requireEditingDraft=true)`; any other value →
  `PARAMETER_VALIDATE_ERROR` "Unsupported uploadAction: ..."; `autoPublishIfNew`
  → `forcePublish(namespaceId, name, targetVersion, true)` **only when
  `meta == null`** (brand-new skill).
* `SkillUploadPrecheckResult.java:40-44` defines the action constants:
  `ACTION_CREATE_DRAFT="CREATE_DRAFT"`,
  `ACTION_OVERWRITE_DRAFT="OVERWRITE_DRAFT"`,
  `ACTION_DELETE_DRAFT_AND_CREATE="DELETE_DRAFT_AND_CREATE"`.
* `overwriteUploadedSkill(requireEditingDraft)` (lines 537-575): with
  `requireEditingDraft=true` and no editing draft, GA throws `RESOURCE_CONFLICT`
  "No editing draft to overwrite" (covers both `meta == null` and `meta exists
  but no editing version`).

### 18.2 Implementation

| Area | Change |
|---|---|
| ADMIN query | `SkillUploadQuery` (`crates/batata-ai/src/api/skill.rs`) gains `target_version`, `upload_action`, `auto_publish_if_new` (camelCase). |
| Console query | `ConsoleSkillUploadQuery` (`crates/batata-console/src/v3/ai_skill.rs`) gains the same three fields. |
| Trait | `SkillService::upload_skill` (`crates/batata-common/src/traits/ai.rs`) gains `target_version: Option<&str>`, `upload_action: Option<&str>`, `auto_publish_if_new: bool`; `#[allow(clippy::too_many_arguments)]`. |
| Service | `SkillOperationService::upload_skill` (`crates/batata-ai/src/service/skill_service.rs`) implements the GA branch table verbatim; `target_version` overrides auto-bump; `batch_upload_from_zip` forwards `None/None/false` (GA batch semantics). |
| SDK | `AiUploadQuery` (`crates/batata-maintainer-client/src/client.rs`) gains the three fields + `commit_msg` with `skip_serializing_if`; `skill_upload_from_zip` exposes them; `skill_batch_upload` / `agentspec_upload_from_zip` keep the faithful 3-arg wire. |
| Callers | `api/skill.rs`, `console/ai_skill.rs` handlers and `tests/client_test.rs` (lines 3047, 3375) updated; wiremock query matchers unaffected (skipped params emit nothing). |

### 18.3 Guards

6 new route tests in `crates/batata-server/tests/skill_admin_routes.rs`:

* `upload_target_version_override` — explicit `targetVersion=2.3.4` pins the
  draft version and suppresses the auto-bumped `0.0.1`.
* `upload_action_overwrite_draft` — `OVERWRITE_DRAFT` on an existing editing
  draft deletes it and creates the next patch.
* `upload_action_create_draft_rejects_existing` — `CREATE_DRAFT` with a working
  version present returns 400 naming the conflict.
* `upload_action_unsupported_is_rejected` — `BOGUS_ACTION` → 400
  "Unsupported uploadAction".
* `upload_auto_publish_if_new_publishes` — `autoPublishIfNew=true` on a
  brand-new skill moves the version to `online`.
* `upload_auto_publish_if_new_keeps_draft` — `autoPublishIfNew=false` leaves it
  as `draft`.

`/tmp/b11_mutants.py` **4/4 KILLED**, all compile-safe and assertion-red:
dropping the `target_version` branch, treating an unsupported action as blank,
inverting the `autoPublishIfNew` guard, and dropping the editing-version check
in the `CREATE_DRAFT` guard — each turns its targeted test red.

### 18.4 Status and promotion

`F-NAC-MAINT-007` is now `🟢` with crate anchors. Summary moves Nacos
178🟢/5🟡 → 179🟢/4🟡 (Total 197, rate 90%→91%) and global 526🟢/10🟡 →
527🟢/9🟡; the F-ID count (577) is unchanged. `features.md` B.8 5🟢/4🟡 →
6🟢/3🟡 (60%), Total 178🟢/5🟡 → 179🟢/4🟡, 90%→91%. README global table and the
§17.4 residual note updated. No row status, F-ID or T-ID count changed beyond
the 🟡→🟢 flip.

### 18.5 Validation and no-ops

`cargo fmt --all -- --check`, `git diff --check`, `py_compile`,
`scripts/compat_check.py`, `cargo check --workspace --all-targets`, clippy
`-D warnings` and `cargo test -p batata-server --test skill_admin_routes`
(13 passed) all pass. No `--full` matrix, commit, push, publish, yank, archive
or tag was performed.

## 19. Implementation batch 12 — GA ARD Agent model Phase A (`F-NAC-AISRV-001` → 🟡)

### 19.1 GA sources and gap

`F-NAC-AISRV-001` is the only genuine server-side AI gap (§13). GA
`AgentAdminController` (18 endpoints) and batata `admin_routes()` (19) agree on
paths but not on model: GA stores an ARD `AgentVersionDetail` with
`callInterfaces` (`protocol`, `protocolVersion`, `endpointSets`,
`nativeDescriptor`) and a `name`/`url` provider, whereas batata stores the A2A
`AgentCard`. Batches 5–6 closed the form-field mapping and the draft-created
metadata sub-gap; the remaining Phase A items were: typed ARD DTOs + storage,
`Page`-shaped `listAgents` / `listVersions`, `AgentSummary` /
`AgentVersionSummary` status semantics, and GA first-draft `callInterfaces`
support (GA `requireInitialDraftContent` `b64bc9118:1171-1177` needs a
non-null `callInterfaces`).

### 19.2 Implementation

| Area | Change |
|---|---|
| Model | `crates/batata-common/src/model/ai/a2a.rs`: added `AgentVersionDetail`, `AgentCallInterface`, `AgentProviderInfo`, `AgentVersionSummary`, `AgentSummary`, `AgentVersionInfo`, `Endpoint`, `EndpointSet`, `EndpointSource`, `RuntimeVersionBinding`, `RuntimeCallInterface`; `Default` on `AgentCardDetailInfo`, `PartialEq` on the ARD value types. |
| Storage | `AgentCardDetailInfo.call_interfaces: Option<Vec<AgentCallInterface>>`; `build_detail` writes it; `version_row` reads it back for `AgentVersionDetail`. |
| First-draft `callInterfaces` | `parse_call_interfaces` (form body) → `resolve_agent_write` → `create_agent_draft(ns, &card, false, Some(cis))` / `update_agent_draft(ns, &card, Some(cis))`; no longer fail-closed. |
| List responses | `list_agents` → `Page<AgentSummary>`, `list_versions` → `Page<AgentVersionSummary>` (GA `Page.total_count` / `page_number` / `pages_available` / `page_items`). |
| Catalog `latestVersion` | `to_agent_summary` surfaces the legacy `AgentCardVersionInfo.latest_published_version` as `AgentSummary.online_versions` (single `AgentVersionSummary`, `labels: ["latest"]`, `status: online`). |
| Status semantics | `AgentVersionSummary.status` resolves `online` / `draft` / `reviewing` via `Page.page_items` (`status == "online"` then highest version), replacing the removed `is_latest` flag. |
| Callers | `ai_handler.rs` (gRPC search/discovery/deregister), `agent_client.rs` (HTTP search/discover/deregister), `agent_watch.rs` (`resolve_version`), `ai_agent.rs` (console), `registry/a2a.rs` migrated to `.page_items` and the new summary shapes. |

### 19.3 Guards

`batata-ai` lib + `a2a_persistence` integration: `ga_call_interfaces_are_accepted_and_parsed` (protocol round-trip, `[]` / `null` / malformed), `ga_call_interfaces_round_trip_through_write`, `draft_stores_call_interfaces_and_round_trips` (create with `Some(cis)` → stored `call_interfaces == Some(cis)`; second draft `None` → `None`). `batata-server` route: `ga_call_interfaces_are_accepted_and_persisted_at_the_route` (200 + `callInterfaces` / `JSONRPC` echoed + stored card present). All new asserts are mutant-checked (compile-safe + assertion-red).

### 19.4 Status and promotion

`F-NAC-AISRV-001` → 🟡 (partial). The `GET /runtime-endpoints` live-support
sub-capability is still deferred (it needs the Naming publication backend; the
RAD gRPC handlers it also needed landed in `F-NAC-CSDK-007`, now 🟢 per §20), so
the row is partial, not 🟢. Counts: B.10 `1🟢/1🟡/0⚪/1⛔` (rate
33%→67%); Nacos Total `179🟢/4🟡/10⚪/4⛔/197` → `179🟢/5🟡/9⚪/4⛔/197` (rate 91%
unchanged). `features.md` B.10 and Summary Total, `README.md` global table and
F-ID narrative updated.

### 19.5 Validation and no-ops

`cargo fmt --all -- --check`, `git diff --check`, `scripts/compat_check.py`,
`cargo check -p batata-ai --tests` and the `batata-ai` + `batata-server` route
suites all pass. No `--full` matrix, commit, push, publish, yank, archive or tag
was performed (commit/push handled separately).

## 20. Implementation batch 13 — GA RAD v1 ability keys + gRPC handlers (`F-NAC-CSDK-007` → 🟢)

### 20.1 GA contract and gap

Nacos 3.3.0 GA declares nine server ability keys in
`AbilityKey.java` (62, 76, 86, 92). batata advertised only five
(`fuzzyWatch`, `lock`, `mcp`, `supportPersistentInstanceByGrpc`, plus one
other), so GA 3.4.0 Java clients never negotiated the RAD v1 / RAD watch
contract. `F-NAC-CSDK-007` was registered `⚪` on purpose: advertising
`radV1` / `radWatchV1` makes GA clients switch to RAD gRPC request types
(`AgentPublishRpcRequest`, `AgentSubscribeRpcRequest`, `AgentUnsubscribeRpcRequest`)
that `batata-ai` did not handle — an unhandled request fails the GA client's
handshake-fast-path and breaks the SDK. The fix is two-sided: advertise the
keys **and** implement the handlers they enable.

### 20.2 Implementation

- **Ability keys** (`crates/batata-core/src/handler/rpc.rs`, `SERVER_ABILITIES`):
  added `mcpDraftRelease`, `agentCardV1`, `radV1`, `radWatchV1` (all `true`);
  `HashMap` capacity raised 5 → 9.
- **Model structs** (`crates/batata-api/src/remote/model.rs`): new
  `AgentPublishRequest` / `AgentPublishRpcRequest` / `AgentPublishRpcResponse`,
  `AgentSubscribeRpcRequest` / `AgentSubscribeRpcResponse`,
  `AgentUnsubscribeRpcRequest` / `AgentUnsubscribeRpcResponse`, all carrying the
  `RequestTrait` / `ResponseTrait` impls so they round-trip through
  `Payload{metadata.type, Any.body}` with the exact GA `type` strings.
- **RAD handlers** (`crates/batata-ai/src/handler/ai_handler.rs`, mirroring the
  existing `AgentSearchRpcHandler` / `AgentDiscoveryRpcHandler` patterns):
  - `AgentPublishRpcHandler` — `can_handle()` → `"AgentPublishRpcRequest"`,
    `auth_requirement()` → `Write`; builds an `AgentCard` from
    `AgentPublishRequest` (mapping `provider` → `AgentProvider`), calls
    `A2aAgentService::publish_agent_from_code`, returns
    `AgentPublishRpcResponse { version_detail: Some(detail) }`.
  - `AgentSubscribeRpcHandler` — `can_handle()` → `"AgentSubscribeRpcRequest"`,
    `auth_requirement()` → `Read`; reads `connection_id` from
    `connection.meta_info`, calls
    `AgentRadWatchService::subscribe`, returns
    `AgentSubscribeRpcResponse { watch_key, observed_fingerprint, refresh_required: false }`.
  - `AgentUnsubscribeRpcHandler` — `can_handle()` →
    `"AgentUnsubscribeRpcRequest"`, `auth_requirement()` → `Read`,
    `resource_from_payload` → `None`; calls
    `AgentRadWatchService::unsubscribe` and returns an empty
    `AgentUnsubscribeRpcResponse`.
- **RAD watch service** (`crates/batata-ai/src/service/agent_rad_watch.rs`, new):
  `AgentRadWatchService` with a `WatchPusher` trait (blanket impl for
  `ConnectionManager`, plus a test stub) so server-push is decoupled from the
  connection map; `subscribe` / `unsubscribe` / `unsubscribe_all` / `size` /
  `valid_client_watch_id` and a broadcast channel
  (`publish_agent_change` / `subscribe_agent_changes`) that fans out a watch
  notification only when the recomputed `DiscoveryProjector` fingerprint
  differs from the observed one.
- **Wiring** (`crates/batata-server/src/startup/grpc.rs`): `register_ai_handlers`
  now takes `&mut AIServices`, constructs
  `AgentRadWatchService::new(a2a_service.clone(), connection_manager.clone())`
  when the A2A service is present, stores it on `AIServices.agent_rad_watch`
  (new field, `None` in both `AIServices` constructors) and registers the three
  handlers; `start_grpc_servers` likewise takes `&mut AIServices`.
  `app_builder.rs` passes `as_mut()`.

### 20.3 Guards

`batata-ai` lib unit tests: three `can_handle()` assertions (one per RAD
handler) plus three `#[tokio::test]` handler tests —
`rad_publish_handler_returns_version_detail` (publish → `version_detail`),
`rad_subscribe_handler_returns_watch_key_and_fingerprint` (subscribe →
`watch_key` + `observed_fingerprint`), `rad_unsubscribe_handler_succeeds` —
driven by `RadTestA2a` / `TestPusher` doubles. `agent_rad_watch.rs` adds five
module tests: `client_watch_id_validation_matches_ga_constraint`,
`subscribe_generates_watch_key_and_stores_registration`,
`unsubscribe_all_clears_connection_watches`,
`change_notifies_only_on_fingerprint_diff`,
`change_via_global_broadcast_is_fanned_out`. All `AgentVersionDetail` /
`AgentRegistryStats` / `BatchRegistrationResponse` gained `#[derive(Default)]`
to support the doubles.

### 20.4 Status and promotion

`F-NAC-CSDK-007` → 🟢. Counts: B.9 `1🟢/1🟡/6⚪/0⛔` (rate 0%→13%); Nacos Total
`179🟢/5🟡/9⚪/4⛔/197` → `180🟢/5🟡/8⚪/4⛔/197` (rate 91% unchanged). `features.md`
B.9 row + Summary Total, `README.md` global table and F-ID narrative updated.
The `GET /runtime-endpoints` live-support sub-capability stays deferred (needs
the Naming publication backend) and is tracked outside this row.

### 20.5 Validation and no-ops

`cargo fmt --all -- --check`, `git diff --check`, `scripts/compat_check.py`,
`cargo check -p batata-ai --tests -p batata-server --tests` and the `batata-ai`
lib suite (`199` tests) all pass. No `--full` matrix, commit, push, publish,
yank, archive or tag was performed (commit/push handled separately).

## 21. Implementation batch 14 — `AiService` facade + `AgentDiscoveryService` (`F-NAC-CSDK-003` → 🟢)

### 21.1 GA contract and gap

Nacos 3.3.0 GA exposes the `AiService` facade (`mcp()` / `skill()` / `prompt()`
/ `agentSpec()` / `agent()`) and the `AgentDiscoveryService` interface
(`searchAgents`, `discoverAgent`, `subscribeAgent` / `unsubscribeAgent`,
`registerAgentEndpoints`, `deregisterAgentEndpoints`, `publishAgent`), all
`@Since("3.3.0")`). `batata-client` only had 10 flat console query methods in
`crates/batata-client/src/ai.rs`; the facade, the typed sub-services and the RAD
surface were unimplemented. `F-NAC-CSDK-003` was registered `⚪` on
2026-11-10.

### 21.2 Source-verified wire contract

Every client DTO and path was read from the server handlers (not guessed):

| Client facade method | Server route | Server handler |
|---------------------|-------------|---------------|
| `McpAiService.get` / `list_mcp_servers` (legacy flat) | `/v3/console/ai/mcp`, `/v3/console/ai/mcp/list` | `crates/batata-ai/src/api/mcp_client.rs::query_server` / `search` |
| `McpAiService.search` (client) | `/v3/client/ai/mcp/search` | `mcp_client.rs` (client scope) |
| `SkillAiService.get` | `/v3/client/ai/skills` → ZIP bytes | `skill.rs::client_query_skill` (returns `application/zip`) |
| `SkillAiService.search` | `/v3/client/ai/skills/search` | `skill.rs::client_search_skills` → `Page<SkillSummary>` |
| `AgentSpecAiService.get` / `search` | `/v3/client/ai/agentspecs`, `/…/search` | `agentspec.rs::client_query_agentspec` / `client_search_agentspecs` |
| `PromptAiService.get` / `search` | `/v3/client/ai/prompt`, `/…/search` | `prompt.rs::client_query_prompt` / `client_search_prompts` |
| `AgentDiscoveryService.search_agents` | `GET /v3/client/ai/agents/search` | `agent_client.rs::search` → `Page<AgentCatalogEntry>` |
| `AgentDiscoveryService.discover_agent` | `GET /v3/client/ai/agents` | `agent_client.rs::discover` → `AgentDiscoveryResult` |
| `subscribe_agent` (watch) | `POST /v3/client/ai/agents/watch` | `agent_client.rs::watch` (reads `clientId` header) |
| `register_agent_endpoints` | `POST /v3/client/ai/agents/endpoints` | `agent_client.rs::register_endpoints` (reads `clientId` header) |
| `deregister_agent_endpoints` | `DELETE /v3/client/ai/agents/endpoints` | `agent_client.rs::deregister_endpoints` (reads `clientId` header) |
| `publish_agent` | `POST /v3/client/ai/agents` | `agent_client.rs::publish` (body = `AgentCard`) |

Key wire facts confirmed by reading the handlers:

- The response envelope is `Result<T>{code,message,data}` (identical shape to the
  client `ApiResponse<T>`), and `Page<T>` serializes as
  `{totalCount,pageNumber,pagesAvailable,pageItems}` (camelCase). `code == 0` is
  success; the client decode helper surfaces a non-zero `code` as an error.
- `AgentCatalogEntry` (the client `AgentSummary`) carries
  `agentName` / `displayName?` / `description?` / `latestVersion` / `versions[]`;
  this is **not** the GA `AgentSummary` POJO (which adds `status` / `owner` /
  `scope`), so the client models the server projection exactly.
- The watch body is `AgentWatchBatchForm` whose `watches` field is a **JSON
  string** of `[{clientWatchId, discoveryRequest:{namespaceId?, reference:{
  agentName, version?, label?}}, materializedFingerprint}]`; the response is
  `{generation, changed, changedClientWatchIds?}`.
- `POST /endpoints` accepts either a JSON body or a form whose `endpoints` is a
  JSON string; it reads the `clientId` header (absent ⇒ admitted
  unconditionally). The `version` field is the GA `runtimeVersion` alias.
- `DELETE /endpoints` consumes `namespaceId` / `agentName` / `protocol` from the
  query and drops whatever the client published; the concrete endpoint list is
  not part of the wire (GA parity).

Because `batata-client` does **not** depend on `batata-common`, all AI response
DTOs are defined locally in `ai.rs` (`AgentSummary`, `AgentVersionSummary`,
`AgentDiscoveryResult`, `Endpoint`, `AgentReference`, `AgentSearchRequest`,
`AgentDiscoveryFilter`, `AgentEndpointRegistrationBatch`, `ClientLivenessInfo`,
`SkillSummary`, `AgentSpec`, `AgentSpecSummary`, `Prompt`, `PromptMetaSummary`,
`AgentVersionDetail`, `ClientAgentCard`). `AgentPublishRequest::from_agent_card_info`
maps the legacy console `AgentCardInfo` onto the server `AgentCard` shape
(supplying the required `protocolVersion`, `capabilities`, `skills`,
`defaultInputModes` / `defaultOutputModes`, `metadata`).

### 21.3 Implementation

- `BatataAiService` keeps the two shared caches (`mcp_cache`, `agent_cache`) as
  `Arc<DashMap<…>>` and lazily constructs the five sub-services via `OnceLock`
  accessors returning `Arc<…>`, all cloning the same `Arc<BatataHttpClient>`.
- The 10 legacy flat methods (`get_mcp_server`, `list_mcp_servers`,
  `get_agent_card`, `list_agent_cards`, `get_cached_*`, `clear_cache`,
  `*_cache_size`) are preserved; `get_mcp_server` / `list_mcp_servers` delegate
  to `mcp()` and keep the **console** paths (`/v3/console/ai/mcp*`,
  TG-03), while `mcp().search()` exercises the client path.
- `AgentAiService` implements the `AgentDiscoveryService` trait (via
  `async_trait` so it is usable as `dyn AgentDiscoveryService`); `BatataAiService`
  also implements the trait by delegating to `agent()`. `subscribe_agent`
  spawns a tokio task that loops the watch long-poll: on a
  `changedClientWatchIds` hit it re-discovers, recomputes a
  `sha256-canonical-json-v1:` fingerprint of the result, and invokes the
  `AgentDiscoveryListener`; `AgentSubscription` exposes `unsubscribe()`
  (`AtomicBool`) and `is_active()`.
- A small `BatataHttpClient::post_json_with_headers` forwards an extra header
  map (used for the `clientId` header) on top of the auth header.
- RAD capability: `enum AgentDiscoveryCapability { Supported, NotSupported,
  Unknown }` mirrors GA `AbilityStatus`; `get_agent_discovery_capability()`
  returns `Supported` for the HTTP discovery transport and explicitly does
  **not** claim the gRPC `radV1` / `radWatchV1` negotiation (that is
  `F-NAC-CSDK-007`, now 🟢 via the gRPC handlers).

### 21.4 Guards

16 `ai.rs` unit tests cover camelCase serde round-trips and the exact on-wire
field names for every new DTO (`AgentSummary`, `AgentDiscoveryResult`,
`AgentSearchRequest`, `AgentEndpointRegistrationBatch` (`runtimeVersion` alias),
`ClientAgentCard` from `AgentCardInfo`, `AgentVersionDetail`, `Endpoint`, the
watch-body `watches`-as-JSON-string shape, fingerprint determinism). Two are
mutant-checked: `decode` must reject a non-zero `code` (deleting the `code == 0`
guard fails the test), and the capability mapping is asserted. The existing 10
console-DTO tests and the TG-03 `tests/ai_service_test.rs` (2 cases) remain green.

### 21.5 Status and promotion

`F-NAC-CSDK-003` → 🟢. Counts: B.9 `2🟢/1🟡/5⚪/0⛔` (rate 13%→25%); Nacos Total
`180🟢/5🟡/8⚪/4⛔/197` → `181🟢/5🟡/7⚪/4⛔/197` (rate 91%→92%). `features.md` B.9
row + Summary Total, `README.md` global table and F-ID narrative updated.

### 21.6 Validation and no-ops

`cargo fmt --all -- --check`, `git diff --check`, `scripts/compat_check.py`,
`cargo check -p batata-client --tests`, `cargo test -p batata-client --lib ai`
(58 tests) and `cargo test -p batata-client --test ai_service_test` (2 tests)
all pass. No `--full` matrix, commit, push, publish, yank, archive or tag was
performed (commit/push handled separately).

## 22. F-NAC-CSDK-006 — HTTP request-id correlation header (2026-11-11)

### 22.1 Summary

Nacos 3.3.0 GA adds a `Nacos-Request-Id` correlation header (a fresh UUID) to
every outgoing client HTTP request through `RequestIdHttpClientRequestInterceptor`
(`client-basic`), registered in both `ConfigHttpClientManager` and
`NamingHttpClientManager`. The batata client set a UUID `request_id` only on the
gRPC path; the HTTP layer (`crates/batata-client/src/http.rs`) sent no such
header. This closes the HTTP-side gap so upstream/Grafana-style correlation
matches GA.

### 22.2 Implementation

`crates/batata-client/src/http.rs`:
- A module const `NACOS_REQUEST_ID = "Nacos-Request-Id"` and a free helper
  `with_request_id(builder)` that appends `.header(NACOS_REQUEST_ID,
  uuid::Uuid::new_v4().to_string())`.
- `request_with_retry` now generates a fresh `request_id` per loop iteration
  (i.e. per physical request, matching the per-request GA interceptor) and
  passes it into each closure, which applies it alongside the auth header.
- The retry-bypassing paths also attach the header: `authenticate()` (JWT
  login), `post_json_with_headers()`, `get_bytes` / `get_bytes_with_query`,
  `post_bytes`, and `post_multipart` / `post_multipart_with_query` /
  `post_multipart_bytes`.

### 22.3 Scope

Client-only, no server change: the batata tracing middleware reads `x-request-id`
on the server side, which is independent of this client egress header (the
header is emitted for upstream/log-correlation parity). The standalone
transports in `auth.rs` (`JwtAuthProvider::login`) and `server_list.rs`
(`build_refresh_client`) are out of scope for this F-ID (they sit outside the
registered gap file `src/http.rs`) and are noted as optional follow-ups.

### 22.4 Guards

3 wiremock cases in `crates/batata-client/tests/request_id_test.rs`:
`get_carries_nacos_request_id` (GET), `post_json_carries_nacos_request_id`
(POST) and `authenticate_carries_nacos_request_id` (JWT login). Each asserts the
recorded request carries `Nacos-Request-Id` and that its value parses as a v4
UUID and is not nil. All 3 are mutant-checked: removing the `.header(...)` from
the GET closure makes `get_carries_nacos_request_id` panic with
`missing Nacos-Request-Id`, confirming the tests bind to the guard rather than
to incidental traffic.

### 22.5 Status and promotion

`F-NAC-CSDK-006` → 🟢. Counts: B.9 `3🟢/1🟡/4⚪/0⛔` (rate 25%→38%); Nacos Total
`181🟢/5🟡/7⚪/4⛔/197` → `182🟢/5🟡/6⚪/4⛔/197` (rate 92% unchanged). `features.md`
B.9 row + Summary Total, `README.md` global table and F-ID narrative updated.

### 22.6 Validation and no-ops

`cargo fmt -p batata-client -- --check`, `git diff --check`,
`scripts/compat_check.py`, `cargo check -p batata-client --tests` and
`cargo test -p batata-client` (372 lib + 2 ai + 11 tls + 25 unit + 3 request_id
+ 2 doctests) all pass. No `--full` matrix, commit, push, publish, yank, archive
or tag was performed (commit/push handled separately).

## 23. F-NAC-CSDK-001 — NamingSelector subscribe overloads (2026-11-11)

### 23.1 Summary

The registered gap was that `crates/batata-client/src/traits.rs:239` exposed
only the 4-argument `subscribe(serviceName, groupName, clusters, listener)`,
while Nacos 3.3.0 GA additionally declares two `NamingSelector` overloads. GA
`b64bc9118` was re-read (not inferred): `NamingService.java:524,530`.

### 23.2 GA contract (source-verified)

- `NamingSelector` extends `api/selector/client/Selector<NamingContext,
  NamingResult>`: one method `select(NamingContext)` returning a result whose
  `getResult()` is `List<Instance>`.
- `NamingContext` carries `serviceName`, `groupName`, `clusters`, `instances`.
- `DefaultNamingSelector` filters with a `Predicate<Instance>` and returns an
  empty list for a null input.
- `NamingSelectorFactory` supplies `EMPTY_SELECTOR`, `HEALTHY_SELECTOR`,
  `newClusterSelector(Collection<String>)` (empty collection → empty selector),
  `newIpSelector(regex)` (`Pattern.matches`, i.e. whole-string),
  `newMetadataSelector(map[, isAny])` and `getUniqueClusterString` (TreeSet,
  comma-joined). For `isAny = true` GA ORs each entry match into the initial
  `metadata.size() >= requested.size()` predicate — a quirk that is preserved
  deliberately, not "corrected".
- `NacosNamingService` shows the selector is **client-side only**: the 4-arg
  overload builds a cluster selector and delegates to `doSubscribe`, which
  merely registers a `NamingSelectorWrapper` with the local change notifier and
  calls `clientProxy.subscribe(...)` with `Constants.NULL` clusters. The wrapper
  filters the current list and each of the added/removed/modified diff buckets
  before invoking the user listener.

### 23.3 Implementation

`crates/batata-client/src/naming/selector.rs` (new): `NamingContext<'a>`
(borrowed instance slice), the `NamingSelector` trait,
`DefaultNamingSelector<F>`, `NamingSelectorFactory` with all five constructors,
and `SelectorEventListener` — the Rust equivalent of `NamingSelectorWrapper`,
whose `filter_event` mirrors `buildListenerEvent`.

`crates/batata-client/src/naming/mod.rs`: `subscribe_with_selector` and
`subscribe_with_selector_default_group`. Both call `SelectorEventListener::wrap`
and then reuse the *same* wrapper's `filter_event` on the initial snapshot, so
the returned list is filtered exactly like GA's `notifyIfSubscribed` path. The
existing 4-arg `subscribe` (redo, cache, gRPC request) is unchanged.

`crates/batata-client/src/traits.rs`: the two contract methods added to
`NamingService` with default implementations, so any implementor inherits
selector support. Rust has no overloads, so the two GA signatures become named
methods; `subscribe_with_selector_default_group` fixes `DEFAULT_GROUP` and an
empty cluster string.

`crates/batata-client/src/lib.rs` re-exports the selector types plus
`EventListener` / `NamingEvent`. `regex = { workspace = true }` was added to the
crate's `[dependencies]` for the IP selector.

### 23.4 Scope

Client-only. No server, wire, gRPC or T-ID change: selectors run locally on the
push-dispatch task. `NamingSelector` requires `Send + Sync` because the filter
executes there. No new F-ID row was registered — this closes an existing ⚪ row.

### 23.5 Guards

12 tests: 11 in `selector.rs` (empty / healthy / cluster / empty-cluster /
IP whole-match / invalid-regex error / metadata all / metadata any /
unique-cluster-string / wrapper diff buckets / wrapper listener invocation) and
1 in `naming/mod.rs` that registers `SelectorEventListener::wrap(...)` into
`subscriptions` and triggers the real `handle_notify_subscriber`, asserting both
the current instance list and the added bucket are filtered.

**4/4 compile-safe mutants killed** (no compile errors, real assertion
failures), each red in both the unit suite and the real-dispatch test:

| Mutant | Change |
|--------|--------|
| M1 | `DefaultNamingSelector::select` drops the predicate (returns all) |
| M2 | `filter_event` leaves diff buckets unfiltered (`event.diff.clone()`) |
| M3 | `on_event` passes the raw event through |
| M4 | `filter_instances` returns all instances |

### 23.6 Status and promotion

`F-NAC-CSDK-001` → 🟢. Counts: B.9 `4🟢/1🟡/3⚪/0⛔` (rate 38%→50%); Nacos Total
`182🟢/5🟡/6⚪/4⛔/197` → `183🟢/5🟡/5⚪/4⛔/197` (rate 92%→93%). `features.md`
B.9 row + Summary Total, `README.md` global table and F-ID narrative updated.

### 23.7 Validation and no-ops

`cargo fmt -p batata-client -- --check`, `scripts/compat_check.py` and
`cargo test -p batata-client` (384 lib + 2 ai + 3 request_id + 11 tls + 25 unit
+ 2 doctests, 0 failed) all pass. No `--full` matrix, commit, push, publish,
yank, archive or tag was performed (commit/push handled separately).
