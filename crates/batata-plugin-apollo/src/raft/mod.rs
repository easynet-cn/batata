//! Apollo plugin handler for the unified Raft state machine.
//!
//! Implements [`RaftPluginHandler`] so Apollo write operations (apps,
//! namespaces, clusters, items, releases, commits, access keys, ...) can be
//! replicated through the core Raft group instead of being applied directly
//! and independently on every node.
//!
//! # Determinism
//!
//! The upstream Apollo project relies 100% on database auto-increment IDs and
//! is otherwise stateless, so it has no built-in replication concept. To make
//! the IDs consistent across replicas we follow the same approach as the
//! Consul plugin:
//!
//! * **Non-deterministic** fields (e.g. `release_key`'s random suffix) are
//!   generated *before* the proposal and shipped inside the payload, so every
//!   replica sees the same value.
//! * **Deterministic** fields — chiefly the auto-increment primary key `id` —
//!   are **derived from the Raft `log_index`** inside [`apply`]. Because every
//!   replica applies the same log entry with the same `log_index`, they all
//!   assign the identical `id`. This keeps the public contract numeric
//!   (`i64`), preserving Apollo's URL/SDK compatibility, while removing the
//!   cross-node ID conflict that the previous local `AtomicI64` approach had
//!   under cluster mode.

use std::sync::Arc;

use batata_consistency::RaftNode;
use batata_consistency::raft::plugin::RaftPluginHandler;
use batata_consistency::raft::request::{RaftRequest, RaftResponse};
use batata_consistency::raft::state_machine::*;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::bincode::{deserialize, serialize};
use crate::persistence::embedded::AccessKeyEmbedded;
use crate::persistence::embedded::AppEmbedded;
use crate::persistence::embedded::ClusterEmbedded;
use crate::persistence::embedded::CommitEmbedded;
use crate::persistence::embedded::GrayReleaseEmbedded;
use crate::persistence::embedded::ItemEmbedded;
use crate::persistence::embedded::NamespaceEmbedded;
use crate::persistence::embedded::ReleaseEmbedded;
use crate::persistence::embedded::ReleaseMessageEmbedded;
use crate::persistence::shared::StoredAccessKey;
use crate::persistence::shared::StoredApp;
use crate::persistence::shared::StoredCluster;
use crate::persistence::shared::StoredCommit;
use crate::persistence::shared::StoredGrayReleaseRule;
use crate::persistence::shared::StoredItem;
use crate::persistence::shared::StoredNamespace;
use crate::persistence::shared::StoredRelease;
use crate::persistence::shared::StoredReleaseMessage;

/// Plugin identifier used in `PluginWrite { plugin_id, .. }`.
pub const APOLLO_PLUGIN_ID: &str = "apollo";

/// Column families owned by the Apollo plugin.
///
/// Must stay in sync with `ApolloPlugin::required_column_families` so that the
/// RocksDB instance opened by the server already contains every CF this
/// handler touches.
const APOLLO_COLUMN_FAMILIES: &[&str] = &[
    CF_APOLLO_APP,
    CF_APOLLO_CLUSTER,
    CF_APOLLO_NAMESPACE,
    CF_APOLLO_ITEM,
    CF_APOLLO_RELEASE,
    CF_APOLLO_COMMIT,
    CF_APOLLO_GRAY_RULE,
    CF_APOLLO_INSTANCE,
    CF_APOLLO_ACCESS_KEY,
    CF_APOLLO_RELEASE_MSG,
    CF_APOLLO_NAMESPACE_LOCK,
    CF_APOLLO_RELEASE_HISTORY,
    CF_APOLLO_APP_NAMESPACE,
    CF_APOLLO_AUDIT,
    CF_APOLLO_CONSUMER,
    CF_APOLLO_CONSUMER_TOKEN,
    CF_APOLLO_CONSUMER_AUDIT,
    CF_APOLLO_CONSUMER_ROLE,
    CF_APOLLO_PERMISSION,
    CF_APOLLO_ROLE,
    CF_APOLLO_ROLE_PERMISSION,
    CF_APOLLO_USER_ROLE,
    CF_APOLLO_USERS,
    CF_APOLLO_FAVORITE,
    CF_APOLLO_SERVER_CONFIG,
    CF_APOLLO_INSTANCE_CONFIG,
];

