//! Deterministic in-process Raft simulation and linearizability checking.
//!
//! This is deliberately not another gRPC cluster test. `Raft::build()` wires
//! three real openraft cores directly to a shared in-process router, using the
//! production `RocksLogStore` / `RocksStateMachine` pair. The router owns the
//! only sources of variability that these tests allow: a fixed election
//! window, explicit elections, logical delay steps, deterministic RPC failures,
//! quotas, partitions and snapshot faults. There is no `tonic`, no OS-seeded
//! election jitter and no dependence on wall-clock scheduling for correctness.
//!
//! P3 covers protocol-level behaviour: election, conflicting log prefixes,
//! snapshot transfer and membership change. P4 uses the same harness for
//! leader crash, partition/heal and snapshot-disk faults, then checks the
//! resulting client history against a sequential register model with a
//! real-time-constrained linearizability search.
//!
//! Scope note: this is deterministic fault scheduling with a logical history
//! clock, not a full virtual-time DST runtime. Timers are disabled and every
//! election/heartbeat that matters is triggered explicitly. The single
//! exception is the leader-lease step after a crash: openraft measures that
//! lease with its own runtime clock, so the harness advances one full lease
//! explicitly instead of replacing `AsyncRuntime`. No verdict depends on
//! scheduler timing; all of them come from explicit assertions afterwards.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::RwLock;

use batata_consistency::bincode;
use batata_consistency::raft::SnapshotFile;
use batata_consistency::raft::log_store::RocksLogStore;
use batata_consistency::raft::request::{ConfigPublishPayload, RaftRequest, RaftResponse};
use batata_consistency::raft::state_machine::{CF_CONFIG, RocksStateMachine, StoredConfig};
use batata_consistency::raft::types::{
    CommittedLeaderId, LogId, NodeId, Raft, RaftMetrics, Snapshot, TypeConfig, Vote,
};
use openraft::async_runtime::WatchReceiver;
use openraft::errors::{NetworkError, RPCError, ReplicationClosed, StreamingError, Unreachable};
use openraft::network::{Backoff, RPCOption, RaftNetworkFactory, RaftNetworkV2};
use openraft::raft::{
    AppendEntriesRequest, AppendError, AppendResult, SnapshotResponse, VoteRequest, VoteResponse,
};
use openraft::{BasicNode, Config, OptionalSend, ReadPolicy};
use tempfile::TempDir;

const NODE_COUNT: u64 = 3;
const ELECTION_TIMEOUT_MIN: u64 = 400;
/// openraft sets `EngineConfig::leader_lease` to
/// `Duration::from_millis(config.election_timeout_max)` and refuses vote
/// requests while that lease is valid. After a leader crash a failover needs
/// one full lease plus margin; this margin is slack, not a timing assumption.
const LEADER_LEASE_MARGIN_MS: u64 = 200;
/// Fixed seed. Fault schedules are derived only from this value with a small
/// LCG; no OS entropy is read. The commit half of `(seed, commit)` is bound by
/// the regression report header, which records HEAD and dirty state.
const SIM_SEED: u64 = 0x5EED_2026;
const SCHEDULE_LEN: usize = 6;
const GROUP: &str = "DEFAULT_GROUP";
const TENANT: &str = "public";

/// Explicit logical step: advance past the dead leader's vote lease.
///
/// This is the only physical-clock wait in the harness. openraft reads its own
/// runtime clock in `handle_vote_req`, and virtualizing it would require
/// replacing `AsyncRuntime` for the whole test. Waiting longer than the lease
/// guarantees expiry by construction; the election result is then asserted,
/// not inferred from elapsed time.
fn leader_lease_step() -> Duration {
    Duration::from_millis(ELECTION_TIMEOUT_MIN + 1 + LEADER_LEASE_MARGIN_MS)
}

#[test]
fn leader_lease_step_covers_openraft_vote_lease() {
    let openraft_lease_ms = sim_config().election_timeout_max;
    assert!(
        leader_lease_step().as_millis() as u64 > openraft_lease_ms,
        "the failover step must outlast the dead leader's vote lease: \
         step={:?} ms, lease={} ms",
        leader_lease_step().as_millis(),
        openraft_lease_ms
    );
}

fn timeout() -> Duration {
    Duration::from_secs(5)
}

fn rpc_option() -> RPCOption {
    RPCOption::new(Duration::from_secs(5))
}

fn rpc_error<E: Debug>(error: E) -> RPCError<TypeConfig> {
    RPCError::Unreachable(Unreachable::<TypeConfig>::from_string(format!(
        "in-process RPC failed: {error:?}"
    )))
}

fn streaming_error<E: Debug>(error: E) -> StreamingError<TypeConfig> {
    StreamingError::Unreachable(Unreachable::<TypeConfig>::from_string(format!(
        "in-process snapshot RPC failed: {error:?}"
    )))
}

