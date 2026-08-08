# Consul Feature Inventory

Upstream baseline: **Hashicorp Consul 2.1.0-dev** (local `~/work/github/easynet-cn/consul`, `api/` package).

> Purpose & scope: track batata's Consul **compatibility** implementation status.
> Consul's external contract is **HTTP `/v1/**` + blocking queries (long-poll)**. gRPC/8502 (xDS, connect) and gossip are **not** required for ordinary SDK / `prometheus_consul_sd` / `consul-template` clients and are out of core scope.
>
> Granularity: each row is one external HTTP contract (method + path + key query semantics). Verified against `api/*.go` request constructors.
>
> Universal contract (applies to nearly all read endpoints): `?index=&wait=` blocking query + `?stale/consistent/cached`, response headers `X-Consul-Index` / `X-Consul-ContentHash` / `X-Consul-LastContact` / `X-Consul-KnownLeader`. ACL via `X-Consul-Token` (or `?token=`).

Status: `🟢 full` | `🟡 partial` | `⚡ in-progress` | `⚪ planned` | `⛔ missing`

---

## 1. Agent — local registry (`F-CON-AGNT-`)

> Service/check registration lands in the **agent-local** registry first, then syncs to catalog. This is the core write path.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-AGNT-001 | GET `/v1/agent/self` | | 🟢 | | agent info (incl. token ack) |
| F-CON-AGNT-002 | GET `/v1/agent/host` | | 🟢 | | host resources |
| F-CON-AGNT-003 | GET `/v1/agent/version` | | 🟢 | | version |
| F-CON-AGNT-004 | GET `/v1/agent/metrics` | | 🟢 | | snapshot metrics |
| F-CON-AGNT-005 | GET `/v1/agent/checks?filter=` | | 🟢 | | local checks (hash-block) |
| F-CON-AGNT-006 | GET `/v1/agent/services?filter=` | | 🟢 | | local services (hash-block) |
| F-CON-AGNT-007 | GET `/v1/agent/service/{serviceID}` | | 🟢 | | single local service |
| F-CON-AGNT-008 | GET `/v1/agent/health/service/name/{service}` | | 🟢 | | aggregated by name |
| F-CON-AGNT-009 | GET `/v1/agent/health/service/id/{serviceID}` | | 🟢 | | aggregated by id |
| F-CON-AGNT-010 | PUT `/v1/agent/service/register` | | 🟢 | | **core**: register (query `?replace-existing-checks=`) |
| F-CON-AGNT-011 | PUT `/v1/agent/service/deregister/{serviceID}` | | 🟢 | | deregister |
| F-CON-AGNT-012 | PUT `/v1/agent/check/register` | | 🟢 | | register check |
| F-CON-AGNT-013 | PUT `/v1/agent/check/deregister/{checkID}` | | 🟢 | | deregister check |
| F-CON-AGNT-014 | PUT `/v1/agent/check/pass/{checkID}` | | 🟢 | | TTL pass (legacy) |
| F-CON-AGNT-015 | PUT `/v1/agent/check/warn/{checkID}` | | 🟢 | | TTL warn (legacy) |
| F-CON-AGNT-016 | PUT `/v1/agent/check/fail/{checkID}` | | 🟢 | | TTL fail (legacy) |
| F-CON-AGNT-017 | PUT `/v1/agent/check/update/{checkID}` | | 🟢 | | TTL update (status+output) |
| F-CON-AGNT-018 | GET `/v1/agent/members?wan=&segment=` | | 🟢 | | gossip members |
| F-CON-AGNT-019 | PUT `/v1/agent/join/{addr}?wan=` | | 🟢 | | join |
| F-CON-AGNT-020 | PUT `/v1/agent/leave` | | 🟢 | | graceful leave |
| F-CON-AGNT-021 | PUT `/v1/agent/force-leave/{node}?prune=` | | 🟢 | | force leave |
| F-CON-AGNT-022 | PUT `/v1/agent/service/maintenance/{serviceID}?enable=&reason=` | | 🟢 | | service maintenance |
| F-CON-AGNT-023 | PUT `/v1/agent/maintenance?enable=&reason=` | | 🟢 | | node maintenance |
| F-CON-AGNT-024 | PUT `/v1/agent/reload` | | 🟡 | | reload config (no-op: batata has no config file to reload) |
| F-CON-AGNT-025 | PUT `/v1/agent/token/{target}` | | 🟡 | | set agent token (stored, not applied to gossip) |
| F-CON-AGNT-026 | GET `/v1/agent/monitor?loglevel=` | | 🟡 | | long-lived log stream (returns recent logs, no continuous stream) |

