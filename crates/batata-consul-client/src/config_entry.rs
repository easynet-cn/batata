//! Config Entry API for service mesh configuration

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::*;

impl ConsulClient {
    /// List config entries by kind
    pub async fn config_entries_list(
        &self,
        kind: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<serde_json::Value>, QueryMeta)> {
        self.get(&format!("/v1/config/{}", kind), opts).await
    }

    /// Get a config entry by kind and name
    pub async fn config_entry_get(
        &self,
        kind: &str,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(serde_json::Value, QueryMeta)> {
        self.get(&format!("/v1/config/{}/{}", kind, name), opts)
            .await
    }

    /// Set (create or update) a config entry
    pub async fn config_entry_set(
        &self,
        entry: &serde_json::Value,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        self.put("/v1/config", Some(entry), opts, &[]).await
    }

    /// Set a config entry with CAS (Compare-And-Swap)
    pub async fn config_entry_cas(
        &self,
        entry: &serde_json::Value,
        cas_index: u64,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let extra = vec![("cas".to_string(), cas_index.to_string())];
        self.put("/v1/config", Some(entry), opts, &extra).await
    }

    /// Delete a config entry
    pub async fn config_entry_delete(
        &self,
        kind: &str,
        name: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        self.delete(&format!("/v1/config/{}/{}", kind, name), opts, &[])
            .await
    }
}
