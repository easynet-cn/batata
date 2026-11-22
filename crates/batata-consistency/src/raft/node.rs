//! RaftNode wrapper for managing Raft lifecycle.
//!
//! Provides a high-level API for interacting with the Raft consensus.

use crate::bincode;
use batata_common::datetime::DateTimeExt;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use openraft::BasicNode;
use openraft::async_runtime::watch::WatchReceiver;
use rocksdb::Options;
use tracing::{debug, info};

use super::config::RaftConfig;
use super::hooks::{self, SharedHooks};
use super::log_store::RocksLogStore;
use super::network::BatataRaftNetworkFactory;
use super::request::{RaftRequest, RaftResponse};
use super::state_machine::RocksStateMachine;
use super::telemetry::{
    RaftHealthSnapshot, RaftHealthTracker, emit_event, record_apply_wait, record_client_write,
    record_forward, record_health, record_leaderless_give_up,
};
use super::types::{NodeId, Raft, RaftMetrics, StoredMembership};
use crate::RaftPluginHandler;

/// Leader-discovery sampling budget, derived from `RaftConfig::timing()`.
///
/// The total wait must cover one maximum election window plus one RPC round
/// (Consul's `rpc_hold_timeout` plays the same role). Sampling at half a
/// heartbeat cannot miss a leader that appears during that window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LeaderlessBudget {
    max_polls: u32,
    poll_interval: Duration,
}

impl LeaderlessBudget {
    fn from_config(config: &RaftConfig) -> Self {
        let wait = config.timing().leaderless_wait;
        let poll_interval = (config.heartbeat_interval() / 2)
            .max(Duration::from_millis(10))
            .min(Duration::from_millis(200));
        let interval_ms = poll_interval.as_millis().max(1);
        let polls = wait.as_millis() / interval_ms;
        let max_polls = u32::try_from(polls).unwrap_or(u32::MAX).max(1);
        Self {
            max_polls,
            poll_interval,
        }
    }

    /// Total time the discovery loop may spend waiting for a leader.
    fn total_wait(&self) -> Duration {
        self.poll_interval * self.max_polls
    }
}

/// `Some(delay)` while an election may still settle, `None` once the budget is
/// exhausted so the write fails in bounded time.
fn leaderless_poll_delay(poll: u32, budget: LeaderlessBudget) -> Option<Duration> {
    (poll < budget.max_polls).then_some(budget.poll_interval)
}

#[derive(Debug, PartialEq, Eq)]
enum LeaderDiscovery {
    Ready(String),
    Wait(u32, Duration),
    GiveUp,
}

/// Decide the next step of write forwarding. A missing leader is an election
/// window, not a failed write: wait for it to settle (within the budget)
/// before giving up.
fn discover_leader(
    polls: u32,
    known_leader: Option<String>,
    budget: LeaderlessBudget,
) -> LeaderDiscovery {
    match known_leader {
        Some(addr) => LeaderDiscovery::Ready(addr),
        None => match leaderless_poll_delay(polls, budget) {
            Some(delay) => LeaderDiscovery::Wait(polls + 1, delay),
            None => LeaderDiscovery::GiveUp,
        },
    }
}

/// Resolve the index to report after a write forwarded to the leader.
///
/// The follower must only answer once its own state machine has applied the
/// entry. Reporting `leader_log_index` when `wait_for_applied` failed would
/// claim a read-after-write guarantee the node cannot honour, and would hide a
/// stalled replication stream behind a successful response.
fn forwarded_write_applied_index(
    wait_result: Result<(), Box<dyn std::error::Error + Send + Sync>>,
    local_applied: Option<u64>,
    leader_log_index: u64,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    wait_result?;
    local_applied.ok_or_else(|| {
        format!("follower has no last_applied index after waiting for {leader_log_index}").into()
    })
}

/// Interpret a leader's reply to a forwarded write.
///
/// `success = false` with a `leader_id` means the target stepped down and the
/// write never reached the state machine: that is a transport-level failure
/// and the caller must retry against the new leader. `success = false` without
/// a `leader_id` is the state machine's own business answer (a CAS index
/// mismatch, a contested lock, a conflicting transaction). That entry is
/// already committed, so the follower must pass the reply back with its
/// `data`/`message` intact; turning it into an error makes an ordinary Consul
/// conflict surface as a 503 and loses the `/v1/txn` error payload.
fn forwarded_write_response(
    response: batata_server_api::raft::ClientWriteResponse,
) -> Result<(RaftResponse, u64), Box<dyn std::error::Error + Send + Sync>> {
    let batata_server_api::raft::ClientWriteResponse {
        success,
        data,
        message,
        leader_id,
        log_index,
        ..
    } = response;
    if !success && leader_id.is_some() {
        return Err(message.into());
    }
    Ok((
        RaftResponse {
            success,
            data: (!data.is_empty()).then_some(data),
            message: (!message.is_empty()).then_some(message),
        },
        log_index,
    ))
}

/// Run the optional leader-side guard for one write.
/// Extracted from `write_with_index` so the leader-only ordering — validate,
/// then record accepted side effects — has a direct unit-test seam. Followers
/// skip the guard because only the leader's wall clock is authoritative;
/// forwarded writes re-enter this function on the leader through gRPC.
async fn run_write_guard(
    is_leader: bool,
    slot: &super::write_guard::SharedRaftWriteGuard,
    request: &RaftRequest,
) -> Result<(), String> {
    if !is_leader {
        return Ok(());
    }
    let Some(guard) = slot.read().await.clone() else {
        return Ok(());
    };
    guard.validate(request).await?;
    guard.on_accepted(request).await;
    Ok(())
}

/// Cached gRPC channel for leader write-forwarding.
/// Reuses the same HTTP/2 connection across requests to avoid per-write connect overhead.
/// Channel is cheap to clone (Arc internally) so we store it rather than the client.
struct LeaderChannel {
    addr: String,
    channel: tonic::transport::Channel,
}

/// High-level wrapper for Raft consensus node
pub struct RaftNode {
    /// Node ID
    node_id: NodeId,

    /// Node address (for Raft communication)
    addr: String,

    /// The underlying Raft instance. Since 0.10 the state machine type is
    /// named by `Raft` itself, so this uses the crate alias
    /// `Raft<TypeConfig, RocksStateMachine>`.
    raft: Raft,

