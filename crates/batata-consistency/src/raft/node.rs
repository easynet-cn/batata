// RaftNode wrapper for managing Raft lifecycle
// Provides a high-level API for interacting with the Raft consensus

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use openraft::{BasicNode, Raft};
use rocksdb::Options;
use tracing::{debug, info};

use tokio::sync::RwLock;

use super::config::RaftConfig;
use super::log_store::RocksLogStore;
use super::network::BatataRaftNetworkFactory;
use super::plugin::PluginRegistry;
use super::request::{RaftRequest, RaftResponse};
use super::state_machine::RocksStateMachine;
use super::types::{NodeId, RaftMetrics, TypeConfig};
use crate::RaftPluginHandler;

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

    /// The underlying Raft instance
    raft: Raft<TypeConfig>,

    /// Configuration (reserved for future dynamic reconfiguration)
    #[allow(dead_code)]
    config: RaftConfig,

    /// Shared RocksDB handle (same instance used by the state machine)
    db: Arc<rocksdb::DB>,

    /// Shared plugin registry (same instance used by the state machine)
    plugin_registry: Arc<RwLock<PluginRegistry>>,

    /// Shared naming apply hook slot (same instance used by the state machine).
    /// Registered post-construction so `NamingService` DashMap stays in sync
    /// with RocksDB `CF_INSTANCES` after every persistent instance apply.
    naming_hook: super::naming_hook::SharedNamingHook,

    /// Cached gRPC channel for forwarding writes to the Raft leader.
    /// Avoids creating a new TCP/HTTP2 connection on every forwarded write.
    leader_channel: tokio::sync::Mutex<Option<LeaderChannel>>,
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
        .await?;
        let db = state_machine.db();
        let plugin_registry = state_machine.plugin_registry();
        let naming_hook = state_machine.naming_hook();

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
                plugin_registry,
                naming_hook,
                leader_channel: tokio::sync::Mutex::new(None),
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

    /// Get the underlying Raft instance
    pub fn raft(&self) -> &Raft<TypeConfig> {
        &self.raft
    }

    /// Get a reference to the shared RocksDB instance.
    pub fn db(&self) -> Arc<rocksdb::DB> {
        self.db.clone()
    }

    /// Register a plugin handler for processing PluginWrite operations.
    ///
    /// Safe to call after Raft startup — the registry is shared between
    /// this node and the state machine via `Arc`.
    pub async fn register_plugin(&self, handler: Arc<dyn RaftPluginHandler>) -> Result<(), String> {
        self.plugin_registry.write().await.register(handler)
    }

    /// Register the naming apply hook so `PersistentInstance*` applies flow
    /// back into the in-memory `NamingService` DashMap.
    ///
    /// Safe to call after Raft startup — the hook slot is shared with the
    /// state machine via `Arc<RwLock<_>>` and consulted on every apply.
    pub async fn register_naming_hook(
        &self,
        hook: Arc<dyn super::naming_hook::NamingApplyHook>,
    ) {
        *self.naming_hook.write().await = Some(hook);
    }

    /// Replay all persistent instances from RocksDB `CF_INSTANCES` into the
    /// registered naming hook. Call once at startup (after `register_naming_hook`)
    /// so `NamingService` reflects the committed state without waiting for
    /// new Raft applies.
    pub async fn replay_persistent_instances(&self) -> Result<usize, String> {
        let hook_guard = self.naming_hook.read().await;
        let Some(hook) = hook_guard.as_ref() else {
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
        self.raft.metrics().borrow().clone()
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
        debug!("Writing through Raft: {}", request.op_type());

        // Try local client_write first (succeeds if this node is the leader)
        match self.raft.client_write(request.clone()).await {
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
                let mut last_err: Box<dyn std::error::Error + Send + Sync> = Box::new(e);

                let request_bytes = serde_json::to_vec(&request)
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
                let op_type = request.op_type();

                for attempt in 0..max_retries {
                    let leader_addr = match self.leader_addr() {
                        Some(addr) => addr,
                        None => break, // No leader known, give up
                    };

                    debug!(
                        "Forwarding write to Raft leader at {} (attempt {}/{}): {}",
                        leader_addr,
                        attempt + 1,
                        max_retries,
                        op_type
                    );

                    match self
                        .forward_write_to_leader_bytes(&leader_addr, &request_bytes)
                        .await
                    {
                        Ok((resp, leader_log_index)) => {
                            // Wait for local state machine to catch up to the
                            // leader's committed index before returning, ensuring
                            // subsequent local reads see the written data.
                            if leader_log_index > 0
                                && let Err(e) = self
                                    .wait_for_applied(leader_log_index, Duration::from_secs(5))
                                    .await
                            {
                                tracing::warn!("Follower wait-for-applied timed out: {}", e);
                            }
                            let idx = self.last_applied_index().unwrap_or(leader_log_index);
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
        use batata_api::raft::ClientWriteRequest;
        use batata_api::raft::raft_management_service_client::RaftManagementServiceClient;

        let channel = {
            let mut guard = self.leader_channel.lock().await;
            match guard.as_ref() {
                Some(cached) if cached.addr == leader_addr => cached.channel.clone(),
                _ => {
                    let endpoint = format!("http://{}", leader_addr);
                    let channel = tonic::transport::Channel::from_shared(endpoint)
                        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                        .connect_timeout(Duration::from_secs(5))
                        .tcp_keepalive(Some(Duration::from_secs(10)))
                        .tcp_nodelay(true)
                        .http2_keep_alive_interval(Duration::from_secs(10))
                        .connect()
                        .await?;
                    *guard = Some(LeaderChannel {
                        addr: leader_addr.to_string(),
                        channel: channel.clone(),
                    });
                    channel
                }
            }
        };

        let mut client = RaftManagementServiceClient::new(channel);
        let response = client
            .write(ClientWriteRequest {
                data: request_bytes.to_vec(),
            })
            .await?
            .into_inner();

        if response.success {
            Ok((
                RaftResponse {
                    success: true,
                    data: if response.data.is_empty() {
                        None
                    } else {
                        Some(response.data)
                    },
                    message: None,
                },
                response.log_index,
            ))
        } else {
            Err(response.message.into())
        }
    }

    /// Perform a linearizable read
    /// This ensures the read sees all committed data up to the point of the read
    pub async fn linearizable_read(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.raft.ensure_linearizable().await?;
        Ok(())
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
    pub fn membership(&self) -> openraft::StoredMembership<NodeId, openraft::BasicNode> {
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
    pub fn new() -> Self {
        Self {
            node_id: None,
            addr: None,
            config: RaftConfig::default(),
        }
    }

    pub fn node_id(mut self, id: NodeId) -> Self {
        self.node_id = Some(id);
        self
    }

    pub fn addr(mut self, addr: impl Into<String>) -> Self {
        self.addr = Some(addr.into());
        self
    }

    pub fn config(mut self, config: RaftConfig) -> Self {
        self.config = config;
        self
    }

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

    #[test]
    fn test_builder() {
        let builder = RaftNodeBuilder::new()
            .node_id(1)
            .addr("127.0.0.1:7848")
            .config(RaftConfig::default());

        assert!(builder.node_id.is_some());
        assert!(builder.addr.is_some());
    }
}
