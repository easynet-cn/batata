use std::sync::Arc;

use crate::api::dto::ConsumerTokenDTO;
use crate::entity::apollo_consumer_token;
use crate::persistence::traits::{ApolloPersistenceService, ConsumerTokenPersistence};

pub struct ConsumerTokenService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConsumerTokenService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, consumer_id: i32, created_by: &str) -> Result<ConsumerTokenDTO, anyhow::Error> {
        let model = ConsumerTokenPersistence::create_consumer_token(&self.persistence, consumer_id, created_by).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn list_by_consumer(&self, consumer_id: i32) -> Result<Vec<ConsumerTokenDTO>, anyhow::Error> {
        let models = ConsumerTokenPersistence::list_tokens_by_consumer(&self.persistence, consumer_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, id: i32) -> Result<(), anyhow::Error> {
        ConsumerTokenPersistence::delete_consumer_token(&self.persistence, id).await?;
        Ok(())
    }

    pub async fn get_by_token(&self, token: &str) -> Result<Option<ConsumerTokenDTO>, anyhow::Error> {
        let model = ConsumerTokenPersistence::get_consumer_token_by_token(&self.persistence, token).await?;
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
