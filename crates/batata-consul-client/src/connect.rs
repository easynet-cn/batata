//! Connect API for service mesh (CA, Intentions, Discovery Chain)

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::*;

impl ConsulClient {
    /// Get Connect CA roots
    pub async fn connect_ca_roots(&self, opts: &QueryOptions) -> Result<(CARoots, QueryMeta)> {
        self.get("/v1/connect/ca/roots", opts).await
    }

    /// Get Connect CA configuration
    pub async fn connect_ca_get_config(
        &self,
        opts: &QueryOptions,
    ) -> Result<(CAConfig, QueryMeta)> {
        self.get("/v1/connect/ca/configuration", opts).await
    }

    /// Set Connect CA configuration
    pub async fn connect_ca_set_config(
        &self,
        config: &CAConfig,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body("/v1/connect/ca/configuration", config, opts)
            .await
    }

    /// List all intentions
    pub async fn connect_intentions(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<Intention>, QueryMeta)> {
        self.get("/v1/connect/intentions", opts).await
    }

    /// Get intention by ID
    pub async fn connect_intention_get(
        &self,
        id: &str,
        opts: &QueryOptions,
    ) -> Result<(Intention, QueryMeta)> {
        self.get(&format!("/v1/connect/intentions/{}", id), opts)
            .await
    }

    /// Create intention
    pub async fn connect_intention_create(
        &self,
        intention: &Intention,
        opts: &WriteOptions,
    ) -> Result<(String, WriteMeta)> {
        self.post("/v1/connect/intentions", Some(intention), opts, &[])
            .await
    }

    /// Update intention by ID
    pub async fn connect_intention_update(
        &self,
        id: &str,
        intention: &Intention,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        self.put_no_response_with_body(&format!("/v1/connect/intentions/{}", id), intention, opts)
            .await
    }

    /// Delete intention by ID
    pub async fn connect_intention_delete(
        &self,
        id: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        self.delete(&format!("/v1/connect/intentions/{}", id), opts, &[])
            .await
    }

    /// Get intention by exact source/destination match
    pub async fn connect_intention_exact(
        &self,
        source: &str,
        destination: &str,
        opts: &QueryOptions,
    ) -> Result<(Intention, QueryMeta)> {
        let extra = vec![
            ("source".to_string(), source.to_string()),
            ("destination".to_string(), destination.to_string()),
        ];
        self.get_with_extra("/v1/connect/intentions/exact", opts, &extra)
            .await
    }

    /// Upsert intention by exact source/destination
    pub async fn connect_intention_upsert(
        &self,
        intention: &Intention,
        source: &str,
        destination: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let extra = vec![
            ("source".to_string(), source.to_string()),
            ("destination".to_string(), destination.to_string()),
        ];
        self.put::<bool, _>(
            "/v1/connect/intentions/exact",
            Some(intention),
            opts,
            &extra,
        )
        .await
        .map(|(_, meta)| meta)
    }

    /// Delete intention by exact source/destination
    pub async fn connect_intention_delete_exact(
        &self,
        source: &str,
        destination: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let extra = vec![
            ("source".to_string(), source.to_string()),
            ("destination".to_string(), destination.to_string()),
        ];
        self.delete("/v1/connect/intentions/exact", opts, &extra)
            .await
    }

    /// Match intentions for a service
    pub async fn connect_intention_match(
        &self,
        by: &str,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        let extra = vec![
            ("by".to_string(), by.to_string()),
            ("name".to_string(), name.to_string()),
        ];
        self.get_with_extra("/v1/connect/intentions/match", opts, &extra)
            .await
    }

    /// Check if a connection between two services is authorized
    pub async fn connect_intention_check(
        &self,
        source: &str,
        destination: &str,
        opts: &QueryOptions,
    ) -> Result<(IntentionCheck, QueryMeta)> {
        let extra = vec![
            ("source".to_string(), source.to_string()),
            ("destination".to_string(), destination.to_string()),
        ];
        self.get_with_extra("/v1/connect/intentions/check", opts, &extra)
            .await
    }

    /// Get discovery chain for a service
    pub async fn discovery_chain(
        &self,
        service: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get(&format!("/v1/discovery-chain/{}", service), opts)
            .await
    }
}
