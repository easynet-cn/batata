/// Consul Raft network factory and connection.
///
/// Handles inter-node Raft communication for the Consul Raft group.
/// Uses HTTP-based JSON transport for simplicity. Can be upgraded to
/// gRPC for better performance in the future.
use std::sync::Arc;

use openraft::error::{InstallSnapshotError, RPCError, RaftError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::BasicNode;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::types::*;

/// Factory that creates Consul Raft network connections to other nodes.
pub struct ConsulRaftNetworkFactory {
    /// HTTP client cache: node_id -> base_url
    clients: Arc<RwLock<std::collections::HashMap<NodeId, reqwest::Client>>>,
}

impl ConsulRaftNetworkFactory {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }
}

impl Default for ConsulRaftNetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftNetworkFactory<ConsulTypeConfig> for ConsulRaftNetworkFactory {
    type Network = ConsulRaftNetworkConnection;

    async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
        ConsulRaftNetworkConnection {
            target,
            target_addr: node.addr.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .connect_timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }
}

pub struct ConsulRaftNetworkConnection {
    target: NodeId,
    target_addr: String,
    client: reqwest::Client,
}

impl ConsulRaftNetworkConnection {
    /// Derive the Consul Raft HTTP endpoint from the target node address.
    /// Uses the Consul HTTP port with a raft API path.
    fn raft_url(&self, path: &str) -> String {
        // Target addr is the main server addr (e.g., 127.0.0.1:8848)
        // Consul HTTP port = main port - 348 (8848 -> 8500) — but we need
        // a dedicated consul raft endpoint. For simplicity, use the main
        // server port with a dedicated path prefix.
        format!("http://{}/consul-raft/{}", self.target_addr, path)
    }

    async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, Unreachable> {
        let url = self.raft_url(path);
        debug!("Consul Raft RPC to {}: {}", self.target, url);

        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| {
                warn!("Consul Raft RPC failed to {}: {}", self.target, e);
                Unreachable::new(&e)
            })?;

        resp.json::<R>().await.map_err(|e| {
            warn!("Consul Raft RPC response parse failed: {}", e);
            Unreachable::new(&e)
        })
    }
}

impl RaftNetwork<ConsulTypeConfig> for ConsulRaftNetworkConnection {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<ConsulTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.post_json("append-entries", &rpc)
            .await
            .map_err(RPCError::Unreachable)
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<ConsulTypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        self.post_json("install-snapshot", &rpc)
            .await
            .map_err(RPCError::Unreachable)
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        self.post_json("vote", &rpc)
            .await
            .map_err(RPCError::Unreachable)
    }
}

/// Derive the Consul Raft port from the main server address.
/// Convention: consul_raft_port = main_port + 1002 (e.g., 8848 → 9850)
pub fn derive_consul_raft_addr(main_addr: &str) -> String {
    if let Some((host, port_str)) = main_addr.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            return format!("{}:{}", host, port + 1002);
        }
    }
    format!("{}:9850", main_addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_consul_raft_addr() {
        assert_eq!(derive_consul_raft_addr("127.0.0.1:8848"), "127.0.0.1:9850");
        assert_eq!(derive_consul_raft_addr("10.0.0.1:8858"), "10.0.0.1:9860");
    }
}
