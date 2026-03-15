//! Operator API for cluster management (Raft, Autopilot, Keyring)

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::*;

impl ConsulClient {
    /// Get the current Raft configuration
    pub async fn operator_raft_config(
        &self,
        opts: &QueryOptions,
    ) -> Result<(RaftConfiguration, QueryMeta)> {
        self.get("/v1/operator/raft/configuration", opts).await
    }

    /// Remove a Raft peer by address
    pub async fn operator_raft_remove_peer_by_address(
        &self,
        address: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![("address".to_string(), address.to_string())];
        self.delete("/v1/operator/raft/peer", opts, &extra)
            .await
            .map(|(_, meta)| meta)
    }

    /// Remove a Raft peer by ID
    pub async fn operator_raft_remove_peer_by_id(
        &self,
        id: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![("id".to_string(), id.to_string())];
        self.delete("/v1/operator/raft/peer", opts, &extra)
            .await
            .map(|(_, meta)| meta)
    }

    /// Transfer Raft leadership
    pub async fn operator_raft_transfer_leader(&self, opts: &WriteOptions) -> Result<WriteMeta> {
        self.post::<serde_json::Value, serde_json::Value>(
            "/v1/operator/raft/transfer-leader",
            None,
            opts,
            &[],
        )
        .await
        .map(|(_, meta)| meta)
    }

    /// Get Autopilot configuration
    pub async fn operator_autopilot_get_configuration(
        &self,
        opts: &QueryOptions,
    ) -> Result<(AutopilotConfiguration, QueryMeta)> {
        self.get("/v1/operator/autopilot/configuration", opts).await
    }

    /// Set Autopilot configuration
    pub async fn operator_autopilot_set_configuration(
        &self,
        config: &AutopilotConfiguration,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body("/v1/operator/autopilot/configuration", config, opts)
            .await
    }

    /// Get Autopilot health
    pub async fn operator_autopilot_health(
        &self,
        opts: &QueryOptions,
    ) -> Result<(AutopilotHealth, QueryMeta)> {
        self.get("/v1/operator/autopilot/health", opts).await
    }

    /// Get Autopilot state
    pub async fn operator_autopilot_state(
        &self,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/operator/autopilot/state", opts).await
    }

    /// List keyring
    pub async fn operator_keyring_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<KeyringResponse>, QueryMeta)> {
        self.get("/v1/operator/keyring", opts).await
    }

    /// Install keyring key
    pub async fn operator_keyring_install(
        &self,
        key: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let body = serde_json::json!({"Key": key});
        self.post::<serde_json::Value, _>("/v1/operator/keyring", Some(&body), opts, &[])
            .await
            .map(|(_, meta)| meta)
    }

    /// Use keyring key
    pub async fn operator_keyring_use(&self, key: &str, opts: &WriteOptions) -> Result<WriteMeta> {
        let body = serde_json::json!({"Key": key});
        self.put_no_response_with_body("/v1/operator/keyring", &body, opts)
            .await
    }

    /// Remove keyring key
    pub async fn operator_keyring_remove(
        &self,
        key: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![("key".to_string(), key.to_string())];
        self.delete("/v1/operator/keyring", opts, &extra)
            .await
            .map(|(_, meta)| meta)
    }

    /// Get operator usage
    pub async fn operator_usage(
        &self,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/operator/usage", opts).await
    }
}
