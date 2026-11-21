# Raft Timeout Baseline, Derivation, and Observability

> This document answers three questions: what the timeouts actually are, what they are
> based on, and how to tell whether the system is healthy.
> It covers tasks T1 (baseline audit), T2 (metrics), T3 (linked timeouts),
> T4 (root cause), T5 (gate), and T6 (root-cause fix).
> Every number is taken from source code. Upstream conclusions carry verifiable
> anchors; anything without an official anchor is explicitly marked "reference only".

## 1. Bottom Line First

1. **The previous description was wrong.** Batata's election window used to be
   described as `3000-6000ms`, but that is only `RaftConfig::default()` (the library
   default, used by unit tests and `RaftNodeBuilder`). The production path
   `Configuration -> typed_config -> startup/persistence.rs` uses
   **1000ms min / 2000ms max / 200ms heartbeat**. The dimensional mismatch
   "leaderless wait 5s < election window 6s" therefore **only holds on the library
   default path**; in production 5s is actually larger than 2s.
2. The nature of the problem is unchanged, though: three layers of defaults
   disagreed with each other, every wait was an unrelated hard-coded constant,
   no inequality was validated anywhere, and the Raft core paths exported zero
   metrics. That is exactly why an occasional anomaly could not be diagnosed.
3. The current approach (T3, implemented): every wait is derived by
   `RaftConfig::timing()` from the election window and the RPC timeout, and
   `RaftConfig::validate()` checks the invariants. A node fails fast at startup.

## 2. Three Layers of Defaults and the Effective Values

| Layer | Location | election min | election max | heartbeat | Notes |
|---|---|---|---|---|---|
| Library default | `crates/batata-consistency/src/raft/config.rs` `RaftConfig::default()` | 3000ms | 6000ms (`to_openraft_config` uses `2 x min`) | 300ms | Unit tests, `RaftNodeBuilder` |
| **Production default** | `crates/batata-server-common/src/model/typed_config.rs:338-346` | **1000ms** | **2000ms** | **200ms** | Injected into `RaftConfig` at `startup/persistence.rs:348` |
| Config-file comment | `conf/application.yml` | 1000ms | not stated (computed as x2 in code) | 200ms | Documentation only; the keys are commented out |

The getter docs in `crates/batata-server-common/src/model/config.rs` previously
claimed "default 5000 / 1000", which did not match reality; T1 corrected them.

Other production values: `rpc_request_timeout_ms = 5000`,
`snapshot_transfer_timeout_ms = 30000`, `forward.max_retries = 3`,
`forward.initial_delay_ms = 200`.

`rpc_request_timeout_ms` is a **channel-level** setting, not a per-RPC
deadline. Its real scope is: the tonic channel ceiling, the fallback when
openraft supplies a zero `RPCOption` TTL, the floor that
`snapshot_transfer_timeout_ms` must exceed, and the `R` term in the
election-cycle derivation. openraft 0.10.1 passes `heartbeat_interval` as
`RPCOption::hard_ttl()` for heartbeat/streaming replication, so an individual
AppendEntries or Vote RPC is bounded by `soft_ttl()` instead (see section 4).

## 3. Pre-Change Call-Site Inventory (T1 Audit)

| Location | Old value | Kind | Problem |
|---|---|---|---|
| `node.rs` `CLIENT_WRITE_TIMEOUT` | 15s | hard-coded | No derivation from the election window |
| `node.rs` `LEADERLESS_MAX_POLLS x LEADERLESS_POLL_INTERVAL` | 25 x 200ms = 5s | hard-coded | Gives up too early under the library default 6s window; unjustifiably long under the production 2s window |
| `node.rs` `wait_for_applied(..., 5s)` | 5s | hard-coded at call site | Same value as the leaderless budget but unrelated to it |
| `config.rs` `election_timeout_ms * 2` | x2 | the only derived value | No comment, no validation |
| `config.rs` field comments | 5000ms / 1000ms | documentation | Drifted from the real defaults |
| `model/config.rs` getter comments | 5000ms / 1000ms | documentation | Drifted from the typed defaults |

## 4. Upstream / Framework Comparison (with Anchors)