/// A deterministic election window has exactly one possible timeout. Automatic
/// elections, heartbeats and ticks are disabled; tests trigger them explicitly.
fn sim_config() -> Arc<Config> {
    let config = Config {
        cluster_name: "batata-deterministic-sim".to_string(),
        election_timeout_min: ELECTION_TIMEOUT_MIN,
        election_timeout_max: ELECTION_TIMEOUT_MIN + 1,
        heartbeat_interval: 50,
        enable_tick: false,
        enable_heartbeat: false,
        enable_elect: false,
        enable_pre_vote: Some(false),
        enable_leader_restore: Some(false),
        backoff: Some("1ms".to_string()),
        max_payload_entries: 64,
        ..Default::default()
    };
    Arc::new(config.validate().expect("valid deterministic Raft config"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LinkFault {
    fail_first_appends: usize,
    append_quota: Option<usize>,
    delay_steps: u64,
    fail_first_votes: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SimStats {
    partitioned_appends: u64,
    failed_appends: u64,
    logical_delay_steps: u64,
    failed_snapshots: u64,
    paused_appends: u64,
    clock_skew_steps: u64,
}

/// One deterministic fault step derived from a seed. Bounds are small enough
/// that openraft's 1ms replication backoff always recovers within the test
/// timeout, so the scenario is reproducible rather than scheduler-dependent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScheduledFault {
    delay_steps: u64,
    fail_first_appends: usize,
}

fn lcg_next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state >> 33
}

fn fault_schedule(seed: u64) -> Vec<ScheduledFault> {
    let mut state = seed;
    (0..SCHEDULE_LEN)
        .map(|_| ScheduledFault {
            delay_steps: lcg_next(&mut state) % 3,
            fail_first_appends: (lcg_next(&mut state) % 2) as usize,
        })
        .collect()
}

#[derive(Default)]
struct RouterState {
    nodes: BTreeMap<NodeId, Raft>,
    partitions: BTreeSet<(NodeId, NodeId)>,
    snapshot_faults: BTreeSet<(NodeId, NodeId)>,
    link_faults: BTreeMap<(NodeId, NodeId), LinkFault>,
    /// A paused node neither sends nor receives RPCs: the process-pause fault.
    paused: BTreeSet<NodeId>,
    /// Clock-skew analog: outgoing RPCs from this node take extra logical
    /// delay steps. openraft's own wall clock cannot be virtualized here.
    skewed_node: Option<NodeId>,
    skew_steps: u64,
    partitioned_appends: u64,
    failed_appends: u64,
    logical_delay_steps: u64,
    failed_snapshots: u64,
    paused_appends: u64,
    clock_skew_steps: u64,
}

#[derive(Clone)]
struct SimRouter {
    state: Arc<RwLock<RouterState>>,
}

impl SimRouter {
    fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(RouterState::default())),
        }
    }

    async fn register_nodes(&self, nodes: &[SimNode]) {
        let mut state = self.state.write().await;
        for node in nodes {
            state.nodes.insert(node.id, node.raft.clone());
        }
    }

    async fn remove_node(&self, id: NodeId) {
        self.state.write().await.nodes.remove(&id);
    }

    async fn node_count(&self) -> usize {
        self.state.read().await.nodes.len()
    }

    async fn raft(&self, id: NodeId) -> Option<Raft> {
        self.state.read().await.nodes.get(&id).cloned()
    }

    async fn link_fault(&self, from: NodeId, to: NodeId) -> LinkFault {
        self.state
            .read()
            .await
            .link_faults
            .get(&(from, to))
            .copied()
            .unwrap_or_default()
    }

    async fn set_link_fault(&self, from: NodeId, to: NodeId, fault: LinkFault) {
        self.state
            .write()
            .await
            .link_faults
            .insert((from, to), fault);
    }

    async fn is_partitioned(&self, from: NodeId, to: NodeId) -> bool {
        self.state.read().await.partitions.contains(&(from, to))
    }

    async fn partition(&self, a: NodeId, b: NodeId) {
        let mut state = self.state.write().await;
        state.partitions.insert((a, b));
        state.partitions.insert((b, a));
    }

    async fn heal(&self, a: NodeId, b: NodeId) {
        let mut state = self.state.write().await;
        state.partitions.remove(&(a, b));
        state.partitions.remove(&(b, a));
    }

    async fn pause(&self, id: NodeId) {
        self.state.write().await.paused.insert(id);
    }

    async fn resume(&self, id: NodeId) {
        self.state.write().await.paused.remove(&id);
    }

    async fn is_paused(&self, id: NodeId) -> bool {
        self.state.read().await.paused.contains(&id)
    }

    async fn set_clock_skew(&self, node: Option<NodeId>, steps: u64) {
        let mut state = self.state.write().await;
        state.skewed_node = node;
        state.skew_steps = steps;
    }

    async fn clock_skew_steps(&self, from: NodeId) -> u64 {
        let state = self.state.read().await;
        if state.skewed_node == Some(from) {
            state.skew_steps
        } else {
            0
        }
    }

    async fn is_snapshot_faulted(&self, from: NodeId, to: NodeId) -> bool {
        self.state
            .read()
            .await
            .snapshot_faults
            .contains(&(from, to))
    }

    async fn set_snapshot_fault(&self, from: NodeId, to: NodeId, enabled: bool) {
        let mut state = self.state.write().await;
        if enabled {
            state.snapshot_faults.insert((from, to));
        } else {
            state.snapshot_faults.remove(&(from, to));
        }
    }

    async fn record_partitioned_append(&self) {
        self.state.write().await.partitioned_appends += 1;
    }

    async fn record_failed_append(&self) {
        self.state.write().await.failed_appends += 1;
    }

    async fn record_delay_steps(&self, steps: u64) {
        self.state.write().await.logical_delay_steps += steps;
    }

    async fn record_failed_snapshot(&self) {
        self.state.write().await.failed_snapshots += 1;
    }

    async fn record_paused_append(&self) {
        self.state.write().await.paused_appends += 1;
    }

    async fn record_skew_steps(&self, steps: u64) {
        self.state.write().await.clock_skew_steps += steps;
    }

    async fn stats(&self) -> SimStats {
        let state = self.state.read().await;
        SimStats {
            partitioned_appends: state.partitioned_appends,
            failed_appends: state.failed_appends,
            logical_delay_steps: state.logical_delay_steps,
            failed_snapshots: state.failed_snapshots,
            paused_appends: state.paused_appends,
            clock_skew_steps: state.clock_skew_steps,
        }
    }
}

struct SimFactory {
    from: NodeId,
    router: SimRouter,
}

impl SimFactory {
    fn new(from: NodeId, router: SimRouter) -> Self {
        Self { from, router }
    }
}

