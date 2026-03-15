//! Prepared Query API

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    PreparedQuery, PreparedQueryExecuteResponse, QueryMeta, QueryOptions, WriteMeta, WriteOptions,
};

/// Prepared Query operations
impl ConsulClient {
    /// List all prepared queries
    pub async fn query_list(&self, opts: &QueryOptions) -> Result<(Vec<PreparedQuery>, QueryMeta)> {
        self.get("/v1/query", opts).await
    }

    /// Create a prepared query, returns the query ID
    pub async fn query_create(
        &self,
        query: &PreparedQuery,
        opts: &WriteOptions,
    ) -> Result<(String, WriteMeta)> {
        let (resp, meta): (serde_json::Value, WriteMeta) =
            self.post("/v1/query", Some(query), opts, &[]).await?;

        let id = resp
            .get("ID")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok((id, meta))
    }

    /// Get a prepared query by ID
    pub async fn query_get(
        &self,
        id: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<PreparedQuery>, QueryMeta)> {
        self.get(&format!("/v1/query/{}", id), opts).await
    }

    /// Update a prepared query
    pub async fn query_update(
        &self,
        id: &str,
        query: &PreparedQuery,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let (_resp, meta): (serde_json::Value, WriteMeta) = self
            .put(&format!("/v1/query/{}", id), Some(query), opts, &[])
            .await?;
        Ok(meta)
    }

    /// Delete a prepared query
    pub async fn query_delete(&self, id: &str, opts: &WriteOptions) -> Result<(bool, WriteMeta)> {
        self.delete(&format!("/v1/query/{}", id), opts, &[]).await
    }

    /// Execute a prepared query
    pub async fn query_execute(
        &self,
        id: &str,
        opts: &QueryOptions,
    ) -> Result<(PreparedQueryExecuteResponse, QueryMeta)> {
        self.get(&format!("/v1/query/{}/execute", id), opts).await
    }
}
