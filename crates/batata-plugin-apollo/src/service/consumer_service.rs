use std::sync::Arc;

use crate::api::dto::ConsumerDTO;
use crate::entity::apollo_consumer;
use crate::persistence::traits::{ApolloPersistenceService, ConsumerPersistence};

pub struct ConsumerService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConsumerService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, dto: ConsumerDTO) -> Result<ConsumerDTO, anyhow::Error> {
        let model = self.persistence.create_consumer(dto).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn get(&self, id: i32) -> Result<Option<ConsumerDTO>, anyhow::Error> {
        let model = self.persistence.get_consumer(id).await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn get_by_app(&self, app_id: &str) -> Result<Option<ConsumerDTO>, anyhow::Error> {
        let model = self.persistence.get_consumer_by_app(app_id).await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list(&self) -> Result<Vec<ConsumerDTO>, anyhow::Error> {
        let models = self.persistence.list_consumers().await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    fn model_to_dto(&self, model: &apollo_consumer::Model) -> ConsumerDTO {
        ConsumerDTO {
            id: Some(model.id),
            app_id: model.app_id.clone(),
            name: model.name.clone(),
            org_id: model.org_id.clone(),
            org_name: model.org_name.clone(),
            owner_name: model.owner_name.clone(),
            owner_email: model.owner_email.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
