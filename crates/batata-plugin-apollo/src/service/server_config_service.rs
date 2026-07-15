use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait, QueryOrder};
use crate::entity::apollo_server_config;
use crate::api::dto::ServerConfigDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct ServerConfigService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ServerConfigService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn get(&self, key: &str) -> Result<Option<ServerConfigDTO>, anyhow::Error> {
        let db = self.db()?;
        let model = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(key))
            .one(&db)
            .await?;

        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list(&self) -> Result<Vec<ServerConfigDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_server_config::Entity::find()
            .order_by_asc(apollo_server_config::Column::Key)
            .all(&db)
            .await?;

        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn create(&self, dto: ServerConfigDTO) -> Result<ServerConfigDTO, anyhow::Error> {
        let db = self.db()?;
        let existing = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(&dto.key))
            .one(&db)
            .await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("ServerConfig already exists: {}", dto.key));
        }

        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let active_model = apollo_server_config::ActiveModel {
            key: Set(dto.key.clone()),
            value: Set(dto.value.clone()),
            comment: Set(dto.comment.clone()),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn update(&self, key: &str, value: &str, operator: &str) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let model = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(key))
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ServerConfig not found: {}", key))?;

        let now = Utc::now().naive_utc();
        let mut active_model: apollo_server_config::ActiveModel = model.into();

        active_model.value = Set(value.to_string());
        active_model.data_change_last_modified_by = Set(Some(operator.to_string()));
        active_model.data_change_last_time = Set(Some(now));

        active_model.update(&db).await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str, _operator: &str) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let model = apollo_server_config::Entity::find()
            .filter(apollo_server_config::Column::Key.eq(key))
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ServerConfig not found: {}", key))?;

        let active_model: apollo_server_config::ActiveModel = model.into();
        active_model.delete(&db).await?;
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
