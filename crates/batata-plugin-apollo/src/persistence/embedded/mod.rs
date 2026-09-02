//! Embedded (RocksDB) backend implementations for Apollo plugin
//!
//! Provides standalone single-node storage using RocksDB without an external database.

mod app_embedded;
mod cluster_embedded;
mod namespace_embedded;
mod item_embedded;
mod release_embedded;
mod commit_embedded;
mod gray_release_embedded;
mod instance_embedded;
mod access_key_embedded;
mod release_message_embedded;
mod service_registry_embedded;
mod namespace_lock_embedded;
mod id_generator;
mod portal;
mod store;

use std::sync::Arc;
use async_trait::async_trait;
use rocksdb::DB;

use crate::persistence::traits::ApolloPersistenceService;
use crate::persistence::shared::*;
use crate::raft::{ApolloRaftRequest, ApolloRaftWriter};
pub use id_generator::IdGenerator;
pub use store::JsonStore;

pub use app_embedded::AppEmbedded;
pub use cluster_embedded::ClusterEmbedded;
pub use namespace_embedded::NamespaceEmbedded;
pub use item_embedded::ItemEmbedded;
pub use release_embedded::ReleaseEmbedded;
pub use commit_embedded::CommitEmbedded;
pub use gray_release_embedded::GrayReleaseEmbedded;
pub use instance_embedded::InstanceEmbedded;
pub use access_key_embedded::AccessKeyEmbedded;
pub use release_message_embedded::ReleaseMessageEmbedded;
pub use service_registry_embedded::ServiceRegistryEmbedded;
pub use namespace_lock_embedded::NamespaceLockEmbedded;

/// Embedded (RocksDB) implementation of all Apollo persistence traits
///
/// This struct holds a RocksDB instance and provides implementations
/// for all persistence operations using embedded storage.
pub struct EmbeddedApolloPersistence {
    #[allow(dead_code)]
    db: Arc<DB>,
    app: AppEmbedded,
    cluster: ClusterEmbedded,
    namespace: NamespaceEmbedded,
    item: ItemEmbedded,
    release: ReleaseEmbedded,
    commit: CommitEmbedded,
    gray_release: GrayReleaseEmbedded,
    instance: InstanceEmbedded,
    access_key: AccessKeyEmbedded,
    release_message: ReleaseMessageEmbedded,
    service_registry: ServiceRegistryEmbedded,
    namespace_lock: NamespaceLockEmbedded,
    portal_id_gen: Arc<IdGenerator>,
    /// When set, write operations are replicated through the unified Raft
    /// group instead of being applied directly. `None` in standalone mode.
    raft_writer: Option<Arc<ApolloRaftWriter>>,
}

impl EmbeddedApolloPersistence {
    /// Creates a new `EmbeddedApolloPersistence`.
    pub fn new(db: Arc<DB>) -> Self {
        // Recover id counters from existing rows so ids stay unique and
        // monotonic across restarts (upstream relies on DB AUTO_INCREMENT).
        let core_gen = Arc::new(IdGenerator::new(
            Self::recover_core_max_id(&db).map_or(1, |m| m + 1),
        ));
        let portal_id_gen = Arc::new(IdGenerator::new(
            Self::recover_portal_max_id(&db).map_or(1, |m| m + 1),
        ));
        let id_gen = core_gen;
        Self {
            app: AppEmbedded::new(db.clone()),
            cluster: ClusterEmbedded::new(db.clone(), id_gen.clone()),
            namespace: NamespaceEmbedded::new(db.clone(), id_gen.clone()),
            item: ItemEmbedded::new(db.clone(), id_gen.clone()),
            release: ReleaseEmbedded::new(db.clone(), id_gen.clone()),
            commit: CommitEmbedded::new(db.clone(), id_gen.clone()),
            gray_release: GrayReleaseEmbedded::new(db.clone(), id_gen.clone()),
            instance: InstanceEmbedded::new(db.clone(), id_gen.clone()),
            access_key: AccessKeyEmbedded::new(db.clone(), id_gen.clone()),
            release_message: ReleaseMessageEmbedded::new(db.clone(), id_gen.clone()),
            service_registry: ServiceRegistryEmbedded::new(db.clone()),
            namespace_lock: NamespaceLockEmbedded::new(db.clone()),
            portal_id_gen,
            raft_writer: None,
            db,
        }
    }

