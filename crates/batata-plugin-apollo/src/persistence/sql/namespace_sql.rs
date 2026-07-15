use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_namespace;
use crate::persistence::shared::StoredNamespace;
use crate::persistence::traits::NamespacePersistence;

pub struct NamespaceSqlPersistence {
    db: DatabaseConnection,
}

impl NamespaceSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredNamespace> for apollo_namespace::ActiveModel {
    fn from(namespace: StoredNamespace) -> Self {
        Self {
            id: Set(namespace.id),
            app_id: Set(namespace.app_id),
            cluster_name: Set(namespace.cluster_name),
            namespace_name: Set(namespace.namespace_name),
            format: Set(namespace.format),
            is_public: Set(namespace.is_public),
            comment: Set(namespace.comment),
            is_deleted: Set(namespace.is_deleted),
            deleted_at: Set(namespace.deleted_at),
            data_change_created_by: Set(namespace.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                namespace.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(namespace.data_change_last_modified_by),
            data_change_last_time: Set(namespace.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_namespace::Model> for StoredNamespace {
    fn from(model: apollo_namespace::Model) -> Self {
        Self {
            id: model.id,
            app_id: model.app_id,
            cluster_name: model.cluster_name,
            namespace_name: model.namespace_name,
            format: model.format,
            is_public: model.is_public,
            comment: model.comment,
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
impl NamespacePersistence for NamespaceSqlPersistence {
    async fn create(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        let active_model: apollo_namespace::ActiveModel = namespace.into();
        let result = apollo_namespace::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_namespace::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created namespace"))?;
        Ok(model.into())
    }

    async fn get(&self, id: i32) -> anyhow::Result<Option<StoredNamespace>> {
        let result = apollo_namespace::Entity::find_by_id(id)
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn get_by_app_cluster(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredNamespace>> {
        let result = apollo_namespace::Entity::find()
            .filter(apollo_namespace::Column::AppId.eq(app_id))
            .filter(apollo_namespace::Column::ClusterName.eq(cluster_name))
            .filter(apollo_namespace::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredNamespace>> {
        let results = apollo_namespace::Entity::find()
            .filter(apollo_namespace::Column::AppId.eq(app_id))
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .order_by_asc(apollo_namespace::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn update(&self, namespace: StoredNamespace) -> anyhow::Result<StoredNamespace> {
        let existing = apollo_namespace::Entity::find_by_id(namespace.id)
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}", namespace.id))?;

        let mut active_model: apollo_namespace::ActiveModel = existing.into();
        active_model.format = Set(namespace.format);
        active_model.is_public = Set(namespace.is_public);
        active_model.comment = Set(namespace.comment);
        active_model.data_change_last_modified_by = Set(namespace.data_change_last_modified_by);
        active_model.data_change_last_time = Set(namespace.data_change_last_time.and_then(|t| {
            chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
        }));

        let updated = active_model.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let existing = apollo_namespace::Entity::find_by_id(id)
            .filter(apollo_namespace::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}", id))?;

        let mut active_model: apollo_namespace::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }
}