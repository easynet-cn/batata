use std::sync::Arc;

use crate::api::dto::ServerConfigDTO;
use crate::entity::apollo_server_config;
use crate::persistence::traits::{ApolloPersistenceService, ServerConfigPersistence};

/// Represents the `ServerConfigService` entity.
pub struct ServerConfigService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ServerConfigService {
    /// Creates a new `ServerConfigService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Performs the `get` operation.
    pub async fn get(&self, key: &str) -> Result<Option<ServerConfigDTO>, anyhow::Error> {
        let model = self.persistence.get_server_config(key).await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    /// Performs the `list` operation.
    pub async fn list(&self) -> Result<Vec<ServerConfigDTO>, anyhow::Error> {
        let models = self.persistence.list_server_config().await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    /// Performs the `create` operation.
    pub async fn create(&self, dto: ServerConfigDTO) -> Result<ServerConfigDTO, anyhow::Error> {
        let model = self.persistence.create_server_config(dto).await?;
        Ok(self.model_to_dto(&model))
    }

    /// Performs the `update` operation.
    pub async fn update(&self, key: &str, value: &str, operator: &str) -> Result<(), anyhow::Error> {
        self.persistence.update_server_config(key, value, operator).await?;
        Ok(())
    }

    /// Performs the `delete` operation.
    pub async fn delete(&self, key: &str, operator: &str) -> Result<(), anyhow::Error> {
        self.persistence.delete_server_config(key, operator).await?;
        Ok(())
    }

    fn model_to_dto(&self, model: &apollo_server_config::Model) -> ServerConfigDTO {
        ServerConfigDTO {
            key: model.key.clone(),
            value: model.value.clone(),
            comment: model.comment.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
