use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait};
use crate::entity::apollo_permission;
use crate::api::dto::PermissionDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct PermissionService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl PermissionService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, permission_type: i32, target_id: &str, created_by: &str) -> Result<PermissionDTO, anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let active_model = apollo_permission::ActiveModel {
            permission_type: Set(permission_type),
            target_id: Set(target_id.to_string()),
            data_change_created_by: Set(created_by.to_string()),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };
        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn list_by_target(&self, target_id: &str) -> Result<Vec<PermissionDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_permission::Entity::find()
            .filter(apollo_permission::Column::TargetId.eq(target_id))
            .all(&db)
            .await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn list_by_type(&self, permission_type: i32) -> Result<Vec<PermissionDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_permission::Entity::find()
            .filter(apollo_permission::Column::PermissionType.eq(permission_type))
            .all(&db)
            .await?;
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
