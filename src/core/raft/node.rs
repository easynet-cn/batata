// RaftNode wrapper for managing Raft lifecycle
// Provides a high-level API for interacting with the Raft consensus

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use openraft::{BasicNode, Raft};
use tracing::{debug, info};

use super::config::RaftConfig;
use super::log_store::RocksLogStore;
use super::network::BatataRaftNetworkFactory;
use super::request::{RaftRequest, RaftResponse};
use super::state_machine::RocksStateMachine;
use super::types::{NodeId, RaftMetrics, TypeConfig};

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
}

impl RaftNode {
    /// Create a new Raft node
    pub async fn new(
        node_id: NodeId,
        addr: String,
        config: RaftConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Creating Raft node: id={}, addr={}, data_dir={:?}",
            node_id, addr, config.data_dir
        );

        // Ensure data directories exist
        config.ensure_dirs()?;

        // Create log store
        let log_store = RocksLogStore::new(config.log_dir()).await?;

        // Create state machine
        let state_machine = RocksStateMachine::new(config.state_machine_dir()).await?;

        // Create network factory
        let network_factory = BatataRaftNetworkFactory::new();

        // Create openraft config
        let raft_config = Arc::new(config.to_openraft_config());

        // Create the Raft instance
        let raft = Raft::new(
            node_id,
            raft_config,
            network_factory,
            log_store,
            state_machine,
        )
        .await?;

        info!("Raft node created successfully: id={}", node_id);

        Ok(Self {
            node_id,
            addr,
            raft,
            config,
        })
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

    /// Write data through Raft consensus
    pub async fn write(
        &self,
        request: RaftRequest,
    ) -> Result<RaftResponse, Box<dyn std::error::Error + Send + Sync>> {
        debug!("Writing through Raft: {}", request.op_type());

        let result = self.raft.client_write(request).await?;

        Ok(result.data)
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

    /// Get the commit index (using last_applied as proxy)
    pub fn commit_index(&self) -> Option<u64> {
        self.metrics().last_applied.map(|l| l.index)
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
