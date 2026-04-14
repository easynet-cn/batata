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
        let extra = vec![("address", address.to_string())];
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
        let extra = vec![("id", id.to_string())];
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
        let extra = vec![("key", key.to_string())];
        self.delete("/v1/operator/keyring", opts, &extra)
            .await
            .map(|(_, meta)| meta)
    }

    /// Set Autopilot configuration with CAS (Compare-And-Swap).
    ///
    /// Uses the `ModifyIndex` from the configuration to perform a CAS operation.
    /// Returns `true` if the update was applied, `false` if the CAS check failed.
    pub async fn operator_autopilot_cas_configuration(
        &self,
        config: &AutopilotConfiguration,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let extra = vec![("cas", config.modify_index.to_string())];
        self.put(
            "/v1/operator/autopilot/configuration",
            Some(config),
            opts,
            &extra,
        )
        .await
    }

    /// Get operator usage (`/v1/operator/usage`).
    pub async fn operator_usage(
        &self,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get("/v1/operator/usage", opts).await
    }

    // -----------------------------------------------------------------
    // operator_utilization.go — report utilization (Enterprise)
    // -----------------------------------------------------------------

    /// Get operator utilization report (Enterprise).
    pub async fn operator_utilization(
        &self,
        send_report: bool,
        todays_metrics_only: bool,
        opts: &WriteOptions,
    ) -> Result<(serde_json::Value, WriteMeta)> {
        let body = serde_json::json!({
            "SendReport": send_report,
            "TodayOnly": todays_metrics_only,
        });
        self.put("/v1/operator/utilization", Some(&body), opts, &[])
            .await
    }

    // -----------------------------------------------------------------
    // operator_segment.go — network segments (Enterprise)
    // -----------------------------------------------------------------

    /// List network segments. Returns an empty list on OSS clusters.
    pub async fn operator_segment_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<String>, QueryMeta)> {
        self.get("/v1/operator/segment", opts).await
    }

    // -----------------------------------------------------------------
    // operator_audit.go — audit hash
    // -----------------------------------------------------------------

    /// Hash a value using the agent's audit key (Enterprise).
    pub async fn operator_audit_hash(
        &self,
        input: &str,
        opts: &QueryOptions,
    ) -> Result<(String, QueryMeta)> {
        let body = serde_json::json!({ "Input": input });
        // POST body via put with PUT-like semantics since GET-with-body is odd
        let (resp, meta): (serde_json::Value, _) = self
            .put(
                "/v1/operator/audit-hash",
                Some(&body),
                &WriteOptions {
                    datacenter: opts.datacenter.clone(),
                    token: opts.token.clone(),
                    ..Default::default()
                },
                &[],
            )
            .await?;
        let hash = resp
            .get("Hash")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        Ok((hash, QueryMeta::default()))
            .map(|(h, _): (String, QueryMeta)| (h, meta_to_query_meta(meta)))
    }

    // -----------------------------------------------------------------
    // operator_area.go — network areas (Enterprise)
    // -----------------------------------------------------------------

    /// Create a new network area. Returns the area ID.
    pub async fn operator_area_create(
        &self,
        area: &serde_json::Value,
        opts: &WriteOptions,
    ) -> Result<(String, WriteMeta)> {
        let (resp, meta): (serde_json::Value, WriteMeta) =
            self.put("/v1/operator/area", Some(area), opts, &[]).await?;
        let id = resp
            .get("ID")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        Ok((id, meta))
    }

    /// Update an existing network area.
    pub async fn operator_area_update(
        &self,
        area_id: &str,
        area: &serde_json::Value,
        opts: &WriteOptions,
    ) -> Result<(String, WriteMeta)> {
        let path = format!("/v1/operator/area/{}", area_id);
        let (resp, meta): (serde_json::Value, WriteMeta) =
            self.put(&path, Some(area), opts, &[]).await?;
        let id = resp
            .get("ID")
            .and_then(|s| s.as_str())
            .unwrap_or(area_id)
            .to_string();
        Ok((id, meta))
    }

    /// Get a specific network area by ID.
    pub async fn operator_area_get(
        &self,
        area_id: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<serde_json::Value>, QueryMeta)> {
        let path = format!("/v1/operator/area/{}", area_id);
        self.get(&path, opts).await
    }

    /// List all network areas.
    pub async fn operator_area_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<serde_json::Value>, QueryMeta)> {
        self.get("/v1/operator/area", opts).await
    }

    /// Delete a network area.
    pub async fn operator_area_delete(
        &self,
        area_id: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/operator/area/{}", area_id);
        self.delete(&path, opts, &[]).await.map(|(_, m)| m)
    }

    /// Join an existing area by contacting one of its members.
    pub async fn operator_area_join(
        &self,
        area_id: &str,
        addresses: &[String],
        opts: &WriteOptions,
    ) -> Result<(Vec<serde_json::Value>, WriteMeta)> {
        let path = format!("/v1/operator/area/{}/join", area_id);
        // Wrap in Vec to satisfy Sized for `put`'s Body bound.
        let body: Vec<String> = addresses.to_vec();
        self.put(&path, Some(&body), opts, &[]).await
    }

    /// List members of an area.
    pub async fn operator_area_members(
        &self,
        area_id: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<serde_json::Value>, QueryMeta)> {
        let path = format!("/v1/operator/area/{}/members", area_id);
        self.get(&path, opts).await
    }
}

fn meta_to_query_meta(_w: WriteMeta) -> QueryMeta {
    QueryMeta::default()
}
