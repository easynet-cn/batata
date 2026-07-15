//! Apollo persistence traits for unified storage abstraction
//!
//! This module defines the core persistence traits that abstract over different
//! storage backends: external database (MySQL/PostgreSQL) and embedded (RocksDB).

mod app;
mod cluster;
mod namespace;
mod item;
mod release;
mod commit;
mod gray_release;
mod instance;
mod access_key;
mod release_message;
mod namespace_lock;

pub use app::AppPersistence;
pub use cluster::ClusterPersistence;
pub use namespace::NamespacePersistence;
pub use item::ItemPersistence;
pub use release::ReleasePersistence;
pub use commit::CommitPersistence;
pub use gray_release::GrayReleasePersistence;
pub use instance::InstancePersistence;
pub use access_key::AccessKeyPersistence;
pub use release_message::ReleaseMessagePersistence;
pub use namespace_lock::NamespaceLockPersistence;

use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait ApolloPersistenceService:
    AppPersistence
    + ClusterPersistence
    + NamespacePersistence
    + ItemPersistence
    + ReleasePersistence
    + CommitPersistence
    + GrayReleasePersistence
    + InstancePersistence
    + AccessKeyPersistence
    + ReleaseMessagePersistence
    + NamespaceLockPersistence
    + Send
    + Sync
{
    async fn health_check(&self) -> anyhow::Result<()>;

    fn get_db_connection(&self) -> Option<sea_orm::DatabaseConnection> {
        None
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ClusterPersistence for Arc<T> {
    async fn create(&self, cluster: crate::persistence::shared::StoredCluster) -> anyhow::Result<crate::persistence::shared::StoredCluster> {
        ClusterPersistence::create(&**self, cluster).await
    }
    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredCluster>> {
        ClusterPersistence::get(&**self, app_id, cluster_name).await
    }
    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredCluster>> {
        ClusterPersistence::list(&**self, app_id).await
    }
    async fn update(&self, cluster: crate::persistence::shared::StoredCluster) -> anyhow::Result<crate::persistence::shared::StoredCluster> {
        ClusterPersistence::update(&**self, cluster).await
    }
    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()> {
        ClusterPersistence::delete(&**self, app_id, cluster_name).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> AppPersistence for Arc<T> {
    async fn create(&self, app: crate::persistence::shared::StoredApp) -> anyhow::Result<crate::persistence::shared::StoredApp> {
        AppPersistence::create(&**self, app).await
    }
    async fn get(&self, app_id: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredApp>> {
        AppPersistence::get(&**self, app_id).await
    }
    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<crate::persistence::shared::StoredApp>> {
        AppPersistence::get_by_ids(&**self, app_ids).await
    }
    async fn list(&self) -> anyhow::Result<Vec<crate::persistence::shared::StoredApp>> {
        AppPersistence::list(&**self).await
    }
    async fn update(&self, app: crate::persistence::shared::StoredApp) -> anyhow::Result<crate::persistence::shared::StoredApp> {
        AppPersistence::update(&**self, app).await
    }
    async fn delete(&self, app_id: &str) -> anyhow::Result<()> {
        AppPersistence::delete(&**self, app_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> NamespacePersistence for Arc<T> {
    async fn create(&self, namespace: crate::persistence::shared::StoredNamespace) -> anyhow::Result<crate::persistence::shared::StoredNamespace> {
        NamespacePersistence::create(&**self, namespace).await
    }
    async fn get(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredNamespace>> {
        NamespacePersistence::get(&**self, id).await
    }
    async fn get_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredNamespace>> {
        NamespacePersistence::get_by_app_cluster(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredNamespace>> {
        NamespacePersistence::list_by_app(&**self, app_id).await
    }
    async fn update(&self, namespace: crate::persistence::shared::StoredNamespace) -> anyhow::Result<crate::persistence::shared::StoredNamespace> {
        NamespacePersistence::update(&**self, namespace).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        NamespacePersistence::delete(&**self, id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ItemPersistence for Arc<T> {
    async fn create(&self, item: crate::persistence::shared::StoredItem) -> anyhow::Result<crate::persistence::shared::StoredItem> {
        ItemPersistence::create(&**self, item).await
    }
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredItem>> {
        ItemPersistence::get_by_id(&**self, id).await
    }
    async fn get_by_key(&self, namespace_id: i32, key: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredItem>> {
        ItemPersistence::get_by_key(&**self, namespace_id, key).await
    }
    async fn list_by_namespace(&self, namespace_id: i32) -> anyhow::Result<Vec<crate::persistence::shared::StoredItem>> {
        ItemPersistence::list_by_namespace(&**self, namespace_id).await
    }
    async fn update(&self, item: crate::persistence::shared::StoredItem) -> anyhow::Result<crate::persistence::shared::StoredItem> {
        ItemPersistence::update(&**self, item).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        ItemPersistence::delete(&**self, id).await
    }
    async fn batch_create(&self, items: Vec<crate::persistence::shared::StoredItem>) -> anyhow::Result<Vec<crate::persistence::shared::StoredItem>> {
        ItemPersistence::batch_create(&**self, items).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ReleasePersistence for Arc<T> {
    async fn create(&self, release: crate::persistence::shared::StoredRelease) -> anyhow::Result<crate::persistence::shared::StoredRelease> {
        ReleasePersistence::create(&**self, release).await
    }
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredRelease>> {
        ReleasePersistence::get_by_id(&**self, id).await
    }
    async fn get_latest(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredRelease>> {
        ReleasePersistence::get_latest(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredRelease>> {
        ReleasePersistence::list_by_namespace(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        ReleasePersistence::delete(&**self, id).await
    }
    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<crate::persistence::shared::StoredRelease>> {
        ReleasePersistence::get_by_release_id(&**self, release_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> CommitPersistence for Arc<T> {
    async fn create(&self, commit: crate::persistence::shared::StoredCommit) -> anyhow::Result<crate::persistence::shared::StoredCommit> {
        CommitPersistence::create(&**self, commit).await
    }
    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredCommit>> {
        CommitPersistence::get_by_id(&**self, id).await
    }
    async fn list_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredCommit>> {
        CommitPersistence::list_by_namespace(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn get_latest(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredCommit>> {
        CommitPersistence::get_latest(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn update(&self, commit: crate::persistence::shared::StoredCommit) -> anyhow::Result<crate::persistence::shared::StoredCommit> {
        CommitPersistence::update(&**self, commit).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> GrayReleasePersistence for Arc<T> {
    async fn create(&self, rule: crate::persistence::shared::StoredGrayReleaseRule) -> anyhow::Result<crate::persistence::shared::StoredGrayReleaseRule> {
        GrayReleasePersistence::create(&**self, rule).await
    }
    async fn get_by_namespace(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredGrayReleaseRule>> {
        GrayReleasePersistence::get_by_namespace(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn update_rules(&self, id: i32, rules: String, release_id: i64) -> anyhow::Result<crate::persistence::shared::StoredGrayReleaseRule> {
        GrayReleasePersistence::update_rules(&**self, id, rules, release_id).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        GrayReleasePersistence::delete(&**self, id).await
    }
    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredGrayReleaseRule>> {
        GrayReleasePersistence::list_by_app(&**self, app_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> InstancePersistence for Arc<T> {
    async fn upsert(&self, instance: crate::persistence::shared::StoredInstance) -> anyhow::Result<crate::persistence::shared::StoredInstance> {
        InstancePersistence::upsert(&**self, instance).await
    }
    async fn get_by_app(&self, app_id: &str, cluster_name: Option<&str>) -> anyhow::Result<Vec<crate::persistence::shared::StoredInstance>> {
        InstancePersistence::get_by_app(&**self, app_id, cluster_name).await
    }
    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        InstancePersistence::delete_expired(&**self, before_timestamp).await
    }
    async fn list_all(&self) -> anyhow::Result<Vec<crate::persistence::shared::StoredInstance>> {
        InstancePersistence::list_all(&**self).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> AccessKeyPersistence for Arc<T> {
    async fn create(&self, access_key: crate::persistence::shared::StoredAccessKey) -> anyhow::Result<crate::persistence::shared::StoredAccessKey> {
        AccessKeyPersistence::create(&**self, access_key).await
    }
    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredAccessKey>> {
        AccessKeyPersistence::get_by_app(&**self, app_id).await
    }
    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredAccessKey>> {
        AccessKeyPersistence::get_by_secret(&**self, secret).await
    }
    async fn update(&self, access_key: crate::persistence::shared::StoredAccessKey) -> anyhow::Result<crate::persistence::shared::StoredAccessKey> {
        AccessKeyPersistence::update(&**self, access_key).await
    }
    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        AccessKeyPersistence::delete(&**self, id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> ReleaseMessagePersistence for Arc<T> {
    async fn create(&self, message: crate::persistence::shared::StoredReleaseMessage) -> anyhow::Result<crate::persistence::shared::StoredReleaseMessage> {
        ReleaseMessagePersistence::create(&**self, message).await
    }
    async fn get_latest(&self) -> anyhow::Result<Option<crate::persistence::shared::StoredReleaseMessage>> {
        ReleaseMessagePersistence::get_latest(&**self).await
    }
    async fn list_all(&self) -> anyhow::Result<Vec<crate::persistence::shared::StoredReleaseMessage>> {
        ReleaseMessagePersistence::list_all(&**self).await
    }
    async fn delete_old(&self, before_id: i32) -> anyhow::Result<usize> {
        ReleaseMessagePersistence::delete_old(&**self, before_id).await
    }
}

#[async_trait]
impl<T: ApolloPersistenceService + ?Sized> NamespaceLockPersistence for Arc<T> {
    async fn lock(&self, lock: crate::persistence::shared::StoredNamespaceLock) -> anyhow::Result<crate::persistence::shared::StoredNamespaceLock> {
        NamespaceLockPersistence::lock(&**self, lock).await
    }
    async fn unlock(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<()> {
        NamespaceLockPersistence::unlock(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn get(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredNamespaceLock>> {
        NamespaceLockPersistence::get(&**self, app_id, cluster_name, namespace_name).await
    }
    async fn is_locked(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> anyhow::Result<bool> {
        NamespaceLockPersistence::is_locked(&**self, app_id, cluster_name, namespace_name).await
    }
}