/// A plugin-specific write operation replicated through Raft.
///
/// The payload is the already-constructed `Stored*` object (with a placeholder
/// `id` of 0). On `apply`, the handler overwrites `id` with the `log_index`
/// before writing, guaranteeing a consistent primary key across replicas.
///
/// Only the entities whose write path has been migrated to the Raft-backed
/// `write_raw` helper are covered here. Entities not yet in this enum keep
/// using the local direct-write path and are simply not replicated in cluster
/// mode (they remain correct in standalone mode).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApolloRaftRequest {
    // ---- App (no auto-increment id; keyed by app_id) ----
    /// Create an application.
    AppCreate(StoredApp),
    /// Update an application.
    AppUpdate(StoredApp),
    /// Delete an application identified by `app_id`.
    AppDelete {
        /// Application id to delete.
        app_id: String,
    },

    // ---- Namespace ----
    /// Create a namespace.
    NamespaceCreate(StoredNamespace),
    /// Update a namespace.
    NamespaceUpdate(StoredNamespace),
    /// Delete a namespace by `id`.
    NamespaceDelete {
        /// Namespace id to delete.
        id: i64,
    },

    // ---- Cluster ----
    /// Create a cluster.
    ClusterCreate(StoredCluster),
    /// Update a cluster.
    ClusterUpdate(StoredCluster),
    /// Delete a cluster by `id`.
    ClusterDelete {
        /// Cluster id to delete.
        id: i64,
    },

    // ---- Item ----
    /// Create a configuration item.
    ItemCreate(StoredItem),
    /// Update a configuration item.
    ItemUpdate(StoredItem),
    /// Delete a configuration item by `id`.
    ItemDelete {
        /// Item id to delete.
        id: i64,
    },

    // ---- Release ----
    /// Create a release.
    ReleaseCreate(StoredRelease),
    /// Update a release.
    ReleaseUpdate(StoredRelease),
    /// Delete a release by `id`.
    ReleaseDelete {
        /// Release id to delete.
        id: i64,
    },

    // ---- Commit (no delete in the CommitPersistence trait) ----
    /// Create a commit.
    CommitCreate(StoredCommit),

    // ---- AccessKey ----
    /// Create an access key.
    AccessKeyCreate(StoredAccessKey),
    /// Update an access key.
    AccessKeyUpdate(StoredAccessKey),
    /// Delete an access key by `id`.
    AccessKeyDelete {
        /// Access key id to delete.
        id: i64,
    },

    // ---- ReleaseMessage (id is the client-visible notificationId) ----
    /// Create a release message.
    ReleaseMessageCreate(StoredReleaseMessage),

    // ---- GrayReleaseRule ----
    /// Create a gray release rule.
    GrayReleaseCreate(StoredGrayReleaseRule),
    /// Update a gray release rule.
    GrayReleaseUpdate(StoredGrayReleaseRule),
    /// Delete a gray release rule by `id`.
    GrayReleaseDelete {
        /// Gray release rule id to delete.
        id: i64,
    },
}

/// Writer facade used by the Apollo persistence layer to submit writes through
/// the unified Raft group. Mirrors `ConsulRaftWriter`.
#[derive(Clone)]
pub struct ApolloRaftWriter {
    core_raft: Arc<RaftNode>,
}

impl ApolloRaftWriter {
    /// Build a writer around a running Raft node.
    pub fn new(core_raft: Arc<RaftNode>) -> Self {
        Self { core_raft }
    }

    /// Build a shared writer.
    pub fn new_arc(core_raft: Arc<RaftNode>) -> Arc<Self> {
        Arc::new(Self::new(core_raft))
    }

    /// Submit an `ApolloRaftRequest` to the Raft group and wait until it has
    /// been committed and applied to the local state machine.
    pub async fn write(&self, request: ApolloRaftRequest) -> Result<(), String> {
        let payload = serialize(&request)
            .map_err(|e| format!("ApolloRaftRequest serialize error: {}", e))?;
        let raft_req = RaftRequest::PluginWrite {
            plugin_id: APOLLO_PLUGIN_ID.to_string(),
            op_type: request.op_type().to_string(),
            payload,
        };
        match self.core_raft.write_with_index(raft_req).await {
            Ok((resp, _log_index)) => {
                if resp.success {
                    Ok(())
                } else {
                    Err(resp.message.unwrap_or_else(|| "raft apply failed".to_string()))
                }
            }
            Err(e) => Err(format!("raft write error: {}", e)),
        }
    }
}

