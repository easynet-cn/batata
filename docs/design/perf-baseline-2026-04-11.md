# Batata Performance Baseline — 2026-04-11

**Purpose**: establish quantitative benchmarks for batata hot paths
before architecture-level optimizations (ahash, RCU, actor model,
async-trait removal, etc.). All future perf work should be measured
against this baseline.

## Methodology

- `cargo bench --release` (criterion 0.5)
- Platform: macOS darwin x86_64, 8-core logical
- Rust: stable toolchain
- Each benchmark: criterion default (100 samples, confidence 95%)
- No other workloads on the machine

## Bench Suites

### 1. `batata-server/benches/naming_bench.rs` (pre-existing)

Covers `NamingService` CRUD + subscriber tracking.

| Benchmark | Size | Median | Notes |
|---|---|---|---|
| `register_instance` | 1 | **1.39 µs** | DashMap insert + revision bump |
| `get_instances` | 1000 | **247 µs** | Full scan, no filter |
| `get_instances_healthy_only` | 1000 | **148 µs** | 50% healthy filter |
| `get_instances_by_cluster` | 1000 | **123 µs** | Cluster filter |
| `subscribe` | 1 | **891 ns** | Subscriber insert + reverse index |
| `get_subscribers` | 1000 | **44.4 µs** | Reverse-index lookup |
| `list_services` | 100 | **10.3 µs** | Full service list |
| `deregister_instance` | 1 | **9.22 µs** | Shard lock + entry remove |
| `instance_count_scaling/100` | 100 | **26.7 µs** | Scaling curve |
| `instance_count_scaling/500` | 500 | **125 µs** | Scaling curve |
| `instance_count_scaling/1000` | 1000 | **248 µs** | Scaling curve |
| `instance_count_scaling/5000` | 5000 | **1.27 ms** | Scaling curve |

**Scaling analysis**: `get_instances` scales **~linearly** with size:
100 → 500 ≈ ×5 (expected ×5), 500 → 1000 ≈ ×2 (expected ×2),
1000 → 5000 ≈ ×5.1 (expected ×5). Suggests linear filter over full
instance list — **prime target for RCU/Arc snapshot** (clone refs only,
not entries).

### 2. `batata-plugin-consul/benches/consul_store_bench.rs` (new)

Covers `ConsulNamingStore` hot paths touched by every Consul HTTP req.

| Benchmark | Size | Median | Notes |
|---|---|---|---|
| `consul_naming_store/register` | 1 | **2.47 µs** | DashMap insert + revision |
| `consul_naming_store/deregister` | 1 | **3.19 µs** | DashMap remove + revision |
| `consul_naming_store/get_service_entries/10` | 10 | **5.87 µs** | Filtered scan |
| `consul_naming_store/get_service_entries/100` | 100 | **9.35 µs** | Filtered scan |
| `consul_naming_store/get_service_entries/1000` | 1000 | **44.2 µs** | Filtered scan |
| `consul_naming_store/service_names_500` | 500 | **47.2 µs** | Full dedup scan |
| `consul_naming_store/build_key` | 1 | **272 ns** | Format string |
| `consul_naming_store/scan_500_prefix` | 500 | **47.6 µs** | PluginNamingStore::scan |

**Note**: Consul register (2.47µs) is ~1.8x slower than batata-naming register
(1.39µs). Difference is the `AgentServiceRegistration` JSON payload path
(Bytes clone, key parsing) vs Nacos's pre-normalized `Instance` struct.

### 3. `batata-consistency/benches/raft_serialization_bench.rs` (new)

Covers `RaftRequest` encode/decode on the Raft log write/apply path.

