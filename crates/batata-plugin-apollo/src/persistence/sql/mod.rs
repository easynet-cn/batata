//! SQL backend implementations for Apollo persistence traits
//!
//! This module provides SQL-based implementations of all Apollo persistence traits
//! using SeaORM for database operations.

mod access_key_sql;
mod app_sql;
mod cluster_sql;
mod commit_sql;
mod gray_release_sql;
mod instance_sql;
mod item_sql;
mod namespace_lock_sql;
mod namespace_sql;
mod release_message_sql;
mod release_sql;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait};

use crate::persistence::traits::{
    AccessKeyPersistence, AppPersistence, ClusterPersistence, CommitPersistence, GrayReleasePersistence,
    InstancePersistence, ItemPersistence, NamespaceLockPersistence, NamespacePersistence,
    ReleaseMessagePersistence, ReleasePersistence,
};

pub use access_key_sql::AccessKeySqlPersistence;
pub use app_sql::AppSqlPersistence;
pub use cluster_sql::ClusterSqlPersistence;
pub use commit_sql::CommitSqlPersistence;
pub use gray_release_sql::GrayReleaseSqlPersistence;
pub use instance_sql::InstanceSqlPersistence;
pub use item_sql::ItemSqlPersistence;
pub use namespace_lock_sql::NamespaceLockSqlPersistence;
pub use namespace_sql::NamespaceSqlPersistence;
pub use release_message_sql::ReleaseMessageSqlPersistence;
pub use release_sql::ReleaseSqlPersistence;

use crate::persistence::traits::ApolloPersistenceService;

/// SQL-based implementation of all Apollo persistence traits
///
/// This struct holds a database connection and provides implementations
/// for all persistence operations using an external SQL database.
pub struct SqlApolloPersistence {
    db: DatabaseConnection,
    app: AppSqlPersistence,
    cluster: ClusterSqlPersistence,
    namespace: NamespaceSqlPersistence,
    item: ItemSqlPersistence,
    release: ReleaseSqlPersistence,
    commit: CommitSqlPersistence,
    gray_release: GrayReleaseSqlPersistence,
    instance: InstanceSqlPersistence,
    access_key: AccessKeySqlPersistence,
    release_message: ReleaseMessageSqlPersistence,
    namespace_lock: NamespaceLockSqlPersistence,
}

impl SqlApolloPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            app: AppSqlPersistence::new(db.clone()),
            cluster: ClusterSqlPersistence::new(db.clone()),
            namespace: NamespaceSqlPersistence::new(db.clone()),
            item: ItemSqlPersistence::new(db.clone()),
            release: ReleaseSqlPersistence::new(db.clone()),
            commit: CommitSqlPersistence::new(db.clone()),
            gray_release: GrayReleaseSqlPersistence::new(db.clone()),
            instance: InstanceSqlPersistence::new(db.clone()),
            access_key: AccessKeySqlPersistence::new(db.clone()),
            release_message: ReleaseMessageSqlPersistence::new(db.clone()),
            namespace_lock: NamespaceLockSqlPersistence::new(db.clone()),
            db,
        }
    }
}

#[async_trait]
impl ClusterPersistence for SqlApolloPersistence {
    async fn create(&self, cluster: crate::persistence::shared::StoredCluster) -> anyhow::Result<crate::persistence::shared::StoredCluster> {
        self.cluster.create(cluster).await
    }
    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredCluster>> {
        self.cluster.get(app_id, cluster_name).await
    }
    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredCluster>> {
        self.cluster.list(app_id).await
    }
    async fn update(&self, cluster: crate::persistence::shared::StoredCluster) -> anyhow::Result<crate::persistence::shared::StoredCluster> {
        self.cluster.update(cluster).await
    }
    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()> {
        self.cluster.delete(app_id, cluster_name).await
    }
}

#[async_trait]
impl AppPersistence for SqlApolloPersistence {
    async fn create(&self, app: crate::persistence::shared::StoredApp) -> anyhow::Result<crate::persistence::shared::StoredApp> {
        self.app.create(app).await
    }