    /// Attach a Raft writer. Caller must also have registered the
    /// `ApolloRaftPluginHandler` with the same Raft node. In cluster mode every
    /// write goes through this writer so the operation is replicated and the
    /// primary key is derived deterministically from the Raft `log_index`.
    pub fn with_raft_writer(mut self, writer: Arc<ApolloRaftWriter>) -> Self {
        self.raft_writer = Some(writer);
        self
    }

    /// Scan the by-id key prefixes of every core column family and return the
    /// highest row id currently stored (None when the database is empty).
    ///
    /// Key layouts are defined next to each store (`{prefix}{id}`); keep this
    /// list in sync when adding an entity (guarded by recovery round-trip test).
    fn recover_core_max_id(db: &DB) -> Option<i64> {
        const CF_PREFIXES: &[(&str, &str)] = &[
            (batata_consistency::raft::state_machine::CF_APOLLO_NAMESPACE, "ns_id:"),
            (batata_consistency::raft::state_machine::CF_APOLLO_ITEM, "item_id:"),
            (batata_consistency::raft::state_machine::CF_APOLLO_RELEASE, "release:"),
            (batata_consistency::raft::state_machine::CF_APOLLO_COMMIT, "commit:"),
            (batata_consistency::raft::state_machine::CF_APOLLO_GRAY_RULE, "gray_id:"),
            (batata_consistency::raft::state_machine::CF_APOLLO_ACCESS_KEY, "ak_id:"),
            (batata_consistency::raft::state_machine::CF_APOLLO_RELEASE_MSG, "rm_id:"),
        ];
        let mut max: Option<i64> = None;
        for (cf_name, prefix) in CF_PREFIXES {
            let Some(cf) = db.cf_handle(cf_name) else { continue };
            for item in db.prefix_iterator_cf(cf, prefix.as_bytes()).flatten() {
                let Ok(key) = std::str::from_utf8(&item.0) else { continue };
                let Some(rest) = key.strip_prefix(prefix) else { continue };
                if let Ok(id) = rest.parse::<i64>() {
                    max = Some(max.unwrap_or(0).max(id));
                }
            }
        }
        max
    }

    /// Portal stores key rows as `id:{n}` inside per-entity column families.
    fn recover_portal_max_id(db: &DB) -> Option<i64> {
        use batata_consistency::raft::state_machine as sm;
        const PORTAL_CFS: &[&str] = &[
            sm::CF_APOLLO_APP_NAMESPACE,
            sm::CF_APOLLO_AUDIT,
            sm::CF_APOLLO_CONSUMER,
            sm::CF_APOLLO_CONSUMER_TOKEN,
            sm::CF_APOLLO_CONSUMER_AUDIT,
            sm::CF_APOLLO_CONSUMER_ROLE,
            sm::CF_APOLLO_PERMISSION,
            sm::CF_APOLLO_ROLE,
            sm::CF_APOLLO_ROLE_PERMISSION,
            sm::CF_APOLLO_USER_ROLE,
            sm::CF_APOLLO_USERS,
            sm::CF_APOLLO_FAVORITE,
            sm::CF_APOLLO_SERVER_CONFIG,
            sm::CF_APOLLO_INSTANCE_CONFIG,
            sm::CF_APOLLO_RELEASE_HISTORY,
        ];
        let mut max: Option<i64> = None;
        for cf_name in PORTAL_CFS {
            let Some(cf) = db.cf_handle(cf_name) else { continue };
            for item in db.prefix_iterator_cf(cf, b"id:").flatten() {
                let Ok(key) = std::str::from_utf8(&item.0) else { continue };
                let Some(rest) = key.strip_prefix("id:") else { continue };
                if let Ok(id) = rest.parse::<i64>() {
                    max = Some(max.unwrap_or(0).max(id));
                }
            }
        }
        max
    }
}