    /// Configuration (reserved for future dynamic reconfiguration)
    config: RaftConfig,

    /// Shared RocksDB handle (same instance used by the state machine)
    db: Arc<rocksdb::DB>,

    /// The single shared handle set (same instance used by the state machine).
    /// Holds the plugin registry plus both apply-back hooks; registration
    /// replaces the inner `Arc<HookSet>` atomically. See `raft/hooks.rs`.
    hooks: SharedHooks,

    /// Optional leader-side pre-commit guard. Consulted only when this node
    /// is the leader, so wall-clock plugin state never reaches the FSM.
    write_guard: super::write_guard::SharedRaftWriteGuard,

    /// Cached gRPC channel for forwarding writes to the Raft leader.
    /// Avoids creating a new TCP/HTTP2 connection on every forwarded write.
    leader_channel: tokio::sync::Mutex<Option<LeaderChannel>>,

    /// Voter ids persisted in the state machine at construction time. Unlike
    /// `metrics()` (filled asynchronously by the engine), this is read directly
    /// from RocksDB before the store is handed to OpenRaft, so startup identity
    /// checks are deterministic and race-free.
    stored_voter_ids: BTreeSet<NodeId>,
}

impl RaftNode {
    /// Create a new Raft node
    pub async fn new(
        node_id: NodeId,
        addr: String,
        config: RaftConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (node, _db) = Self::new_with_db(node_id, addr, config).await?;
        Ok(node)
    }