    async fn get(&self, app_id: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredApp>> {
        self.app.get(app_id).await
    }

    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<crate::persistence::shared::StoredApp>> {
        self.app.get_by_ids(app_ids).await
    }

    async fn list(&self) -> anyhow::Result<Vec<crate::persistence::shared::StoredApp>> {
        self.app.list().await
    }

    async fn update(&self, app: crate::persistence::shared::StoredApp) -> anyhow::Result<crate::persistence::shared::StoredApp> {
        self.app.update(app).await
    }

    async fn delete(&self, app_id: &str) -> anyhow::Result<()> {
        self.app.delete(app_id).await
    }
}

#[async_trait]
impl NamespacePersistence for SqlApolloPersistence {
    async fn create(&self, namespace: crate::persistence::shared::StoredNamespace) -> anyhow::Result<crate::persistence::shared::StoredNamespace> {
        self.namespace.create(namespace).await
    }

    async fn get(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredNamespace>> {
        self.namespace.get(id).await
    }

    async fn get_by_app_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<crate::persistence::shared::StoredNamespace>> {
        self.namespace.get_by_app_cluster(app_id, cluster_name, namespace_name).await
    }

    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredNamespace>> {
        self.namespace.list_by_app(app_id).await
    }

    async fn update(&self, namespace: crate::persistence::shared::StoredNamespace) -> anyhow::Result<crate::persistence::shared::StoredNamespace> {
        self.namespace.update(namespace).await
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.namespace.delete(id).await
    }
}

#[async_trait]
impl ItemPersistence for SqlApolloPersistence {
    async fn create(&self, item: crate::persistence::shared::StoredItem) -> anyhow::Result<crate::persistence::shared::StoredItem> {
        self.item.create(item).await
    }

