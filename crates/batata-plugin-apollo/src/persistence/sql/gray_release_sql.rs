use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_gray_release_rule;
use crate::persistence::shared::StoredGrayReleaseRule;
use crate::persistence::traits::GrayReleasePersistence;

pub struct GrayReleaseSqlPersistence {
    db: DatabaseConnection,
}

impl GrayReleaseSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredGrayReleaseRule> for apollo_gray_release_rule::ActiveModel {
    fn from(rule: StoredGrayReleaseRule) -> Self {
        Self {
            id: Set(rule.id),
            app_id: Set(rule.app_id),
            cluster_name: Set(rule.cluster_name),
            namespace_name: Set(rule.namespace_name),
            branch_name: Set(rule.branch_name),
            rules: Set(rule.rules),
            release_id: Set(rule.release_id),
            branch_status: Set(rule.branch_status),
            is_deleted: Set(rule.is_deleted),
            deleted_at: Set(rule.deleted_at),
            data_change_created_by: Set(rule.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                rule.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(rule.data_change_last_modified_by),
            data_change_last_time: Set(rule.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_gray_release_rule::Model> for StoredGrayReleaseRule {
    fn from(model: apollo_gray_release_rule::Model) -> Self {
        Self {
            id: model.id,
            app_id: model.app_id,
            cluster_name: model.cluster_name,
            namespace_name: model.namespace_name,
            branch_name: model.branch_name,
            rules: model.rules,
            release_id: model.release_id,
            branch_status: model.branch_status,
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
impl GrayReleasePersistence for GrayReleaseSqlPersistence {
    async fn create(&self, rule: StoredGrayReleaseRule) -> anyhow::Result<StoredGrayReleaseRule> {
        let active_model: apollo_gray_release_rule::ActiveModel = rule.into();
        let result = apollo_gray_release_rule::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_gray_release_rule::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created gray release rule"))?;
        Ok(model.into())
    }

    async fn get_by_namespace(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> anyhow::Result<Option<StoredGrayReleaseRule>> {
        let result = apollo_gray_release_rule::Entity::find()
            .filter(apollo_gray_release_rule::Column::AppId.eq(app_id))
            .filter(apollo_gray_release_rule::Column::ClusterName.eq(cluster_name))
            .filter(apollo_gray_release_rule::Column::NamespaceName.eq(namespace_name))
            .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn update_rules(
        &self,
        id: i32,
        rules: String,
        release_id: i64,
    ) -> anyhow::Result<StoredGrayReleaseRule> {
        let existing = apollo_gray_release_rule::Entity::find_by_id(id)
            .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Gray release rule not found: {}", id))?;

        let mut active_model: apollo_gray_release_rule::ActiveModel = existing.into();
        active_model.rules = Set(rules);
        active_model.release_id = Set(release_id);
        active_model.data_change_last_time = Set(Some(chrono::Utc::now().naive_utc()));

        let updated = active_model.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let existing = apollo_gray_release_rule::Entity::find_by_id(id)
            .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Gray release rule not found: {}", id))?;

        let mut active_model: apollo_gray_release_rule::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }

    async fn list_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredGrayReleaseRule>> {
        let results = apollo_gray_release_rule::Entity::find()
            .filter(apollo_gray_release_rule::Column::AppId.eq(app_id))
            .filter(apollo_gray_release_rule::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_gray_release_rule::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }
}