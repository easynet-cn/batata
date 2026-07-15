use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait, QueryOrder};
use crate::entity::apollo_favorite;
use crate::api::dto::FavoriteDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct FavoriteService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl FavoriteService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, dto: FavoriteDTO) -> Result<FavoriteDTO, anyhow::Error> {
        let db = self.db()?;
        let existing = apollo_favorite::Entity::find()
            .filter(apollo_favorite::Column::UserId.eq(&dto.user_id))
            .filter(apollo_favorite::Column::AppId.eq(&dto.app_id))
            .one(&db)
            .await?;

        if let Some(model) = existing {
            return Ok(self.model_to_dto(&model));
        }

        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| dto.user_id.clone());

        let active_model = apollo_favorite::ActiveModel {
            user_id: Set(dto.user_id),
            app_id: Set(dto.app_id),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn list_by_user(&self, user_id: &str) -> Result<Vec<FavoriteDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_favorite::Entity::find()
            .filter(apollo_favorite::Column::UserId.eq(user_id))
            .order_by_desc(apollo_favorite::Column::DataChangeCreatedTime)
            .all(&db)
            .await?;

        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, id: i32, user_id: &str) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let model = apollo_favorite::Entity::find_by_id(id)
            .filter(apollo_favorite::Column::UserId.eq(user_id))
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Favorite not found: {}", id))?;

        let active_model: apollo_favorite::ActiveModel = model.into();
        active_model.delete(&db).await?;
        Ok(())
    }

    fn model_to_dto(&self, model: &apollo_favorite::Model) -> FavoriteDTO {
        FavoriteDTO {
            id: Some(model.id),
            user_id: model.user_id.clone(),
            app_id: model.app_id.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