#[async_trait]
impl crate::persistence::traits::ClusterPersistence for EmbeddedApolloPersistence {
    async fn create(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::ClusterCreate(cluster.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            // Apply has landed on the local state machine; read it back by
            // business key so the returned object carries the Raft-derived id.
            return self
                .cluster
                .get(&cluster.app_id, &cluster.name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("cluster not found after raft apply"));
        }
        self.cluster.create(cluster).await
    }
    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<StoredCluster>> {
        self.cluster.get(app_id, cluster_name).await
    }
    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<StoredCluster>> {
        self.cluster.list(app_id).await
    }
    async fn update(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::ClusterUpdate(cluster.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .cluster
                .get(&cluster.app_id, &cluster.name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("cluster not found after raft apply"));
        }
        self.cluster.update(cluster).await
    }
    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            let cluster = self
                .cluster
                .get(app_id, cluster_name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("cluster not found before raft delete"))?;
            return writer
                .write(ApolloRaftRequest::ClusterDelete { id: cluster.id })
                .await
                .map_err(|e| anyhow::anyhow!(e));
        }
        self.cluster.delete(app_id, cluster_name).await
    }
}

#[async_trait]
impl crate::persistence::traits::AppPersistence for EmbeddedApolloPersistence {
    async fn create(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::AppCreate(app.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .app
                .get(&app.app_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("app not found after raft apply"));
        }
        self.app.create(app).await
    }
    async fn get(&self, app_id: &str) -> anyhow::Result<Option<StoredApp>> {
        self.app.get(app_id).await
    }
    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<StoredApp>> {
        self.app.get_by_ids(app_ids).await
    }
    async fn list(&self) -> anyhow::Result<Vec<StoredApp>> {
        self.app.list().await
    }
    async fn update(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::AppUpdate(app.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .app
                .get(&app.app_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("app not found after raft apply"));
        }
        self.app.update(app).await
    }
    async fn delete(&self, app_id: &str) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            return writer
                .write(ApolloRaftRequest::AppDelete {
                    app_id: app_id.to_string(),
                })
                .await
                .map_err(|e| anyhow::anyhow!(e));
        }
        self.app.delete(app_id).await
    }
}

#[async_trait]
impl crate::persistence::traits::NamespacePersistence for EmbeddedApolloPersistence {
    async fn create(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::NamespaceCreate(namespace.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .namespace
                .get_by_app_cluster(
                    &namespace.app_id,
                    &namespace.cluster_name,
                    &namespace.namespace_name,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("namespace not found after raft apply"));
        }
        self.namespace.create(namespace).await
    }
    async fn get(&self, id: i64) -> anyhow::Result<Option<StoredNamespace>> {
        self.namespace.get(id).await
    }
    async fn get_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredNamespace>> {
        self.namespace.get_by_app_cluster(app_id, cluster_name, namespace_name).await
    }
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredNamespace>> {
        self.namespace.list_by_app(app_id).await
    }
    async fn list_all(&self) -> anyhow::Result<Vec<StoredNamespace>> {
        self.namespace.list_all().await
    }
    async fn update(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::NamespaceUpdate(namespace.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .namespace
                .get_by_app_cluster(
                    &namespace.app_id,
                    &namespace.cluster_name,
                    &namespace.namespace_name,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("namespace not found after raft apply"));
        }
        self.namespace.update(namespace).await
    }
    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            return writer
                .write(ApolloRaftRequest::NamespaceDelete { id })
                .await
                .map_err(|e| anyhow::anyhow!(e));
        }
        self.namespace.delete(id).await
    }
}

