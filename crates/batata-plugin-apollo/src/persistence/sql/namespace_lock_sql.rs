use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};

use crate::entity::apollo_namespace_lock;
use crate::persistence::shared::StoredNamespaceLock;
use crate::persistence::traits::NamespaceLockPersistence;

pub struct NamespaceLockSqlPersistence {
    db: DatabaseConnection,
}

impl NamespaceLockSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredNamespaceLock> for apollo_namespace_lock::ActiveModel {
    fn from(lock: StoredNamespaceLock) -> Self {
        Self {
            id: Set(lock.id),
            app_id: Set(lock.app_id),
            cluster_name: Set(lock.cluster_name),
            namespace_name: Set(lock.namespace_name),
            locked_by: Set(lock.locked_by),
            locked_at: Set(chrono::DateTime::from_timestamp_millis(lock.locked_at)
                .unwrap_or_default()
                .naive_utc()),
            data_change_created_by: Set(lock.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                lock.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(lock.data_change_last_modified_by),
            data_change_last_time: Set(lock.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_namespace_lock::Model> for StoredNamespaceLock {
    fn from(model: apollo_namespace_lock::Model) -> Self {
        Self {
            id: model.id,
            app_id: model.app_id,
            cluster_name: model.cluster_name,
            namespace_name: model.namespace_name,
            locked_by: model.locked_by,
            locked_at: model.locked_at.and_utc().timestamp_millis(),
            data_change_created_by: model.data_change_created_by,
            data_change_created_time: model.data_change_created_time.and_utc().timestamp_millis(),
            data_change_last_modified_by: model.data_change_last_modified_by,
            data_change_last_time: model.data_change_last_time.map(|t| t.and_utc().timestamp_millis()),
        }
    }
}

#[async_trait]
impl NamespaceLockPersistence for NamespaceLockSqlPersistence {
    async fn lock(&self, lock: StoredNamespaceLock) -> anyhow::Result<StoredNamespaceLock> {
        // Check if already locked
        let existing = self
            .get(&lock.app_id, &lock.cluster_name, &lock.namespace_name)
            .await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("Namespace is already locked"));
        }

        let active_model: apollo_namespace_lock::ActiveModel = lock.into();
        let result = apollo_namespace_lock::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_namespace_lock::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created namespace lock"))?;
        Ok(model.into())
    }

    async fn unlock(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<()> {
        apollo_namespace_lock::Entity::delete_many()
            .filter(apollo_namespace_lock::Column::AppId.eq(app_id))
            .filter(apollo_namespace_lock::Column::ClusterName.eq(cluster_name))
            .filter(apollo_namespace_lock::Column::NamespaceName.eq(namespace_name))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn get(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespaceLock>> {
        let result = apollo_namespace_lock::Entity::find()
            .filter(apollo_namespace_lock::Column::AppId.eq(app_id))
            .filter(apollo_namespace_lock::Column::ClusterName.eq(cluster_name))
            .filter(apollo_namespace_lock::Column::NamespaceName.eq(namespace_name))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn is_locked(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<bool> {
        let result = self.get(app_id, cluster_name, namespace_name).await?;
        Ok(result.is_some())
    }
}