## 2. Catalog — central directory (`F-CON-CAT-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-CAT-001 | PUT `/v1/catalog/register` | | 🟢 | | **core**: direct catalog write |
| F-CON-CAT-002 | PUT `/v1/catalog/deregister` | | 🟢 | | deregister |
| F-CON-CAT-003 | GET `/v1/catalog/datacenters` | | 🟢 | | (non-blocking) |
| F-CON-CAT-004 | GET `/v1/catalog/nodes?near=&node-meta=&filter=` | | 🟢 | | blockable; filter expr supported |
| F-CON-CAT-005 | GET `/v1/catalog/services?node-meta=` | | 🟢 | | service→tags |
| F-CON-CAT-006 | GET `/v1/catalog/service/{service}?tag=&filter=` | | 🟢 | | **core**: instances by service |
| F-CON-CAT-007 | GET `/v1/catalog/connect/{service}?tag=` | | 🟢 | | connect-only instances |
| F-CON-CAT-008 | GET `/v1/catalog/node/{node}?filter=` | | 🟢 | | services on node |
| F-CON-CAT-009 | GET `/v1/catalog/node-services/{node}` | | 🟢 | | node services (ns `*`) |
| F-CON-CAT-010 | GET `/v1/catalog/gateway-services/{gateway}` | | 🟢 | | gateway |

## 3. Health (`F-CON-HEALTH-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-HEALTH-001 | GET `/v1/health/node/{node}?filter=` | | 🟢 | | blockable; filter expr supported |
| F-CON-HEALTH-002 | GET `/v1/health/checks/{service}?tag=&filter=` | | 🟢 | | checks for service |
| F-CON-HEALTH-003 | GET `/v1/health/service/{service}?tag=&passing=&near=&filter=` | | 🟢 | | **core**: service health |
| F-CON-HEALTH-004 | GET `/v1/health/connect/{service}?tag=&passing=&filter=` | | 🟢 | | connect-only |
| F-CON-HEALTH-005 | GET `/v1/health/ingress/{service}?passing=` | | 🟢 | | ingress gateway |
| F-CON-HEALTH-006 | GET `/v1/health/state/{any\|passing\|warning\|critical}?filter=` | | 🟢 | | by state; filter expr supported |

## 4. KV — key-value store (`F-CON-KV-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-KV-001 | GET `/v1/kv/{key}` | | 🟢 | | single; `?recurse` list; `?keys=` names; `?raw` raw; 404=missing; blockable |
| F-CON-KV-002 | PUT `/v1/kv/{key}?flags=&cas=&acquire=&release=` | | 🟢 | | write; returns true/false; session lock |
| F-CON-KV-003 | DELETE `/v1/kv/{key}?cas=&recurse=` | | 🟢 | | delete |

## 5. Session (`F-CON-SES-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-SES-001 | PUT `/v1/session/create` | | 🟢 | | returns id |
| F-CON-SES-002 | PUT `/v1/session/destroy/{id}` | | 🟢 | | |
| F-CON-SES-003 | PUT `/v1/session/renew/{id}` | | 🟢 | | TTL renew |
| F-CON-SES-004 | GET `/v1/session/info/{id}` | | 🟢 | | blockable |
| F-CON-SES-005 | GET `/v1/session/node/{node}` | | 🟢 | | blockable |
| F-CON-SES-006 | GET `/v1/session/list` | | 🟢 | | blockable |

## 6. Status / Coordinate / Event / Snapshot / Query / Txn (`F-CON-OTH-`)

