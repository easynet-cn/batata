use std::sync::Arc;

use crate::api::dto::AuditDTO;
use crate::entity::apollo_audit;
use crate::persistence::traits::{ApolloPersistenceService, AuditPersistence};

/// Represents the `AuditService` entity.
pub struct AuditService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AuditService {
    /// Creates a new `AuditService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Performs the `create` operation.
    pub async fn create(&self, dto: AuditDTO) -> Result<AuditDTO, anyhow::Error> {
        let model = self.persistence.create_audit(dto).await?;
        Ok(self.model_to_dto(&model))
    }

    /// Performs the `list` operation.
    pub async fn list(&self, page: u64, size: u64) -> Result<(Vec<AuditDTO>, u64), anyhow::Error> {
        let (models, total) = self.persistence.list_audit(page, size).await?;
        Ok((models.iter().map(|m| self.model_to_dto(m)).collect(), total))
    }

    /// Returns the requested value.
    pub async fn list_by_entity(&self, entity_name: &str, entity_id: &str) -> Result<Vec<AuditDTO>, anyhow::Error> {
        let models = self.persistence.list_audit_by_entity(entity_name, entity_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    fn model_to_dto(&self, model: &apollo_audit::Model) -> AuditDTO {
        AuditDTO {
            id: Some(model.id),
            audit_key: model.audit_key.clone(),
            entity_name: model.entity_name.clone(),
            entity_id: model.entity_id.clone(),
            op_name: model.op_name.clone(),
            op_time: model.op_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string(),
            op_by: model.op_by.clone(),
            op_client_ip: model.op_client_ip.clone(),
            detail: model.detail.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
        }
    }
}
