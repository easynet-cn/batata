use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_cluster;
use crate::persistence::shared::StoredCluster;
use crate::persistence::traits::ClusterPersistence;

pub struct ClusterSqlPersistence {
    db: DatabaseConnection,
}

impl ClusterSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredCluster> for apollo_cluster::ActiveModel {
    fn from(cluster: StoredCluster) -> Self {
        Self {
            id: Set(cluster.id),
            name: Set(cluster.name),
            app_id: Set(cluster.app_id),
            parent_cluster_id: Set(cluster.parent_cluster_id),
            is_deleted: Set(cluster.is_deleted),
            deleted_at: Set(cluster.deleted_at),
            data_change_created_by: Set(cluster.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                cluster.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(cluster.data_change_last_modified_by),
            data_change_last_time: Set(cluster.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_cluster::Model> for StoredCluster {
    fn from(model: apollo_cluster::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            app_id: model.app_id,
            parent_cluster_id: model.parent_cluster_id,
            is_deleted: model.is_deleted,
            deleted_at: model.deleted_at,
            data_change_created_by: model.data_change_created_by,
            data_change_created_time: model.data_change_created_time.and_utc().timestamp_millis(),
            data_change_last_modified_by: model.data_change_last_modified_by,
            data_change_last_time: model.data_change_last_time.map(|t| t.and_utc().timestamp_millis()),
        }
    }
}

#[async_trait]
impl ClusterPersistence for ClusterSqlPersistence {
    async fn create(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        let active_model: apollo_cluster::ActiveModel = cluster.into();
        let result = apollo_cluster::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_cluster::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created cluster"))?;
        Ok(model.into())
    }

    async fn get(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<Option<StoredCluster>> {
        let result = apollo_cluster::Entity::find()
            .filter(apollo_cluster::Column::AppId.eq(app_id))
            .filter(apollo_cluster::Column::Name.eq(cluster_name))
            .filter(apollo_cluster::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn list(&self, app_id: &str) -> anyhow::Result<Vec<StoredCluster>> {
        let results = apollo_cluster::Entity::find()
            .filter(apollo_cluster::Column::AppId.eq(app_id))
            .filter(apollo_cluster::Column::IsDeleted.eq(false))
            .order_by_asc(apollo_cluster::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn update(&self, cluster: StoredCluster) -> anyhow::Result<StoredCluster> {
        let existing = apollo_cluster::Entity::find()
            .filter(apollo_cluster::Column::AppId.eq(&cluster.app_id))
            .filter(apollo_cluster::Column::Name.eq(&cluster.name))
            .filter(apollo_cluster::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cluster not found: {}", cluster.name))?;

        let mut active_model: apollo_cluster::ActiveModel = existing.into();
        active_model.parent_cluster_id = Set(cluster.parent_cluster_id);
        active_model.data_change_last_modified_by = Set(cluster.data_change_last_modified_by);
        active_model.data_change_last_time = Set(cluster.data_change_last_time.and_then(|t| {
            chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
        }));

        let updated = active_model.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete(&self, app_id: &str, cluster_name: &str) -> anyhow::Result<()> {
        let existing = apollo_cluster::Entity::find()
            .filter(apollo_cluster::Column::AppId.eq(app_id))
            .filter(apollo_cluster::Column::Name.eq(cluster_name))
            .filter(apollo_cluster::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Cluster not found: {}", cluster_name))?;

        let mut active_model: apollo_cluster::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }
}