impl RaftNetworkFactory<TypeConfig> for SimFactory {
    type Network = SimNetwork;

    async fn new_client(&mut self, target: NodeId, _node: &BasicNode) -> SimNetwork {
        let fault = self.router.link_fault(self.from, target).await;
        SimNetwork::with_fault(self.from, target, self.router.clone(), fault)
    }
}

struct SimNetwork {
    from: NodeId,
    target: NodeId,
    router: SimRouter,
    fail_first_appends: usize,
    append_quota: Option<usize>,
    delay_steps: u64,
    fail_first_votes: usize,
    append_calls: usize,
    vote_calls: usize,
}

impl SimNetwork {
    fn with_fault(from: NodeId, target: NodeId, router: SimRouter, fault: LinkFault) -> Self {
        Self {
            from,
            target,
            router,
            fail_first_appends: fault.fail_first_appends,
            append_quota: fault.append_quota,
            delay_steps: fault.delay_steps,
            fail_first_votes: fault.fail_first_votes,
            append_calls: 0,
            vote_calls: 0,
        }
    }

    fn append_calls(&self) -> usize {
        self.append_calls
    }

    async fn link_is_paused(&self) -> bool {
        self.router.is_paused(self.from).await || self.router.is_paused(self.target).await
    }
}

impl RaftNetworkV2<TypeConfig> for SimNetwork {
    type SnapshotData = SnapshotFile;

    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendResult<TypeConfig>, RPCError<TypeConfig>> {
        self.append_calls += 1;

        // A paused process cannot send or receive; both directions must be
        // dropped or the fault only half-simulates a process pause.
        if self.router.is_paused(self.from).await || self.router.is_paused(self.target).await {
            self.router.record_paused_append().await;
            return Err(RPCError::Unreachable(
                Unreachable::<TypeConfig>::from_string("deterministic process pause"),
            ));
        }

        if self.router.is_partitioned(self.from, self.target).await {
            self.router.record_partitioned_append().await;
            return Err(RPCError::Unreachable(
                Unreachable::<TypeConfig>::from_string("deterministic partition"),
            ));
        }

        if self.append_calls <= self.fail_first_appends {
            self.router.record_failed_append().await;
            return Err(RPCError::Unreachable(
                Unreachable::<TypeConfig>::from_string("deterministic fail_rpc"),
            ));
        }

        if self
            .append_quota
            .is_some_and(|quota| self.append_calls > quota)
        {
            self.router.record_failed_append().await;
            return Err(RPCError::Unreachable(
                Unreachable::<TypeConfig>::from_string("deterministic append quota exhausted"),
            ));
        }

        if self.delay_steps > 0 {
            for _ in 0..self.delay_steps {
                tokio::task::yield_now().await;
            }
            self.router.record_delay_steps(self.delay_steps).await;
        }

        // Clock-skew analog: a node whose clock is ahead has its outgoing
        // RPCs shifted by extra logical steps. This is deterministic delay
        // injection, not a virtual-time replacement of openraft's clock.
        let skew = self.router.clock_skew_steps(self.from).await;
        if skew > 0 {
            for _ in 0..skew {
                tokio::task::yield_now().await;
            }
            self.router.record_skew_steps(skew).await;
        }

        let target = self.router.raft(self.target).await.ok_or_else(|| {
            RPCError::Unreachable(Unreachable::<TypeConfig>::from_string(
                "missing target node",
            ))
        })?;
        target.append_entries(rpc).await.map_err(rpc_error)
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig>> {
        self.vote_calls += 1;

        if self.link_is_paused().await {
            return Err(RPCError::Network(NetworkError::<TypeConfig>::from_string(
                "deterministic process pause",
            )));
        }
        if self.router.is_partitioned(self.from, self.target).await {
            return Err(RPCError::Network(NetworkError::<TypeConfig>::from_string(
                "deterministic partition",
            )));
        }
        if self.vote_calls <= self.fail_first_votes {
            return Err(RPCError::Network(NetworkError::<TypeConfig>::from_string(
                "deterministic fail_vote",
            )));
        }

        let target = self.router.raft(self.target).await.ok_or_else(|| {
            RPCError::Unreachable(Unreachable::<TypeConfig>::from_string(
                "missing target node",
            ))
        })?;
        target.vote(rpc).await.map_err(rpc_error)
    }

    async fn pre_vote(
        &mut self,
        rpc: VoteRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig>> {
        if self.link_is_paused().await {
            return Err(RPCError::Network(NetworkError::<TypeConfig>::from_string(
                "deterministic process pause",
            )));
        }
        if self.router.is_partitioned(self.from, self.target).await {
            return Err(RPCError::Network(NetworkError::<TypeConfig>::from_string(
                "deterministic partition",
            )));
        }
        let target = self.router.raft(self.target).await.ok_or_else(|| {
            RPCError::Unreachable(Unreachable::<TypeConfig>::from_string(
                "missing target node",
            ))
        })?;
        target.pre_vote(rpc).await.map_err(rpc_error)
    }

    async fn full_snapshot(
        &mut self,
        vote: Vote,
        snapshot: Snapshot,
        cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<TypeConfig>, StreamingError<TypeConfig>> {
        if self.link_is_paused().await {
            return Err(StreamingError::Network(
                NetworkError::<TypeConfig>::from_string("deterministic process pause"),
            ));
        }
        if self
            .router
            .is_snapshot_faulted(self.from, self.target)
            .await
        {
            self.router.record_failed_snapshot().await;
            return Err(StreamingError::Network(
                NetworkError::<TypeConfig>::from_string("deterministic snapshot disk fault"),
            ));
        }

        let target = self.router.raft(self.target).await.ok_or_else(|| {
            StreamingError::Unreachable(Unreachable::<TypeConfig>::from_string(
                "missing target node",
            ))
        })?;

        let install = target.install_full_snapshot(vote, snapshot);
        let mut cancel: Pin<Box<dyn Future<Output = ReplicationClosed> + Send + 'static>> =
            Box::pin(cancel);
        tokio::select! {
            result = install => result.map_err(streaming_error),
            _ = cancel.as_mut() => Err(StreamingError::Closed(
                ReplicationClosed::new("snapshot transfer cancelled"),
            )),
        }
    }

    fn backoff(&self) -> Option<Backoff> {
        Some(Backoff::new(std::iter::repeat(Duration::from_millis(1))))
    }
}

struct SimNode {
    id: NodeId,
    raft: Raft,
    db: Arc<rocksdb::DB>,
    _dir: TempDir,
}

impl SimNode {
    fn read_config(&self, data_id: &str) -> Option<StoredConfig> {
        let cf = self.db.cf_handle(CF_CONFIG).expect("CF_CONFIG");
        let key = RocksStateMachine::config_key(data_id, GROUP, TENANT);
        self.db
            .get_cf(&cf, key.as_bytes())
            .ok()
            .flatten()
            .and_then(|bytes| bincode::deserialize::<StoredConfig>(&bytes).ok())
    }
}

async fn build_cluster() -> (SimRouter, Vec<SimNode>) {
    let router = SimRouter::new();
    let config = sim_config();
    let mut nodes = Vec::new();

    for id in 1..=NODE_COUNT {
        let dir = tempfile::tempdir().expect("sim node temp dir");
        let logs = RocksLogStore::new(dir.path().join("raft-log"))
            .await
            .expect("sim log store");
        let state_machine = RocksStateMachine::new(dir.path().join("state"))
            .await
            .expect("sim state machine")
            .with_snapshot_dir(dir.path().join("snapshot"));
        let db = state_machine.db();

        let raft = Raft::build(
            id,
            config.clone(),
            SimFactory::new(id, router.clone()),
            logs,
            state_machine,
        )
        .await
        .expect("build in-process Raft");
        nodes.push(SimNode {
            id,
            raft,
            db,
            _dir: dir,
        });
    }

    router.register_nodes(&nodes).await;
    for node in &nodes {
        node.raft.run();
    }
    (router, nodes)
}

async fn initialize_cluster(nodes: &[SimNode]) {
    let mut members = BTreeMap::new();
    for node in nodes {
        members.insert(
            node.id,
            BasicNode {
                addr: format!("sim:{}", node.id),
            },
        );
    }

    nodes[0]
        .raft
        .initialize(members)
        .await
        .expect("initialize deterministic cluster");
    nodes[0]
        .raft
        .wait(Some(timeout()))
        .current_leader(1, "designated leader")
        .await
        .expect("designated leader elected");
    nodes[0]
        .raft
        .trigger()
        .heartbeat()
        .await
        .expect("initial heartbeat");

    for node in &nodes[1..] {
        node.raft
            .wait(Some(timeout()))
            .current_leader(1, "follower sees leader")
            .await
            .expect("follower sees designated leader");
    }

    let response = nodes[0]
        .raft
        .client_write(RaftRequest::Noop)
        .await
        .expect("initial noop commit");
    assert_eq!(response.data, RaftResponse::success());
    for node in nodes {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(response.log_id.index), "noop applied")
            .await
            .expect("noop applied on every node");
    }
}

async fn shutdown_all(nodes: &[SimNode]) {
    for node in nodes {
        let _ = node.raft.shutdown().await;
    }
}

fn metrics(raft: &Raft) -> RaftMetrics {
    let receiver = raft.metrics();
    let guard = receiver.borrow_watched();
    (*guard).clone()
}

fn last_applied_index(raft: &Raft) -> u64 {
    metrics(raft)
        .last_applied
        .map(|log_id| log_id.index)
        .unwrap_or(0)
}

fn publish_request(data_id: &str, content: &str) -> RaftRequest {
    RaftRequest::ConfigPublish(Box::new(ConfigPublishPayload {
        data_id: data_id.to_string(),
        group: GROUP.to_string(),
        tenant: TENANT.to_string(),
        content: content.to_string(),
        md5: format!("md5-{data_id}"),
        config_type: Some("text".to_string()),
        app_name: None,
        tag: None,
        desc: None,
        src_user: Some("deterministic-sim".to_string()),
        src_ip: None,
        r#use: None,
        effect: None,
        schema: None,
        encrypted_data_key: None,
        cas_md5: None,
        history: None,
        now_ms: 1_000,
    }))
}

#[derive(Clone)]
struct LogicalClock {
    inner: Arc<AtomicU64>,
}

impl LogicalClock {
    fn new() -> Self {
        Self {
            inner: Arc::new(AtomicU64::new(0)),
        }
    }

    fn tick(&self) -> u64 {
        self.inner.fetch_add(1, Ordering::SeqCst) + 1
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum OpKind {
    Write { key: String, value: String },
    Read { key: String, value: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Operation {
    start: u64,
    end: u64,
    kind: OpKind,
}

fn write_op(start: u64, end: u64, key: &str, value: &str) -> Operation {
    Operation {
        start,
        end,
        kind: OpKind::Write {
            key: key.to_string(),
            value: value.to_string(),
        },
    }
}

fn read_op(start: u64, end: u64, key: &str, value: Option<&str>) -> Operation {
    Operation {
        start,
        end,
        kind: OpKind::Read {
            key: key.to_string(),
            value: value.map(str::to_string),
        },
    }
}

type Model = BTreeMap<String, Option<String>>;

fn apply_operation(model: &Model, operation: &Operation) -> Option<Model> {
    let mut next = model.clone();
    match &operation.kind {
        OpKind::Write { key, value } => {
            next.insert(key.clone(), Some(value.clone()));
            Some(next)
        }
        OpKind::Read { key, value } => {
            let current = next.get(key).cloned().unwrap_or(None);
            (&current == value).then_some(next)
        }
    }
}

fn search(
    operations: &[Operation],
    remaining: &[usize],
    model: &Model,
    cache: &mut HashSet<(Vec<usize>, Model)>,
) -> bool {
    if remaining.is_empty() {
        return true;
    }
    if !cache.insert((remaining.to_vec(), model.clone())) {
        return false;
    }

    for &index in remaining {
        let operation = &operations[index];
        let must_precede = remaining
            .iter()
            .any(|&other| other != index && operations[other].end < operation.start);
        if must_precede {
            continue;
        }
        if let Some(next_model) = apply_operation(model, operation) {
            let next_remaining: Vec<usize> = remaining
                .iter()
                .copied()
                .filter(|&other| other != index)
                .collect();
            if search(operations, &next_remaining, &next_model, cache) {
                return true;
            }
        }
    }
    false
}

/// Search for a sequential order consistent with both the register model and
/// real-time precedence: if operation A completed before B began, A must be
/// linearized before B. Overlapping operations may be reordered.
fn is_linearizable(history: &[Operation]) -> bool {
    assert!(
        history
            .iter()
            .all(|operation| operation.start <= operation.end),
        "operation intervals must be well formed"
    );
    let remaining: Vec<usize> = (0..history.len()).collect();
    let mut cache = HashSet::new();
    search(history, &remaining, &Model::new(), &mut cache)
}

async fn perform_write(
    clock: &LogicalClock,
    raft: &Raft,
    key: &str,
    value: &str,
) -> (Operation, u64) {
    let start = clock.tick();
    let response = raft
        .client_write(publish_request(key, value))
        .await
        .expect("client write must succeed");
    let end = clock.tick();
    assert!(
        response.data.success,
        "write was rejected: {:?}",
        response.data
    );
    (write_op(start, end, key, value), response.log_id.index)
}

async fn perform_read(
    clock: &LogicalClock,
    leader: &Raft,
    reader: &SimNode,
    key: &str,
) -> Operation {
    let start = clock.tick();
    let linearizer = leader
        .get_read_linearizer(ReadPolicy::ReadIndex)
        .await
        .expect("obtain read linearizer");
    linearizer
        .await_ready(&reader.raft)
        .await
        .expect("reader must catch up to the read barrier");
    let value = reader.read_config(key).map(|stored| stored.content);
    let end = clock.tick();
    Operation {
        start,
        end,
        kind: OpKind::Read {
            key: key.to_string(),
            value,
        },
    }
}

#[test]
fn simulation_config_has_no_random_election_window() {
    let config = sim_config();
    assert_eq!(
        config.election_timeout_max - config.election_timeout_min,
        1,
        "random_range(min..max) must have exactly one outcome"
    );
    assert!(!config.enable_elect, "automatic elections must be disabled");
    assert!(!config.enable_tick, "automatic ticks must be disabled");
    assert!(
        !config.enable_heartbeat,
        "automatic heartbeats must be disabled"
    );
    assert_eq!(config.enable_pre_vote, Some(false));
    assert_eq!(config.backoff.as_deref(), Some("1ms"));
}

#[test]
fn simulation_coverage_guards_are_intact() {
    let source = include_str!("deterministic_raft_simulation.rs");
    for token in [
        concat!("Raft::", "build("),
        concat!(".with_snapshot_dir", "(dir.path().join(\"snapshot\"))"),
        concat!("router.partition", "(1, 3).await"),
        concat!("get_read_linearizer", "(ReadPolicy::ReadIndex)"),
        concat!("is_linearizable", "(&history)"),
        concat!("set_snapshot_fault", "(1, 3, true)"),
        concat!("leader published snapshot", " metadata"),
        concat!("leader_with_quorum_acked", "(None,"),
        concat!("leader_lease_step", "()"),
        concat!(".elect", "(false)"),
        concat!("router.pause", "(3).await"),
        concat!("set_clock_skew", "(Some(1), 3)"),
        concat!("fault_schedule", "(SIM_SEED)"),
        concat!("paused_appends", " > 0"),
        concat!("clock_skew_steps", " > 0"),
    ] {
        assert!(source.contains(token), "simulation guard missing: {token}");
    }
    let grpc = concat!("tonic::", "transport");
    assert!(!source.contains(grpc), "simulation must not use gRPC");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deterministic_three_node_cluster_commits_without_grpc() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;

    assert_eq!(router.node_count().await, NODE_COUNT as usize);
    assert_eq!(metrics(&nodes[0].raft).current_leader, Some(1));
    let (_operation, index) =
        perform_write(&LogicalClock::new(), &nodes[0].raft, "sim-basic", "v1").await;
    for node in &nodes {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(index), "deterministic write applied")
            .await
            .expect("every replica applies the committed write");
    }
    assert_eq!(
        nodes[2]
            .read_config("sim-basic")
            .map(|stored| stored.content),
        Some("v1".to_string())
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deterministic_link_faults_affect_live_replication() {
    let (router, nodes) = build_cluster().await;
    router
        .set_link_fault(
            1,
            3,
            LinkFault {
                delay_steps: 2,
                ..Default::default()
            },
        )
        .await;
    initialize_cluster(&nodes).await;

    let _ = perform_write(&LogicalClock::new(), &nodes[0].raft, "sim-delay", "v1").await;
    let stats = router.stats().await;
    assert!(
        stats.logical_delay_steps >= 2,
        "the deterministic delay schedule must be exercised: {stats:?}"
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn injected_link_faults_are_reproducible() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;

    let vote = metrics(&nodes[0].raft).vote;
    let fault = LinkFault {
        fail_first_appends: 2,
        append_quota: Some(5),
        delay_steps: 3,
        fail_first_votes: 0,
    };
    let mut network = SimNetwork::with_fault(1, 3, router.clone(), fault);

    for call in 1..=6usize {
        let request = AppendEntriesRequest {
            vote,
            prev_log_id: None,
            entries: vec![],
            leader_commit: None,
        };
        let result = network.append_entries(request, rpc_option()).await;
        match call {
            1..=2 => assert!(
                matches!(result, Err(RPCError::Unreachable(_))),
                "call {call} must be dropped by fail_rpc"
            ),
            3..=5 => {
                let accepted = result.expect("RPC must reach the target");
                assert!(
                    accepted.is_ok(),
                    "empty append must be accepted: {accepted:?}"
                );
            }
            _ => assert!(
                matches!(result, Err(RPCError::Unreachable(_))),
                "call {call} must be rejected by append quota"
            ),
        }
    }

    assert_eq!(network.append_calls(), 6);
    let stats = router.stats().await;
    assert_eq!(
        stats.failed_appends, 3,
        "two fail_rpc plus one quota rejection"
    );
    assert!(
        stats.logical_delay_steps >= 9,
        "three successful calls must each take three logical delay steps: {stats:?}"
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conflicting_prev_log_id_is_rejected() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;

    let vote = metrics(&nodes[0].raft).vote;
    let bogus_prev = LogId::new(
        CommittedLeaderId {
            term: 1,
            node_id: 1,
        },
        9_999,
    );
    let request = AppendEntriesRequest {
        vote,
        prev_log_id: Some(bogus_prev),
        entries: vec![],
        leader_commit: None,
    };
    let mut network = SimNetwork::with_fault(1, 3, router, LinkFault::default());
    let accepted = network
        .append_entries(request, rpc_option())
        .await
        .expect("RPC must reach the target");
    assert!(
        matches!(accepted, Err(AppendError::Conflict { .. })),
        "a missing prev_log_id must produce a conflict: {accepted:?}"
    );

    shutdown_all(&nodes).await;
}

async fn leader_snapshot(nodes: &[SimNode]) -> Snapshot {
    nodes[0]
        .raft
        .trigger()
        .snapshot()
        .await
        .expect("trigger snapshot");
    // `Trigger::snapshot` only queues an sm::Command::build_snapshot. A
    // yield-counting poll can drain before the RocksDB snapshot is written,
    // so wait on the metrics watch, which is event-driven and not a race
    // against scheduler progress.
    nodes[0]
        .raft
        .wait(Some(timeout()))
        .metrics(
            |m| m.snapshot.is_some(),
            "leader published snapshot metadata",
        )
        .await
        .expect("snapshot metadata published");
    nodes[0]
        .raft
        .get_snapshot()
        .await
        .expect("read leader snapshot")
        .expect("leader snapshot must be present")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_transfer_and_membership_change_are_deterministic() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    router.partition(1, 3).await;

    let (_operation, write_index) =
        perform_write(&LogicalClock::new(), &nodes[0].raft, "sim-snapshot", "v1").await;
    for node in &nodes[..2] {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(write_index), "quorum applied snapshot source")
            .await
            .expect("quorum applies the write");
    }
    assert!(last_applied_index(&nodes[2].raft) < write_index);

    let snapshot = leader_snapshot(&nodes).await;
    let snapshot_index = snapshot
        .meta
        .last_log_id
        .as_ref()
        .map(|log_id| log_id.index)
        .unwrap_or(0);
    assert!(snapshot_index >= write_index);

    let expected_vote = metrics(&nodes[0].raft).vote;
    let mut network = SimNetwork::with_fault(1, 3, router.clone(), LinkFault::default());
    let response = network
        .full_snapshot(
            expected_vote,
            snapshot,
            std::future::pending::<ReplicationClosed>(),
            rpc_option(),
        )
        .await
        .expect("snapshot transfer must succeed");
    assert_eq!(response.vote, expected_vote);

    nodes[2]
        .raft
        .wait(Some(timeout()))
        .applied_index_at_least(Some(snapshot_index), "follower installed snapshot")
        .await
        .expect("follower must install the snapshot");
    assert_eq!(
        nodes[2]
            .read_config("sim-snapshot")
            .map(|stored| stored.content),
        Some("v1".to_string())
    );

    nodes[0]
        .raft
        .change_membership(BTreeSet::from([1u64, 2u64]), true)
        .await
        .expect("change membership");
    for node in &nodes[..2] {
        node.raft
            .wait(Some(timeout()))
            .voter_ids([1u64, 2u64], "membership converged")
            .await
            .expect("membership must converge on the quorum");
    }

    shutdown_all(&nodes).await;
}

#[test]
fn linearizability_checker_accepts_valid_history() {
    let overlapping = vec![
        write_op(1, 4, "k", "v1"),
        write_op(2, 5, "k", "v2"),
        read_op(3, 6, "k", Some("v2")),
    ];
    assert!(is_linearizable(&overlapping));

    let sequential = vec![write_op(1, 2, "k", "v1"), read_op(3, 4, "k", Some("v1"))];
    assert!(is_linearizable(&sequential));
}

#[test]
fn linearizability_checker_rejects_stale_read_after_completed_write() {
    let history = vec![write_op(1, 2, "k", "v1"), read_op(3, 4, "k", None)];
    assert!(
        !is_linearizable(&history),
        "a completed write cannot be followed by a stale read"
    );
}

#[test]
fn linearizability_checker_rejects_impossible_concurrent_history() {
    let history = vec![
        write_op(1, 4, "k", "v1"),
        write_op(2, 5, "k", "v2"),
        read_op(3, 6, "k", Some("v3")),
    ];
    assert!(
        !is_linearizable(&history),
        "v3 was never written in any legal order"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn overlapping_writes_and_linearizable_read_form_valid_history() {
    let (_router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    let clock = LogicalClock::new();

    let (first, second, read) = tokio::join!(
        perform_write(&clock, &nodes[0].raft, "sim-overlap", "v1"),
        perform_write(&clock, &nodes[0].raft, "sim-overlap", "v2"),
        perform_read(&clock, &nodes[0].raft, &nodes[1], "sim-overlap"),
    );
    let history = vec![first.0, second.0, read];
    assert!(
        is_linearizable(&history),
        "overlapping writes and a barrier read must be linearizable: {history:?}"
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn partition_heal_and_checker_reject_stale_follower_read() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    let clock = LogicalClock::new();
    router.partition(1, 3).await;

    let (first_write, write_index) =
        perform_write(&clock, &nodes[0].raft, "sim-partition", "v1").await;
    for node in &nodes[..2] {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(write_index), "quorum applied partitioned write")
            .await
            .expect("quorum applies the write");
    }
    assert!(last_applied_index(&nodes[2].raft) < write_index);

    let start = clock.tick();
    let stale_value = nodes[2]
        .read_config("sim-partition")
        .map(|stored| stored.content);
    let end = clock.tick();
    let stale_read = Operation {
        start,
        end,
        kind: OpKind::Read {
            key: "sim-partition".to_string(),
            value: stale_value,
        },
    };
    match &stale_read.kind {
        OpKind::Read { value, .. } => assert_eq!(value.as_deref(), None),
        other => panic!("expected a read operation, got {other:?}"),
    }
    assert!(
        !is_linearizable(&[first_write.clone(), stale_read]),
        "a stale follower read without a read barrier must be rejected"
    );

    let mut partitioned = false;
    for _ in 0..200 {
        if router.stats().await.partitioned_appends > 0 {
            partitioned = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        partitioned,
        "the partition must drop at least one append RPC"
    );

    router.heal(1, 3).await;
    nodes[0]
        .raft
        .trigger()
        .heartbeat()
        .await
        .expect("replication heartbeat after heal");
    nodes[2]
        .raft
        .wait(Some(timeout()))
        .applied_index_at_least(Some(write_index), "follower caught up")
        .await
        .expect("follower must catch up after heal");

    let read = perform_read(&clock, &nodes[0].raft, &nodes[2], "sim-partition").await;
    match &read.kind {
        OpKind::Read { value, .. } => assert_eq!(value.as_deref(), Some("v1")),
        other => panic!("expected a read operation, got {other:?}"),
    }
    assert!(
        is_linearizable(&[first_write, read]),
        "the barrier read must observe the completed write"
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn leader_crash_preserves_committed_history() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    let clock = LogicalClock::new();

    let (first_write, _) = perform_write(&clock, &nodes[0].raft, "sim-crash", "v1").await;

    router.remove_node(1).await;
    assert!(
        nodes[0].raft.shutdown().await.is_ok(),
        "leader shutdown must succeed"
    );
    // With `enable_elect=false` the failover election is a one-shot command:
    // openraft rejects it while the dead leader's vote lease is still valid,
    // and no timer retries it. Advance one explicit lease step first.
    tokio::time::sleep(leader_lease_step()).await;
    nodes[1]
        .raft
        .trigger()
        .elect(false)
        .await
        .expect("trigger deterministic failover election");
    for node in &nodes[1..] {
        node.raft
            .wait(Some(timeout()))
            .current_leader(2, "new leader after crash")
            .await
            .expect("surviving nodes must elect node 2");
    }
    // openraft 0.10 rejects proposals from a leader whose quorum-ack lease is
    // not yet established. The election's own replication renews that lease;
    // wait for the barrier explicitly instead of racing the next write.
    nodes[1]
        .raft
        .wait(Some(timeout()))
        .leader_with_quorum_acked(None, "new leader has a quorum-acked lease")
        .await
        .expect("new leader established a writable lease");

    let (second_write, _) = perform_write(&clock, &nodes[1].raft, "sim-crash", "v2").await;
    let read = perform_read(&clock, &nodes[1].raft, &nodes[2], "sim-crash").await;
    match &read.kind {
        OpKind::Read { value, .. } => assert_eq!(value.as_deref(), Some("v2")),
        other => panic!("expected a read operation, got {other:?}"),
    }
    assert!(
        is_linearizable(&[first_write, second_write, read]),
        "committed history must survive a leader crash"
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn snapshot_disk_fault_is_fail_closed_then_recoverable() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    router.partition(1, 3).await;

    let (_operation, write_index) =
        perform_write(&LogicalClock::new(), &nodes[0].raft, "sim-disk", "v1").await;
    for node in &nodes[..2] {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(write_index), "quorum applied disk-fault source")
            .await
            .expect("quorum applies the write");
    }

    let snapshot = leader_snapshot(&nodes).await;
    let snapshot_index = snapshot
        .meta
        .last_log_id
        .as_ref()
        .map(|log_id| log_id.index)
        .unwrap_or(0);
    let before = last_applied_index(&nodes[2].raft);

    router.set_snapshot_fault(1, 3, true).await;
    let mut network = SimNetwork::with_fault(1, 3, router.clone(), LinkFault::default());
    let expected_vote = metrics(&nodes[0].raft).vote;
    let failure = network
        .full_snapshot(
            expected_vote,
            snapshot,
            std::future::pending::<ReplicationClosed>(),
            rpc_option(),
        )
        .await;
    assert!(
        matches!(failure, Err(StreamingError::Network(_))),
        "the injected disk fault must fail closed: {failure:?}"
    );
    assert_eq!(router.stats().await.failed_snapshots, 1);
    assert_eq!(last_applied_index(&nodes[2].raft), before);
    assert!(nodes[2].read_config("sim-disk").is_none());

    router.set_snapshot_fault(1, 3, false).await;
    let snapshot = leader_snapshot(&nodes).await;
    let response = network
        .full_snapshot(
            expected_vote,
            snapshot,
            std::future::pending::<ReplicationClosed>(),
            rpc_option(),
        )
        .await
        .expect("snapshot transfer must recover after the disk fault is cleared");
    assert_eq!(response.vote, expected_vote);

    nodes[2]
        .raft
        .wait(Some(timeout()))
        .applied_index_at_least(Some(snapshot_index), "follower recovered from snapshot")
        .await
        .expect("follower must install the recovered snapshot");
    assert_eq!(
        nodes[2]
            .read_config("sim-disk")
            .map(|stored| stored.content),
        Some("v1".to_string())
    );

    shutdown_all(&nodes).await;
}

/// Replay one seeded fault schedule against an isolated in-process network.
/// Each step constructs a fresh `SimNetwork` with the generated fault, so the
/// schedule is actually applied (live replication clients snapshot their link
/// fault at construction time). The trace records only deterministic facts.
async fn seeded_trace(seed: u64) -> Vec<String> {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    let vote = metrics(&nodes[0].raft).vote;
    let mut trace = vec![format!("seed={seed}")];

    for fault in fault_schedule(seed) {
        let mut network = SimNetwork::with_fault(
            1,
            3,
            router.clone(),
            LinkFault {
                delay_steps: fault.delay_steps,
                fail_first_appends: fault.fail_first_appends,
                ..Default::default()
            },
        );
        let before = router.stats().await;
        let mut outcomes = Vec::new();
        for _ in 0..4 {
            let request = AppendEntriesRequest {
                vote,
                prev_log_id: None,
                entries: vec![],
                leader_commit: None,
            };
            let result = network.append_entries(request, rpc_option()).await;
            outcomes.push(format!("{}", result.is_ok()));
        }
        let after = router.stats().await;
        trace.push(format!(
            "{:?}|{}|{}|{}",
            fault,
            outcomes.join(","),
            after.failed_appends - before.failed_appends,
            after.logical_delay_steps - before.logical_delay_steps,
        ));
    }

    shutdown_all(&nodes).await;
    trace
}

#[test]
fn seed_replays_an_identical_fault_schedule() {
    let first = fault_schedule(SIM_SEED);
    let second = fault_schedule(SIM_SEED);
    assert_eq!(first, second, "a fixed seed must not consult OS entropy");
    assert_eq!(first.len(), SCHEDULE_LEN);
    assert!(
        first
            .iter()
            .all(|f| f.delay_steps < 3 && f.fail_first_appends < 2),
        "generated faults must stay inside the router's bounded schedule"
    );
    assert_ne!(
        fault_schedule(SIM_SEED),
        fault_schedule(SIM_SEED ^ 1),
        "a different seed must produce a different schedule"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seeded_fault_run_is_reproducible() {
    let first = seeded_trace(SIM_SEED).await;
    let second = seeded_trace(SIM_SEED).await;
    assert_eq!(
        first, second,
        "the same (seed, source revision) must replay an identical trace"
    );
    assert_eq!(first.len(), SCHEDULE_LEN + 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paused_follower_catches_up_without_stale_read() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    let clock = LogicalClock::new();
    router.pause(3).await;

    let (write, index) = perform_write(&clock, &nodes[0].raft, "sim-pause", "v1").await;
    for node in &nodes[..2] {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(index), "quorum applied while follower is paused")
            .await
            .expect("the live quorum must commit during a process pause");
    }

    let mut paused = false;
    for _ in 0..200 {
        if router.stats().await.paused_appends > 0 {
            paused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(paused, "the pause must drop at least one append RPC");
    assert!(last_applied_index(&nodes[2].raft) < index);

    let stale = Operation {
        start: clock.tick(),
        end: clock.tick(),
        kind: OpKind::Read {
            key: "sim-pause".to_string(),
            value: nodes[2]
                .read_config("sim-pause")
                .map(|stored| stored.content),
        },
    };
    assert!(
        !is_linearizable(&[write.clone(), stale]),
        "a stale read from a paused follower must be rejected"
    );

    router.resume(3).await;
    nodes[0]
        .raft
        .trigger()
        .heartbeat()
        .await
        .expect("replication heartbeat after resume");
    nodes[2]
        .raft
        .wait(Some(timeout()))
        .applied_index_at_least(Some(index), "paused follower caught up")
        .await
        .expect("a resumed follower must catch up");

    let read = perform_read(&clock, &nodes[0].raft, &nodes[2], "sim-pause").await;
    match &read.kind {
        OpKind::Read { value, .. } => assert_eq!(value.as_deref(), Some("v1")),
        other => panic!("expected a read operation, got {other:?}"),
    }
    assert!(
        is_linearizable(&[write, read]),
        "the barrier read after resume must observe the committed write"
    );

    shutdown_all(&nodes).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clock_skew_delay_is_observable_and_cluster_converges() {
    let (router, nodes) = build_cluster().await;
    initialize_cluster(&nodes).await;
    router.set_clock_skew(Some(1), 3).await;
    let clock = LogicalClock::new();

    let (write, index) = perform_write(&clock, &nodes[0].raft, "sim-skew", "v1").await;
    for node in &nodes {
        node.raft
            .wait(Some(timeout()))
            .applied_index_at_least(Some(index), "every replica applies under clock skew")
            .await
            .expect("asymmetric logical delay must not lose a commit");
    }
    assert!(
        router.stats().await.clock_skew_steps > 0,
        "the injected skew must be observable in the simulation stats"
    );

    let read = perform_read(&clock, &nodes[0].raft, &nodes[2], "sim-skew").await;
    match &read.kind {
        OpKind::Read { value, .. } => assert_eq!(value.as_deref(), Some("v1")),
        other => panic!("expected a read operation, got {other:?}"),
    }
    assert!(
        is_linearizable(&[write, read]),
        "operations recorded under skew must still be linearizable"
    );

    shutdown_all(&nodes).await;
}
