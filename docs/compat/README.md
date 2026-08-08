# Batata Compatibility Tracking

This directory uses **upstream protocol feature inventories** as a single source of truth to systematically track batata's feature implementation, test coverage, and bug fixes across **Nacos / Consul / Apollo**, quantifying project maturity and quality.

- Upstream inventory source: official sources at `~/work/github/easynet-cn/{nacos,consul,apollo}` plus official docs.
- All files are Markdown, maintained by hand, but follow **fixed formats** so metrics can be computed with `grep`.

## Layout

```
docs/compat/
├── README.md         # This file: conventions + metric definitions
├── review-report.md  # Source-code audit report (2025-08-07)
├── nacos/
│   ├── features.md   # Upstream feature × batata implementation status (181 features)
│   └── tests.md      # Upstream test inventory (419 T-IDs mapped to F-IDs)
├── consul/
│   ├── features.md   # 181 features
│   └── tests.md      # 267 T-IDs
├── apollo/
│   ├── features.md   # 179 features
│   └── tests.md      # 430 T-IDs
└── bugs.md           # Bug inventory (linked to GitHub Issues) — TODO
```

## Global summary (2025-08-08)

| Protocol | Features | 🟢 | 🟡 | ⚪ | Impl rate | Test cases | Test types |
|----------|----------|----|----|----|-----------|------------| -----|
| Nacos    | 181      | 168 | 2  | 11 | 94%       | 419        | HTTP_API, GRPC_API, SDK_CLIENT |
| Consul   | 181      | 165 | 8  | 8  | 96%       | 267        | HTTP_API, SDK_CLIENT |
| Apollo   | 179      | 76  | 54 | 49 | 73%       | 430        | HTTP_API, SDK_CLIENT, INTERNAL |
| **Total** | **541** | **409** | **64** | **68** | **88%** | **1116** | |

### Test inventory (`tests.md`)

Each protocol's `tests.md` maps **upstream API contract tests** to batata F-IDs. Tests are the source of truth for compatibility — if upstream tests pass against batata, batata is compatible.

- **T-ID format**: `T-<feature-id>-<NN>` (e.g. `T-NAC-CFG-012-01`)
- **Upstream source**: each T-ID references the upstream test file + method name
- **Test type**: `HTTP_API` (HTTP integration), `GRPC_API` (gRPC integration, Nacos only), `SDK_CLIENT` (SDK client API), `INTERNAL` (Mockito unit test, not portable to Rust)
- **Port status**: `⬜ not ported` | `🔄 in progress` | `✅ ported` | `⏳ pending` | `⏭️ skip (feature ⚪/⛔)`
- **Strategy**: adapt upstream tests to point at batata server; skip tests for unimplemented features; fix batata bugs when tests fail
- **Scope exclusions**:
  - Nacos: internal unit tests (`@RunWith(MockitoJUnitRunner.class)`) excluded
  - Consul: internal RPC tests (`agent/consul/*_test.go`) and gRPC tests (`agent/grpc-external/*`) excluded — gRPC/8502 out of scope
  - Apollo: pure Mockito unit tests and AOP aspect tests listed as INTERNAL (not ported)

## Status enums (grep-able; use no other markers)

### Feature status (`features.md` "Status" column)

| Enum     | Meaning                                     |
|----------|---------------------------------------------|
| 🟢 full  | Fully implemented and verified in batata    |
| 🟡 partial | Partially implemented / known gaps         |
| ⚡ in-progress | Under active development              |
| ⚪ planned | Planned, not started                       |
| ⛔ missing | Explicitly absent / won't support (with reason) |

### Bug status (`bugs.md` "Status" column)

`open` | `fixing` | `done` | `regressed`

### Test result (`tests.md` "Status" column)

`✅ pass` | `⏳ pending` | `❌ fail` | `⏭️ skip` (feature ⚪/⛔) | `⬜ not ported` (Nacos)

## Metrics (countable via grep)

| Metric      | Definition                                  |
|-------------|---------------------------------------------|
| Total features | Number of rows in `features.md` containing a status enum |
| Implemented % | 🟢 / total (can be grouped per protocol / module) |
| Gap list       | ⚪ + ⛔ rows = todo roadmap                  |
| Test coverage % | Features with a ✅ in `tests.md` / 🟢+🟡 features |
| Bug health    | Counts per status in `bugs.md`, open bugs grouped by feature |

Example — count implemented features in a file:

```bash
grep -c "🟢" docs/compat/nacos/features.md
```

### Automated validation (`scripts/compat_check.py`)

Validates every `features.md` (format, global ID uniqueness, per-module contiguous numbering, valid status enum, Summary-total consistency). Run from repo root:

```bash
python3 scripts/compat_check.py
```

## Maintenance conventions

1. **Feature ID** is the primary key and the single anchor for all tracking. Every feature, test, and bug carries a globally-unique ID; tests and bugs reference the feature they belong to (see **ID scheme** below).
2. New feature → add as `⚪ planned` → set `⚡` while developing → set `🟢` once verified.
3. A bug marked `done` must include a GitHub Issue link or fixing commit; `open` rows are the todo backlog.
4. After running `scripts/run_sdk_matrix.sh`, batch-refresh the "Latest result" column of `tests.md`.
5. When a PR touches a module, review must confirm status columns match reality.

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

Same anchor; `NN` increments independently of tests. Example: `B-NAC-CFG-012-01`. The GitHub Issue number lives in the `bugs.md` `Issue` column; `B-...` is the stable in-repo code.

## Relationship

```
F-NAC-CFG-012  ──┬─ T-NAC-CFG-012-01 (smoke)
                 ├─ T-NAC-CFG-012-02 (regression)
                 └─ B-NAC-CFG-012-01 (defect)
```

`grep F-NAC-CFG-012 docs/compat -r` returns implementation + all tests + all bugs for one feature.

## Global registry enforcement

- Uniqueness is defined on the **`PROTO + MODULE`** pair: the same acronym may appear under different protocols/SYS, since the full ID (e.g. `F-NAC-AUTH-001` vs `F-SYS-AUTH-001`) is always distinct.
- Module short-names are fixed in the **registry below** (one abbreviation per module → no collisions within a protocol).
- `NNN` increments within a module and tags are never reused.
- `docs/compat/_index.md` acts as a **registration ledger**: every feature's ID, title, status, and create-date is registered there before its row is added. This keeps uniqueness reliable even though files are hand-maintained.
- Optional (`recommended`): `scripts/compat_check.sh` validates ID format regex, global uniqueness, referenced `F-...` existence, and legal status enums.

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