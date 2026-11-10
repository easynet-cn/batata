# Client SDK compatibility review — 2026-11-09 (updated 2026-11-10)

**Scope**: `crates/batata-client` and `crates/batata-maintainer-client`
against the upstream **Nacos Java client SDK** (`api/`, `client/`,
`client-basic/`, `maintainer-client/`) and its test suites. Server-side
controllers, console UI, Consul and Apollo are **out of scope**.

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