> misc core endpoints.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-OTH-001 | GET `/v1/status/leader` | | 🟢 | | |
| F-CON-OTH-002 | GET `/v1/status/peers` | | 🟢 | | |
| F-CON-OTH-003 | GET `/v1/coordinate/datacenters` | | 🟢 | | |
| F-CON-OTH-004 | GET `/v1/coordinate/nodes` | | 🟢 | | blockable |
| F-CON-OTH-005 | GET `/v1/coordinate/node/{node}` | | 🟢 | | |
| F-CON-OTH-006 | PUT `/v1/coordinate/update` | | 🟢 | | |
| F-CON-OTH-007 | PUT `/v1/event/fire/{name}?node=&service=&tag=` | | 🟢 | | fire event |
| F-CON-OTH-008 | GET `/v1/event/list?name=` | | 🟢 | | quasi-blocking |
| F-CON-OTH-009 | GET `/v1/snapshot?stale=` | | 🟢 | | snapshot export |
| F-CON-OTH-010 | PUT `/v1/snapshot` | | 🟢 | | snapshot restore |
| F-CON-OTH-011 | POST `/v1/query` | | 🟢 | | create prepared query |
| F-CON-OTH-012 | GET `/v1/query` | | 🟢 | | list |
| F-CON-OTH-013 | GET `/v1/query/{id}` | | 🟢 | | read |
| F-CON-OTH-014 | PUT `/v1/query/{id}` | | 🟢 | | update |
| F-CON-OTH-015 | DELETE `/v1/query/{id}` | | 🟢 | | delete |
| F-CON-OTH-016 | GET `/v1/query/{id-or-name}/execute?near=&connect=` | | 🟢 | | **blockable** |
| F-CON-OTH-017 | GET `/v1/query/{id}/explain` | | 🟢 | | explain prepared query |
| F-CON-OTH-018 | PUT `/v1/txn` | | 🟢 | | atomic txn (KV/node/service/check) |

## 7. ACL (`F-CON-ACL-`)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-ACL-001 | PUT `/v1/acl/bootstrap` | | 🟢 | | first token |
| F-CON-ACL-002 | PUT `/v1/acl/token` | | 🟢 | | create token |
| F-CON-ACL-003 | PUT `/v1/acl/token/{accessorID}` | | 🟢 | | update token |
| F-CON-ACL-004 | PUT `/v1/acl/token/{id}/clone` | | 🟢 | | clone |
| F-CON-ACL-005 | GET `/v1/acl/token/{id}` | | 🟢 | | read |
| F-CON-ACL-006 | DELETE `/v1/acl/token/{id}` | | 🟢 | | delete |
| F-CON-ACL-007 | GET `/v1/acl/token/self` | | 🟢 | | own token |
| F-CON-ACL-008 | GET `/v1/acl/tokens?policy=&role=` | | 🟢 | | list |
| F-CON-ACL-009 | PUT `/v1/acl/policy` | | 🟢 | | create policy |
| F-CON-ACL-010 | PUT `/v1/acl/policy/{id}` | | 🟢 | | update |
| F-CON-ACL-011 | GET `/v1/acl/policy/{id}` | | 🟢 | | read |
| F-CON-ACL-012 | DELETE `/v1/acl/policy/{id}` | | 🟢 | | delete |
| F-CON-ACL-013 | GET `/v1/acl/policy/name/{name}` | | 🟢 | | read by name |
| F-CON-ACL-014 | GET `/v1/acl/policies` | | 🟢 | | list |
| F-CON-ACL-015 | PUT `/v1/acl/role` | | 🟢 | | create role |
| F-CON-ACL-016 | PUT `/v1/acl/role/{id}` | | 🟢 | | update |
| F-CON-ACL-017 | GET `/v1/acl/role/{id}` | | 🟢 | | read |
| F-CON-ACL-018 | DELETE `/v1/acl/role/{id}` | | 🟢 | | delete |
| F-CON-ACL-019 | GET `/v1/acl/role/name/{name}` | | 🟢 | | |
| F-CON-ACL-020 | GET `/v1/acl/roles` | | 🟢 | | list |
| F-CON-ACL-021 | POST `/v1/acl/login` | | 🟢 | | login (auth method) |
| F-CON-ACL-022 | POST `/v1/acl/logout` | | 🟢 | | |
| F-CON-ACL-023 | PUT `/v1/acl/auth-method` | | 🟢 | | create |
| F-CON-ACL-024 | PUT `/v1/acl/auth-method/{name}` | | 🟢 | | update |
| F-CON-ACL-025 | GET `/v1/acl/auth-method/{name}` | | 🟢 | | |
| F-CON-ACL-026 | DELETE `/v1/acl/auth-method/{name}` | | 🟢 | | |
| F-CON-ACL-027 | GET `/v1/acl/auth-methods` | | 🟢 | | |
| F-CON-ACL-028 | POST `/v1/acl/authorize` | | 🟢 | | batch ACL authorization check |
| F-CON-ACL-029 | GET `/v1/acl/binding-rules` | | 🟢 | | list binding rules |
| F-CON-ACL-030 | PUT `/v1/acl/binding-rule` | | 🟢 | | create binding rule (Raft-replicated) |
| F-CON-ACL-031 | GET `/v1/acl/binding-rule/{id}` | | 🟢 | | read |
| F-CON-ACL-032 | PUT `/v1/acl/binding-rule/{id}` | | 🟢 | | update (Raft-replicated) |
| F-CON-ACL-033 | DELETE `/v1/acl/binding-rule/{id}` | | 🟢 | | delete (Raft-replicated) |
| F-CON-ACL-034 | GET `/v1/acl/templated-policies` | | 🟡 | | list built-in templates (service/node/dns) |
| F-CON-ACL-035 | GET `/v1/acl/templated-policy/name/{name}` | | 🟡 | | read built-in template |
| F-CON-ACL-036 | POST `/v1/acl/templated-policy/preview/{name}` | | 🟡 | | preview rendered template |
| F-CON-ACL-037 | POST `/v1/acl/oidc/auth-url` | | ⚪ | | OIDC auth URL |
| F-CON-ACL-038 | POST `/v1/acl/oidc/callback` | | ⚪ | | OIDC callback |
| F-CON-ACL-039 | GET `/v1/acl/replication` | | 🟢 | | |