    /// Create a new Raft node and return the underlying RocksDB handle.
    ///
    /// The returned `Arc<DB>` shares the same RocksDB instance used by the
    /// Raft state machine, so it can be used to construct a `RocksDbReader`
    /// for read-only queries against the state machine's data.
    pub async fn new_with_db(
        node_id: NodeId,
        addr: String,
        config: RaftConfig,
    ) -> Result<(Self, Arc<rocksdb::DB>), Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_db_and_options(node_id, addr, config, None, None, &[]).await
    }

    /// Create a new Raft node with custom RocksDB options and extra column families.
    ///
    /// `extra_cf_names` are additional column families required by plugins (e.g., Consul CFs).
    /// They are created alongside the core CFs when RocksDB is opened.
    pub async fn new_with_db_and_options(
        node_id: NodeId,
        addr: String,
        config: RaftConfig,
        db_opts: Option<Options>,
        cf_opts: Option<Options>,
        extra_cf_names: &[String],
    ) -> Result<(Self, Arc<rocksdb::DB>), Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_full_options(
            node_id,
            addr,
            config,
            db_opts,
            cf_opts,
            None,
            None,
            extra_cf_names,
        )
        .await
    }

    /// Create a new Raft node with full RocksDB customization including
    /// separate history CF options and WriteOptions.
    pub async fn new_with_full_options(
        node_id: NodeId,
        addr: String,
        config: RaftConfig,
        db_opts: Option<Options>,
        cf_opts: Option<Options>,
        history_cf_opts: Option<Options>,
        write_opts: Option<rocksdb::WriteOptions>,
        extra_cf_names: &[String],
    ) -> Result<(Self, Arc<rocksdb::DB>), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Creating Raft node: id={}, addr={}, data_dir={:?}",
            node_id, addr, config.data_dir
        );

        // Reject timing combinations that openraft or the derived write
        // deadlines cannot honour before any directory or DB is opened.
        config
            .validate()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
        let timing = config.timing();
        info!(
            "Raft timing derived: election={:?}..{:?} heartbeat={:?} election_cycle={:?} leaderless_wait={:?} apply_wait={:?} client_write={:?} leaderless_budget={:?}",
            timing.election_timeout_min,
            timing.election_timeout_max,
            config.heartbeat_interval(),
            timing.election_cycle,
            timing.leaderless_wait,
            timing.apply_wait,
            timing.client_write,
            LeaderlessBudget::from_config(&config).total_wait(),
        );

        // Ensure data directories exist
        config.ensure_dirs().await?;

        // Create log store with custom options
        let log_store =
            RocksLogStore::with_options(config.log_dir(), db_opts.clone(), cf_opts.clone()).await?;

        // Create state machine with full options (history CF, WriteOptions)
        let state_machine = RocksStateMachine::with_full_options(
            config.state_machine_dir(),
            db_opts,
            cf_opts,
            history_cf_opts,
            write_opts,
            extra_cf_names,
        )
        .await?
        .with_snapshot_dir(config.snapshot_dir());
        let db = state_machine.db();
        let hooks = state_machine.hooks();
        // Snapshot the persisted membership before Raft::new consumes the state
        // machine. This is the authoritative identity generation of the data
        // directory and is available synchronously, unlike the metrics watcher.
        let stored_voter_ids = state_machine.stored_voter_ids().await;

        // Create network factory with configured timeouts
        let network_config = super::network::RaftNetworkConfig::from_raft_config(&config);
        let network_factory = BatataRaftNetworkFactory::with_config(network_config);

        // Create openraft config
        let raft_config = Arc::new(config.to_openraft_config());

        // Create the Raft instance (consumes state_machine)
        let raft = Raft::new(
            node_id,
            raft_config,
            network_factory,
            log_store,
            state_machine,
        )
        .await?;

        info!("Raft node created successfully: id={}", node_id);

        Ok((
            Self {
                node_id,
                addr,
                raft,
                config,
                db: db.clone(),
                hooks,
                write_guard: super::write_guard::new_shared_write_guard(),
                leader_channel: tokio::sync::Mutex::new(None),
                stored_voter_ids,
            },
            db,
        ))
    }

    /// Get the node ID
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get the node address
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Voter ids persisted in the state machine when this node was created.
    /// Empty on a fresh data dir; otherwise the identity generation stored on
    /// disk, used to reject reuse across a node-id scheme change.
    pub fn stored_voter_ids(&self) -> &BTreeSet<NodeId> {
        &self.stored_voter_ids
    }

    /// Get the underlying Raft instance
    pub fn raft(&self) -> &Raft {
        &self.raft
    }

    /// Get a reference to the shared RocksDB instance.
    pub fn db(&self) -> Arc<rocksdb::DB> {
        self.db.clone()
    }

    /// Register a plugin handler for processing PluginWrite operations.
    ///
    /// Safe to call after Raft startup — the hook set is shared between
    /// this node and the state machine via `Arc`.
    pub async fn register_plugin(&self, handler: Arc<dyn RaftPluginHandler>) -> Result<(), String> {
        hooks::register_plugin_handler(&self.hooks, handler).await
    }

    /// Register the naming apply hook so `PersistentInstance*` applies flow
    /// back into the in-memory `NamingService` DashMap.
    ///
    /// Safe to call after Raft startup — the hook set is shared with the
    /// state machine via `Arc<RwLock<Arc<HookSet>>>` and consulted on every
    /// apply. A second registration replaces the first.
    pub async fn register_naming_hook(&self, hook: Arc<dyn super::naming_hook::NamingApplyHook>) {
        hooks::install_naming_hook(&self.hooks, hook).await;
    }

    /// Register the health-check apply hook so `HealthCheckStatusUpdate` /
    /// `HealthCheckTtlUpdate` Raft entries flow back into the in-memory
    /// `InstanceCheckRegistry` on every cluster node. Mirror of
    /// `register_naming_hook`.
    ///
    /// The slot is single-owner, so a second registration replaces the first.
    /// That is logged rather than silently accepted — see
    /// `install_health_check_hook` for why this matters and what must change
    /// before a second facade (Nacos included) can write through this channel.
    pub async fn register_health_check_hook(
        &self,
        hook: Arc<dyn super::health_check_hook::HealthCheckApplyHook>,
    ) {
        hooks::install_health_check_hook(&self.hooks, hook).await;
    }

    /// Install a leader-side pre-commit guard. The slot is single-owner; a
    /// second registration replaces the first, which is appropriate because
    /// the leader must apply exactly one plugin's validation policy.
    pub async fn register_write_guard(&self, guard: Arc<dyn super::write_guard::RaftWriteGuard>) {
        *self.write_guard.write().await = Some(guard);
    }

    /// Replay all persistent instances from RocksDB `CF_INSTANCES` into the
    /// registered naming hook. Call once at startup (after `register_naming_hook`)
    /// so `NamingService` reflects the committed state without waiting for
    /// new Raft applies.
    pub async fn replay_persistent_instances(&self) -> Result<usize, String> {
        let hook = {
            let guard = self.hooks.read().await;
            guard.naming.clone()
        };
        let Some(hook) = hook else {
            return Ok(0);
        };

        let cf = self
            .db
            .cf_handle(super::state_machine::CF_INSTANCES)
            .ok_or_else(|| "CF_INSTANCES not found".to_string())?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        let mut count = 0usize;
        for item in iter {
            let (_k, v) = match item {
                Ok(kv) => kv,
                Err(e) => {
                    tracing::warn!("Failed to read instance during replay: {}", e);
                    continue;
                }
            };
            // bincode-encoded StoredInstance written by apply_instance_register.
            // Persistent storage format is intentionally unversioned —
            // RocksDB data dirs must be wiped on upgrade.
            let stored: super::state_machine::StoredInstance = match bincode::deserialize(&v) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to decode StoredInstance during replay: {}", e);
                    continue;
                }
            };
            hook.on_register(
                &stored.namespace_id,
                &stored.group_name,
                &stored.service_name,
                &stored.instance_id,
                &stored.ip,
                stored.port,
                stored.weight,
                stored.healthy,
                stored.enabled,
                &stored.metadata,
                &stored.cluster_name,
            );
            count += 1;
        }
        tracing::info!(
            "Replayed {} persistent instances from CF_INSTANCES to NamingService",
            count
        );
        Ok(count)
    }

    /// Get current metrics
    pub fn metrics(&self) -> RaftMetrics {
        self.raft.metrics().borrow_watched().clone()
    }

    /// Directory snapshots live in, including the `incoming/` staging area used
    /// while a snapshot streams in from the leader.
    pub fn snapshot_dir(&self) -> std::path::PathBuf {
        self.config.snapshot_dir()
    }

    /// Check whether this node is a voting member of the Raft group.
    ///
    /// Nacos equivalent: `CircuitFilter` only opens the config service once this
    /// node's own Raft address shows up in the Raft group's peer list. Until the
    /// group has been formed the local store holds no (or stale) replicated
    /// data, so serving reads from it would silently return missing or outdated
    /// values.
    ///
    /// Note this is weaker than [`RaftNode::is_ready`]: a member that is still
    /// replaying log entries is reported as a member here, matching upstream
    /// which accepts eventual consistency for config reads.
    pub fn is_group_member(&self) -> bool {
        let watcher = self.raft.metrics();
        let metrics = watcher.borrow_watched();
        let self_id = self.node_id();
        metrics
            .membership_config
            .voter_ids()
            .any(|id| id == self_id)
    }

    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        let metrics = self.metrics();
        matches!(metrics.state, openraft::ServerState::Leader)
    }

    /// Get the current leader ID
    pub fn leader_id(&self) -> Option<NodeId> {
        let metrics = self.metrics();
        metrics.current_leader
    }

    /// Get the current leader's address (if known)
    pub fn leader_addr(&self) -> Option<String> {
        let metrics = self.metrics();
        if let Some(leader_id) = metrics.current_leader {
            // Look up the leader's address in the membership config
            let membership = metrics.membership_config.membership();
            if let Some(node) = membership.get_node(&leader_id) {
                return Some(node.addr.clone());
            }
        }
        None
    }

    /// Get a node's address by ID from the current membership
    pub fn get_node_addr(&self, node_id: NodeId) -> Option<String> {
        let metrics = self.metrics();
        let membership = metrics.membership_config.membership();
        membership.get_node(&node_id).map(|n| n.addr.clone())
    }

    /// Initialize the cluster with the given members
    /// This should only be called on the first node of a new cluster
    pub async fn initialize(
        &self,
        members: BTreeMap<NodeId, BasicNode>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Initializing Raft cluster with {} members", members.len());

        self.raft.initialize(members).await?;

        info!("Raft cluster initialized successfully");
        Ok(())
    }

    /// Add a learner node to the cluster
    pub async fn add_learner(
        &self,
        node_id: NodeId,
        addr: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Adding learner: id={}, addr={}", node_id, addr);

        let node = BasicNode { addr };
        self.raft.add_learner(node_id, node, true).await?;

        info!("Learner added successfully: id={}", node_id);
        Ok(())
    }

    /// Change cluster membership
    pub async fn change_membership(
        &self,
        members: BTreeSet<NodeId>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Changing membership to: {:?}", members);

        self.raft.change_membership(members, false).await?;

        info!("Membership changed successfully");
        Ok(())
    }

    /// Write data through Raft consensus.
    ///
    /// If this node is the leader, applies the write locally.
    /// If this node is a follower, forwards the write to the Raft leader
    /// via the RaftManagementService gRPC endpoint, matching Nacos's
    /// JRaft write-forwarding behavior.
    pub async fn write(
        &self,
        request: RaftRequest,
    ) -> Result<RaftResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (resp, _log_index) = self.write_with_index(request).await?;
        Ok(resp)
    }

    /// Write a request through Raft and return both the response and the Raft log index.
    ///
    /// The log index is the globally consistent Raft commit index for this write,
    /// suitable for use as CreateIndex/ModifyIndex/X-Consul-Index.
    pub async fn write_with_index(
        &self,
        request: RaftRequest,
    ) -> Result<(RaftResponse, u64), Box<dyn std::error::Error + Send + Sync>> {
        let started = std::time::Instant::now();
        let timing = self.config.timing();
        let budget = LeaderlessBudget::from_config(&self.config);
        debug!("Writing through Raft: {}", request.op_type());

        // Leader-local wall-clock checks (e.g. Consul lock delay) must happen
        // before the log entry exists. Followers skip the guard: their copy of
        // leader-local state is advisory, and forwarded writes run this same
        // guard on the leader before `client_write`.
        if let Err(e) = run_write_guard(self.is_leader(), &self.write_guard, &request).await {
            return Err(e.into());
        }

        // Try local client_write first (succeeds if this node is the leader).
        // Bound it: openraft has no deadline of its own, and a leadership
        // change used to leave this future pending until the client gave up.
        let local_write =
            tokio::time::timeout(timing.client_write, self.raft.client_write(request.clone()))
                .await;
        let local_timed_out = local_write.is_err();
        record_client_write(started.elapsed(), local_timed_out);
        let local_result: Result<_, Box<dyn std::error::Error + Send + Sync>> = match local_write {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(Box::new(e)),
            Err(_) => Err(format!(
                "local client_write timed out after {:?}",
                timing.client_write
            )
            .into()),
        };

        match local_result {
            Ok(result) => {
                let log_index = result.log_id.index;
                Ok((result.data, log_index))
            }
            Err(e) => {
                // Forward to leader with retry, exponential backoff, and re-discovery.
                // Serialize the request once for all retry attempts to avoid
                // repeated serialization of potentially large config content.
                let max_retries = self.config.forward_max_retries;
                let initial_delay = self.config.forward_initial_delay_ms;
                let mut last_err = e;
                let mut leaderless_polls = 0u32;

                let request_bytes = serde_json::to_vec(&request)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
                let op_type = request.op_type();

                for attempt in 0..max_retries {
                    // Raft metrics briefly report no leader during an election.
                    // Poll for a bounded time so the write can still be
                    // forwarded once the election settles, instead of
                    // returning a spurious 503 (or hanging on a stale future).
                    let leader_addr = loop {
                        match discover_leader(leaderless_polls, self.leader_addr(), budget) {
                            LeaderDiscovery::Ready(addr) => break Some(addr),
                            LeaderDiscovery::Wait(next_polls, delay) => {
                                leaderless_polls = next_polls;
                                last_err = "no Raft leader known while an election is in flight"
                                    .to_string()
                                    .into();
                                tokio::time::sleep(delay).await;
                            }
                            LeaderDiscovery::GiveUp => break None,
                        }
                    };
                    let Some(leader_addr) = leader_addr else {
                        record_leaderless_give_up(started.elapsed());
                        break;
                    };

                    debug!(
                        "Forwarding write to Raft leader at {} (attempt {}/{}): {}",
                        leader_addr,
                        attempt + 1,
                        max_retries,
                        op_type
                    );

                    let forward_started = std::time::Instant::now();
                    let forwarded = self
                        .forward_write_to_leader_bytes(&leader_addr, &request_bytes)
                        .await;
                    record_forward(forward_started.elapsed(), forwarded.is_ok());
                    match forwarded {
                        Ok((resp, leader_log_index)) => {
                            // Wait for the local state machine to catch up to
                            // the leader's committed index before returning, so
                            // subsequent local reads see the written data.
                            let wait_started = std::time::Instant::now();
                            let wait_result = if leader_log_index > 0 {
                                self.wait_for_applied(leader_log_index, timing.apply_wait)
                                    .await
                            } else {
                                Ok(())
                            };
                            record_apply_wait(wait_started.elapsed(), wait_result.is_err());
                            let idx = forwarded_write_applied_index(
                                wait_result,
                                self.last_applied_index(),
                                leader_log_index,
                            )
                            .map_err(|e| {
                                tracing::error!("Follower wait-for-applied failed: {}", e);
                                e
                            })?;
                            return Ok((resp, idx));
                        }
                        Err(forward_err) => {
                            tracing::warn!(
                                "Forward to leader {} failed (attempt {}/{}): {}",
                                leader_addr,
                                attempt + 1,
                                max_retries,
                                forward_err
                            );
                            last_err = forward_err;
                            // Exponential backoff with jitter before re-discovering leader
                            let delay = initial_delay * (1u64 << attempt.min(4));
                            let jitter = delay / 4;
                            let actual_delay = if jitter > 0 {
                                delay
                                    + (std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .subsec_nanos()
                                        as u64
                                        % jitter)
                            } else {
                                delay
                            };
                            tokio::time::sleep(Duration::from_millis(actual_delay)).await;
                        }
                    }
                }

                Err(last_err)
            }
        }
    }

    /// Forward pre-serialized request bytes to the leader.
    /// Avoids re-serializing the request on each retry attempt.
    async fn forward_write_to_leader_bytes(
        &self,
        leader_addr: &str,
        request_bytes: &[u8],
    ) -> Result<(RaftResponse, u64), Box<dyn std::error::Error + Send + Sync>> {
        use batata_server_api::raft::ClientWriteRequest;
        use batata_server_api::raft::raft_management_service_client::RaftManagementServiceClient;

        // Cache lookup — release the lock immediately so concurrent forwards
        // don't serialize on each other. Holding the Mutex across
        // `Channel::connect().await` (up to 5s) previously turned parallel
        // follower writes into a single-file queue.
        let cached_channel = {
            let guard = self.leader_channel.lock().await;
            guard
                .as_ref()
                .filter(|cached| cached.addr == leader_addr)
                .map(|cached| cached.channel.clone())
        };

        let channel = match cached_channel {
            Some(channel) => channel,
            None => {
                // Cache miss: connect without holding the Mutex. Multiple
                // concurrent misses may race to create channels; that's fine
                // because `connect()` is idempotent and the install step below
                // is double-checked so only one channel ends up cached.
                let endpoint = format!("http://{}", leader_addr);
                let new_channel = tonic::transport::Channel::from_shared(endpoint)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                    .connect_timeout(Duration::from_secs(5))
                    .tcp_keepalive(Some(Duration::from_secs(10)))
                    .tcp_nodelay(true)
                    .http2_keep_alive_interval(Duration::from_secs(10))
                    .connect()
                    .await?;

                let mut guard = self.leader_channel.lock().await;
                match guard.as_ref() {
                    // Another task already installed a channel for the same
                    // leader — prefer the cached one so the racing duplicate
                    // drops when we return.
                    Some(cached) if cached.addr == leader_addr => cached.channel.clone(),
                    _ => {
                        *guard = Some(LeaderChannel {
                            addr: leader_addr.to_string(),
                            channel: new_channel.clone(),
                        });
                        new_channel
                    }
                }
            }
        };

        let mut client = RaftManagementServiceClient::new(channel);
        // Bound the leader RPC: the channel only has a connect timeout, so a
        // stalled leader connection would hang the follower's forwarded write
        // (and its retry loop) indefinitely.
        let write_fut = client.write(ClientWriteRequest {
            data: request_bytes.to_vec(),
        });
        let response = match tokio::time::timeout(Duration::from_secs(30), write_fut).await {
            Ok(Ok(resp)) => resp.into_inner(),
            Ok(Err(e)) => return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            Err(_) => {
                return Err("forward write to leader timed out after 30s"
                    .to_string()
                    .into());
            }
        };

        forwarded_write_response(response)
    }

    /// Trigger an election on a remote node (for leader transfer).
    ///
    /// This sends a gRPC `TriggerElection` request to the specified node,
    /// causing it to call `raft.trigger().elect(false)` and start a new election.
    /// Used by the `transfer_leader` HTTP handler to move leadership to the
    /// target follower without going through openraft's vote-request based
    /// transfer path.
    pub async fn trigger_remote_election(&self, addr: &str) -> Result<(), String> {
        use batata_server_api::raft::TriggerElectionRequest;
        use batata_server_api::raft::raft_management_service_client::RaftManagementServiceClient;

        let endpoint = format!("http://{}", addr);
        let mut client = RaftManagementServiceClient::connect(endpoint)
            .await
            .map_err(|e| format!("Failed to connect to node {}: {}", addr, e))?;

        let request = tonic::Request::new(TriggerElectionRequest { node_id: 0 });
        let response = client
            .trigger_election(request)
            .await
            .map_err(|e| format!("TriggerElection RPC to {} failed: {}", addr, e))?
            .into_inner();

        if response.success {
            info!("Remote election triggered on {}", addr);
            Ok(())
        } else {
            Err(format!(
                "Remote election failed on {}: {}",
                addr, response.message
            ))
        }
    }

    /// Wait for the leader to change from the current leader ID.
    ///
    /// Returns `Ok(())` if the leader changes within the timeout, `Err` otherwise.
    /// Used by the `transfer_leader` HTTP handler to confirm that the election
    /// triggered on the target follower actually resulted in a new leader.
    pub async fn wait_for_leader_change(
        &self,
        old_leader: NodeId,
        timeout: Duration,
    ) -> Result<(), String> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() >= timeout {
                return Err(format!(
                    "Leader transfer timed out after {}s",
                    timeout.as_secs()
                ));
            }

            let metrics = self.metrics();
            if let Some(new_leader) = metrics.current_leader
                && new_leader != old_leader
            {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Perform a linearizable read
    /// This ensures the read sees all committed data up to the point of the read
    pub async fn linearizable_read(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 0.10 requires an explicit read policy. ReadIndex performs a quorum
        // round trip, matching the 0.9 `ensure_linearizable()` semantics.
        self.raft
            .ensure_linearizable(openraft::ReadPolicy::ReadIndex)
            .await?;
        Ok(())
    }

    /// Start a background task that publishes Raft health metrics.
    ///
    /// Instead of polling on a timer, the task subscribes to openraft's
    /// metrics watch channel: every state change (including a Candidate state
    /// that lasts only a few milliseconds) is observed. A 100ms poll was
    /// measured to miss two of three elections on a fast leader node.
    /// The task only reads metrics; it never writes or changes protocol state.
    /// It stops when the owning `Arc<RaftNode>` is dropped or the Raft handle
    /// closes the channel.
    pub fn start_health_telemetry(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        use std::sync::Weak;

        let weak: Weak<Self> = Arc::downgrade(self);
        let node_id = self.node_id;
        let mut metrics_rx = self.raft.metrics();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            info!(
                target: super::telemetry::HEALTH_LOG_TARGET,
                raft_event = "telemetry_started",
                node_id = node_id,
                "Raft health telemetry started (metrics watch)"
            );
            let mut tracker = RaftHealthTracker::default();
            loop {
                if weak.upgrade().is_none() {
                    debug!("Raft health telemetry exiting (RaftNode dropped)");
                    break;
                }
                let current = metrics_rx.borrow_watched().clone();
                let snapshot = RaftHealthSnapshot::from(&current);
                let now_ms = started.elapsed().as_millis() as u64;
                for event in tracker.observe(snapshot, now_ms) {
                    emit_event(node_id, &event);
                }
                record_health(&snapshot, tracker.steady_state_elections);
                if metrics_rx.changed().await.is_err() {
                    debug!("Raft health telemetry exiting (metrics channel closed)");
                    break;
                }
            }
        })
    }

    /// Start a background task that periodically reaps expired locks.
    ///
    /// The task runs on every node but only issues `LockExpire` writes when
    /// this node is the Raft leader (followers are no-ops to avoid duplicate
    /// write amplification). Each scan iterates `CF_LOCKS`, decodes the JSON
    /// blob to find entries whose `expires_at` is in the past and whose
    /// state is still `Locked`, then issues a replicated `LockExpire` for
    /// each — matching Nacos's `LockExpireTask` semantics.
    ///
    /// The returned `JoinHandle` can be awaited during shutdown. The task
    /// terminates when the holding `Arc<RaftNode>` is dropped (it holds a
    /// `Weak` reference to observe shutdown).
    pub fn start_lock_expire_scanner(
        self: &Arc<Self>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        use std::sync::Weak;
        let weak: Weak<Self> = Arc::downgrade(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // First tick fires immediately; skip it so we don't burst on startup
            // before the node has finished joining the cluster.
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let Some(node) = weak.upgrade() else {
                    debug!("Lock expire scanner exiting (RaftNode dropped)");
                    break;
                };
                // Every node scans the local RocksDB to publish the
                // alive-count gauge (so follower dashboards stay truthful),
                // but only the leader issues LockExpire writes.
                let (expired, alive_count) = match node.scan_expired_locks() {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("Lock expire scanner: scan failed: {}", e);
                        continue;
                    }
                };
                metrics::gauge!("batata_monitor_lock_alive_count").set(alive_count as f64);

                if !node.is_leader() {
                    continue;
                }
                if expired.is_empty() {
                    continue;
                }
                debug!("Lock expire scanner: {} expired lock(s)", expired.len());
                for (namespace, name) in expired {
                    let req = super::request::RaftRequest::LockExpire {
                        namespace: namespace.clone(),
                        name: name.clone(),
                    };
                    if let Err(e) = node.write(req).await {
                        tracing::warn!(
                            "Lock expire scanner: write LockExpire({}::{}) failed: {}",
                            namespace,
                            name,
                            e
                        );
                    }
                }
            }
        })
    }

    /// List all lock entries from `CF_LOCKS`. Returns the raw JSON values
    /// for admin introspection. Filters by namespace when non-empty.
    pub fn list_locks(
        &self,
        namespace_filter: &str,
    ) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
        let cf = self
            .db
            .cf_handle(super::state_machine::CF_LOCKS)
            .ok_or("CF_LOCKS missing")?;
        let mut results = Vec::new();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_key, val) = match item {
                Ok(kv) => kv,
                Err(e) => return Err(Box::new(e)),
            };
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&val) else {
                continue;
            };
            if !namespace_filter.is_empty() && v["namespace"].as_str() != Some(namespace_filter) {
                continue;
            }
            results.push(v);
        }
        Ok(results)
    }

    /// Scan `CF_LOCKS` for entries whose `expires_at` is in the past and
    /// whose `state == "Locked"`. Returns `(expired, alive_count)` where
    /// `expired` is the list of `(namespace, name)` to reap and
    /// `alive_count` is the number of currently-held (non-expired,
    /// `state == "Locked"`) entries observed in this pass.
    ///
    /// This is a read-only scan — it does NOT mutate state. Callers (e.g.
    /// the expire scanner) must route the actual expire through Raft so
    /// followers apply the same state change.
    fn scan_expired_locks(
        &self,
    ) -> Result<(Vec<(String, String)>, u64), Box<dyn std::error::Error + Send + Sync>> {
        let cf = self
            .db
            .cf_handle(super::state_machine::CF_LOCKS)
            .ok_or("CF_LOCKS missing")?;
        let now_ms = batata_common::datetime::Utc::now().timestamp_millis();
        let mut expired = Vec::new();
        let mut alive_count: u64 = 0;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (_key, val) = match item {
                Ok(kv) => kv,
                Err(e) => return Err(Box::new(e)),
            };
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&val) else {
                continue;
            };
            if v["state"].as_str() != Some("Locked") {
                continue;
            }
            let Some(exp) = v["expires_at"].as_i64() else {
                continue;
            };
            if exp > now_ms {
                alive_count += 1;
                continue;
            }
            let ns = v["namespace"].as_str().unwrap_or("").to_string();
            let name = v["name"].as_str().unwrap_or("").to_string();
            if !name.is_empty() {
                expired.push((ns, name));
            }
        }
        Ok((expired, alive_count))
    }

    /// Shutdown the Raft node
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down Raft node: id={}", self.node_id);

        self.raft.shutdown().await?;

        info!("Raft node shutdown complete");
        Ok(())
    }

    /// Wait for the node to become leader (with timeout)
    pub async fn wait_for_leader(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Option<NodeId>, Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();

        loop {
            let metrics = self.metrics();
            if let Some(leader_id) = metrics.current_leader {
                return Ok(Some(leader_id));
            }

            if start.elapsed() > timeout {
                return Ok(None);
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    /// Get cluster membership information
    pub fn membership(&self) -> StoredMembership {
        (*self.metrics().membership_config).clone()
    }

    /// Get the last applied log index
    pub fn last_applied_index(&self) -> Option<u64> {
        self.metrics().last_applied.map(|l| l.index)
    }

    /// Wait until the local state machine has applied at least up to `target_index`.
    ///
    /// This is used by follower nodes after forwarding a write to the leader.
    /// The leader returns the committed log index; the follower must wait for
    /// its local state machine to catch up before serving reads.
    pub async fn wait_for_applied(
        &self,
        target_index: u64,
        timeout: Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let start = std::time::Instant::now();
        loop {
            let applied = self.last_applied_index().unwrap_or(0);
            if applied >= target_index {
                return Ok(());
            }
            if start.elapsed() > timeout {
                return Err(format!(
                    "Timed out waiting for local apply: applied={}, target={}",
                    applied, target_index
                )
                .into());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Get the commit index (using last_applied as proxy)
    pub fn commit_index(&self) -> Option<u64> {
        self.metrics().last_applied.map(|l| l.index)
    }

    /// Check whether the Raft node is ready to serve reads.
    ///
    /// Ready means:
    /// 1. A leader has been elected (cluster is functional).
    /// 2. This node has applied all received log entries to its state machine
    ///    (config, namespace, user data in RocksDB is up-to-date).
    ///
    /// Returns `(ready, reason)` where `reason` explains why the node is not ready.
    pub fn is_ready(&self) -> (bool, Option<String>) {
        let metrics = self.metrics();

        // 1. Leader must exist
        if metrics.current_leader.is_none() {
            return (false, Some("raft leader not elected".to_string()));
        }

        // 2. All received logs must be applied to the state machine
        let last_log = metrics.last_log_index.unwrap_or(0);
        let last_applied = metrics.last_applied.map(|l| l.index).unwrap_or(0);

        if last_applied < last_log {
            return (
                false,
                Some(format!(
                    "raft log replay in progress ({}/{})",
                    last_applied, last_log
                )),
            );
        }

        (true, None)
    }

    /// Get the current term
    pub fn current_term(&self) -> u64 {
        self.metrics().current_term
    }

    /// Get the current state (Leader, Follower, Candidate, etc.)
    pub fn state(&self) -> openraft::ServerState {
        self.metrics().state
    }
}

/// Builder for creating RaftNode instances
pub struct RaftNodeBuilder {
    node_id: Option<NodeId>,
    addr: Option<String>,
    config: RaftConfig,
}

impl RaftNodeBuilder {
    /// Create a new builder with no node ID, address, or custom configuration set.
    pub fn new() -> Self {
        Self {
            node_id: None,
            addr: None,
            config: RaftConfig::default(),
        }
    }

    /// Set the node ID for the node being built.
    pub fn node_id(mut self, id: NodeId) -> Self {
        self.node_id = Some(id);
        self
    }

    /// Set the node's Raft communication address.
    pub fn addr(mut self, addr: impl Into<String>) -> Self {
        self.addr = Some(addr.into());
        self
    }

    /// Set the Raft configuration.
    pub fn config(mut self, config: RaftConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the [`RaftNode`], returning an error if the node ID or address is missing.
    pub async fn build(self) -> Result<RaftNode, Box<dyn std::error::Error + Send + Sync>> {
        let node_id = self.node_id.ok_or("Node ID is required")?;
        let addr = self.addr.ok_or("Address is required")?;

        RaftNode::new(node_id, addr, self.config).await
    }
}

impl Default for RaftNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;

    use crate::raft::write_guard::{RaftWriteGuard, new_shared_write_guard};

    #[derive(Default)]
    struct RecordingGuard {
        calls: Arc<std::sync::Mutex<Vec<&'static str>>>,
        reject: bool,
    }

    #[async_trait]
    impl RaftWriteGuard for RecordingGuard {
        async fn validate(&self, _request: &RaftRequest) -> Result<(), String> {
            self.calls.lock().unwrap().push("validate");
            if self.reject {
                Err("rejected".to_string())
            } else {
                Ok(())
            }
        }

        async fn on_accepted(&self, _request: &RaftRequest) {
            self.calls.lock().unwrap().push("accepted");
        }
    }

    #[tokio::test]
    async fn leader_guard_records_accepted_after_validation() {
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let slot = new_shared_write_guard();
        *slot.write().await = Some(Arc::new(RecordingGuard {
            calls: calls.clone(),
            reject: false,
        }));

        run_write_guard(true, &slot, &RaftRequest::Noop)
            .await
            .unwrap();
        assert_eq!(*calls.lock().unwrap(), vec!["validate", "accepted"]);
    }

    #[tokio::test]
    async fn follower_skips_leader_guard() {
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let slot = new_shared_write_guard();
        *slot.write().await = Some(Arc::new(RecordingGuard {
            calls: calls.clone(),
            reject: false,
        }));

        run_write_guard(false, &slot, &RaftRequest::Noop)
            .await
            .unwrap();
        assert!(calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn rejected_guard_does_not_record_accepted() {
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let slot = new_shared_write_guard();
        *slot.write().await = Some(Arc::new(RecordingGuard {
            calls: calls.clone(),
            reject: true,
        }));

        assert!(
            run_write_guard(true, &slot, &RaftRequest::Noop)
                .await
                .is_err()
        );
        assert_eq!(*calls.lock().unwrap(), vec!["validate"]);
    }

    #[test]
    fn test_builder() {
        let builder = RaftNodeBuilder::new()
            .node_id(1)
            .addr("127.0.0.1:7848")
            .config(RaftConfig::default());

        assert!(builder.node_id.is_some());
        assert!(builder.addr.is_some());
    }

    #[test]
    fn forwarded_write_reports_local_applied_index() {
        let idx = forwarded_write_applied_index(Ok(()), Some(12), 12).unwrap();
        assert_eq!(idx, 12);
    }

    #[test]
    fn forwarded_write_does_not_claim_leader_index_when_wait_timed_out() {
        let err: Box<dyn std::error::Error + Send + Sync> = "timed out".into();
        // Removing the `wait_result?` guard would return 12 here.
        assert!(forwarded_write_applied_index(Err(err), Some(3), 12).is_err());
    }

    #[test]
    fn forwarded_write_does_not_invent_an_index_when_metrics_are_missing() {
        let err = forwarded_write_applied_index(Ok(()), None, 12).unwrap_err();
        assert!(err.to_string().contains("no last_applied index"), "{err}");
    }

    #[test]
    fn forwarded_write_keeps_committed_state_machine_conflict() {
        use batata_server_api::raft::ClientWriteResponse;
        // A CAS mismatch / txn conflict is a committed business result, not a
        // transport failure. Dropping `data` here is what made a follower
        // answer /v1/txn with 503 instead of 409 + Errors.
        let payload = br#"{"Results":[],"Errors":[{"OpIndex":0,"What":"CAS failed"}]}"#.to_vec();
        let (out, idx) = forwarded_write_response(ClientWriteResponse {
            success: false,
            data: payload.clone(),
            message: "CAS failed: index mismatch".into(),
            leader_id: None,
            leader_addr: None,
            log_index: 42,
        })
        .unwrap();
        assert!(!out.success);
        assert_eq!(idx, 42);
        assert_eq!(out.data, Some(payload));
        assert_eq!(out.message.as_deref(), Some("CAS failed: index mismatch"));
    }

    #[test]
    fn forwarded_write_keeps_success_payload_and_message() {
        use batata_server_api::raft::ClientWriteResponse;
        let (out, idx) = forwarded_write_response(ClientWriteResponse {
            success: true,
            data: vec![9, 8, 7],
            message: String::new(),
            leader_id: None,
            leader_addr: None,
            log_index: 7,
        })
        .unwrap();
        assert!(out.success);
        assert_eq!(idx, 7);
        assert_eq!(out.data, Some(vec![9, 8, 7]));
        assert_eq!(out.message, None);
    }

    #[test]
    fn forwarded_write_still_retries_when_target_is_not_leader() {
        use batata_server_api::raft::ClientWriteResponse;
        // Losing the `leader_id` distinction would make the retry loop accept
        // a redirect as a committed conflict and silently drop the write.
        let err = forwarded_write_response(ClientWriteResponse {
            success: false,
            data: Vec::new(),
            message: "Not leader, redirect to leader 2".into(),
            leader_id: Some(2),
            leader_addr: Some("127.0.0.1:7858".into()),
            log_index: 0,
        })
        .unwrap_err();
        assert!(err.to_string().contains("Not leader"), "{err}");
    }

    fn default_budget() -> LeaderlessBudget {
        LeaderlessBudget::from_config(&RaftConfig::default())
    }

    #[test]
    fn leaderless_discovery_is_bounded_and_derived() {
        let budget = default_budget();
        assert_eq!(leaderless_poll_delay(0, budget), Some(budget.poll_interval));
        assert_eq!(
            leaderless_poll_delay(budget.max_polls - 1, budget),
            Some(budget.poll_interval)
        );
        // Removing this budget would spin forever on a genuinely leaderless
        // cluster instead of failing the write in bounded time.
        assert_eq!(leaderless_poll_delay(budget.max_polls, budget), None);
        assert_eq!(leaderless_poll_delay(u32::MAX, budget), None);

        let timing = RaftConfig::default().timing();
        assert!(budget.total_wait() <= timing.leaderless_wait);
        assert!(budget.total_wait() + budget.poll_interval >= timing.leaderless_wait);
        assert!(budget.total_wait() >= timing.election_timeout_max);
    }

    /// The production typed defaults (1000ms election min / 200ms heartbeat)
    /// produce a 7s leaderless wait sampled every 100ms, i.e. 70 polls.
    #[test]
    fn production_budget_covers_one_election_cycle() {
        let config = RaftConfig {
            election_timeout_ms: 1000,
            heartbeat_interval_ms: 200,
            ..Default::default()
        };
        let budget = LeaderlessBudget::from_config(&config);
        let timing = config.timing();
        assert_eq!(budget.poll_interval, Duration::from_millis(100));
        assert_eq!(budget.max_polls, 70);
        assert_eq!(budget.total_wait(), Duration::from_millis(7000));
        assert_eq!(timing.apply_wait, Duration::from_millis(7000));
        assert!(budget.total_wait() >= timing.election_timeout_max);
    }

    #[test]
    fn election_window_waits_for_a_leader_instead_of_failing() {
        let budget = default_budget();
        let addr = "127.0.0.1:7848".to_string();
        assert_eq!(
            discover_leader(0, Some(addr.clone()), budget),
            LeaderDiscovery::Ready(addr)
        );
        // Old behavior (`None => break`) is what produced the 503s in
        // cluster-mysql while an election was in flight.
        assert_eq!(
            discover_leader(0, None, budget),
            LeaderDiscovery::Wait(1, budget.poll_interval)
        );
        assert_eq!(
            discover_leader(budget.max_polls, None, budget),
            LeaderDiscovery::GiveUp
        );
    }

    /// Fault injection without a live cluster: make the leader appear exactly
    /// one maximum election window after the write starts. The old hardcoded
    /// 5s budget gave up before the library default's 6s window closed.
    #[test]
    fn leader_appearing_after_one_election_window_is_still_discovered() {
        let config = RaftConfig::default();
        let budget = LeaderlessBudget::from_config(&config);
        let timing = config.timing();
        let mut polls = 0u32;
        let mut waited = Duration::ZERO;
        loop {
            let known =
                (waited >= timing.election_timeout_max).then(|| "127.0.0.1:7848".to_string());
            match discover_leader(polls, known, budget) {
                LeaderDiscovery::Ready(_) => break,
                LeaderDiscovery::Wait(next, delay) => {
                    polls = next;
                    waited += delay;
                }
                LeaderDiscovery::GiveUp => {
                    panic!("gave up after {waited:?} although a leader appeared")
                }
            }
        }
        assert!(waited >= timing.election_timeout_max);
        assert!(waited <= timing.leaderless_wait);
    }

    /// Startup must reject a timing combination that openraft would reject
    /// anyway -- but do so before any directory or RocksDB handle is opened.
    #[tokio::test]
    async fn node_rejects_invalid_timing_before_opening_state() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = RaftConfig {
            election_timeout_ms: 100,
            heartbeat_interval_ms: 200,
            data_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        // `RaftNode` is not `Debug`, so `Result::expect_err` cannot be used here.
        let err = RaftNode::new(1, "127.0.0.1:7848".into(), config)
            .await
            .err()
            .expect("invalid config must be rejected");
        assert!(
            err.to_string().contains("election_timeout_ms"),
            "unexpected error: {err}"
        );
        assert!(!dir.path().join("log").exists());
    }

    #[test]
    fn local_client_write_deadline_is_finite_and_derived() {
        let production = RaftConfig {
            election_timeout_ms: 1000,
            heartbeat_interval_ms: 200,
            ..Default::default()
        };
        for config in [RaftConfig::default(), production] {
            let timing = config.timing();
            // The Go SDK suite allows 10 minutes; an unbounded openraft
            // client_write used to consume all of it on a single request.
            assert!(timing.client_write > Duration::ZERO);
            assert!(timing.client_write < Duration::from_secs(60));
            assert!(timing.client_write > timing.leaderless_wait);
        }
    }
}
