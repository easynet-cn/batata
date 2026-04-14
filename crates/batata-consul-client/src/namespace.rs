//! Namespaces API (Consul Enterprise).
//!
//! Maps 1:1 to Consul Go SDK's `api/namespace.go`. All endpoints live
//! under `/v1/namespace/*` and `/v1/namespaces`.

use serde::{Deserialize, Serialize};

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{ACLLink, QueryMeta, QueryOptions, WriteMeta, WriteOptions};

/// Namespace definition, wire-compatible with Consul's `api.Namespace`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Namespace {
    /// Name must be a DNS-label unique identifier.
    #[serde(rename = "Name")]
    pub name: String,
    /// Free-form description.
    #[serde(rename = "Description", default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// ACL defaults for tokens scoped to this namespace.
    #[serde(rename = "ACLs", default, skip_serializing_if = "Option::is_none")]
    pub acls: Option<NamespaceACLConfig>,
    /// Arbitrary metadata.
    #[serde(rename = "Meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<std::collections::HashMap<String, String>>,
    /// Time the namespace was marked for deletion (soft delete).
    ///
    /// Accepted under both PascalCase (Go HTTP default) and snake_case
    /// (some older Consul builds).
    #[serde(
        rename = "DeletedAt",
        alias = "deleted_at",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Admin partition this namespace belongs to (Enterprise).
    #[serde(rename = "Partition", default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    /// Raft creation index.
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    /// Raft last-modified index.
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
}

/// Namespace-scoped ACL defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NamespaceACLConfig {
    #[serde(
        rename = "PolicyDefaults",
        alias = "policy_defaults",
        default
    )]
    pub policy_defaults: Vec<ACLLink>,
    #[serde(
        rename = "RoleDefaults",
        alias = "role_defaults",
        default
    )]
    pub role_defaults: Vec<ACLLink>,
}

impl ConsulClient {
    /// Create a new namespace. Returns the created namespace as echoed by
    /// the server (with CreateIndex/ModifyIndex populated).
    pub async fn namespace_create(
        &self,
        ns: &Namespace,
        opts: &WriteOptions,
    ) -> Result<(Namespace, WriteMeta)> {
        self.put("/v1/namespace", Some(ns), opts, &[]).await
    }

    /// Update an existing namespace in place.
    pub async fn namespace_update(
        &self,
        ns: &Namespace,
        opts: &WriteOptions,
    ) -> Result<(Namespace, WriteMeta)> {
        let path = format!("/v1/namespace/{}", ns.name);
        self.put(&path, Some(ns), opts, &[]).await
    }

    /// Read a namespace by name.
    pub async fn namespace_read(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(Option<Namespace>, QueryMeta)> {
        let path = format!("/v1/namespace/{}", name);
        match self.get::<Namespace>(&path, opts).await {
            Ok((ns, meta)) => Ok((Some(ns), meta)),
            Err(e) if e.is_not_found() => Ok((None, QueryMeta::default())),
            Err(e) => Err(e),
        }
    }

    /// Delete a namespace by name. Returns WriteMeta on success.
    pub async fn namespace_delete(
        &self,
        name: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let path = format!("/v1/namespace/{}", name);
        let (_ok, meta) = self.delete(&path, opts, &[]).await?;
        Ok(meta)
    }

    /// List all namespaces visible to the current token.
    pub async fn namespace_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<Namespace>, QueryMeta)> {
        self.get("/v1/namespaces", opts).await
    }
}
