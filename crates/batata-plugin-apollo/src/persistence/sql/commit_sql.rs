use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_commit;
use crate::persistence::shared::StoredCommit;
use crate::persistence::traits::CommitPersistence;

pub struct CommitSqlPersistence {
    db: DatabaseConnection,
}

impl CommitSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredCommit> for apollo_commit::ActiveModel {
    fn from(commit: StoredCommit) -> Self {
        Self {
            id: Set(commit.id),
            change_sets: Set(commit.change_sets),
            app_id: Set(commit.app_id),
            cluster_name: Set(commit.cluster_name),
            namespace_name: Set(commit.namespace_name),
            comment: Set(commit.comment),
            is_deleted: Set(commit.is_deleted),
            deleted_at: Set(commit.deleted_at),
            data_change_created_by: Set(commit.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                commit.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(commit.data_change_last_modified_by),
            data_change_last_time: Set(commit.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_commit::Model> for StoredCommit {
    fn from(model: apollo_commit::Model) -> Self {
        Self {
            id: model.id,
            change_sets: model.change_sets,
            app_id: model.app_id,
            cluster_name: model.cluster_name,
            namespace_name: model.namespace_name,
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
impl CommitPersistence for CommitSqlPersistence {
    async fn create(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        let active_model: apollo_commit::ActiveModel = commit.into();
        let result = apollo_commit::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_commit::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created commit"))?;
        Ok(model.into())
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredCommit>> {
        let result = apollo_commit::Entity::find_by_id(id)
            .filter(apollo_commit::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn list_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Vec<StoredCommit>> {
        let results = apollo_commit::Entity::find()
            .filter(apollo_commit::Column::AppId.eq(app_id))
            .filter(apollo_commit::Column::ClusterName.eq(cluster_name))
            .filter(apollo_commit::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_commit::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_commit::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn get_latest(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredCommit>> {
        let result = apollo_commit::Entity::find()
            .filter(apollo_commit::Column::AppId.eq(app_id))
            .filter(apollo_commit::Column::ClusterName.eq(cluster_name))
            .filter(apollo_commit::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_commit::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_commit::Column::Id)
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn update(&self, commit: StoredCommit) -> anyhow::Result<StoredCommit> {
        let commit_id = commit.id;
        let active_model: apollo_commit::ActiveModel = commit.into();
        let model = apollo_commit::Entity::update(active_model)
            .filter(apollo_commit::Column::Id.eq(commit_id))
            .exec(&self.db)
            .await?;
        Ok(model.into())
    }
}