| Source | heartbeat | election | write / RPC timeout | Evidence |
|---|---|---|---|---|
| openraft 0.10.1 (this project's framework) | 50ms | 150-300ms | no overall deadline | `~/.cargo/registry/.../openraft-0.10.1/src/config/config.rs:59-84` `DEFAULTS` |
| openraft official guidance | `approx. election/3` | `approx. 10-20x RTT` | - | docs.rs `openraft/0.10.1/struct.Config.html`, section "Timing Constraints" |
| openraft hard constraint | `election_timeout_min > heartbeat_interval` | `min < max` | - | Same file, `Config::validate()` (`:901-935`) |
| hashicorp/raft `DefaultConfig()` | 1000ms | 1000ms | Commit 50ms, LeaderLease 500ms | `github.com/hashicorp/raft/config.go` `DefaultConfig()` |
| hashicorp/raft `ValidateConfig` | `lease <= heartbeat <= election` | | | Same file, verbatim: `LeaderLeaseTimeout ... cannot be larger than heartbeat timeout`, `ElectionTimeout ... must be equal or greater than Heartbeat Timeout` |
| Consul default (multiplier=5) | 5000ms | 5000ms | `rpc_hold_timeout = 7s` | Consul official [General parameters](https://developer.hashicorp.com/consul/docs/reference/agent/configuration-file/general) and [Server Performance](https://developer.hashicorp.com/consul/docs/v1.18.x/reference/architecture/server) |
| etcd (reference only, no official anchor) | 100ms | 1000-2000ms | - | Community material: `ElectionTick=10 / HeartbeatTick=1 / TickMs=100`. Claims about the PreVote default contradict each other, so **this is not used as a derivation basis**. |
| **batata production (before the change)** | **200ms** | **1000-2000ms** | **15s / 5s / 5s** | No linkage, no validation, no metrics |

Consul's official definition of `rpc_hold_timeout`: *"A duration that a client or
server will retry internal RPC requests during leader elections. Under normal
circumstances, this can prevent clients from experiencing 'no leader' errors.
Defaults to 7s."* Batata's leaderless wait is semantically exactly that:
**an election window plus one RPC retry budget**. Consul derives all of its timing
from a single `raft_multiplier`; batata previously hard-coded each constant.

Consul's documentation also names the suspicious behavior seen in this project:
*"Spurious leader elections can be caused by networking issues between the servers
or insufficient CPU resources"*, and recommends observing
`consul.raft.leader.lastContact`. This motivated T2/T4.

### RPCOption Timeout Semantics (openraft 0.10.1)

`RPCOption::hard_ttl()` is the point at which openraft may cancel an in-flight
RPC. Its source documentation says network implementations "should use
`RPCOption::soft_ttl` to configure their transport timeout, deadline, or
equivalent mechanism", and `soft_ttl()` is `hard_ttl * 3 / 4` (the ratio is
explicitly not part of the contract). openraft sets the hard TTL from
`heartbeat_interval` in `src/core/heartbeat/worker.rs:106` and
`src/replication/mod.rs:334`; `src/core/heartbeat/worker.rs:118` additionally
wraps the call in `C::timeout(heartbeat_interval)`. Batata's transport
therefore enforces `soft_ttl()` (150ms with the production 200ms heartbeat),
clamped so it can never outlive the hard TTL, and falls back to
`rpc_request_timeout_ms` only when the hard TTL is zero. `RaftConfig::validate()`
rejects `rpc_request_timeout_ms <= heartbeat_interval_ms`, because a channel
timeout at or below the heartbeat would preempt the soft TTL and make healthy
heartbeats look like network failures.

### Two openraft Defaults (Key Clue for T4, Handled in T6)

- `enable_pre_vote: Option<bool>` defaults to `None`. docs.rs, verbatim: *"None
  (the default) leaves the choice to Openraft, which currently treats it as
  disabled."* Before the change, batata's `to_openraft_config()` used
  `..Default::default()` and did not set this field, so **PreVote was off**;
  T6 set it explicitly to `Some(true)` (see section 6.2).
- `enable_leader_restore` is enabled by default: `Config::enable_leader_restore()`
  is `unwrap_or(true)`, and `engine_impl.rs:211` calls `elect()` immediately at
  startup when a restarted node was the former leader. The regression matrix
  restarts nodes between its six combinations, so this can amplify term
  inflation. T3 sets it explicitly to `Some(false)`. The upstream rationale for
  the default is to avoid reusing a log id when the old leader replicated an
  entry before its local flush; `RocksLogStore::append` fsyncs every batch
  before returning and signals `IOFlushed` only after that, so the window does
  not exist in batata. With PreVote enabled, the restarted node follows the
  normal pre-elect / election timing instead.
- Both were initially used to explain "multiple elections in steady state".
  After the T6-a measurement, the real trigger was confirmed to be a synchronous
  lock held across `.await` (section 6.1 point 3 / section 6.2); having PreVote
  off merely amplified the consequence of the stall.

## 5. Derivation Formulas and Invariants (T3, Implemented)

Let `E = election_timeout_ms`, `E_max = 2E`, `H = heartbeat_interval_ms`,
`R = rpc_request_timeout_ms`, and let `C = E_max + R` be "one election cycle".
`R` is the configured RPC round used to size that cycle and the channel
ceiling; it is deliberately not a per-RPC deadline (see section 4):

| Derived value | Formula | Production | Library default | Meaning |
|---|---|---|---|---|
| `election_timeout_min/max` | `E` / `2E` | 1000 / 2000ms | 3000 / 6000ms | openraft election window |
| `election_cycle` | `C = E_max + R` | 7000ms | 11000ms | Aligned with Consul `rpc_hold_timeout` (5s election -> 7s) |
| `leaderless_wait` | `C` | 7000ms | 11000ms | Follower waits for a leader to appear |
| `apply_wait` | `C` | 7000ms | 11000ms | Follower waits for its local FSM to catch up |
| `client_write` | `2C` | 14000ms | 22000ms | Covers one leadership change during a write |
| leaderless polling | `H/2`, clamped to `[10ms, 200ms]` | 100ms x 70 polls | 150ms x 73 polls | Faster than an election, so a Candidate state cannot be missed |

`RaftConfig::validate()` enforces:

- `H > 0`, `E > H` (openraft hard constraint), `R > 0`;
- `snapshot_transfer_timeout_ms > R` (previously required only by a comment, with
  no code guarantee);
- `forward_initial_delay_ms > 0` (when `forward_max_retries > 0`);
- overflow checks (`2E`, `E_max + R`, `2C`);
- `leaderless_wait >= E_max` (**the assertion that removes the 5s < 6s
  mismatch**) and `client_write > leaderless_wait`.

`RaftNode::new_with_full_options` calls `validate()` at node creation, before any
directory or RocksDB instance is opened, so it fails fast, and logs the derived
values at startup for on-site verification.

## 6. Diagnosability Rules (T2 Metrics + T5 Gate)

T2 added `crates/batata-consistency/src/raft/telemetry.rs`, which subscribes to
openraft's metrics watch channel (not periodic polling: a measured 100ms poll
missed a Candidate state on the leader that lasted only a few tens of
milliseconds). It exports Prometheus metrics and structured logs under the
`batata_raft_health` target (naming aligned with Consul telemetry, prefixed with
`batata_`):