#[async_trait]
impl crate::persistence::traits::ItemPersistence for EmbeddedApolloPersistence {
    async fn create(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::ItemCreate(item.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .item
                .get_by_key(item.namespace_id, &item.key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("item not found after raft apply"));
        }
        self.item.create(item).await
    }
    async fn get_by_key(&self, namespace_id: i64, key: &str) -> anyhow::Result<Option<StoredItem>> {
        self.item.get_by_key(namespace_id, key).await
    }
    async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<StoredItem>> {
        self.item.get_by_id(id).await
    }
    async fn list_by_namespace(&self, namespace_id: i64) -> anyhow::Result<Vec<StoredItem>> {
        self.item.list_by_namespace(namespace_id).await
    }
    async fn update(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::ItemUpdate(item.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .item
                .get_by_key(item.namespace_id, &item.key)
                .await?
                .ok_or_else(|| anyhow::anyhow!("item not found after raft apply"));
        }
        self.item.update(item).await
    }
    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            return writer
                .write(ApolloRaftRequest::ItemDelete { id })
                .await
                .map_err(|e| anyhow::anyhow!(e));
        }
        self.item.delete(id).await
    }
    async fn batch_create(&self, items: Vec<StoredItem>) -> anyhow::Result<Vec<StoredItem>> {
        self.item.batch_create(items).await
    }
    async fn list_deleted_items(&self, namespace_id: i64) -> anyhow::Result<Vec<StoredItem>> {
        self.item.list_deleted_items(namespace_id).await
    }
    async fn find_namespace_ids_by_item_key(&self, key: &str) -> anyhow::Result<Vec<i64>> {
        self.item.find_namespace_ids_by_item_key(key).await
    }
}

#[async_trait]
impl crate::persistence::traits::ReleasePersistence for EmbeddedApolloPersistence {
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::ReleaseCreate(release.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .release
                .get_latest(
                    &release.app_id,
                    &release.cluster_name,
                    &release.namespace_name,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("release not found after raft apply"));
        }
        self.release.create(release).await
    }
    async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<StoredRelease>> {
        self.release.get_by_id(id).await
    }
    async fn get_latest(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredRelease>> {
        self.release.get_latest(app_id, cluster_name, namespace_name).await
    }
    async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<StoredRelease>> {
        self.release.list_by_namespace(app_id, cluster_name, namespace_name).await
    }
    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            return writer
                .write(ApolloRaftRequest::ReleaseDelete { id })
                .await
                .map_err(|e| anyhow::anyhow!(e));
        }
        self.release.delete(id).await
    }
    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>> {
        self.release.get_by_release_id(release_id).await
    }
    async fn update(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::ReleaseUpdate(release.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .release
                .get_latest(
                    &release.app_id,
                    &release.cluster_name,
                    &release.namespace_name,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("release not found after raft apply"));
        }
        self.release.update(release).await
    }
    async fn list_active(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>> {
        self.release.list_active(app_id, cluster_name, namespace_name).await
    }
}

