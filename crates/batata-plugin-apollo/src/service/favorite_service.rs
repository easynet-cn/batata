use std::sync::Arc;

use crate::api::dto::FavoriteDTO;
use crate::entity::apollo_favorite;
use crate::persistence::traits::{ApolloPersistenceService, FavoritePersistence};

/// Represents the `FavoriteService` entity.
pub struct FavoriteService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl FavoriteService {
    /// Creates a new `FavoriteService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Performs the `create` operation.
    pub async fn create(&self, dto: FavoriteDTO) -> Result<FavoriteDTO, anyhow::Error> {
        let model = self.persistence.create_favorite(dto).await?;
        Ok(self.model_to_dto(&model))
    }

    /// Returns the requested value.
    pub async fn list_by_user(&self, user_id: &str) -> Result<Vec<FavoriteDTO>, anyhow::Error> {
        let models = self.persistence.list_favorite_by_user(user_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    /// Performs the `delete` operation.
    pub async fn delete(&self, id: i32, user_id: &str) -> Result<(), anyhow::Error> {
        self.persistence.delete_favorite(id, user_id).await?;
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
