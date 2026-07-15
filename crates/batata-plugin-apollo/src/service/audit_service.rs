use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait, QueryOrder, PaginatorTrait};
use crate::entity::apollo_audit;
use crate::api::dto::AuditDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct AuditService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AuditService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, dto: AuditDTO) -> Result<AuditDTO, anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "system".to_string());

        let active_model = apollo_audit::ActiveModel {
            audit_key: Set(dto.audit_key),
            entity_name: Set(dto.entity_name),
            entity_id: Set(dto.entity_id),
            op_name: Set(dto.op_name),
            op_time: Set(now),
            op_by: Set(dto.op_by),
            op_client_ip: Set(dto.op_client_ip),
            detail: Set(dto.detail),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn list(&self, page: u64, size: u64) -> Result<(Vec<AuditDTO>, u64), anyhow::Error> {
        let db = self.db()?;
        let query = apollo_audit::Entity::find()
            .order_by_desc(apollo_audit::Column::OpTime);

        let total = query.clone().count(&db).await?;
        let models = query.paginate(&db, size).fetch_page(page).await?;

        Ok((models.iter().map(|m| self.model_to_dto(m)).collect(), total))
    }

    pub async fn list_by_entity(&self, entity_name: &str, entity_id: &str) -> Result<Vec<AuditDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_audit::Entity::find()
            .filter(apollo_audit::Column::EntityName.eq(entity_name))
            .filter(apollo_audit::Column::EntityId.eq(entity_id))
            .order_by_desc(apollo_audit::Column::OpTime)
            .all(&db)
            .await?;

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
