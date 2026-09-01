use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::entity::apollo_release_message;
use crate::persistence::shared::StoredReleaseMessage;
use crate::persistence::traits::ReleaseMessagePersistence;

/// Represents the `ReleaseMessageSqlPersistence` entity.
pub struct ReleaseMessageSqlPersistence {
    db: DatabaseConnection,
}

impl ReleaseMessageSqlPersistence {
    /// Creates a new `ReleaseMessageSqlPersistence`.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<StoredReleaseMessage> for apollo_release_message::ActiveModel {
    fn from(message: StoredReleaseMessage) -> Self {
        Self {
            id: if message.id == 0 { sea_orm::ActiveValue::NotSet } else { sea_orm::Set(message.id) },
            message: Set(message.message),
            data_change_created_time: Set(chrono::DateTime::from_timestamp_millis(
                message.data_change_created_time,
            )
            .unwrap_or_default()
            .naive_utc()),
        }
    }
}

impl From<apollo_release_message::Model> for StoredReleaseMessage {
    fn from(model: apollo_release_message::Model) -> Self {
        Self {
            id: model.id,
            message: model.message,
            data_change_created_time: model.data_change_created_time.and_utc().timestamp_millis(),
        }
    }
}

#[async_trait]
impl ReleaseMessagePersistence for ReleaseMessageSqlPersistence {
    async fn create(&self, message: StoredReleaseMessage) -> anyhow::Result<StoredReleaseMessage> {
        let active_model: apollo_release_message::ActiveModel = message.into();
        let result = apollo_release_message::Entity::insert(active_model)
            .exec(&self.db)
            .await?;
        let model = apollo_release_message::Entity::find_by_id(result.last_insert_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to fetch created release message"))?;
        Ok(model.into())
    }

    async fn find_latest_by_message(
        &self,
        message: &str,
    ) -> anyhow::Result<Option<StoredReleaseMessage>> {
        let result = apollo_release_message::Entity::find()
            .filter(apollo_release_message::Column::Message.eq(message))
            .order_by_desc(apollo_release_message::Column::Id)
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn get_latest(&self) -> anyhow::Result<Option<StoredReleaseMessage>> {
        let result = apollo_release_message::Entity::find()
            .order_by_desc(apollo_release_message::Column::Id)
            .one(&self.db)
            .await?;
        Ok(result.map(|m| m.into()))
    }

    async fn list_all(&self) -> anyhow::Result<Vec<StoredReleaseMessage>> {
        let results = apollo_release_message::Entity::find()
            .order_by_desc(apollo_release_message::Column::Id)
            .all(&self.db)
            .await?;
        Ok(results.into_iter().map(|m| m.into()).collect())
    }

    async fn delete_by_id(&self, id: i64) -> anyhow::Result<()> {
        apollo_release_message::Entity::delete_by_id(id)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn delete_old(&self, before_id: i64) -> anyhow::Result<usize> {
        let results = apollo_release_message::Entity::find()
            .filter(apollo_release_message::Column::Id.lt(before_id))
            .all(&self.db)
            .await?;

        let count = results.len();
        for model in results {
            apollo_release_message::Entity::delete_by_id(model.id)
                .exec(&self.db)
                .await?;
        }

        Ok(count)
    }
}