use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait};
use crate::entity::apollo_consumer_token;
use crate::api::dto::ConsumerTokenDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct ConsumerTokenService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConsumerTokenService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, consumer_id: i32, created_by: &str) -> Result<ConsumerTokenDTO, anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let token = format!("{}-{}", consumer_id, now.and_utc().timestamp_millis());

        let active_model = apollo_consumer_token::ActiveModel {
            consumer_id: Set(consumer_id),
            token: Set(token),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn list_by_consumer(&self, consumer_id: i32) -> Result<Vec<ConsumerTokenDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_consumer_token::Entity::find()
            .filter(apollo_consumer_token::Column::ConsumerId.eq(consumer_id))
            .all(&db)
            .await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, id: i32) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let model = apollo_consumer_token::Entity::find_by_id(id)
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Token not found: {}", id))?;

        let active_model: apollo_consumer_token::ActiveModel = model.into();
        active_model.delete(&db).await?;
        Ok(())
    }

    pub async fn get_by_token(&self, token: &str) -> Result<Option<ConsumerTokenDTO>, anyhow::Error> {
        let db = self.db()?;
        let model = apollo_consumer_token::Entity::find()
            .filter(apollo_consumer_token::Column::Token.eq(token))
            .one(&db)
            .await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    fn model_to_dto(&self, model: &apollo_consumer_token::Model) -> ConsumerTokenDTO {
        ConsumerTokenDTO {
            id: Some(model.id),
            consumer_id: model.consumer_id,
            token: model.token.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