> Legacy `/v1/acl/create|update|destroy|clone|info|list` (pre-1.4) are dead — do **not** implement.

## 8. Config Entries (`F-CON-CFGE-`) (extension)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-CFGE-001 | GET `/v1/config/{kind}/{name}` | | 🟢 | | read; blockable |
| F-CON-CFGE-002 | GET `/v1/config/{kind}` | | 🟢 | | list kind |
| F-CON-CFGE-003 | PUT `/v1/config?cas=` | | 🟢 | | write/update |
| F-CON-CFGE-004 | DELETE `/v1/config/{kind}/{name}?cas=` | | 🟢 | | delete |

## 9. Operator (`F-CON-OP-`) (ops/extension)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-OP-001 | GET `/v1/operator/raft/configuration?stale=` | | 🟢 | | |
| F-CON-OP-002 | POST `/v1/operator/raft/transfer-leader` | | 🟡 | | validation only (openraft 0.9 no transfer) |
| F-CON-OP-003 | DELETE `/v1/operator/raft/peer?address=&id=` | | 🟢 | | |
| F-CON-OP-004 | GET/POST/DELETE/PUT `/v1/operator/keyring` | | 🟡 | | stub (keyring not implemented) |
| F-CON-OP-005 | GET `/v1/operator/autopilot/configuration` | | 🟢 | | |
| F-CON-OP-006 | PUT `/v1/operator/autopilot/configuration?cas=` | | 🟢 | | CAS supported |
| F-CON-OP-007 | GET `/v1/operator/autopilot/health` | | 🟢 | | |
| F-CON-OP-008 | GET `/v1/operator/autopilot/state` | | 🟢 | | |
| F-CON-OP-009 | GET `/v1/operator/usage?global=` | | 🟢 | | real usage stats |
| F-CON-OP-010 | GET `/v1/operator/utilization` | | 🟢 | | returns 501 (matches Consul OSS) |

