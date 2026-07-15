use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait};
use crate::entity::apollo_consumer;
use crate::api::dto::ConsumerDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct ConsumerService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ConsumerService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, dto: ConsumerDTO) -> Result<ConsumerDTO, anyhow::Error> {
        let db = self.db()?;
        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());

        let active_model = apollo_consumer::ActiveModel {
            app_id: Set(dto.app_id),
            name: Set(dto.name),
            org_id: Set(dto.org_id),
            org_name: Set(dto.org_name),
            owner_name: Set(dto.owner_name),
            owner_email: Set(dto.owner_email),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn get(&self, id: i32) -> Result<Option<ConsumerDTO>, anyhow::Error> {
        let db = self.db()?;
        let model = apollo_consumer::Entity::find_by_id(id)
            .one(&db)
            .await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn get_by_app(&self, app_id: &str) -> Result<Option<ConsumerDTO>, anyhow::Error> {
        let db = self.db()?;
        let model = apollo_consumer::Entity::find()
            .filter(apollo_consumer::Column::AppId.eq(app_id))
            .one(&db)
            .await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list(&self) -> Result<Vec<ConsumerDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_consumer::Entity::find()
            .all(&db)
            .await?;
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