impl ApolloRaftRequest {
    /// Stable operation-type string carried in `PluginWrite.op_type`.
    fn op_type(&self) -> &'static str {
        match self {
            ApolloRaftRequest::AppCreate(_) => "apollo_app_create",
            ApolloRaftRequest::AppUpdate(_) => "apollo_app_update",
            ApolloRaftRequest::AppDelete { .. } => "apollo_app_delete",
            ApolloRaftRequest::NamespaceCreate(_) => "apollo_namespace_create",
            ApolloRaftRequest::NamespaceUpdate(_) => "apollo_namespace_update",
            ApolloRaftRequest::NamespaceDelete { .. } => "apollo_namespace_delete",
            ApolloRaftRequest::ClusterCreate(_) => "apollo_cluster_create",
            ApolloRaftRequest::ClusterUpdate(_) => "apollo_cluster_update",
            ApolloRaftRequest::ClusterDelete { .. } => "apollo_cluster_delete",
            ApolloRaftRequest::ItemCreate(_) => "apollo_item_create",
            ApolloRaftRequest::ItemUpdate(_) => "apollo_item_update",
            ApolloRaftRequest::ItemDelete { .. } => "apollo_item_delete",
            ApolloRaftRequest::ReleaseCreate(_) => "apollo_release_create",
            ApolloRaftRequest::ReleaseUpdate(_) => "apollo_release_update",
            ApolloRaftRequest::ReleaseDelete { .. } => "apollo_release_delete",
            ApolloRaftRequest::CommitCreate(_) => "apollo_commit_create",
            ApolloRaftRequest::AccessKeyCreate(_) => "apollo_access_key_create",
            ApolloRaftRequest::AccessKeyUpdate(_) => "apollo_access_key_update",
            ApolloRaftRequest::AccessKeyDelete { .. } => "apollo_access_key_delete",
            ApolloRaftRequest::ReleaseMessageCreate(_) => "apollo_release_message_create",
            ApolloRaftRequest::GrayReleaseCreate(_) => "apollo_gray_release_create",
            ApolloRaftRequest::GrayReleaseUpdate(_) => "apollo_gray_release_update",
            ApolloRaftRequest::GrayReleaseDelete { .. } => "apollo_gray_release_delete",
        }
    }
}

/// Raft plugin handler for Apollo.
pub struct ApolloRaftPluginHandler;

impl ApolloRaftPluginHandler {
    /// Build a new handler.
    pub fn new() -> Self {
        Self
    }

    /// Build a shared handler.
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }
}

impl Default for ApolloRaftPluginHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl RaftPluginHandler for ApolloRaftPluginHandler {
    fn plugin_id(&self) -> &str {
        APOLLO_PLUGIN_ID
    }

    fn column_families(&self) -> Vec<String> {
        APOLLO_COLUMN_FAMILIES.iter().map(|s| s.to_string()).collect()
    }

    fn apply(&self, db: &DB, op_type: &str, payload: &[u8], log_index: u64) -> RaftResponse {
        // The deterministic primary key for newly-created entities is derived
        // from the Raft log index so that every replica assigns the same id.
        let id = log_index as i64;
        debug!(op_type = %op_type, log_index, "Apollo Raft apply");

        let request: ApolloRaftRequest = match deserialize(payload) {
            Ok(r) => r,
            Err(e) => {
                return RaftResponse::failure(format!("apollo deserialize error: {}", e));
            }
        };

        let result = apply_apollo_request(db, request, id);
        match result {
            Ok(()) => RaftResponse::success(),
            Err(e) => {
                error!(op_type = %op_type, error = %e, "Apollo Raft apply failed");
                RaftResponse::failure(format!("apollo apply failed: {}", e))
            }
        }
    }

    fn build_snapshot(&self, db: &DB) -> Result<Vec<u8>, String> {
        let mut snapshot: Vec<(String, Vec<u8>, Vec<u8>)> = Vec::new();
        for cf_name in APOLLO_COLUMN_FAMILIES {
            let cf = db
                .cf_handle(cf_name)
                .ok_or_else(|| format!("CF {} not found during snapshot", cf_name))?;
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            for item in iter.flatten() {
                let (key, value) = item;
                snapshot.push((cf_name.to_string(), key.to_vec(), value.to_vec()));
            }
        }
        serialize(&snapshot).map_err(|e| format!("snapshot serialize error: {}", e))
    }

    fn install_snapshot(&self, db: &DB, data: &[u8]) -> Result<(), String> {
        let snapshot: Vec<(String, Vec<u8>, Vec<u8>)> =
            deserialize(data).map_err(|e| format!("snapshot deserialize error: {}", e))?;
        // Group by CF and rewrite atomically per CF.
        let mut by_cf: std::collections::HashMap<String, Vec<(Vec<u8>, Vec<u8>)>> =
            std::collections::HashMap::new();
        for (cf, k, v) in snapshot {
            by_cf.entry(cf).or_default().push((k, v));
        }
        for (cf_name, pairs) in by_cf {
            let cf = db
                .cf_handle(&cf_name)
                .ok_or_else(|| format!("CF {} not found during install_snapshot", cf_name))?;
            // Clear existing CF contents.
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            let keys: Vec<Vec<u8>> = iter
                .flatten()
                .map(|(k, _)| k.to_vec())
                .collect::<Vec<_>>();
            for k in keys {
                db.delete_cf(cf, k)
                    .map_err(|e| format!("delete {} error: {}", cf_name, e))?;
            }
            // Write snapshot contents.
            for (k, v) in pairs {
                db.put_cf(cf, k, v)
                    .map_err(|e| format!("put {} error: {}", cf_name, e))?;
            }
        }
        Ok(())
    }
}

