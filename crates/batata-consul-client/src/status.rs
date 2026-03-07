use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions};

/// Status API operations
impl ConsulClient {
    /// Get the Raft leader address
    pub async fn status_leader(&self, opts: &QueryOptions) -> Result<(String, QueryMeta)> {
        self.get("/v1/status/leader", opts).await
    }

    /// Get the Raft peers
    pub async fn status_peers(&self, opts: &QueryOptions) -> Result<(Vec<String>, QueryMeta)> {
        self.get("/v1/status/peers", opts).await
    }
}
