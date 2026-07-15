use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_item;
use crate::persistence::shared::StoredItem;
use crate::persistence::traits::ItemPersistence;

pub struct ItemSqlPersistence {
    db: DatabaseConnection,
}

impl ItemSqlPersistence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredItem> for apollo_item::ActiveModel {
    fn from(item: StoredItem) -> Self {
        Self {
            id: Set(item.id),
            namespace_id: Set(item.namespace_id),
            key: Set(item.key),
            r#type: Set(item.r#type),
            value: Set(item.value),
            comment: Set(item.comment),
            line_num: Set(item.line_num),
            is_deleted: Set(item.is_deleted),
            deleted_at: Set(item.deleted_at),
            data_change_created_by: Set(item.data_change_created_by),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                item.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
            data_change_last_modified_by: Set(item.data_change_last_modified_by),
            data_change_last_time: Set(item.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
            })),
        }
    }
}

impl From<apollo_item::Model> for StoredItem {
    fn from(model: apollo_item::Model) -> Self {
        Self {
            id: model.id,
            namespace_id: model.namespace_id,
            key: model.key,
            r#type: model.r#type,
            value: model.value,
            comment: model.comment,
            line_num: model.line_num,
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
impl ItemPersistence for ItemSqlPersistence {
    async fn create(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        let active_model: apollo_item::ActiveModel = item.into();
        let result = apollo_item::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_item::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created item"))?;
        Ok(model.into())
    }

    async fn get_by_key(&self, namespace_id: i32, key: &str) -> anyhow::Result<Option<StoredItem>> {
        let result = apollo_item::Entity::find()
            .filter(apollo_item::Column::NamespaceId.eq(namespace_id))
            .filter(apollo_item::Column::Key.eq(key))
            .filter(apollo_item::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn get_by_id(&self, id: i32) -> anyhow::Result<Option<StoredItem>> {
        let result = apollo_item::Entity::find_by_id(id)
            .filter(apollo_item::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn list_by_namespace(&self, namespace_id: i32) -> anyhow::Result<Vec<StoredItem>> {
        let results = apollo_item::Entity::find()
            .filter(apollo_item::Column::NamespaceId.eq(namespace_id))
            .filter(apollo_item::Column::IsDeleted.eq(false))
            .order_by_asc(apollo_item::Column::LineNum)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn update(&self, item: StoredItem) -> anyhow::Result<StoredItem> {
        let existing = apollo_item::Entity::find_by_id(item.id)
            .filter(apollo_item::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", item.id))?;

        let mut active_model: apollo_item::ActiveModel = existing.into();
        active_model.key = Set(item.key);
        active_model.r#type = Set(item.r#type);
        active_model.value = Set(item.value);
        active_model.comment = Set(item.comment);
        active_model.line_num = Set(item.line_num);
        active_model.data_change_last_modified_by = Set(item.data_change_last_modified_by);
        active_model.data_change_last_time = Set(item.data_change_last_time.and_then(|t| {
            chrono::DateTime::from_timestamp_millis(t).map(|dt| dt.naive_utc())
        }));

        let updated = active_model.update(&self.db).await?;
        Ok(updated.into())
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let existing = apollo_item::Entity::find_by_id(id)
            .filter(apollo_item::Column::IsDeleted.eq(false))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Item not found: {}", id))?;

        let mut active_model: apollo_item::ActiveModel = existing.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(chrono::Utc::now().timestamp_millis());
        active_model.update(&self.db).await?;
        Ok(())
    }

    async fn batch_create(&self, items: Vec<StoredItem>) -> anyhow::Result<Vec<StoredItem>> {
        let active_models: Vec<apollo_item::ActiveModel> =
            items.into_iter().map(|item| item.into()).collect();

        if active_models.is_empty() {
            return Ok(Vec::new());
        }

        let results = apollo_item::Entity::insert_many(active_models)
            .exec(&self.db)
            .await?;

        // Fetch all created items
        let created_items = apollo_item::Entity::find()
            .filter(apollo_item::Column::Id.gte(results.last_insert_id as i32 - results.last_insert_id as i32 + 1))
            .filter(apollo_item::Column::Id.lte(results.last_insert_id as i32))
            .filter(apollo_item::Column::IsDeleted.eq(false))
            .all(&self.db)
            .await?;

        Ok(created_items.into_iter().map(|m| m.into()).collect())
    }
}