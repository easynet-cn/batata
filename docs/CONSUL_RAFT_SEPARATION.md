# Consul Raft Separation Design

## Goal

Split the shared Raft instance into two independent Raft groups:
- **Nacos Raft**: Handles config, naming (persistent), auth, namespace operations
- **Consul Raft**: Handles KV, session, catalog operations

Each has its own Raft log, state machine, RocksDB, and log index space.

## Current Architecture

```
                    ┌─────────────────────────────┐
                    │      Shared RaftNode         │
                    │  (one Raft log, one leader)  │
                    └──────────┬──────────────────┘
                               │
                    ┌──────────▼──────────────────┐
                    │   RocksStateMachine          │
                    │                              │
                    │  Nacos CFs:                  │
                    │    config, config_history,    │
                    │    config_gray, namespace,    │
                    │    users, roles, permissions, │
                    │    instances, locks           │
                    │                              │
                    │  Consul CFs:                 │
                    │    consul_kv, consul_sessions,│
                    │    consul_acl, consul_queries │
                    │                              │
                    │  Storage: data/raft/state/   │
                    └─────────────────────────────┘
```

**Problems**:
1. Raft log index is shared — Nacos writes inflate Consul's index space
2. Consul blocking queries can't use Raft index correctly
3. Nacos high-throughput writes compete with Consul for Raft bandwidth
4. Consul plugin can't be cleanly disabled without affecting Raft

## Target Architecture

```
┌───────────────────────┐     ┌───────────────────────┐
│     Nacos RaftNode     │     │    Consul RaftNode     │
│ (existing, unchanged)  │     │  (new, Consul-only)    │
│                        │     │                        │
│ Log:  data/raft/logs/  │     │ Log:  {consul_dir}/    │
│ State: data/raft/state/│     │        raft/logs/      │
│                        │     │ State: {consul_dir}/   │
│ CFs: config, users,    │     │        raft/state/     │
│   roles, permissions,  │     │                        │
│   namespace, instances,│     │ CFs: consul_kv,        │
│   config_history,      │     │   consul_sessions,     │
│   config_gray, locks   │     │   consul_acl,          │
│                        │     │   consul_queries       │
│ gRPC: port 9849        │     │                        │
│ (cluster Raft)         │     │ gRPC: port 9849        │
│                        │     │ (shared, message route) │
└───────────────────────┘     └───────────────────────┘
```

## Design Details

### 1. Consul State Machine

**New file**: `crates/batata-plugin-consul/src/raft/state_machine.rs`

A dedicated `ConsulStateMachine` that:
- Opens its own RocksDB at `{consul_data_dir}/raft/state/`
- Has only 4 CFs: `consul_kv`, `consul_sessions`, `consul_acl`, `consul_queries`
- Applies only Consul-specific `RaftRequest` variants
- Returns `RaftResponse` (same type as existing)

```rust
pub struct ConsulStateMachine {
    db: Arc<DB>,
    // ... snapshot state
}

impl RaftStateMachine for ConsulStateMachine {
    async fn apply(entries: Vec<Entry>) -> Vec<RaftResponse> {
        for entry in entries {
            let (request, log_index) = parse(entry);
            match request {
                ConsulKVPut { .. } => self.apply_kv_put(log_index, ...),
                ConsulSessionCreate { .. } => self.apply_session_create(log_index, ...),
                // ... only Consul operations
            }
        }
    }
}
```

Key difference: **`log_index` is passed to every apply function** and used to set `CreateIndex`/`ModifyIndex` directly. No more client-side index assignment.

### 2. Consul Raft Node

**New file**: `crates/batata-plugin-consul/src/raft/node.rs`

A `ConsulRaftNode` that wraps OpenRaft with `ConsulStateMachine`:
- Own Raft log store at `{consul_data_dir}/raft/logs/`
- Own cluster membership (same 3 nodes, but independent leader election)
- `write_with_index()` returns `(RaftResponse, raft_log_index)`

### 3. Consul Raft Request Types

**New file**: `crates/batata-plugin-consul/src/raft/request.rs`

Extract Consul-specific variants from the existing `RaftRequest` enum into a dedicated `ConsulRaftRequest`:

```rust
pub enum ConsulRaftRequest {
    KVPut { key: String, stored_kv_json: String },
    KVDelete { key: String },
    KVDeletePrefix { prefix: String },
    KVAcquireSession { key: String, session_id: String, stored_kv_json: String },
    KVReleaseSessionKey { key: String, session_id: String, stored_kv_json: String },
    KVReleaseSession { session_id: String, updates: Vec<(String, String)>, ... },
    KVCas { key: String, stored_kv_json: String, expected_modify_index: u64 },
    KVTransaction { puts: Vec<...>, deletes: Vec<...>, ... },
    SessionCreate { session_id: String, stored_session_json: String },
    SessionDestroy { session_id: String },
    SessionRenew { session_id: String, stored_session_json: String },
    SessionCleanupExpired { session_ids: Vec<String>, ... },
}
```

### 4. Per-Table Index (ConsulTableIndex)

**Refactored file**: `crates/batata-plugin-consul/src/index_provider.rs`

