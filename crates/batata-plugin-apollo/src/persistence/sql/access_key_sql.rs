use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_access_key;
use crate::persistence::shared::StoredAccessKey;
use crate::persistence::traits::AccessKeyPersistence;

pub struct AccessKeySqlPersistence {
    db: DatabaseConnection,
}

impl AccessKeySqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredAccessKey> for apollo_access_key::ActiveModel {
    fn from(access_key: StoredAccessKey) -> Self {
        Self {
            id: Set(access_key.id),
            app_id: Set(access_key.app_id),
            secret: Set(access_key.secret),
            mode: Set(access_key.mode),
            is_enabled: Set(access_key.is_enabled),
            is_deleted: Set(access_key.is_deleted),
            deleted_at: Set(access_key.deleted_at),
            data_change_created_by: Set(access_key.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                access_key.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(access_key.data_change_last_modified_by),
            data_change_last_time: Set(access_key.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_access_key::Model> for StoredAccessKey {
    fn from(model: apollo_access_key::Model) -> Self {
        Self {
            id: model.id,
            app_id: model.app_id,
            secret: model.secret,
            mode: model.mode,
            is_enabled: model.is_enabled,
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
impl AccessKeyPersistence for AccessKeySqlPersistence {
    async fn create(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        let active_model: apollo_access_key::ActiveModel = access_key.into();
        let result = apollo_access_key::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_access_key::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created access key"))?;
        Ok(model.into())
    }

    async fn get_by_app(&self, app_id: &str) -> anyhow::Result<Vec<StoredAccessKey>> {
        let results = apollo_access_key::Entity::find()
            .filter(apollo_access_key::Column::AppId.eq(app_id))
            .filter(apollo_access_key::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_access_key::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn get_by_secret(&self, secret: &str) -> anyhow::Result<Option<StoredAccessKey>> {
        let result = apollo_access_key::Entity::find()
            .filter(apollo_access_key::Column::Secret.eq(secret))
            .filter(apollo_access_key::Column::IsDeleted.eq(false))
            .filter(apollo_access_key::Column::IsEnabled.eq(true))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn update(&self, access_key: StoredAccessKey) -> anyhow::Result<StoredAccessKey> {
        let existing = apollo_access_key::Entity::find_by_id(access_key.id)
            .filter(apollo_access_key::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Access key not found: {}", access_key.id))?;

        let mut active_model: apollo_access_key::ActiveModel = existing.into();
        active_model.is_enabled = Set(access_key.is_enabled);
        active_model.data_change_last_modified_by = Set(access_key.data_change_last_modified_by);
        active_model.data_change_last_time = Set(access_key.data_change_last_time.and_then(|t| {
            chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
        }));

        let updated = active_model.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let existing = apollo_access_key::Entity::find_by_id(id)
            .filter(apollo_access_key::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Access key not found: {}", id))?;

        let mut active_model: apollo_access_key::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }
}