#[async_trait]
impl crate::persistence::traits::CommitPersistence for EmbeddedApolloPersistence {
    async fn create(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::CommitCreate(commit.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .commit
                .get_latest(
                    &commit.app_id,
                    &commit.cluster_name,
                    &commit.namespace_name,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("commit not found after raft apply"));
        }
        self.commit.create(commit).await
    }
    async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<StoredCommit>> {
        self.commit.get_by_id(id).await
    }
    async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<StoredCommit>> {
        self.commit.list_by_namespace(app_id, cluster_name, namespace_name).await
    }
    async fn get_latest(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredCommit>> {
        self.commit.get_latest(app_id, cluster_name, namespace_name).await
    }
    async fn update(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::CommitCreate(commit.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .commit
                .get_latest(
                    &commit.app_id,
                    &commit.cluster_name,
                    &commit.namespace_name,
                )
                .await?
                .ok_or_else(|| anyhow::anyhow!("commit not found after raft apply"));
        }
        self.commit.update(commit).await
    }
}

#[async_trait]
impl crate::persistence::traits::GrayReleasePersistence for EmbeddedApolloPersistence {
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule> {
        if let Some(writer) = &self.raft_writer {
            // Replicate so every node sees the same gray rule (the id is
            // assigned by the Raft apply phase from the log_index).
            writer
                .write(ApolloRaftRequest::GrayReleaseCreate(rule.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            // Read back the applied rule (id is now deterministic).
            self.gray_release
                .get_by_namespace(&rule.app_id, &rule.cluster_name, &rule.namespace_name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("gray release rule missing after raft apply"))
        } else {
            self.gray_release.create(rule).await
        }
    }
    async fn get_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredGrayReleaseRule>> {
        self.gray_release.get_by_namespace(app_id, cluster_name, namespace_name).await
    }
    async fn update_rules(&self, id: i64, rules: String, release_id: i64) -> anyhow::Result<StoredGrayReleaseRule> {
        if let Some(writer) = &self.raft_writer {
            // Reconstruct the full updated rule locally, then replicate the whole
            // object so the apply phase can write both keys deterministically.
            let mut rule = GrayReleaseEmbedded::get_by_id(&self.db, id)?;
            rule.rules = rules;
            rule.release_id = release_id;
            rule.data_change_last_time = Some(chrono::Utc::now().timestamp_millis());
            writer
                .write(ApolloRaftRequest::GrayReleaseUpdate(rule.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            Ok(rule)
        } else {
            self.gray_release.update_rules(id, rules, release_id).await
        }
    }
    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::GrayReleaseDelete { id })
                .await
                .map_err(|e| anyhow::anyhow!(e))
        } else {
            self.gray_release.delete(id).await
        }
    }
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredGrayReleaseRule>> {
        self.gray_release.list_by_app(app_id).await
    }
}

#[async_trait]
impl crate::persistence::traits::InstancePersistence for EmbeddedApolloPersistence {
    async fn upsert(&self, instance: StoredInstance) -> anyhow::Result<StoredInstance> {
        self.instance.upsert(instance).await
    }
    async fn get_by_app(&self, app_id: &str, cluster_name: Option<&str>) -> anyhow::Result<Vec<StoredInstance>> {
        self.instance.get_by_app(app_id, cluster_name).await
    }
    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        self.instance.delete_expired(before_timestamp).await
    }
    async fn list_all(&self) -> anyhow::Result<Vec<StoredInstance>> {
        self.instance.list_all().await
    }
    async fn get_instance_by_id(&self, id: i64) -> anyhow::Result<Option<StoredInstance>> {
        self.instance.get_instance_by_id(id).await
    }
}

#[async_trait]
impl crate::persistence::traits::AccessKeyPersistence for EmbeddedApolloPersistence {
    async fn create(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::AccessKeyCreate(access_key.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .access_key
                .get_by_secret(&access_key.secret)
                .await?
                .ok_or_else(|| anyhow::anyhow!("access key not found after raft apply"));
        }
        self.access_key.create(access_key).await
    }
    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredAccessKey>> {
        self.access_key.get_by_app(app_id).await
    }
    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<StoredAccessKey>> {
        self.access_key.get_by_secret(secret).await
    }
    async fn update(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        if let Some(writer) = &self.raft_writer {
            writer
                .write(ApolloRaftRequest::AccessKeyUpdate(access_key.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            return self
                .access_key
                .get_by_secret(&access_key.secret)
                .await?
                .ok_or_else(|| anyhow::anyhow!("access key not found after raft apply"));
        }
        self.access_key.update(access_key).await
    }
    async fn delete(&self, id: i64) -> anyhow::Result<()> {
        if let Some(writer) = &self.raft_writer {
            return writer
                .write(ApolloRaftRequest::AccessKeyDelete { id })
                .await
                .map_err(|e| anyhow::anyhow!(e));
        }
        self.access_key.delete(id).await
    }
}

#[async_trait]
impl crate::persistence::traits::ReleaseMessagePersistence for EmbeddedApolloPersistence {
    async fn create(&self, message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage> {
        if let Some(writer) = &self.raft_writer {
            // Replicate so every node sees the same notificationId (the row id).
            writer
                .write(ApolloRaftRequest::ReleaseMessageCreate(message.clone()))
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            // Read back the applied row (the id was assigned by the Raft apply
            // phase from the log_index, so it is now deterministic).
            self.release_message
                .find_latest_by_message(&message.message)
                .await?
                .ok_or_else(|| anyhow::anyhow!("release message missing after raft apply"))
        } else {
            self.release_message.create(message).await
        }
    }
    async fn find_latest_by_message(
        &self,
        message: &str,
    ) -> anyhow::Result<Option<StoredReleaseMessage>> {
        self.release_message.find_latest_by_message(message).await
    }
    async fn delete_by_id(&self, id: i64) -> anyhow::Result<()> {
        self.release_message.delete_by_id(id).await
    }
    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>> {
        self.release_message.get_latest().await
    }
    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>> {
        self.release_message.list_all().await
    }
    async fn delete_old(&self, before_id: i64) -> anyhow::Result<usize> {
        self.release_message.delete_old(before_id).await
    }
}

#[async_trait]
impl crate::persistence::traits::NamespaceLockPersistence for EmbeddedApolloPersistence {
    async fn lock(&self, lock: StoredNamespaceLock) -> anyhow::Result<StoredNamespaceLock> {
        self.namespace_lock.lock(lock).await
    }
    async fn unlock(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<()> {
        self.namespace_lock.unlock(app_id, cluster_name, namespace_name).await
    }
    async fn get(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredNamespaceLock>> {
        self.namespace_lock.get(app_id, cluster_name, namespace_name).await
    }
    async fn is_locked(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<bool> {
        self.namespace_lock.is_locked(app_id, cluster_name, namespace_name).await
    }
}

#[async_trait]
impl ApolloPersistenceService for EmbeddedApolloPersistence {
    async fn health_check(&self) -> anyhow::Result<()> {
        // Verify that Apollo column families exist
        use batata_consistency::raft::state_machine::*;
        let cfs = [
            CF_APOLLO_APP, CF_APOLLO_NAMESPACE, CF_APOLLO_ITEM,
            CF_APOLLO_RELEASE, CF_APOLLO_COMMIT, CF_APOLLO_GRAY_RULE,
            CF_APOLLO_INSTANCE, CF_APOLLO_ACCESS_KEY, CF_APOLLO_RELEASE_MSG,
            CF_APOLLO_NAMESPACE_LOCK, CF_APOLLO_RELEASE_HISTORY,
            CF_APOLLO_APP_NAMESPACE, CF_APOLLO_AUDIT, CF_APOLLO_CONSUMER,
            CF_APOLLO_CONSUMER_TOKEN, CF_APOLLO_CONSUMER_AUDIT, CF_APOLLO_PERMISSION,
            CF_APOLLO_CONSUMER_ROLE,
            CF_APOLLO_ROLE, CF_APOLLO_ROLE_PERMISSION, CF_APOLLO_USER_ROLE,
            CF_APOLLO_USERS, CF_APOLLO_FAVORITE, CF_APOLLO_SERVER_CONFIG,
            CF_APOLLO_INSTANCE_CONFIG,
        ];
        for cf_name in cfs {
            self.db.cf_handle(cf_name)
                .ok_or_else(|| anyhow::anyhow!("Column family {} not found", cf_name))?;
        }
        Ok(())
    }
}


#[async_trait]
impl crate::persistence::traits::service_registry::ServiceRegistryPersistence for EmbeddedApolloPersistence {
    async fn heartbeat(&self, service_name: &str, uri: &str, cluster: &str) -> anyhow::Result<()> {
        self.service_registry.heartbeat(service_name, uri, cluster).await
    }
    async fn deregister(&self, service_name: &str, uri: &str) -> anyhow::Result<()> {
        self.service_registry.deregister(service_name, uri).await
    }
    async fn find_alive(
        &self,
        service_name: &str,
        window_secs: i64,
    ) -> anyhow::Result<Vec<crate::persistence::traits::service_registry::ServiceRegistryEntry>> {
        self.service_registry.find_alive(service_name, window_secs).await
    }
}
