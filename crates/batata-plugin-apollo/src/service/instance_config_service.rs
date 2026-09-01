use std::sync::Arc;

use crate::api::dto::InstanceConfigDTO;
use crate::entity::apollo_instance_config;
use crate::persistence::traits::{ApolloPersistenceService, InstanceConfigPersistence};

/// Represents the `InstanceConfigService` entity.
pub struct InstanceConfigService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl InstanceConfigService {
    /// Creates a new `InstanceConfigService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Creates a new resource.
    pub async fn create_or_update(&self, dto: InstanceConfigDTO) -> Result<InstanceConfigDTO, anyhow::Error> {
        let model = self.persistence.create_or_update_instance_config(dto).await?;
        Ok(self.model_to_dto(&model))
    }

    /// Returns the requested value.
    pub async fn get_by_instance(&self, instance_id: i64) -> Result<Vec<InstanceConfigDTO>, anyhow::Error> {
        let models = self.persistence.get_instance_config_by_instance(instance_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    /// Returns the requested value.
    pub async fn list_by_app_cluster(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Vec<InstanceConfigDTO>, anyhow::Error> {
        let models = self.persistence.list_instance_config_by_app_cluster(app_id, cluster_name, namespace_name).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    /// Deletes the specified resource.
    pub async fn delete_by_instance(&self, instance_id: i64) -> Result<(), anyhow::Error> {
        self.persistence.delete_instance_config_by_instance(instance_id).await?;
        Ok(())
    }

    fn model_to_dto(&self, model: &apollo_instance_config::Model) -> InstanceConfigDTO {
        InstanceConfigDTO {
            id: Some(model.id),
            instance_id: model.instance_id,
            config_app_id: Some(model.config_app_id.clone()),
            namespace_name: model.namespace_name.clone(),
            cluster_name: model.cluster_name.clone(),
            release_key: model.release_key.clone(),
            configurations: model.configurations.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
            data_change_last_time: model.data_change_last_time.map(|t| t.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