## 10. Connect mesh (`F-CON-CONN-`) (extension)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-CONN-001 | GET `/v1/connect/ca/roots` | | 🟢 | | blockable; `?format=pem` supported |
| F-CON-CONN-002 | GET `/v1/connect/ca/configuration` | | 🟢 | | |
| F-CON-CONN-003 | PUT `/v1/connect/ca/configuration` | | 🟢 | | persisted to RocksDB |
| F-CON-CONN-004 | GET `/v1/connect/intentions` | | 🟢 | | |
| F-CON-CONN-005 | GET `/v1/connect/intentions/{id}` | | 🟢 | | |
| F-CON-CONN-006 | POST `/v1/connect/intentions` | | 🟢 | | create |
| F-CON-CONN-007 | PUT `/v1/connect/intentions/{id}` | | 🟢 | | update |
| F-CON-CONN-008 | GET `/v1/connect/intentions/exact?source=&destination=` | | 🟢 | | |
| F-CON-CONN-009 | PUT `/v1/connect/intentions/exact` | | 🟢 | | upsert |
| F-CON-CONN-010 | DELETE `/v1/connect/intentions/exact` | | 🟢 | | |
| F-CON-CONN-011 | DELETE `/v1/connect/intentions/{id}` | | 🟢 | | |
| F-CON-CONN-012 | GET `/v1/connect/intentions/match?by=&name=` | | 🟢 | | |
| F-CON-CONN-013 | GET `/v1/connect/intentions/check?source=&destination=` | | 🟢 | | |
| F-CON-CONN-014 | POST `/v1/agent/connect/authorize` | | 🟢 | | |
| F-CON-CONN-015 | GET `/v1/agent/connect/ca/roots` | | 🟢 | | blockable |
| F-CON-CONN-016 | GET `/v1/agent/connect/ca/leaf/{serviceID}` | | 🟢 | | leaf cert; blockable |

## 11. Discovery Chain (`F-CON-DC-`)

> Service mesh discovery chain compilation (Consul 1.6+).

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-DC-001 | GET `/v1/discovery-chain/{service}` | | 🟢 | | compiled chain; blockable |
| F-CON-DC-002 | POST `/v1/discovery-chain/{service}` | | 🟢 | | with overrides |

## 12. Peering / Partition / Namespace (`F-CON-PEER-`) (extension, Enterprise/OSS)

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-PEER-001 | GET `/v1/peering/{name}` | | 🟢 | | read peering |
| F-CON-PEER-002 | DELETE `/v1/peering/{name}` | | 🟢 | | delete peering |
| F-CON-PEER-003 | POST `/v1/peering/token` | | 🟢 | | generate token |
| F-CON-PEER-004 | POST `/v1/peering/establish` | | 🟢 | | establish |
| F-CON-PEER-005 | GET `/v1/peerings` | | 🟢 | | list |
| F-CON-PEER-006 | GET `/v1/imported-services` | | 🟢 | | |
| F-CON-PEER-007 | GET `/v1/exported-services` | | 🟢 | | |
| F-CON-PEER-008 | PUT `/v1/namespace` | | 🟢 | | create ns |
| F-CON-PEER-009 | PUT `/v1/namespace/{name}` | | 🟢 | | update ns |
| F-CON-PEER-010 | GET `/v1/namespace/{name}` | | 🟢 | | read |
| F-CON-PEER-011 | DELETE `/v1/namespace/{name}` | | 🟢 | | delete |
| F-CON-PEER-012 | GET `/v1/namespaces` | | 🟢 | | list |
| F-CON-PEER-013 | PUT `/v1/partition` | | ⚪ | | create partition |
| F-CON-PEER-014 | PUT `/v1/partition/{name}` | | ⚪ | | update |
| F-CON-PEER-015 | GET `/v1/partition/{name}` | | ⚪ | | read |
| F-CON-PEER-016 | DELETE `/v1/partition/{name}` | | ⚪ | | delete |
| F-CON-PEER-017 | GET `/v1/partitions` | | ⚪ | | list |

## 13. Internal API (`F-CON-INT-`)

> Consul internal endpoints used by UI and cross-DC operations. Not part of the public SDK contract but required for UI/CLI compatibility.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-INT-001 | GET `/v1/internal/ui/services` | | 🟢 | | UI service list |
| F-CON-INT-002 | GET `/v1/internal/ui/nodes` | | 🟢 | | UI node dump |
| F-CON-INT-003 | GET `/v1/internal/ui/node/{node}` | | 🟢 | | UI single node info |
| F-CON-INT-004 | GET `/v1/internal/ui/exported-services` | | 🟢 | | |
| F-CON-INT-005 | GET `/v1/internal/ui/catalog-overview` | | 🟢 | | |
| F-CON-INT-006 | GET `/v1/internal/ui/gateway-services-nodes/{gateway}` | | 🟢 | | |
| F-CON-INT-007 | GET `/v1/internal/ui/gateway-intentions/{gateway}` | | 🟢 | | |
| F-CON-INT-008 | GET `/v1/internal/ui/service-topology/{service}` | | 🟢 | | |
| F-CON-INT-009 | GET `/v1/internal/ui/metrics-proxy/{path}` | | 🟢 | | |
| F-CON-INT-010 | GET `/v1/internal/federation-states` | | 🟢 | | |
| F-CON-INT-011 | GET `/v1/internal/federation-states/mesh-gateways` | | 🟢 | | |
| F-CON-INT-012 | GET `/v1/internal/federation-state/{dc}` | | 🟢 | | |
| F-CON-INT-013 | PUT `/v1/internal/service-virtual-ip` | | 🟢 | | assign service VIP |
| F-CON-INT-014 | POST `/v1/internal/acl/authorize` | | 🟢 | | internal ACL authorize |
| F-CON-INT-015 | GET `/v1/internal/rpc-methods` | | 🟢 | | list advertised RPC methods |

