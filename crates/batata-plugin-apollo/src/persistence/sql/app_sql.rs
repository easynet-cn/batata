use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_app;
use crate::persistence::shared::StoredApp;
use crate::persistence::traits::AppPersistence;

pub struct AppSqlPersistence {
    db: DatabaseConnection,
}

impl AppSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredApp> for apollo_app::ActiveModel {
    fn from(app: StoredApp) -> Self {
        Self {
            id: Set(0), // Will be auto-generated
            app_id: Set(app.app_id),
            name: Set(app.name),
            org_id: Set(app.org_id),
            org_name: Set(app.org_name),
            owner_name: Set(app.owner_name),
            owner_email: Set(app.owner_email),
            is_deleted: Set(app.is_deleted),
            deleted_at: Set(app.deleted_at),
            data_change_created_by: Set(app.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                app.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(app.data_change_last_modified_by),
            data_change_last_time: Set(app.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_app::Model> for StoredApp {
    fn from(model: apollo_app::Model) -> Self {
        Self {
            app_id: model.app_id,
            name: model.name,
            org_id: model.org_id,
            org_name: model.org_name,
            owner_name: model.owner_name,
            owner_email: model.owner_email,
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
impl AppPersistence for AppSqlPersistence {
    async fn create(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
        let active_model: apollo_app::ActiveModel = app.into();
        let result = apollo_app::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_app::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created app"))?;
        Ok(model.into())
    }

    async fn get(&self, app_id: &str) -> anyhow::Result<Option<StoredApp>> {
        let result = apollo_app::Entity::find()
            .filter(apollo_app::Column::AppId.eq(app_id))
            .filter(apollo_app::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn get_by_ids(&self, app_ids: &[String]) -> anyhow::Result<Vec<StoredApp>> {
        let results = apollo_app::Entity::find()
            .filter(apollo_app::Column::AppId.is_in(app_ids.to_vec()))
            .filter(apollo_app::Column::IsDeleted.eq(false))
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn list(&self) -> anyhow::Result<Vec<StoredApp>> {
        let results = apollo_app::Entity::find()
            .filter(apollo_app::Column::IsDeleted.eq(false))
            .order_by_asc(apollo_app::Column::AppId)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn update(&self, app: StoredApp) -> anyhow::Result<StoredApp> {
        let existing = apollo_app::Entity::find()
            .filter(apollo_app::Column::AppId.eq(&app.app_id))
            .filter(apollo_app::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("App not found: {}", app.app_id))?;

        let mut active_model: apollo_app::ActiveModel = existing.into();
        active_model.name = Set(app.name);
        active_model.org_id = Set(app.org_id);
        active_model.org_name = Set(app.org_name);
        active_model.owner_name = Set(app.owner_name);
        active_model.owner_email = Set(app.owner_email);
        active_model.data_change_last_modified_by = Set(app.data_change_last_modified_by);
        active_model.data_change_last_time = Set(app.data_change_last_time.and_then(|t| {
            chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
        }));

        let updated = active_model.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete(&self, app_id: &str) -> anyhow::Result<()> {
        let existing = apollo_app::Entity::find()
            .filter(apollo_app::Column::AppId.eq(app_id))
            .filter(apollo_app::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("App not found: {}", app_id))?;

        let mut active_model: apollo_app::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }
}