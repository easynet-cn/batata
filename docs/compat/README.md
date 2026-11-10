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
│   ├── features.md     # Upstream feature × batata implementation status (181 F-IDs)
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

## Global summary (2026-11-09)

Recomputed from F-ID / T-ID rows (table header rows excluded) and enforced by
`scripts/compat_check.py`.

| Protocol | Features | 🟢 | 🟡 | ⚡ | ⚪ | ⛔ | Impl rate | Test cases | Test types |
|----------|----------|----|----|---|----|----|-----------|------------|------------|
| Nacos    | 181      | 176 | 2  | 0 | 0  | 3  | 97%       | 401        | HTTP_API, GRPC_API, SDK_CLIENT |
| Consul   | 181      | 179 | 2  | 0 | 0  | 0  | 99%       | 251        | HTTP_API, SDK_CLIENT |
| Apollo   | 199      | 169 | 3  | 0 | 27 | 0  | 85%       | 388        | HTTP_API, SDK_CLIENT, INTERNAL |
| **Total** | **561** | **524** | **7** | **0** | **27** | **3** | **93%** | **1040** | |

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
| `~/work/github/easynet-cn/nacos` develop `pom.xml:91` | `3.3.0-RC` (@ `1511b5a85`) | Pre-release source snapshot used for `git log -S` evidence |

The local checkout has no `3.3.0` tag and its `origin` is blocked by the
network proxy, so GA was verified against the official Maven Central
`-sources.jar` for `nacos-api` / `nacos-client` / `nacos-maintainer-client`
3.3.0. GA and RC are byte-identical outside `api/ai` (9 files changed, 4
added), so the inventory and the gap list remain valid. The full gate has since
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

### Known Rust client SDK gaps (2026-11-09)

The audit in `review-report.md` compares `batata-client` /
`batata-maintainer-client` with the upstream Nacos Java client. `🟢` rows that
were falsely marked full have been downgraded; the following capabilities are
**not** exposed by the Rust SDKs. Most are pure SDK wrapping gaps (the batata
server/console routes already exist); `NamingSelector`, 304 conditional GET,
OIDC and HTTP `requestId` additionally need client-side wire work:

| Area | Missing capability | Anchor |
|------|--------------------|--------|
| maintainer-client | MCP version lifecycle (draft/submit/publish/forcePublish/redraft/online/offline/listVersions) | `crates/batata-maintainer-client/src/client.rs` (98 `pub fn`, none of these) |
| maintainer-client | `AiMaintainerService` delegates `skill()` / `prompt()` / `agentSpec()` / `pipeline()` + ZIP uploads | upstream `maintainer-client/.../ai/AiMaintainerService.java` |
| maintainer-client | `AgentMaintainerService` draft/publish + `agent_get(version)` + cross-namespace `cloneConfig` | `crates/batata-maintainer-client/src/client.rs:3478` |
| batata-client | `NamingSelector` subscribe overload | `crates/batata-client/src/traits.rs:239` |
| batata-client | `LockService::renew` / watchdog | `crates/batata-client/src/lock.rs` (only lock/unlock/try_lock/lock_guard) |
| batata-client | `AiService` sub-services + `AgentDiscoveryService` | `crates/batata-client/src/ai.rs` (10 methods, MCP/A2A query + cache only) |
| batata-client | config 304 conditional GET (`localMd5`), OIDC, HTTP `requestId` correlation | `crates/batata-api/src`, `crates/batata-client/src/http.rs` |

One additional gap is **server-side**, not an SDK wrapper: Nacos 3.3.0 GA adds
the `agentCardV1`, `mcpDraftRelease`, `radV1` and `radWatchV1` ability keys,
which batata's `SetupAckRequest.ability_table` does not advertise. GA clients
therefore fall back to the legacy RAD path. See `review-report.md` §5.

These are **not yet registered as F-IDs**. Add them as `⚪ planned` rows before
starting implementation, then promote to `⚡`/`🟢` with source anchors.

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
contiguous numbering, valid status enum), recomputes each file's Summary row and
the global summary in this README, checks that every Nacos `🟢` row has a
resolvable `batata impl` source anchor, and rejects dangling F-ID references and
dead file links. Run from repo root:

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