## 14. Filter expressions (`F-CON-FILT-`)

> Consul's bexpr filter expression syntax on read endpoints.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-FILT-001 | `?filter=` on health endpoints | | 🟢 | | health/service, health/checks, health/node, health/state |
| F-CON-FILT-002 | `?filter=` on catalog endpoints | | 🟢 | | catalog/nodes, catalog/service, catalog/node |
| F-CON-FILT-003 | `?filter=` on agent endpoints | | ⚪ | | agent/services, agent/checks — not supported |

## 15. Blocking query contract (`F-CON-CORE-`)

> Cross-cutting mechanism: all service-discovery reads must support it.

| ID | HTTP action (method + path) | batata impl | Status | Tests | Notes |
|----|------------------------------|-------------|--------|-------|-------|
| F-CON-CORE-001 | `?index=&wait=` on read endpoints | | 🟢 | | long-poll until change/timeout |
| F-CON-CORE-002 | Response `X-Consul-Index` monotonic index | | 🟢 | | feed back to next `?index=` |
| F-CON-CORE-003 | Hash-based blocking `?hash=` + `X-Consul-ContentHash` (agent-local) | | 🟢 | | for `/v1/agent/*` |
| F-CON-CORE-004 | Response `X-Consul-LastContact` / `X-Consul-KnownLeader` | | 🟢 | | |
| F-CON-CORE-005 | Consistency modes `?stale/consistent/cached` | | 🟢 | | default stale |
| F-CON-CORE-006 | ACL auth `X-Consul-Token` / `?token=` | | 🟢 | | all endpoints |

---

# Summary (auto-updated)

| Module | 🟢 | 🟡 | ⚡ | ⚪ | ⛔ | Total | Impl rate |
|--------|----|----|----|----|----|-------|-----------|
| Agent | 23 | 3 | 0 | 0 | 0 | 26 | 94% |
| Catalog | 10 | 0 | 0 | 0 | 0 | 10 | 100% |
| Health | 6 | 0 | 0 | 0 | 0 | 6 | 100% |
| KV | 3 | 0 | 0 | 0 | 0 | 3 | 100% |
| Session | 6 | 0 | 0 | 0 | 0 | 6 | 100% |
| OTH (status/coord/event/snap/query/txn) | 18 | 0 | 0 | 0 | 0 | 18 | 100% |
| ACL | 34 | 3 | 0 | 2 | 0 | 39 | 95% |
| Config entries | 4 | 0 | 0 | 0 | 0 | 4 | 100% |
| Operator | 8 | 2 | 0 | 0 | 0 | 10 | 90% |
| Connect mesh | 16 | 0 | 0 | 0 | 0 | 16 | 100% |
| Discovery Chain | 2 | 0 | 0 | 0 | 0 | 2 | 100% |
| Peering/Partition/NS | 12 | 0 | 0 | 5 | 0 | 17 | 71% |
| Internal API | 15 | 0 | 0 | 0 | 0 | 15 | 100% |
| Filter expressions | 2 | 0 | 0 | 1 | 0 | 3 | 67% |
| Blocking contract | 6 | 0 | 0 | 0 | 0 | 6 | 100% |
| **Total** | 165 | 8 | 0 | 8 | 0 | 181 | 96% |

> HTTP paths verified against `api/*.go`; blocking-query + header contract is the hard interop dependency for Consul SDKs. gRPC/8502 + gossip out of scope. Update statuses per actual implementation and sync this table.