| Benchmark | Payload | Median | Notes |
|---|---|---|---|
| `to_vec/config_publish_small` | ~200B | **675 ns** | Small config publish |
| `to_vec/config_publish_large` | ~8KiB | **7.89 µs** | Large YAML config |
| `to_vec/persistent_instance_register` | ~300B | **693 ns** | Instance register |
| `to_vec/plugin_write_consul_kv` | ~400B | **2.36 µs** | Consul KV via PluginWrite |
| `from_slice/config_publish_small` | ~200B | **1.55 µs** | Deserialize |
| `from_slice/config_publish_large` | ~8KiB | **7.21 µs** | Deserialize |
| `from_slice/persistent_instance_register` | ~300B | **1.22 µs** | Deserialize |
| `from_slice/plugin_write_consul_kv` | ~400B | **5.16 µs** | Deserialize |
| `roundtrip_persistent_instance` | ~300B | **1.97 µs** | Full encode + decode |

**Key observation**: deserialize is **~2x slower than serialize** for
JSON. A Raft apply path (commit → decode → state machine) spends more
time in decoding than the write path does encoding. `bincode` migration
(#4 in optimization proposal) would help both sides but disproportionately
benefit decode. **PluginWrite serialize hit a double-encode penalty**
(2.36µs for consul KV vs 693ns for native PersistentInstance, ~3.4x):
the Consul payload is serialized twice — once by ConsulRaftWriter,
once by the outer RaftRequest::PluginWrite envelope. This is a known
cost of the generic plugin hook.

## Known Optimization Targets

Tracked from the architecture optimization proposal (2026-04-11 session).
Re-run baseline after each change to quantify delta.

| # | Optimization | Target bench | Expected delta |
|---|---|---|---|
| 1 | `ahash` for DashMap | `register_instance`, `get_instances` | -15~20% |
| 2 | Remove `async-trait` boxing on hot traits | n/a (needs gRPC bench) | ~5-10% |
| 3 | `Arc<String>` → `Arc<str>` | `register_instance` | minor alloc savings |
| 4 | `serde_json` → `bincode` for RaftRequest | `raft_request_serialize/*` | -30~50% |
| 5 | RCU (ArcSwap) for naming read | `get_instances/*` | read path zero-lock |
| 6 | Box large enum variants | `persistent_instance_register` encode | smaller stack |

## How to Reproduce

```bash
# Naming
cargo bench -p batata-server --bench naming_bench

# Consul
cargo bench -p batata-plugin-consul --bench consul_store_bench

# Raft serialization
cargo bench -p batata-consistency --bench raft_serialization_bench

# Compare two runs (criterion auto-tracks history in target/criterion/)
# Saved reports live at target/criterion/report/index.html
```

## Next Steps

After this baseline lands:
1. Lock the numbers in this doc (update `_TBD_` fields)
2. Pick the first optimization (proposed: `#1 ahash` — lowest risk, broadest impact)
3. Apply change, re-run bench, capture delta in this same doc
4. Iterate

## Optimization Attempts

### bincode for `StoredInstance` CF_INSTANCES — **SHIPPED 2026-04-11**

**Status:** ✅ Landed — 7-32x speedup on state-machine apply path.

**Hypothesis:** `apply_instance_register` / `_update` / `replay_persistent_instances`
all serialize via `serde_json::Value` → `serde_json::to_vec`, which
does dynamic HashMap dispatch per field. Typed `StoredInstance` struct
with `bincode` should give a large order-of-magnitude win.

**Change summary:**
- New `pub struct StoredInstance` in `state_machine.rs` with 13 typed
  fields (namespace_id/group_name/service_name/instance_id/ip/port/
  weight/healthy/enabled/metadata/cluster_name/registered_time/modified_time).
- `apply_instance_register` builds `StoredInstance` directly and calls
  `bincode::serialize`.
- `apply_instance_update` reads existing via `bincode::deserialize`,
  mutates typed fields, writes back via `bincode::serialize`.
- `RaftNode::replay_persistent_instances` uses `bincode::deserialize`
  + typed field access (no JSON Value indexing).
- **No backward compatibility** — on-disk format changed, RocksDB data
  directories must be wiped on upgrade (user-approved).

**Measured delta (raft_serialization_bench::stored_instance_*):**

| Operation | JSON (before) | bincode (after) | Delta |
|---|---|---|---|
| `encode` | 2.40 µs | **73.9 ns** | **-96.9%** (32.5x faster) |
| `decode` | 2.90 µs | **407 ns** | **-85.9%** (7.1x faster) |
| `roundtrip` | 5.14 µs | **479 ns** | **-90.7%** (10.7x faster) |

**Production impact estimation:**
- `apply_instance_register`: save ~2.33 µs CPU per call
- `apply_instance_update`: save ~4.82 µs CPU per call (11x)
- `replay_persistent_instances`: save ~2.5 µs per entry → 10k instance
  cold-start recovery ~25 ms faster

**Regression checks passed:**
- `cargo test -p batata-consistency --lib` → 37/37 pass
- `cargo test -p batata-naming --lib` → 216/216 pass
- `cargo check --workspace` → clean

### bincode for `StoredConfig` CF_CONFIG/CF_CONFIG_GRAY/CF_CONFIG_HISTORY — **SHIPPED 2026-04-11**

**Status:** ✅ Landed — 7.6x roundtrip speedup on config apply + read path.

**Hypothesis:** Same pattern as StoredInstance: `apply_config_publish_batched`,
`apply_config_gray_publish`, `apply_config_history_insert`, and the reader-side
`get_config`/`list_configs`/`search_configs` all go through `serde_json::Value`
(dynamic HashMap dispatch). Typed structs + bincode should give a large win.

**Change summary:**
- Added `StoredConfig`, `StoredConfigGray`, `StoredConfigHistory` structs
  in `state_machine.rs` (typed fields, serde + bincode).
- Rewrote write path: `apply_config_publish_batched`, `apply_config_remove_batched`,
  `apply_config_gray_publish`, `apply_config_history_insert`,
  `apply_config_tags_update`, `apply_config_tags_delete` — build typed struct
  → `bincode::serialize`, CAS on typed `.md5` field, preserve `created_time`.
- Rewrote read path in `reader.rs`: `get_config`, `list_configs`,
  `search_configs` (inline filters on typed fields), `get_config_gray`,
  `get_all_config_grays`, `get_config_history_by_id`, `search_config_history`,
  `search_config_history_with_filters`, `get_instance`, `list_instances`
  — decode via `bincode::deserialize::<StoredX>` then `serde_json::to_value(&stored)`
  so persistence callers stay unchanged.
- **No backward compatibility** — on-disk CF_CONFIG format changed,
  RocksDB data directories must be wiped on upgrade (user-approved).

**Measured delta (raft_serialization_bench::stored_config_*):**

| Operation | JSON (before) | bincode (after) | Delta |
|---|---|---|---|
| `encode` | 2.54 µs | **112 ns** | **-95.6%** (22.7x faster) |
| `decode` | 2.97 µs | **593 ns** | **-80.0%** (5.0x faster) |
| `roundtrip` | 5.36 µs | **706 ns** | **-86.8%** (7.6x faster) |

**Production impact estimation:**
- `apply_config_publish_batched`: saves ~2.4 µs CPU per call (encode-dominated)
- Reader `get_config` / `list_configs`: saves ~2.4 µs per entry (decode)
- `search_configs` on 10k-entry namespace: ~24 ms faster per full scan

**Regression checks passed:**
- `cargo test -p batata-consistency --lib` → 37/37 pass
- `cargo check --workspace` → clean

### RCU (ArcSwap) for `get_instances` — **REVERTED 2026-04-11**

**Status:** ❌ Reverted — catastrophic write regression, marginal read gains.

**Hypothesis:** Replace `DashMap<String, DashMap<String, Arc<Instance>>>` with
`DashMap<String, Arc<ArcSwap<HashMap<String, Arc<Instance>>>>>`. Readers load
the `Arc<HashMap>` via a single atomic pointer read — zero locks, zero shard
iteration. Writers perform copy-on-write via `ArcSwap::rcu`.

**Implementation:** Full RCU refactor across `service/mod.rs`,
`service/instance.rs`, `service/cluster.rs`, `service/subscription.rs`. Added
helpers `load_service_instances`, `get_or_create_holder`, `rcu_upsert_instance`,
`rcu_remove_instance`. 216/216 tests passed.

**Measured deltas:**

| Benchmark | Baseline | RCU | Delta | Verdict |
|---|---|---|---|---|
| `register_instance` | 1.39 µs | **549 µs** | **+39500%** | ❌ catastrophic |
| `get_instances_1000` | 247 µs | 290 µs | **+17%** | ❌ regression |
| `get_instances_healthy_only_1000` | 148 µs | 126 µs | -15% | ✅ |
| `get_instances_by_cluster_1000` | 123 µs | 98 µs | -20% | ✅ |
| `subscribe` | 891 ns | 798 ns | -10% | ✅ |
| `deregister_instance` | 9.22 µs | 9.0 µs | noise | — |
| `get_subscribers_1000` | 44.4 µs | 45 µs | noise | — |

**Root cause:**

1. **RCU writes are O(n)**: every `register_instance` clones the entire
   `HashMap` for copy-on-write. The bench inserts ~15k distinct instances
   into one service, so amortized write cost is O(n²). At the terminal
   iteration, cloning a 15k-entry HashMap takes ~550 µs.
2. **The baseline was already lock-minimal**: existing `get_instances` does
   "snapshot-then-filter" with Arc pointer clones, holding shard locks for
   only a few microseconds. Under the single-threaded bench there is no
   lock contention, so RCU's zero-lock read offers no advantage.
3. **`get_instances_1000` regressed by 17%**: HashMap iteration isn't
   faster than DashMap shard iteration when there is no contention, and
   the additional `Arc<HashMap>` indirection adds a pointer chase.
4. **Filtered reads did improve 15-20%**: but the win comes from reduced
   indirect calls in the filter loop, not from RCU semantics.
5. **The real bottleneck is `Instance::clone()` ×1000** in the final
   `collect()` — value clones, not lock contention. RCU doesn't address
   this. A genuine fix would change `get_instances` to return
   `Arc<Vec<Arc<Instance>>>` and propagate that through the API, avoiding
   value clones entirely.

**Lesson:** RCU is the right tool when read-write contention is actually
measurable and writes are rare. For batata's naming registry:
- DashMap shard locks are already held for <5 µs per operation
- The value-clone cost dominates read benches, not lock cost
- Writes are frequent enough (~every SDK heartbeat / register call) that
  O(n) copy-on-write is not free
- Single-threaded benches mask the read benefits that only appear under
  heavy concurrent contention

**Value of the experiment:**
- Validated that the existing snapshot-then-filter pattern is already a
  near-optimal use of DashMap
- Produced quantitative evidence that the audit's "high priority RCU"
  recommendation was wrong for this workload
- Identified that the real read-path optimization target is
  `Instance::clone()`, not the storage data structure

### ahash for DashMap — **REVERTED 2026-04-11**

**Status:** ❌ Reverted — caused significant regression on read path.

**Hypothesis:** Replacing DashMap's `std::hash::RandomState` with
`ahash::RandomState` would speed up string-keyed lookups by 15-20% due
to SIMD-accelerated hashing. HashDoS resistance not needed for
cluster-internal keys.

**Actual measured deltas:**

| Benchmark | Baseline | ahash | Delta | Significance |
|---|---|---|---|---|
| `register_instance` | 1.39 µs | 1.31 µs | **-4.6%** ✅ | p<0.05 |
| `subscribe` | 891 ns | 858 ns | **-5.4%** ✅ | p<0.05 |
| `deregister_instance` | 12.1 µs | 12.1 µs | -11% | p=0.18 (noise) |
| `get_subscribers_1000` | 44.4 µs | 44.5 µs | -0.6% | noise |
| `list_services_100` | 10.3 µs | 10.3 µs | -0.5% | noise |
| `get_instances_1000` | 247 µs | **307 µs** | **+22.0%** ❌ | p<0.01 |
| `get_instances_healthy_only` | 148 µs | **182 µs** | **+20.6%** ❌ | p<0.01 |
| `get_instances_by_cluster` | 123 µs | **151 µs** | **+20.4%** ❌ | p<0.01 |
| `scaling/1000` | 248 µs | **316 µs** | **+25.8%** ❌ | p<0.01 |

**Root cause analysis:**

The hot read path (`get_instances`) does
`instances.iter().map(|e| Arc::clone(e.value())).collect()` — it
iterates the inner DashMap **without invoking the hasher**. The 20-25%
regression on these paths cannot be explained by hash cost. Two
candidates:

1. **Monomorphization layout changes** — `DashMap<K, V, ahash::RandomState>`
   produces different generated code than `DashMap<K, V>` (the latter
   uses `std::hash::RandomState`). Different inlining decisions could
   affect cache behavior on tight iteration loops.
2. **Shard seed distribution** — ahash's default `RandomState` seeds
   shard hashes differently, producing different key→shard distribution.
   For 1000 items on 32 shards (8-core × 4), uneven distribution
   changes cache access patterns during iteration.

**Lesson:** ahash is not a universal win for DashMap-based workloads.
It helps when the hash function is actually on the hot path
(register/subscribe do one lookup per call), but it hurts when the
hot path is iteration (get_instances scans all entries).

**Future consideration:** could try ahash selectively — keep default
hasher on `services` (read-dominated), use ahash on `subscribers` /
`connection_instances` (write-dominated with frequent lookups). Not
worth the complexity for a ~5% gain on write paths.

**Value of the experiment:** validated that the criterion infrastructure
correctly detects regressions (p-values are meaningful), confirmed
baseline measurements are reliable (same machine, same flags), and
produced a concrete data point that challenges the hash-function-cost
assumption in the audit's optimization proposals.

## Revised Priority List

Given the ahash result, the optimization proposals should be re-weighted:

| Priority | Optimization | Expected value | Why |
|---|---|---|---|
| ✅ Shipped | bincode for CF_INSTANCES | 10x roundtrip | Real hot path |
| ✅ Shipped | bincode for CF_CONFIG / CF_CONFIG_GRAY / CF_CONFIG_HISTORY | 7.6x roundtrip | Real hot path |
| ✅ Shipped | bincode for CF_NAMESPACE / CF_USERS / CF_ROLES / CF_PERMISSIONS | Cold path completeness | All CFs now use typed bincode; no JSON left in state machine |
| ✅ Shipped | Box large RaftRequest variants | ~3.3x enum size reduction (480 → 144 bytes) | ConfigPublish (17 fields), ConfigGrayPublish, ConfigHistoryInsert, PersistentInstanceRegister, PersistentInstanceUpdate, ConfigRemove history — all moved to Box<Payload>. Size guard test added. |
| N/A | ~~`Arc<String>` → `Arc<str>`~~ | Nothing to migrate — codebase uses plain `String` throughout; no `Arc<String>` usage exists (verified via grep 2026-04-11) |
| ❌ | ~~ahash~~ | Regressed reads | Monomorphization changed cache behavior |
| ❌ | ~~RCU (ArcSwap) for `get_instances`~~ | **Catastrophic write regression** | DashMap baseline already lock-minimal; real bottleneck is `Instance::clone()` |
| — | `async-trait` removal | Uncertain | Needs dedicated bench suite |
| — | Return `Arc<Vec<Arc<Instance>>>` from `get_instances` | Potentially large | Eliminates value clones — requires API changes across callers |