| Type | Metric | Purpose |
|---|---|---|
| Gauge | `batata_raft_term/role/leader_id/last_log_index/last_applied_index/apply_lag/local_committed_index/cluster_committed_index/commit_lag/steady_state_elections` | State and lag |
| Gauge | `batata_raft_resource_cpu_percent`, `batata_raft_resource_process_memory_mb` | Latest resource sample in the Raft namespace (one scrape is self-contained) |
| Gauge | `batata_raft_anomaly_cpu_percent`, `batata_raft_anomaly_process_memory_mb` (labelled by `event`) | Resources captured at the instant of an anomaly |
| Counter | `batata_raft_elections_total`, `steady_state_elections_total`, `leader_changes_total`, `term_changes_total`, `client_write_timeouts_total`, `apply_wait_timeouts_total`, `forwarded_writes_total`, `forward_failures_total`, `leaderless_give_ups_total` | Anomaly counts |
| Counter | `batata_raft_anomalies_with_resource_context_total`, `batata_raft_high_cpu_anomalies_total` (labelled by `event`) | Correlation counts |
| Histogram | `batata_raft_election_duration_seconds`, `client_write_duration_seconds`, `apply_wait_duration_seconds`, `forward_duration_seconds` | Latency distributions |

Decision rules:

- **Steady state** (after a leader has already been seen) showing Candidate =>
  record `steady_state_election` and emit WARN; the first election of a cold start
  is not an anomaly (corresponding to Consul's `candidate == 0 while leader
  exists`).
- The gate counts only `steady_state_election` / `apply_wait_timeout` /
  `client_write_timeout` / `leaderless_give_up` as blockers and requires all of
  them to be 0. A cold-start election is only recorded (INFO), not counted. If a
  cluster combination is missing `telemetry_started`, the gate fails closed, so
  "telemetry never started" cannot be misread as healthy.
- Correlation rules: CPU saturated at election time => runtime starvation; CPU
  normal but AppendEntries/forward failing => network or port cross-talk;
  `apply_lag` or `apply_wait` spike => RocksDB/FSM blocking. T4 added
  `commit_lag` (cluster-committed minus local-committed) to separate "entries
  have not reached this node" (replication/network) from "entries arrived but
  were not applied" (FSM blocking), which `apply_lag` alone cannot distinguish.
- The resource side reuses the existing periodic `system_cpu_usage_percent` /
  `process_memory_bytes` sampling; no second metrics stack was added. Follow-up
  T6 made that sampling available to Raft anomaly logging through a provider,
  so CPU and memory no longer have to be aligned by hand (section 6.3).

### 6.1 T4 Steady-State Election Root Cause (Measured Evidence)

Evidence directory `/tmp/batata-raft-health-1795179141/`: fresh release binary,
3-node cold start. Both the Go Consul SDK suite and the `batata-consul-client`
suite returned `rc=0`, but the T2 metrics captured:

| Symptom | Measurement |
|---|---|
| node1 steady-state elections / apply timeouts | 3 / 3; final term 7, 7 leader changes |
| node2 | stable (0 steady-state elections) |
| node3 | 7 term changes, following node1's term increases |
| `apply_lag` at the end | 0 (not "behind but catching up", but a complete stall during the elections) |
| CPU / memory | `system_cpu_usage_percent` 4.29-11.27%, process 171-206MB |

All three openraft election deadlines were handled **about 3.6-3.7s late**:

| `lease_expires_at` | Actually fired | Delay |
|---|---|---|
| 12:53:08.950191 | 12:53:12.639813 | 3.690s |
| 12:53:17.221654 | 12:53:20.795244 | 3.574s |
| 12:53:26.320849 | 12:53:29.891071 | 3.570s |

Before each firing, node1 also showed about **7s of process-wide log silence**
(e.g. 12:53:05.33 -> 12:53:12.64). All three deadline lines in the log showed
`pre_vote_retry_at: None`, directly confirming that PreVote was disabled.

Source-level explanation chain (revised after the T6-a review; the final root
cause is in section 6.2):

1. `crates/batata-server/src/main.rs` uses `#[actix_web::main]`; `actix-rt 2.16.0`
   from `Cargo.lock` uses `Builder::new_current_thread()` at `runtime.rs:23`, and
   `openraft-rt-tokio`'s `C::spawn` is simply `tokio::spawn`. RaftCore, the SM
   worker, the Raft gRPC server, and xDS snapshot publication all end up on the
   same `main` thread.
2. `network.rs::effective_timeout()` directly adopts `RPCOption::hard_ttl()` when
   it is non-zero. openraft 0.10.1's `heartbeat/worker.rs:105` and
   `replication/mod.rs:333` set the TTL to `heartbeat_interval`, and
   `heartbeat/worker.rs:118` wraps it in another `C::timeout(200ms)`. Therefore
   **`rpc_request_timeout_ms=5000` in the production config has no effect at all
   on AppendEntries / Vote; the effective hard timeout is only 200ms** (the
   leader measured 217 `AppendEntries RPC timed out after 200ms` events in 24s).
   This describes the pre-T1 implementation. T1 later changed
   `effective_timeout()` to `soft_ttl()` as openraft requires (section 4), so
   the per-RPC budget became 150ms. The measurement is kept as the historical
   evidence that `rpc_request_timeout_ms` never governed these RPCs.
3. **The real trigger is a synchronous lock held across `.await`** (section 6.2):
   `connect_ca.rs::update_intention()` held a DashMap shard write guard all the
   way through `raft.write(...).await`, while the FSM apply hook on `main` needed
   `intentions.clear()`. The single-threaded executor was frozen for about 7s
   (exactly one `apply_wait`), during which every heartbeat / AppendEntries timed
   out at 200ms.
4. With PreVote off (`to_openraft_config` used `..Default::default()`, openraft's
   `get_enable_pre_vote()` is `unwrap_or(false)`; the log showed
   `pre_vote_retry_at: None`), a missed heartbeat immediately raised the term and
   disrupted the leader => in-flight writes were lost => `wait_for_applied` timed
   out after 7s.

So **`apply_wait_timeout` is a downstream symptom, not an independent defect**;
making the timeouts internally consistent in T3 cannot remove the trigger. CPU
at only 4-11% also rules out the common explanation "CPU saturation caused a
spurious election" and points instead to **synchronous blocking on a
single-threaded runtime / a lock held across await**, not to parameters that are
too small.

`enable_leader_restore` (a restarted former leader immediately campaigns in a
higher term) may still amplify term inflation when the matrix restarts nodes
between combinations, but this targeted validation involved no node restarts, so
it was not the trigger for these three elections. T3 subsequently disabled
`enable_leader_restore` explicitly (section 4); this paragraph is kept as the
historical classification of the measured run.

### 6.2 T6 Root-Cause Fix (PreVote + Sync I/O Isolation + Cross-Await Lock Fix)

**(1) Final root cause: a DashMap shard write guard held across `.await`.**

`crates/batata-plugin-consul/src/connect_ca.rs::update_intention()` was written as
`if let Some(mut entry) = self.intentions.get_mut(id) { ... raft.write(...).await ... }`:
the HTTP handler, on an actix arbiter thread, held the synchronous shard write
lock of `intentions` until the forwarded write completed. At the same time, Raft
FSM apply on the `main` thread committed `IntentionUpsert` and dispatched
`HookDispatch::CaChange` (`raft/plugin_handler.rs:213/242`) -> `plugin.rs:139
on_ca_change()` -> `refresh_from_rocks()` -> `intentions.clear()` plus one insert
per entry (`connect_ca.rs:1674`), which needs **all shard write locks** of the
same map. On a single-threaded runtime this froze `main` (RaftCore / Raft gRPC
server / heartbeat / deadline notifier) until `wait_for_applied` on the arbiter
timed out after 7s and released the guard. That explains the three 7s stalls,
`main` reviving at the exact moment of the timeout, and one AppendEntries timeout
every 200ms in between. The equivalent code in `peering.rs` already called
`drop(peering)` explicitly; only this site missed it.

Fix: move the mutation into a **synchronous** private helper
`build_updated_intention()` that returns an owned `Intention`. The compiler then
guarantees the guard cannot survive into any `.await` of `update_intention`.
`update_intention_has_no_shard_guard_across_await` is an architecture guard test:
putting `get_mut(` back into the async function body turns it red immediately
(mutant verified).

**(2) Protocol-level fallback: enable PreVote.**

`to_openraft_config()` explicitly sets `enable_pre_vote: Some(true)`. openraft
0.10.1's `pre_elect()` (`engine_impl.rs:335-375`) states that it "neither updates
`vote` nor changes the server state: the node stays a Follower until a quorum
would grant the vote", and it skips pre-elect entirely while the leader lease is
still valid. A transient stall therefore no longer becomes a real term increase.

**(3) Sync I/O isolation (not the root cause, but removes the same head-of-line risk).**

`RocksLogStore`'s `save_vote` / `append` / `truncate_after` / `purge` used
`set_sync(true)` to fsync synchronously on the RaftCore thread; they now all go
through `blocking_write()` onto `spawn_blocking`. Durability is unchanged
(`IOFlushed` still fires only after fsync completes) and ordering is unchanged
(RaftCore awaits each call in sequence), but a busy disk no longer monopolizes
the runtime thread.

T2 completed the isolation. `RocksLogStore` reads (`try_get_log_entries` and
startup cache loading) now go through `blocking_read()` on the blocking pool,
and `RocksStateMachine` was wrapped as `Arc<StateMachineInner>` plus `Deref` so
`apply()` can dispatch `apply_request` and the durable last-applied/membership
markers through `spawn_blocking`. Shared handles are resolved once per entry on
the async side (`ApplyHooks`: naming hook, health-check hook, plugin registry
snapshot) and moved into the closure as owned `Arc`s, so no
`tokio::sync::RwLock` is ever awaited from a blocking thread. Apply stays
strictly serialised: `apply()` awaits each join before pulling the next entry,
so log order, responder timing and durability are unchanged. Architecture guard
tests assert both the presence of the blocking hop and that it runs on a
different thread; four mutants (helper, apply call site, read helper, read call
site) were killed.

### 6.3 Resource Correlation (Follow-up Round T6)

The original observability request asked that abnormal latency, CPU or memory
be diagnosable from logs and metrics. The T1-T6 round reached "diagnosable" but
left the two sides disconnected: resource gauges were refreshed every 15s while
Raft anomalies were emitted at the moment of failure, so correlating them was a
manual timestamp exercise.

`telemetry.rs` now defines a `ResourceSnapshot` (`cpu_pct`,
`process_memory_mb`) and a process-wide `ResourceSnapshotProvider`. The
consensus crate deliberately does not depend on `sysinfo`: `batata-server`
installs the provider in `init_metrics()` and
`start_system_stats_reporter()` pushes each sample into the shared slot. The
provider is a short mutex read, never I/O, because it is called from Raft write
paths and from the metrics watcher. When system stats reporting is disabled no
sample exists and the fields log the numeric sentinel `-1`, which is distinct
from a real 0% reading.

Every one of the four gate-blocking anomaly WARNs
(`steady_state_election`, `client_write_timeout`, `apply_wait_timeout`,
`leaderless_give_up`) builds one `AnomalyContext` at the moment of the anomaly
and therefore attaches the same sample to:

- the structured log, as `cpu_pct`, `process_memory_mb` and
  `resource_pressure`;
- the Prometheus correlation gauges/counters listed in section 6, labelled by
  `event`, so a single scrape can join cause and effect;
- `record_health()`, which also publishes the latest sample as
  `batata_raft_resource_*` gauges alongside the Raft state.

When CPU is at or above `HIGH_CPU_PCT` (85%), an additional
`raft_event=resource_pressure` warning names the anomaly it accompanied and
points at runtime starvation rather than timeout tuning. This event is
**diagnostic, not a fifth blocker**: `scripts/raft_health_check.py` still counts
only the four anomaly names, and its self-test asserts that a resource-pressure
line alone leaves a cluster run green. The threshold never changes Raft
behaviour or any timeout.

Boundary: the sample is at most one reporter interval old (15s by default), and
the existing reporter measures system-wide CPU averaged across cores, not
per-thread CPU. So this closes the "manual alignment" gap, not the "automatic
root-cause attribution" gap: a high-CPU anomaly is a strong hint to inspect
runtime starvation, not proof of it. An architecture guard in `telemetry.rs`
asserts that all four anomaly sites sample, publish, log and alert; the
`batata-server` metrics module has a companion wiring guard. Fourteen mutants
covering thresholds, unit conversion, provider wiring, per-site fields and the
gate classifier were killed.

Two smaller items from the same follow-up round are worth recording here for
traceability. T5 removed the duplicated SDK port list in `scripts/test.sh`: the
quick path now calls `sdk_ports_clear` from `scripts/sdk_port_guard.sh`, whose
`SDK_PORT_LIST` is the single source of truth and already contains 8502 (Consul
Connect xDS), 8082 and 8083. `sdk_port_guard.sh self-test` fails if a hardcoded
numeric sweep, an inline `lsof | xargs kill`, a missing `source` line, a missing
delegation, or a missing critical port reappears; seven mutants were killed.
Both script gates are now gated, not merely available: `regression.sh`'s `check`
stage runs `sdk_port_guard.sh self-test` and `raft_health_check.py --self-test`,
records a blocker when either fails, and also fails when the self-test
artifacts are missing (so dropping the wiring is a red gate). Five mutants
covering port-list drift, a weakened anomaly classifier, both dropped
invocations and a broken artifact check were killed.
T1-T4 of this round also closed the `RPCOption::soft_ttl()` mismatch, moved the
remaining synchronous RocksDB reads and SM `apply` to the blocking pool,
explicitly disabled `enable_leader_restore`, and added the cross-node
`commit_lag` view; those changes are described in sections 4-7.

## 7. Remaining Gaps and Boundaries

- The T1-T6 round **does not change** the `E/H/R` defaults (that would be a
  behavior change); it only links the derived waits. Whether to tighten the
  production `rpc_request_timeout_ms` (5000ms, 2.5x `E_max`) still needs a
  separate evaluation. T6-a found that this setting **does not bound**
  AppendEntries / Vote (section 6.1 point 2), and T1 fixed the resulting
  semantic mismatch: the transport now enforces `RPCOption::soft_ttl()` instead
  of `hard_ttl()`, `rpc_request_timeout_ms` is documented as the channel
  ceiling / fallback / derivation term, and `RaftConfig::validate()` also
  requires `R > H` so the channel timeout cannot preempt the soft TTL. Truly
  relaxing the per-RPC budget still requires raising `heartbeat_interval` and
  the election window together; that is a behavioral decision and has not been
  taken.
- The root cause has been fixed as described in section 6.2. T5's blocking
  semantics are unchanged: the matrix stage of `regression.sh --full` continues
  to treat `steady_state_election` / `apply_wait_timeout` /
  `client_write_timeout` / `leaderless_give_up` as blockers, and a cluster
  combination missing `telemetry_started` fails closed. A red result is not a
  false positive from the gate; it prevents an occasional anomaly from being
  hidden by a re-run.
- `apply_lag` is `last_log_index - last_applied`, a local view. T4 added the
  cross-node view from openraft 0.10 `RaftMetrics`:
  `local_committed_index`, `cluster_committed_index`, and
  `commit_lag = cluster - local` (saturating, because a leader change can move
  `cluster_committed` backwards on a follower). This is the view that was
  missing when the T1-T6 evidence had to be correlated by hand.
- The `RocksLogStore` read path (`try_get_log_entries`, startup cache loading)
  and the SM `apply` path were the last synchronous RocksDB calls on the main
  runtime; T2 moved them to `spawn_blocking` (see section 6.2). Snapshot build,
  snapshot restore and post-install naming reconciliation still iterate RocksDB
  inline, but those are not on the per-entry heartbeat/replication path. If
  runtime stalls reappear under high write load, look for new inline DB work
  first rather than increasing timeouts.
- Resource correlation (section 6.3) is now automatic in the logs and in the
  Prometheus namespace, but it is still **correlation, not attribution**: the
  sample is up to one 15s reporter interval old, CPU is system-wide rather than
  per-thread, and no code path decides a root cause. Automatic attribution
  (per-thread CPU, per-apply duration breakdown, or causal tracing) remains a
  non-goal. The performance baseline is still the advisory 11-15
  `mac1714-18t` capture; a perf health check does not refresh it.

## References

- openraft 0.10.1 `Config` documentation: https://docs.rs/openraft/0.10.1/openraft/struct.Config.html
- openraft 0.10.1 source `src/config/config.rs` (local cargo registry)
- Consul General parameters: https://developer.hashicorp.com/consul/docs/reference/agent/configuration-file/general
- Consul Server Performance: https://developer.hashicorp.com/consul/docs/v1.18.x/reference/architecture/server
- hashicorp/raft `config.go`: https://github.com/hashicorp/raft/blob/main/config.go
