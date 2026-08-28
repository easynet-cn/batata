use std::sync::Arc;

use crate::api::dto::PermissionDTO;
use crate::entity::apollo_permission;
use crate::persistence::traits::{ApolloPersistenceService, PermissionPersistence};

pub struct PermissionService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl PermissionService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, permission_type: i32, target_id: &str, created_by: &str) -> Result<PermissionDTO, anyhow::Error> {
        let model = self.persistence.create_permission(permission_type, target_id, created_by).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn list_by_target(&self, target_id: &str) -> Result<Vec<PermissionDTO>, anyhow::Error> {
        let models = self.persistence.list_permission_by_target(target_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn list_by_type(&self, permission_type: i32) -> Result<Vec<PermissionDTO>, anyhow::Error> {
        let models = self.persistence.list_permission_by_type(permission_type).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    fn model_to_dto(&self, model: &apollo_permission::Model) -> PermissionDTO {
        PermissionDTO {
            id: Some(model.id),
            permission_type: model.permission_type,
            target_id: model.target_id.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
