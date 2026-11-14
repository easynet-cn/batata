# Architecture

## Project Structure

Batata uses a multi-crate workspace architecture with **25 internal crates** for modularity and maintainability:

```
batata/
├── crates/
│   ├── batata-common/            # Common utilities & types
│   ├── batata-api/               # API definitions & gRPC proto
│   ├── batata-persistence/       # Database entities (SeaORM)
│   ├── batata-migration/         # Database schema migration
│   ├── batata-auth/              # Authentication & authorization
│   ├── batata-consistency/       # Raft consensus protocol
│   ├── batata-core/              # Core abstractions & cluster
│   ├── batata-server-common/     # Shared server infrastructure
│   ├── batata-config/            # Configuration service
│   ├── batata-naming/            # Service discovery
│   ├── batata-plugin/            # Plugin interfaces
│   ├── batata-plugin-consul/     # Consul compatibility plugin
│   ├── batata-plugin-apollo/     # Apollo compatibility plugin (Config/Admin/OpenAPI)
│   ├── batata-plugin-cloud/      # Prometheus & Kubernetes plugin
│   ├── batata-console/           # Console backend service
│   ├── batata-client/            # Client SDK
│   ├── batata-maintainer-client/ # Maintainer client utilities
│   ├── batata-consul-client/     # Consul HTTP client SDK (published to crates.io)
│   ├── batata-ai/                # AI integration (MCP/A2A)
│   ├── batata-mesh/              # Service mesh (xDS, Istio MCP)
│   ├── batata-copilot/           # AI copilot / operations assistant
│   ├── batata-visibility/        # Metrics & observability
│   ├── batata-integration-tests/ # Cross-crate integration tests
│   ├── batata-server-api/        # Shared server API contracts (proto/gRPC types)
│   └── batata-server/            # Main server (HTTP, gRPC, Console)
│       ├── src/
│       │   ├── api/              # API handlers (HTTP, gRPC, Consul)
│       │   ├── auth/             # Auth types & routes (re-exports)
│       │   ├── config/           # Config models (re-exports)
│       │   ├── console/          # Console AI handlers (re-exports)
│       │   ├── middleware/       # Middleware re-exports; http_tracing.rs is local
│       │   ├── model/            # Application state & config (re-exports)
│       │   ├── service/          # gRPC handlers
│       │   ├── startup/          # Server initialization
│       │   ├── lib.rs            # Library exports
│       │   └── main.rs           # Entry point
│       ├── tests/                # Integration tests
│       └── benches/              # Performance benchmarks
├── conf/                         # Configuration files
├── docs/                         # Documentation
└── proto/                        # Protocol buffer definitions
```

`batata-server/src/{model,auth,console,middleware}` are compatibility
re-export layers: the implementations live in `batata-server-common`,
`batata-console` and `batata-auth`. The one exception is
`middleware/http_tracing.rs`, which still holds the local access-log
middleware implementation. `startup/` is the real composition root.

## Crate Dependencies

```
batata-server (main binary)
├── batata-api (API types, gRPC proto)
├── batata-server-api (shared server API contracts)
├── batata-auth (JWT, RBAC, LDAP, OAuth2)
├── batata-config (config service)
├── batata-naming (service discovery)
├── batata-console (console backend)
├── batata-core (cluster, connections, datacenter)
├── batata-server-common (shared server infrastructure)
├── batata-consistency (Raft consensus, RocksDB state machine)
├── batata-mesh (xDS, Istio MCP)
├── batata-ai (MCP/A2A registry)
├── batata-plugin-consul (Consul API)
├── batata-plugin-cloud (Prometheus, K8s)
├── batata-migration (database migration)
└── batata-persistence (database)
```

