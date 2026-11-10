# Client SDK compatibility review — 2026-11-09

**Scope**: `crates/batata-client` and `crates/batata-maintainer-client`
against the upstream **Nacos Java client SDK** (`api/`, `client/`,
`client-basic/`, `maintainer-client/`) and its test suites. Server-side
controllers, console UI, Consul and Apollo are **out of scope**.

**Method**: read-only source audit. Every claim below was checked with `Grep`
in both repositories plus `git log -S` in the upstream checkout. Upstream
`develop` = `1511b5a85` (2026-10-14), `pom.xml:91` revision `3.3.0-RC`.
**Alignment target was later set to Nacos 3.3.0 GA** (released 2026-10-29);
section 1 was updated accordingly. The audit itself was read-only; the
2026-11-10 runtime re-run did change production code (section 7).

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
| `~/work/github/easynet-cn/nacos` develop | `3.3.0-RC` @ `1511b5a85` | Pre-release source snapshot; used only for `git log -S` evidence |

Because the local checkout has no `3.3.0` tag and its `origin` is blocked by
the proxy, the GA contract was verified from the official `-sources.jar` of
`nacos-api`, `nacos-client` and `nacos-maintainer-client` 3.3.0 on Maven
Central. Byte-level diff GA vs RC snapshot: **0 changed files** in
`maintainer-client`; **0** in `ConfigService`, `NamingService`, `LockService`;
`api` has 9 modified + 4 added files, all under `api/ai` (RAD discovery
capability via `getAgentDiscoveryCapability()`, `AgentConverter` SPI,
`AgentPublishRequest.autoSubmit`, synchronous `Watch` admission in
`AgentDiscoveryService`). Therefore the comparison tables below are still
valid for 3.3.0 GA, with the AI-domain gap being slightly larger than the
RC snapshot suggested.

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

`crates/batata-maintainer-client/src/client.rs` has **98 public methods**;
`src/model/ai.rs` has **11 public DTOs**.

| Upstream capability | Upstream evidence | batata evidence | Verdict |
|---------------------|-------------------|-----------------|---------|
| Config / Naming / Core maintainer services, import+export | `maintainer-client/.../{config,naming,core}/` | `src/client.rs` | ✅ aligned (import/export is an extra vs upstream SDK) |
| `overrideExisting`, `registrationType`, plugin `localOnly` params | `.../ai/AiMaintainerService.java`, `.../core/` | `src/client.rs:3350/3448/3770` | ✅ aligned |
| MCP CRUD (`list/search/detail/create/update/delete`) | `.../ai/McpMaintainerService.java` | `src/client.rs:3153-3437` | ✅ aligned |
| A2A agent register/get/update/delete/list/list_versions/search | `.../ai/A2aMaintainerService.java` | `src/client.rs:3444-3730` | ✅ aligned (basic CRUD only) |
| MCP version lifecycle: `listMcpServerVersions`, `getMcpServerVersion`, `deleteMcpServerDraft`, `submit/publish/forcePublish/redraft/online/offline`, labels/scope/status | `.../ai/McpMaintainerService.java`, `AiMaintainerService.java:172-249` | no such method among the 98 | ❌ gap |
| AI delegates `skill()`, `prompt()`, `agentSpec()`, `pipeline()`, `a2a()`, `mcp()` | `AiMaintainerService.java:47-73` | no delegate API; `model/ai.rs` holds only DTOs | ❌ gap |
| Skill/Prompt/AgentSpec ZIP uploads, precheck, batch upload, version detail | `.../ai/{Skill,Prompt,AgentSpec}MaintainerService.java` | none | ❌ gap |
| `AgentMaintainerService` draft lifecycle (`createDraft/updateDraft/submit/publish/forcePublish/redraft/online/offline/updateLabels`) | `.../ai/AgentMaintainerService.java:281-498` | none | ❌ gap |
| `agent_get(version)` / `getAgentVersion` / runtime endpoints | `AgentMaintainerService.java:227,256` | `src/client.rs:3478` has no `version` param | ❌ small gap |
| cross-namespace `cloneConfig` overload | `.../config/ConfigMaintainerService.java` | `src/client.rs:1137` takes a single `namespace_id` | ❌ small gap |

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
| `maintainer-client/src/test/java` | 342 `@Test` | maintainer crate: 37 `src` + 111 `tests/` | 148 | same |
| `test/java-sdk-test` | 120 `@Test` | `sdk-tests/nacos-java-tests` | 588 `@Test` | ✅ batata suite is larger |
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
| P1 | maintainer-client: MCP version lifecycle (8 methods) | SDK wrapping only; server routes exist |
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