/// Dispatch an `ApolloRaftRequest` to the appropriate embedded write helper.
///
/// `id` is the `log_index`-derived deterministic primary key, used to overwrite
/// the placeholder `id` of created entities.
fn apply_apollo_request(db: &DB, request: ApolloRaftRequest, id: i64) -> Result<(), String> {
    match request {
        // ---- App (no numeric id) ----
        ApolloRaftRequest::AppCreate(app) => {
            AppEmbedded::write_raw(db, &app).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::AppUpdate(app) => {
            AppEmbedded::write_raw(db, &app).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::AppDelete { app_id } => {
            AppEmbedded::delete_raw(db, &app_id).map_err(|e| e.to_string())
        }

        // ---- Namespace ----
        ApolloRaftRequest::NamespaceCreate(mut ns) => {
            ns.id = id;
            NamespaceEmbedded::write_raw(db, &ns).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::NamespaceUpdate(mut ns) => {
            if ns.id == 0 {
                ns.id = id;
            }
            NamespaceEmbedded::write_raw(db, &ns).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::NamespaceDelete { id } => {
            NamespaceEmbedded::delete_raw(db, id).map_err(|e| e.to_string())
        }

        // ---- Cluster ----
        ApolloRaftRequest::ClusterCreate(mut c) => {
            c.id = id;
            ClusterEmbedded::write_raw(db, &c).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::ClusterUpdate(mut c) => {
            if c.id == 0 {
                c.id = id;
            }
            ClusterEmbedded::write_raw(db, &c).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::ClusterDelete { id } => {
            ClusterEmbedded::delete_raw(db, id).map_err(|e| e.to_string())
        }

        // ---- Item ----
        ApolloRaftRequest::ItemCreate(mut it) => {
            it.id = id;
            ItemEmbedded::write_raw(db, &it).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::ItemUpdate(mut it) => {
            if it.id == 0 {
                it.id = id;
            }
            ItemEmbedded::write_raw(db, &it).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::ItemDelete { id } => {
            ItemEmbedded::delete_raw(db, id).map_err(|e| e.to_string())
        }

        // ---- Release ----
        ApolloRaftRequest::ReleaseCreate(mut r) => {
            r.id = id;
            ReleaseEmbedded::write_raw(db, &r).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::ReleaseUpdate(mut r) => {
            if r.id == 0 {
                r.id = id;
            }
            ReleaseEmbedded::write_raw(db, &r).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::ReleaseDelete { id } => {
            ReleaseEmbedded::delete_raw(db, id).map_err(|e| e.to_string())
        }

        // ---- Commit ----
        ApolloRaftRequest::CommitCreate(mut c) => {
            c.id = id;
            CommitEmbedded::write_raw(db, &c).map(|_| ()).map_err(|e| e.to_string())
        }

        // ---- AccessKey ----
        ApolloRaftRequest::AccessKeyCreate(mut ak) => {
            ak.id = id;
            AccessKeyEmbedded::write_raw(db, &ak).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::AccessKeyUpdate(mut ak) => {
            if ak.id == 0 {
                ak.id = id;
            }
            AccessKeyEmbedded::write_raw(db, &ak).map(|_| ()).map_err(|e| e.to_string())
        }
        ApolloRaftRequest::AccessKeyDelete { id } => {
            AccessKeyEmbedded::delete_raw(db, id).map_err(|e| e.to_string())
        }

        // ---- ReleaseMessage (id is the client-visible notificationId) ----
        ApolloRaftRequest::ReleaseMessageCreate(mut m) => {
            m.id = id;
            ReleaseMessageEmbedded::write_raw(db, &m)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        // ---- GrayReleaseRule ----
        ApolloRaftRequest::GrayReleaseCreate(mut rule) => {
            rule.id = id;
            GrayReleaseEmbedded::write_raw(db, &rule)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        ApolloRaftRequest::GrayReleaseUpdate(mut rule) => {
            if rule.id == 0 {
                rule.id = id;
            }
            GrayReleaseEmbedded::write_raw(db, &rule)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        ApolloRaftRequest::GrayReleaseDelete { id } => {
            GrayReleaseEmbedded::delete_raw(db, id).map_err(|e| e.to_string())
        }
    }
}

/// Convenience: register the Apollo Raft handler with a running Raft node.
pub async fn register_apollo_raft(raft_node: &Arc<RaftNode>) -> Result<(), String> {
    raft_node.register_plugin(ApolloRaftPluginHandler::new_arc()).await
}