```rust
pub enum ConsulTable { KVS, Sessions, Catalog }

pub struct ConsulTableIndex {
    tables: HashMap<ConsulTable, TableState>,
}

struct TableState {
    index: AtomicU64,
    notify: Notify,
}

impl ConsulTableIndex {
    // After successful Raft write, update table high water mark
    pub fn update(&self, table: ConsulTable, raft_log_index: u64) { ... }

    // For blocking queries
    pub async fn wait_for_change(&self, table: ConsulTable, min_index: u64, timeout: Duration) -> bool { ... }

    // For X-Consul-Index header
    pub fn current_index(&self, table: ConsulTable) -> u64 { ... }

    // For multi-table queries (e.g., KV reads need max of kvs + tombstones)
    pub fn max_index(&self, tables: &[ConsulTable]) -> u64 { ... }
}
```

### 5. Index Flow (KV PUT example)

```
HTTP PUT /v1/kv/mykey
    │
    ▼
put_kv handler:
    │── build StoredKV (create_index=0, modify_index=0)
    │── serialize to JSON
    │── consul_raft.write_with_index(ConsulRaftRequest::KVPut { ... })
    │
    ▼
Consul Raft consensus (leader → followers)
    │
    ▼
ConsulStateMachine::apply(entry):  // entry.log_id.index = 42
    │── deserialize StoredKV from JSON
    │── if create_index == 0: create_index = 42 (new key)
    │── modify_index = 42
    │── serialize back to JSON
    │── db.put_cf(consul_kv, key, json)
    │── return RaftResponse::success()
    │
    ▼
Back in handler:
    │── raft_log_index = 42
    │── table_index.update(ConsulTable::KVS, 42)  ← notify blocking queries
    │── X-Consul-Index: 42
    │── body: { "CreateIndex": 42, "ModifyIndex": 42, ... }
```

### 6. Blocking Query Flow

```
GET /v1/kv/mykey?index=42&wait=30s
    │
    ▼
get_kv handler:
    │── table_index.wait_for_change(ConsulTable::KVS, 42, 30s)
    │       │
    │       │── current KVS index = 42, not > 42, block...
    │       │
    │       │   [Another request: PUT /v1/kv/other-key]
    │       │   ConsulStateMachine::apply → log_index = 43
    │       │   table_index.update(KVS, 43)
    │       │   notify.notify_waiters() ← WAKE UP
    │       │
    │       │── KVS index = 43 > 42, return true
    │
    ▼
    │── read KVPair from RocksDB (now has modify_index = 43)
    │── X-Consul-Index: 43
    │── body: { ... }
```

### 7. Cluster gRPC Routing

Current cluster gRPC (port 9849) carries Nacos Raft messages. Consul Raft needs its own channel.

**Options**:
- **A. Separate port** (e.g., 9850): Simplest but uses another port
- **B. Multiplexed on 9849**: Add a service type prefix to route messages

**Recommended**: Option A for simplicity. Add config `batata.plugin.consul.raft.port` (default: main_port + 1002, e.g., 9850).

### 8. File Structure

```
crates/batata-plugin-consul/src/
├── raft/
│   ├── mod.rs              # Consul Raft module
│   ├── node.rs             # ConsulRaftNode
│   ├── state_machine.rs    # ConsulStateMachine
│   ├── request.rs          # ConsulRaftRequest enum
│   ├── log_store.rs        # Consul RocksDB log store (or reuse existing)
│   └── grpc_service.rs     # Consul Raft cluster gRPC
├── index_provider.rs       # ConsulTableIndex (per-table)
├── kv.rs                   # KV service (uses ConsulRaftNode)
├── session.rs              # Session service (uses ConsulRaftNode)
├── catalog.rs              # Catalog handlers
└── ...
```

### 9. Cleanup: Remove Consul from Shared Raft

After Consul has its own Raft:
- Remove all `ConsulKV*`, `ConsulSession*` variants from `batata_consistency::RaftRequest`
- Remove `apply_consul_*` methods from `RocksStateMachine`
- Remove `CF_CONSUL_*` column families from shared state machine
- `ConsulKVService::with_raft()` takes `ConsulRaftNode` instead of `RaftNode`

### 10. Startup Flow

```rust
// main.rs (simplified)

// 1. Nacos Raft (existing, unchanged)
let nacos_raft = RaftNode::new_with_db(..., "data/raft/")?;

// 2. Consul Raft (new, only if consul enabled)
let consul_raft = if consul_enabled {
    let consul_dir = config.consul_data_dir(); // "data/consul_rocksdb"
    Some(ConsulRaftNode::new(..., &consul_dir)?)
} else {
    None
};

// 3. Consul services
let consul_services = if consul_enabled {
    ConsulServices::new(consul_raft, ...)
} else {
    None
};
```

### 11. Migration

Existing data in `data/raft/state/` (consul CFs) needs migration to `data/consul_rocksdb/raft/state/`. Provide a migration tool or handle on first startup:

1. Check if old consul CFs exist in `data/raft/state/`
2. If yes, copy data to new location
3. Delete old CFs from shared state machine

## Implementation Order

1. **Create Consul Raft module** (`raft/` directory in plugin-consul)
2. **Implement ConsulStateMachine** with log_index-based CreateIndex/ModifyIndex
3. **Implement ConsulRaftNode** with write_with_index
4. **Implement ConsulTableIndex** (per-table index + notify)
5. **Refactor KV/Session services** to use ConsulRaftNode + ConsulTableIndex
6. **Add Consul Raft gRPC** for cluster communication
7. **Update startup** to create Consul Raft separately
8. **Remove Consul operations** from shared Raft state machine
9. **Test**: All Consul cluster tests should pass
10. **Migration**: Handle existing data
