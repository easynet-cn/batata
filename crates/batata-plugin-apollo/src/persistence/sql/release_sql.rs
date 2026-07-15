use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_release;
use crate::persistence::shared::StoredRelease;
use crate::persistence::traits::ReleasePersistence;

pub struct ReleaseSqlPersistence {
    db: DatabaseConnection,
}

impl ReleaseSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredRelease> for apollo_release::ActiveModel {
    fn from(release: StoredRelease) -> Self {
        Self {
            id: Set(release.id),
            release_key: Set(release.release_key),
            name: Set(release.name),
            comment: Set(release.comment),
            app_id: Set(release.app_id),
            cluster_name: Set(release.cluster_name),
            namespace_name: Set(release.namespace_name),
            configurations: Set(release.configurations),
            release_id: Set(release.release_id),
            is_abandoned: Set(release.is_abandoned),
            is_deleted: Set(release.is_deleted),
            deleted_at: Set(release.deleted_at),
            data_change_created_by: Set(release.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                release.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(release.data_change_last_modified_by),
            data_change_last_time: Set(release.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_release::Model> for StoredRelease {
    fn from(model: apollo_release::Model) -> Self {
        Self {
            id: model.id,
            release_key: model.release_key,
            name: model.name,
            comment: model.comment,
            app_id: model.app_id,
            cluster_name: model.cluster_name,
            namespace_name: model.namespace_name,
            configurations: model.configurations,
            release_id: model.release_id,
            is_abandoned: model.is_abandoned,
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
impl ReleasePersistence for ReleaseSqlPersistence {
    async fn create(&self, release: StoredRelease) -> anyhow::Result<StoredRelease> {
        let active_model: apollo_release::ActiveModel = release.into();
        let result = apollo_release::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_release::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created release"))?;
        Ok(model.into())
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredRelease>> {
        let result = apollo_release::Entity::find_by_id(id)
            .filter(apollo_release::Column::IsDeleted.eq(false))
            .filter(apollo_release::Column::IsAbandoned.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredRelease>> {
        let result = apollo_release::Entity::find()
            .filter(apollo_release::Column::AppId.eq(app_id))
            .filter(apollo_release::Column::ClusterName.eq(cluster_name))
            .filter(apollo_release::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_release::Column::IsDeleted.eq(false))
            .filter(apollo_release::Column::IsAbandoned.eq(false))
            .order_by_desc(apollo_release::Column::Id)
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredRelease>> {
        let results = apollo_release::Entity::find()
            .filter(apollo_release::Column::AppId.eq(app_id))
            .filter(apollo_release::Column::ClusterName.eq(cluster_name))
            .filter(apollo_release::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_release::Column::IsDeleted.eq(false))
            .filter(apollo_release::Column::IsAbandoned.eq(false))
            .order_by_desc(apollo_release::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let existing = apollo_release::Entity::find_by_id(id)
            .filter(apollo_release::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Release not found: {}", id))?;

        let mut active_model: apollo_release::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }

    async fn get_by_release_id(&self, release_id: i64) -> anyhow::Result<Option<StoredRelease>> {
        let result = apollo_release::Entity::find()
            .filter(apollo_release::Column::ReleaseId.eq(Some(release_id)))
            .filter(apollo_release::Column::IsDeleted.eq(false))
            .filter(apollo_release::Column::IsAbandoned.eq(false))
            .order_by_desc(apollo_release::Column::Id)
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }
}