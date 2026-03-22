/// Consul Raft Node — independent Raft group for Consul operations.
///
/// Owns its own log store, state machine, and RocksDB instance.
/// Provides `write_with_index()` that returns the Raft log index
/// for use as CreateIndex/ModifyIndex in Consul data structures.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use openraft::BasicNode;
use rocksdb::DB;
use tracing::{debug, info, warn};

use super::log_store::ConsulLogStore;
use super::network::ConsulRaftNetworkFactory;
use super::request::{ConsulRaftRequest, ConsulRaftResponse};
use super::state_machine::ConsulStateMachine;
use super::types::*;

pub struct ConsulRaftNode {
    node_id: NodeId,
    addr: String,
    raft: ConsulRaft,
    data_dir: PathBuf,
}

impl ConsulRaftNode {
    /// Create a new Consul Raft node.
    ///
    /// `data_dir` is the Consul data directory (e.g., `data/consul_rocksdb`).
    /// Raft logs stored at `{data_dir}/raft/logs/`, state at `{data_dir}/raft/state/`.
    pub async fn new(
        node_id: NodeId,
        addr: String,
        data_dir: impl AsRef<Path>,
    ) -> Result<(Self, Arc<DB>), Box<dyn std::error::Error + Send + Sync>> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let raft_dir = data_dir.join("raft");

        info!(
            "Creating Consul Raft node: id={}, addr={}, dir={:?}",
            node_id, addr, raft_dir
        );

        // Ensure directories exist
        tokio::fs::create_dir_all(raft_dir.join("logs")).await?;
        tokio::fs::create_dir_all(raft_dir.join("state")).await?;

        // Create log store
        let log_store = ConsulLogStore::open(raft_dir.join("logs")).await?;

        // Create state machine
        let state_machine = ConsulStateMachine::open(raft_dir.join("state")).await?;
        let db = state_machine.db();

        // Create network factory
        let network_factory = ConsulRaftNetworkFactory::new();

        // OpenRaft config
        let raft_config = Arc::new(
            openraft::Config {
                cluster_name: "consul-raft".to_string(),
                heartbeat_interval: 500,
                election_timeout_min: 1500,
                election_timeout_max: 3000,
                install_snapshot_timeout: 10000,
                max_in_snapshot_log_to_keep: 500,
                ..Default::default()
            },
        );

        // Create the Raft instance
        let raft = ConsulRaft::new(
            node_id,
            raft_config,
            network_factory,
            log_store,
            state_machine,
        )
        .await?;

        info!("Consul Raft node created: id={}", node_id);

        Ok((
            Self {
                node_id,
                addr,
                raft,
                data_dir,
            },
            db,
        ))
    }

    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Initialize the Consul Raft cluster with the given members.
    pub async fn initialize(
        &self,
        members: BTreeMap<NodeId, BasicNode>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Initializing Consul Raft cluster with {} members", members.len());
        self.raft
            .initialize(members)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Write a Consul request through Raft and return both the response
    /// and the Raft log index.
    ///
    /// The log index is globally consistent across all cluster nodes and
    /// should be used as CreateIndex/ModifyIndex/X-Consul-Index.
    pub async fn write_with_index(
        &self,
        request: ConsulRaftRequest,
    ) -> Result<(ConsulRaftResponse, u64), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Consul Raft write: {}", request.op_type());

        match self.raft.client_write(request.clone()).await {
            Ok(result) => {
                let log_index = result.log_id.index;
                Ok((result.data, log_index))
            }
            Err(e) => {
                // Check if we need to forward to the leader
                if let Some(leader_id) = self.leader_id() {
                    if leader_id != self.node_id {
                        warn!(
                            "Consul Raft: not leader, forwarding to node {}",
                            leader_id
                        );
                        // TODO: Implement leader forwarding via gRPC
                        // For now, return error
                        return Err(format!(
                            "Not leader. Leader is node {}. Forwarding not yet implemented.",
                            leader_id
                        )
                        .into());
                    }
                }
                Err(Box::new(e))
            }
        }
    }

    /// Write without returning the log index (convenience method).
    pub async fn write(
        &self,
        request: ConsulRaftRequest,
    ) -> Result<ConsulRaftResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (resp, _) = self.write_with_index(request).await?;
        Ok(resp)
    }

    /// Get the current leader's node ID.
    pub fn leader_id(&self) -> Option<NodeId> {
        let metrics = self.raft.metrics().borrow().clone();
        metrics.current_leader
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.leader_id() == Some(self.node_id)
    }

    /// Get the last applied log index.
    pub fn last_applied_index(&self) -> Option<u64> {
        self.raft.metrics().borrow().last_applied.map(|l| l.index)
    }

    /// Get Raft metrics for monitoring.
    pub fn metrics(&self) -> ConsulRaftMetrics {
        self.raft.metrics().borrow().clone()
    }

    /// Add a learner to the Consul Raft cluster.
    pub async fn add_learner(
        &self,
        node_id: NodeId,
        node: BasicNode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.raft
            .add_learner(node_id, node, true)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }

    /// Change the cluster membership.
    pub async fn change_membership(
        &self,
        member_ids: std::collections::BTreeSet<NodeId>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.raft
            .change_membership(member_ids, false)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }

    /// Get the underlying Raft instance (for gRPC service handler).
    pub fn raft(&self) -> &ConsulRaft {
        &self.raft
    }
}