## Runtime Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Batata Server                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐  │
│  │   HTTP API    │  │   gRPC API    │  │    Console API      │  │
│  │   (Actix)     │  │   (Tonic)     │  │    (Actix)          │  │
│  │   Port:8848   │  │   Port:9848   │  │    Port:8081        │  │
│  └───────┬───────┘  └───────┬───────┘  └──────────┬──────────┘  │
│          │                  │                      │             │
│  ┌───────▼──────────────────▼──────────────────────▼───────────┐ │
│  │                    Service Layer                             │ │
│  │  ┌──────────┐  ┌──────────┐  ┌────────┐  ┌───────────────┐  │ │
│  │  │  Config  │  │  Naming  │  │  Auth  │  │   Namespace   │  │ │
│  │  │ Service  │  │ Service  │  │Service │  │   Service     │  │ │
│  │  └──────────┘  └──────────┘  └────────┘  └───────────────┘  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Storage Layer                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │ │
│  │  │ MySQL/PgSQL │  │   RocksDB   │  │    Moka Cache       │  │ │
│  │  │  (SeaORM)   │  │ (Raft Log)  │  │   (In-Memory)       │  │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Cluster Layer                             │ │
│  │  ┌─────────────────────────────────────────────────────────┐│ │
│  │  │               Raft Consensus (OpenRaft)                 ││ │
│  │  │                    Port: 9849                           ││ │
│  │  └─────────────────────────────────────────────────────────┘│ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Port Separation

The runtime exposes independent port families. This separation is intentional:

| Port family | Port(s) | Serves |
|---|---|---|
| Console / UI | `8081` | Web console (frontend static assets + console APIs) |
| SDK HTTP | `8848` | Nacos-compatible REST API for SDK clients |
| SDK gRPC | `9848` (main + 1000) | gRPC for SDK clients |
| Cluster gRPC | `9849` (main + 1001) | Inter-node / Raft communication |

The console port serves the UI only; it does not expose the Nacos-compatible data APIs.
See [Configuration](configuration.md) for the relevant keys.

## Architecture Decision Records

### ADR-0001: persistence / consensus boundary (Proposed, 2026-11-08)

**Context.** `batata-persistence` declares `batata-consistency` as a normal
dependency. `distributed/mod.rs` imports `RaftNode`, `RocksDbReader`,
`RaftRequest`/`RaftResponse` and `RocksStateMachine`; `embedded/mod.rs` (the
standalone, no-Raft backend) also imports `RocksDbReader`, the `CF_*` constants
and the bincode `Stored*` record types. There is **no cycle**: the consensus
crate does not depend on the persistence crate. The "layering inversion" is a
clean-architecture reading of the crate names — the Runtime Architecture diagram
above deliberately places the Storage Layer above the Cluster Layer.

**Decision.** Phase 2 does **not** split crates. It introduces a narrow
`RaftStatePort` (`crates/batata-persistence/src/distributed/port.rs`) with
`write(RaftRequest) -> RaftResponse` and `has_leader()`, plus a `RaftNodePort`
adapter injected by `startup/persistence.rs`. `DistributedPersistService` now
holds `Arc<dyn RaftStatePort>` instead of `Arc<RaftNode>`. This buys
injectability and mockability for the write path and fixes the ownership
boundary for later work.

**Honest limitation.** This does **not** remove the Cargo dependency: the port
trait still names `RaftRequest`/`RaftResponse`, and both backends still use the
consensus crate's column families, key builders and on-disk record schema. A
true inversion requires a third neutral crate holding the wire types, `CF_*`
constants, key builders and `Stored*` records, with byte-compatible bincode
layouts — a multi-PR effort with data-migration risk.

**Consequences.** Reads remain on `RocksDbReader` (a much wider surface, out of
scope here). `AppState.raft_node` and the other ~10 concrete `RaftNode`
consumers are unchanged; only the persistence backend sees the trait object.
On-disk data is untouched.

### ADR-0002: bounded history scans and audit backpressure (Accepted, 2026-11-08)

**Context.** The Phase 3 plan proposed three optimisations for config history
(`RocksDbReader`) plus a lighter distro verify path and a bounded audit queue.
Source review invalidated three of the history assumptions before any code was
changed; the records below are the reasons, so the same ideas are not retried
later.

**Rejected: reverse-iterator paging.** History keys are
`tenant@@group@@data_id@@{id}` (`state_machine/keys.rs`) and the trailing id is
a non-padded decimal string, so lexicographic order does not match numeric
order (`"9"` > `"100"`). `distributed/mod.rs::config_history_get_previous`
already documents the same constraint. A reverse iterator would silently change
page order.

**Rejected: point read for `get_config_history_by_id`.** The API receives only
`nid`; it has no tenant/group/dataId with which to build the compound key. An
`id -> key` index would change the column-family layout and snapshot format,
which is explicitly out of scope.

