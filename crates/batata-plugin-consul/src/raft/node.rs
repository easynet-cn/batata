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
use tracing::{debug, info};

use super::log_store::ConsulLogStore;
use super::network::ConsulRaftNetworkFactory;
use super::request::{ConsulRaftRequest, ConsulRaftResponse};
use super::state_machine::ConsulStateMachine;
use super::types::*;
use crate::index_provider::ConsulTableIndex;

pub struct ConsulRaftNode {
    node_id: NodeId,
    #[allow(dead_code)]
    addr: String,
    raft: ConsulRaft,
    #[allow(dead_code)]
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
    ) -> Result<(Self, Arc<DB>, ConsulTableIndex), Box<dyn std::error::Error + Send + Sync>> {
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
        let table_index = state_machine.table_index();

        // Create network factory
        let network_factory = ConsulRaftNetworkFactory::new();

        // OpenRaft config
        let raft_config = Arc::new(openraft::Config {
            cluster_name: "consul-raft".to_string(),
            heartbeat_interval: 500,
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            install_snapshot_timeout: 10000,
            max_in_snapshot_log_to_keep: 500,
            ..Default::default()
        });

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
            table_index,
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
        info!(
            "Initializing Consul Raft cluster with {} members",
            members.len()
        );
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
                // Forward to leader if known
                if let Some(leader_addr) = self.leader_addr() {
                    debug!("Consul Raft: forwarding write to leader at {}", leader_addr);
                    return self.forward_to_leader(&leader_addr, request).await;
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

    /// Get the leader's network address from the Raft membership.
    pub fn leader_addr(&self) -> Option<String> {
        let metrics = self.raft.metrics().borrow().clone();
        let leader_id = metrics.current_leader?;
        if leader_id == self.node_id {
            return None; // We are the leader
        }
        metrics
            .membership_config
            .membership()
            .get_node(&leader_id)
            .map(|n| n.addr.clone())
    }

    /// Forward a write request to the Consul Raft leader via gRPC.
    async fn forward_to_leader(
        &self,
        leader_addr: &str,
        request: ConsulRaftRequest,
    ) -> Result<(ConsulRaftResponse, u64), Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = format!("http://{}", leader_addr);
        let channel = tonic::transport::Channel::from_shared(endpoint)?
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .connect()
            .await?;

        let mut client =
            batata_api::raft::consul_raft_management_service_client::ConsulRaftManagementServiceClient::new(channel);

        let data = serde_json::to_vec(&request)?;
        let resp = client
            .write(batata_api::raft::ClientWriteRequest { data })
            .await?
            .into_inner();

        if resp.success {
            let consul_resp: ConsulRaftResponse = serde_json::from_slice(&resp.data)
                .unwrap_or_else(|_| ConsulRaftResponse::success());
            let leader_log_index = resp.log_index;

            // Wait for our local state machine to apply up to the leader's log index.
            // This ensures read-after-write consistency on this follower.
            if let Some(local_idx) = self.last_applied_index()
                && local_idx < leader_log_index
            {
                // Wait up to 2 seconds for replication to catch up
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    self.wait_for_applied(leader_log_index),
                )
                .await;
            }

            let idx = self.last_applied_index().unwrap_or(leader_log_index);
            Ok((consul_resp, idx))
        } else {
            let msg = if resp.message.is_empty() {
                "Leader write failed".to_string()
            } else {
                resp.message
            };
            Err(msg.into())
        }
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.leader_id() == Some(self.node_id)
    }

    /// Get the last applied log index.
    pub fn last_applied_index(&self) -> Option<u64> {
        self.raft.metrics().borrow().last_applied.map(|l| l.index)
    }

    /// Wait until the local state machine has applied at least `target_index`.
    /// Used to ensure read-after-write consistency on followers after forwarding.
    async fn wait_for_applied(&self, target_index: u64) {
        let mut rx = self.raft.metrics();
        loop {
            let m = rx.borrow().clone();
            if let Some(applied) = m.last_applied
                && applied.index >= target_index
            {
                return;
            }
            // Wait for metrics change
            if rx.changed().await.is_err() {
                return; // Raft shut down
            }
        }
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
