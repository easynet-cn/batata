//! Embedded (RocksDB) backend implementations for Apollo plugin
//!
//! Provides standalone single-node storage using RocksDB without an external database.

use crate::bincode;
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
mod namespace_lock_embedded;
mod id_generator;

use std::sync::Arc;
use async_trait::async_trait;
use rocksdb::DB;

use crate::persistence::traits::ApolloPersistenceService;
use crate::persistence::shared::*;
pub use id_generator::IdGenerator;

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
    namespace_lock: NamespaceLockEmbedded,
}

impl EmbeddedApolloPersistence {
    pub fn new(db: Arc<DB>) -> Self {
        let id_gen = Arc::new(IdGenerator::new(1));
        Self {
            app: AppEmbedded::new(db.clone()),
            cluster: ClusterEmbedded::new(db.clone()),
            namespace: NamespaceEmbedded::new(db.clone(), id_gen.clone()),
            item: ItemEmbedded::new(db.clone(), id_gen.clone()),
            release: ReleaseEmbedded::new(db.clone(), id_gen.clone()),
            commit: CommitEmbedded::new(db.clone(), id_gen.clone()),
            gray_release: GrayReleaseEmbedded::new(db.clone(), id_gen.clone()),
            instance: InstanceEmbedded::new(db.clone(), id_gen.clone()),
            access_key: AccessKeyEmbedded::new(db.clone(), id_gen.clone()),
            release_message: ReleaseMessageEmbedded::new(db.clone(), id_gen.clone()),
            namespace_lock: NamespaceLockEmbedded::new(db.clone()),
            db,
        }
    }
}

#[async_trait]
impl crate::persistence::traits::ClusterPersistence for EmbeddedApolloPersistence {
    async fn create(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        self.cluster.create(cluster).await
    }
    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<StoredCluster>> {
        self.cluster.get(app_id, cluster_name).await
    }
    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<StoredCluster>> {
        self.cluster.list(app_id).await
    }
    async fn update(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        self.cluster.update(cluster).await
    }
    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()> {
        self.cluster.delete(app_id, cluster_name).await
    }
}

#[async_trait]
impl crate::persistence::traits::AppPersistence for EmbeddedApolloPersistence {
    async fn create(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
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
        self.app.update(app).await
    }
    async fn delete(&self, app_id: &str) -> anyhow::Result<()> {
        self.app.delete(app_id).await
    }
}

#[async_trait]
impl crate::persistence::traits::NamespacePersistence for EmbeddedApolloPersistence {
    async fn create(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        self.namespace.create(namespace).await
    }
    async fn get(&self, id: i32) -> anyhow::Result<Option<StoredNamespace>> {
        self.namespace.get(id).await
    }
    async fn get_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredNamespace>> {
        self.namespace.get_by_app_cluster(app_id, cluster_name, namespace_name).await
    }
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredNamespace>> {
        self.namespace.list_by_app(app_id).await
    }
    async fn update(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        self.namespace.update(namespace).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.namespace.delete(id).await
    }
}

#[async_trait]
impl crate::persistence::traits::ItemPersistence for EmbeddedApolloPersistence {
    async fn create(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        self.item.create(item).await
    }
    async fn get_by_key(&self, namespace_id: i32, key: &str) -> anyhow::Result<Option<StoredItem>> {
        self.item.get_by_key(namespace_id, key).await
    }
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredItem>> {
        self.item.get_by_id(id).await
    }
    async fn list_by_namespace(&self, namespace_id: i32) -> anyhow::Result<Vec<StoredItem>> {
        self.item.list_by_namespace(namespace_id).await
    }
    async fn update(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        self.item.update(item).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.item.delete(id).await
    }
    async fn batch_create(&self, items: Vec<StoredItem>) -> anyhow::Result<Vec<StoredItem>> {
        self.item.batch_create(items).await
    }
}

#[async_trait]
impl crate::persistence::traits::ReleasePersistence for EmbeddedApolloPersistence {
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        self.release.create(release).await
    }
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredRelease>> {
        self.release.get_by_id(id).await
    }
    async fn get_latest(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredRelease>> {
        self.release.get_latest(app_id, cluster_name, namespace_name).await
    }
    async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<StoredRelease>> {
        self.release.list_by_namespace(app_id, cluster_name, namespace_name).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.release.delete(id).await
    }
    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>> {
        self.release.get_by_release_id(release_id).await
    }
}

#[async_trait]
impl crate::persistence::traits::CommitPersistence for EmbeddedApolloPersistence {
    async fn create(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        self.commit.create(commit).await
    }
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredCommit>> {
        self.commit.get_by_id(id).await
    }
    async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<StoredCommit>> {
        self.commit.list_by_namespace(app_id, cluster_name, namespace_name).await
    }
    async fn get_latest(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredCommit>> {
        self.commit.get_latest(app_id, cluster_name, namespace_name).await
    }
    async fn update(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        self.commit.update(commit).await
    }
}

#[async_trait]
impl crate::persistence::traits::GrayReleasePersistence for EmbeddedApolloPersistence {
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule> {
        self.gray_release.create(rule).await
    }
    async fn get_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<StoredGrayReleaseRule>> {
        self.gray_release.get_by_namespace(app_id, cluster_name, namespace_name).await
    }
    async fn update_rules(&self, id: i32, rules: String, release_id: i64) -> anyhow::Result<StoredGrayReleaseRule> {
        self.gray_release.update_rules(id, rules, release_id).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.gray_release.delete(id).await
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
}

#[async_trait]
impl crate::persistence::traits::AccessKeyPersistence for EmbeddedApolloPersistence {
    async fn create(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        self.access_key.create(access_key).await
    }
    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredAccessKey>> {
        self.access_key.get_by_app(app_id).await
    }
    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<StoredAccessKey>> {
        self.access_key.get_by_secret(secret).await
    }
    async fn update(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        self.access_key.update(access_key).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.access_key.delete(id).await
    }
}

#[async_trait]
impl crate::persistence::traits::ReleaseMessagePersistence for EmbeddedApolloPersistence {
    async fn create(&self, message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage> {
        self.release_message.create(message).await
    }
    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>> {
        self.release_message.get_latest().await
    }
    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>> {
        self.release_message.list_all().await
    }
    async fn delete_old(&self, before_id: i32) -> anyhow::Result<usize> {
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
            CF_APOLLO_NAMESPACE_LOCK,
        ];
        for cf_name in cfs {
            self.db.cf_handle(cf_name)
                .ok_or_else(|| anyhow::anyhow!("Column family {} not found", cf_name))?;
        }
        Ok(())
    }
}