**Rejected: moving RocksDB scans into `spawn_blocking`.** `distributed/mod.rs`
records that the previous `linearizable_read` + blocking-thread-hop design
collapsed throughput under the cluster RBAC-off A/B benchmark; local reads are
now inline to match the standalone backend. Reintroducing the thread hop is a
known regression risk, not an optimisation.

**Decision (F13).** `raft/history.rs` provides `history_id_from_key` and a
bounded numeric-id top-K selector. `search_config_history` ranks keys by the
numeric id and deserialises only the `offset + page_size` candidates; the
filtered variant keeps the required per-record decode but no longer holds or
sorts the full result set. Any key with an unparsable id, or whose stored id
disagrees with its key, falls back to `history_bounded_page` — the page order
never depends on the key layout being intact. `get_config_history_by_id` skips
decoding rows whose key id cannot match, while still checking `stored.id`.
Deep pages beyond `MAX_BOUNDED_HISTORY_KEYS` use the same bounded fallback.

**Follow-up (F16, 2026-11-09): remove the remaining full scans.** `search_users`,
`search_roles` and `search_permissions` used to deserialise the whole column
family into `Vec<serde_json::Value>`, filter it, then `skip`/`take`. They now
share `paginate_bincode_cf`, which streams the CF, counts every matching row for
an exact `total`, and materialises only the requested window. The history
fallback (`history_bounded_page`) no longer retains one decoded
`StoredConfigHistory` per row either: it ranks compact `(id, key)` entries in an
`offset + page_size` top-K and re-reads the requested window by exact key. For
pages deeper than `MAX_BOUNDED_HISTORY_KEYS` retention is O(number of matching
keys) compact entries instead of O(number of records) full records. The old
full-vector implementation survives only as a `#[cfg(test)]` equivalence oracle.
Wire contracts, key builders and snapshot formats are unchanged.

**Decision (F14).** `DistroDataHandler::get_version` is a narrow port with a
default implementation that delegates to `get_data`; verify rounds call it
instead of rebuilding a full sync payload per key. `NamingInstanceDistroHandler`
overrides it with a direct read of the local distro version counter. The
`sync_round` delta/verify protocol is unchanged.

**Decision (F15).** `AuditPlugin` uses a bounded `mpsc::channel` with an
explicit **drop-newest** policy. The request path uses `try_send` and never
awaits the queue, so a slow audit store cannot apply backpressure to config
publish/remove or grow the heap. `AuditQueueStats` exposes accepted, dropped
and append-failure counters, and full/closed-queue warnings are rate-limited.

**Follow-up (F17, 2026-11-14): shard the standalone config CAS lock.**
`EmbeddedPersistService` previously held one process-wide `Mutex<()>` across
the read -> `casMd5` check -> WriteBatch sequence, which serialized every
standalone config publish. It now holds a configurable number of mutexes
(`batata.persistence.embedded.cas_shards`, default 64) selected by a stable
hash of `(dataId, groupId, tenantId)`; gray publishes deliberately share the
main config's shard. Only the `(Embedded, Standalone)` quadrant benefits:
cluster writes are ordered by the Raft log and external databases use
`SELECT FOR UPDATE`, so both ignore the setting. A same-session A/B (3 rounds
x 15s, one release binary with the shard count injected) measured no
single-key regression and, at the default count, +34.8% RPS on a 64-key write
load and +46.3% on a 256-key load. Shard counts from 16 to 128 are within
noise of one another except for a marginal +4.6% at 128 on the 256-key load.
A concurrent-CAS comparison showed identical semantics to the old single
lock. See [Database](database.md#config-cas-lock-sharding) for the tuning
notes.

**Verification.** Absolute RPS is not used as evidence. The A/B comparison runs
the full-scan reference and the bounded path in the same process, against the
same RocksDB fixture and the same release binary; correctness equivalence is
asserted before the ratio is printed. Both streaming paths are asserted equal to
their legacy full-vector oracles across filters, accurate/fuzzy matching, edge
pages and saturated offsets; test-only decode/materialisation counters prove the
bounded path actually avoids the full work rather than merely returning the same
answer. Every new guard is mutant-checked, including the fallback branches and
the queue counters.
