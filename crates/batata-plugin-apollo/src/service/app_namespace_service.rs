use std::sync::Arc;

use crate::api::dto::AppNamespaceDTO;
use crate::entity::apollo_app_namespace;
use crate::persistence::traits::{ApolloPersistenceService, AppNamespacePersistence};

pub struct AppNamespaceService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AppNamespaceService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, dto: AppNamespaceDTO) -> Result<AppNamespaceDTO, anyhow::Error> {
        use crate::persistence::shared::StoredNamespace;
        use crate::persistence::traits::{ClusterPersistence, NamespacePersistence};

        let model = self.persistence.create_app_namespace(dto.clone()).await?;

        // Upstream portal instantiates the namespace under every root cluster
        // of the owning app so items/releases can be authored immediately.
        let operator = dto
            .data_change_created_by
            .clone()
            .unwrap_or_else(|| "apollo".to_string());
        for cluster in ClusterPersistence::list(&self.persistence, &dto.app_id)
            .await
            .unwrap_or_default()
        {
            if cluster.is_deleted || cluster.parent_cluster_id != 0 {
                continue;
            }
            if self
                .persistence
                .get_by_app_cluster(&dto.app_id, &cluster.name, &dto.name)
                .await?
                .is_some()
            {
                continue;
            }
            let now = chrono::Utc::now().timestamp_millis();
            NamespacePersistence::create(
                &self.persistence,
                StoredNamespace {
                    id: 0,
                    app_id: dto.app_id.clone(),
                    cluster_name: cluster.name.clone(),
                    namespace_name: dto.name.clone(),
                    format: dto.format.clone(),
                    is_public: dto.is_public,
                    comment: Some(dto.comment.clone()),
                    is_deleted: false,
                    deleted_at: 0,
                    data_change_created_by: operator.clone(),
                    data_change_created_time: now,
                    data_change_last_modified_by: None,
                    data_change_last_time: Some(now),
                },
            )
            .await?;
        }

        Ok(self.model_to_dto(&model))
    }

    pub async fn get(&self, app_id: &str, name: &str) -> Result<Option<AppNamespaceDTO>, anyhow::Error> {
        let model = self.persistence.get_app_namespace(app_id, name).await?;
        Ok(model.map(|m| self.model_to_dto(&m)))
    }

    pub async fn list_by_app(&self, app_id: &str) -> Result<Vec<AppNamespaceDTO>, anyhow::Error> {
        let models = self.persistence.list_app_namespace_by_app(app_id).await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn list_public(&self) -> Result<Vec<AppNamespaceDTO>, anyhow::Error> {
        let models = self.persistence.list_public_app_namespace().await?;
        Ok(models.iter().map(|m| self.model_to_dto(m)).collect())
    }

    pub async fn delete(&self, app_id: &str, name: &str, operator: &str) -> Result<(), anyhow::Error> {
        self.persistence.delete_app_namespace(app_id, name, operator).await?;
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