    async fn get_by_key(&self, namespace_id: i32, key: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredItem>> {
        self.item.get_by_key(namespace_id, key).await
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredItem>> {
        self.item.get_by_id(id).await
    }

    async fn list_by_namespace(&self, namespace_id: i32) -> anyhow::Result<Vec<crate::persistence::shared::StoredItem>> {
        self.item.list_by_namespace(namespace_id).await
    }

    async fn update(&self, item: crate::persistence::shared::StoredItem) -> anyhow::Result<crate::persistence::shared::StoredItem> {
        self.item.update(item).await
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.item.delete(id).await
    }

    async fn batch_create(&self, items: Vec<crate::persistence::shared::StoredItem>) -> anyhow::Result<Vec<crate::persistence::shared::StoredItem>> {
        self.item.batch_create(items).await
    }
}

#[async_trait]
impl ReleasePersistence for SqlApolloPersistence {
    async fn create(&self, release: crate::persistence::shared::StoredRelease) -> anyhow::Result<crate::persistence::shared::StoredRelease> {
        self.release.create(release).await
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredRelease>> {
        self.release.get_by_id(id).await
    }

    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<crate::persistence::shared::StoredRelease>> {
        self.release.get_latest(app_id, cluster_name, namespace_name).await
    }

    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<crate::persistence::shared::StoredRelease>> {
        self.release.list_by_namespace(app_id, cluster_name, namespace_name).await
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.release.delete(id).await
    }

    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<crate::persistence::shared::StoredRelease>> {
        self.release.get_by_release_id(release_id).await
    }
}

#[async_trait]
impl CommitPersistence for SqlApolloPersistence {
    async fn create(&self, commit: crate::persistence::shared::StoredCommit) -> anyhow::Result<crate::persistence::shared::StoredCommit> {
        self.commit.create(commit).await
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<crate::persistence::shared::StoredCommit>> {
        self.commit.get_by_id(id).await
    }

    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<crate::persistence::shared::StoredCommit>> {
        self.commit.list_by_namespace(app_id, cluster_name, namespace_name).await
    }

    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<crate::persistence::shared::StoredCommit>> {
        self.commit.get_latest(app_id, cluster_name, namespace_name).await
    }

    async fn update(&self, commit: crate::persistence::shared::StoredCommit) -> anyhow::Result<crate::persistence::shared::StoredCommit> {
        self.commit.update(commit).await
    }
}

#[async_trait]
impl GrayReleasePersistence for SqlApolloPersistence {
    async fn create(&self, rule: crate::persistence::shared::StoredGrayReleaseRule) -> anyhow::Result<crate::persistence::shared::StoredGrayReleaseRule> {
        self.gray_release.create(rule).await
    }

    async fn get_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<crate::persistence::shared::StoredGrayReleaseRule>> {
        self.gray_release.get_by_namespace(app_id, cluster_name, namespace_name).await
    }

    async fn update_rules(
        &self,
        id: i32,
        rules: String,
        release_id: i64,
    ) -> anyhow::Result<crate::persistence::shared::StoredGrayReleaseRule> {
        self.gray_release.update_rules(id, rules, release_id).await
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.gray_release.delete(id).await
    }

    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredGrayReleaseRule>> {
        self.gray_release.list_by_app(app_id).await
    }
}

#[async_trait]
impl InstancePersistence for SqlApolloPersistence {
    async fn upsert(&self, instance: crate::persistence::shared::StoredInstance) -> anyhow::Result<crate::persistence::shared::StoredInstance> {
        self.instance.upsert(instance).await
    }

    async fn get_by_app(
        &self,
        app_id: &str,
        cluster_name: Option<&str>,
    ) -> anyhow::Result<Vec<crate::persistence::shared::StoredInstance>> {
        self.instance.get_by_app(app_id, cluster_name).await
    }

    async fn delete_expired(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        self.instance.delete_expired(before_timestamp).await
    }

    async fn list_all(&self) -> anyhow::Result<Vec<crate::persistence::shared::StoredInstance>> {
        self.instance.list_all().await
    }
}

#[async_trait]
impl AccessKeyPersistence for SqlApolloPersistence {
    async fn create(&self, access_key: crate::persistence::shared::StoredAccessKey) -> anyhow::Result<crate::persistence::shared::StoredAccessKey> {
        self.access_key.create(access_key).await
    }

    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<crate::persistence::shared::StoredAccessKey>> {
        self.access_key.get_by_app(app_id).await
    }

    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<crate::persistence::shared::StoredAccessKey>> {
        self.access_key.get_by_secret(secret).await
    }

    async fn update(&self, access_key: crate::persistence::shared::StoredAccessKey) -> anyhow::Result<crate::persistence::shared::StoredAccessKey> {
        self.access_key.update(access_key).await
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.access_key.delete(id).await
    }
}

#[async_trait]
impl ReleaseMessagePersistence for SqlApolloPersistence {
    async fn create(&self, message: crate::persistence::shared::StoredReleaseMessage) -> anyhow::Result<crate::persistence::shared::StoredReleaseMessage> {
        self.release_message.create(message).await
    }

    async fn get_latest(&self) -> anyhow::Result<Option<crate::persistence::shared::StoredReleaseMessage>> {
        self.release_message.get_latest().await
    }

    async fn list_all(&self) -> anyhow::Result<Vec<crate::persistence::shared::StoredReleaseMessage>> {
        self.release_message.list_all().await
    }

    async fn delete_old(&self, before_id: i32) -> anyhow::Result<usize> {
        self.release_message.delete_old(before_id).await
    }
}

#[async_trait]
impl NamespaceLockPersistence for SqlApolloPersistence {
    async fn lock(&self, lock: crate::persistence::shared::StoredNamespaceLock) -> anyhow::Result<crate::persistence::shared::StoredNamespaceLock> {
        self.namespace_lock.lock(lock).await
    }

    async fn unlock(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<()> {
        self.namespace_lock.unlock(app_id, cluster_name, namespace_name).await
    }

    async fn get(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<crate::persistence::shared::StoredNamespaceLock>> {
        self.namespace_lock.get(app_id, cluster_name, namespace_name).await
    }

    async fn is_locked(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<bool> {
        self.namespace_lock.is_locked(app_id, cluster_name, namespace_name).await
    }
}

#[async_trait]
impl ApolloPersistenceService for SqlApolloPersistence {
    async fn health_check(&self) -> anyhow::Result<()> {
        use crate::entity::prelude::ApolloApp;
        use sea_orm::prelude::Expr;
        use sea_orm::QuerySelect;

        ApolloApp::find()
            .select_only()
            .column_as(Expr::cust("1"), "health")
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;
        Ok(())
    }

    fn get_db_connection(&self) -> Option<DatabaseConnection> {
        Some(self.db.clone())
    }
}