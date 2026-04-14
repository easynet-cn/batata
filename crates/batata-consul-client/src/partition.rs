//! Admin Partitions API (Consul Enterprise).
//!
//! Maps 1:1 to Consul Go SDK's `api/partition.go`. Endpoints live under
//! `/v1/partition` and `/v1/partitions`.

use serde::{Deserialize, Serialize};

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions, WriteMeta, WriteOptions};

/// Admin partition definition, wire-compatible with Consul's `api.Partition`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Partition {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description", default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(
        rename = "DeletedAt",
        alias = "deleted_at",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
}

impl ConsulClient {
    /// Create a new admin partition.
    pub async fn partition_create(
        &self,
        p: &Partition,
        opts: &WriteOptions,
    ) -> Result<(Partition, WriteMeta)> {
        self.put("/v1/partition", Some(p), opts, &[]).await
    }

    /// Update an existing admin partition in place.
    pub async fn partition_update(
        &self,
        p: &Partition,
        opts: &WriteOptions,
    ) -> Result<(Partition, WriteMeta)> {
        let path = format!("/v1/partition/{}", p.name);
        self.put(&path, Some(p), opts, &[]).await
    }

    /// Read a partition by name.
    pub async fn partition_read(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(Option<Partition>, QueryMeta)> {
        let path = format!("/v1/partition/{}", name);
        match self.get::<Partition>(&path, opts).await {
            Ok((p, meta)) => Ok((Some(p), meta)),
            Err(e) if e.is_not_found() => Ok((None, QueryMeta::default())),
            Err(e) => Err(e),
        }
    }

    /// Delete a partition by name.
    pub async fn partition_delete(&self, name: &str, opts: &WriteOptions) -> Result<WriteMeta> {
        let path = format!("/v1/partition/{}", name);
        let (_ok, meta) = self.delete(&path, opts, &[]).await?;
        Ok(meta)
    }

    /// List all admin partitions visible to the token.
    pub async fn partition_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<Partition>, QueryMeta)> {
        self.get("/v1/partitions", opts).await
    }
}
