use std::sync::Arc;

use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, Set, ActiveModelTrait, ColumnTrait, QueryOrder};
use crate::entity::apollo_app_namespace;
use crate::api::dto::AppNamespaceDTO;
use crate::persistence::traits::ApolloPersistenceService;
use chrono::Utc;

pub struct AppNamespaceService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AppNamespaceService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    fn db(&self) -> Result<DatabaseConnection, anyhow::Error> {
        self.persistence.get_db_connection().ok_or_else(|| anyhow::anyhow!("Database connection not available"))
    }

    pub async fn create(&self, dto: AppNamespaceDTO) -> Result<AppNamespaceDTO, anyhow::Error> {
        let db = self.db()?;
        let existing = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(&dto.app_id))
            .filter(apollo_app_namespace::Column::Name.eq(&dto.name))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .one(&db)
            .await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("AppNamespace already exists: {}/{}", dto.app_id, dto.name));
        }

        let now = Utc::now().naive_utc();
        let created_by = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());

        let active_model = apollo_app_namespace::ActiveModel {
            app_id: Set(dto.app_id.clone()),
            name: Set(dto.name.clone()),
            format: Set(dto.format.clone()),
            is_public: Set(dto.is_public),
            comment: Set(dto.comment.clone()),
            is_deleted: Set(false),
            deleted_at: Set(0),
            data_change_created_by: Set(created_by),
            data_change_created_time: Set(now),
            data_change_last_modified_by: Set(None),
            data_change_last_time: Set(Some(now)),
            ..Default::default()
        };

        let model = active_model.insert(&db).await?;
        Ok(self.model_to_dto(&model))
    }

    pub async fn get(&self, app_id: &str, name: &str) -> Result<Option<AppNamespaceDTO>, anyhow::Error> {
        let db = self.db()?;
        let model = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(app_id))
            .filter(apollo_app_namespace::Column::Name.eq(name))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .one(&db)
            .await?;

        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list_by_app(&self, app_id: &str) -> Result<Vec<AppNamespaceDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(app_id))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_app_namespace::Column::DataChangeCreatedTime)
            .all(&db)
            .await?;

        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn list_public(&self) -> Result<Vec<AppNamespaceDTO>, anyhow::Error> {
        let db = self.db()?;
        let models = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::IsPublic.eq(true))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .order_by_desc(apollo_app_namespace::Column::DataChangeCreatedTime)
            .all(&db)
            .await?;

        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, app_id: &str, name: &str, operator: &str) -> Result<(), anyhow::Error> {
        let db = self.db()?;
        let model = apollo_app_namespace::Entity::find()
            .filter(apollo_app_namespace::Column::AppId.eq(app_id))
            .filter(apollo_app_namespace::Column::Name.eq(name))
            .filter(apollo_app_namespace::Column::IsDeleted.eq(false))
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("AppNamespace not found: {}/{}", app_id, name))?;

        let now = Utc::now().naive_utc();
        let mut active_model: apollo_app_namespace::ActiveModel = model.into();
        active_model.is_deleted = Set(true);
        active_model.deleted_at = Set(now.and_utc().timestamp_millis());
        active_model.data_change_last_modified_by = Set(Some(operator.to_string()));
        active_model.data_change_last_time = Set(Some(now));
        active_model.update(&db).await?;

        Ok(())
    }

    fn model_to_dto(&self, model: &apollo_app_namespace::Model) -> AppNamespaceDTO {
        AppNamespaceDTO {
            id: Some(model.id),
            name: model.name.clone(),
            app_id: model.app_id.clone(),
            format: model.format.clone(),
            is_public: model.is_public,
            comment: model.comment.clone(),
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(model.data_change_created_time.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()),
        }
    }
}
