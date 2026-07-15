use std::sync::Arc;

use crate::api::dto::NamespaceDTO;
use crate::persistence::shared::StoredNamespace;
use crate::persistence::traits::{ApolloPersistenceService, NamespacePersistence};
use chrono::Utc;

pub struct NamespaceService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl NamespaceService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, app_id: &str, cluster_name: &str, dto: NamespaceDTO) -> Result<NamespaceDTO, anyhow::Error> {
        let existing = self.persistence.get_by_app_cluster(app_id, cluster_name, &dto.namespace_name).await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("Namespace already exists: {}/{}/{}", app_id, cluster_name, dto.namespace_name));
        }

        let now = Utc::now().timestamp_millis();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let stored = StoredNamespace {
            id: 0,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: dto.namespace_name.clone(),
            format: dto.format.unwrap_or_else(|| "properties".to_string()),
            is_public: dto.is_public.unwrap_or(false),
            comment: dto.comment,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        let created = self.persistence.create(stored).await?;
        Ok(created.into())
    }

    pub async fn get(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Option<NamespaceDTO>, anyhow::Error> {
        let stored = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?;
        Ok(stored.map(|s| s.into()))
    }

    pub async fn list(&self, app_id: &str, cluster_name: &str) -> Result<Vec<NamespaceDTO>, anyhow::Error> {
        let stored_list = self.persistence.list_by_app(app_id).await?;
        Ok(stored_list
            .into_iter()
            .filter(|s| s.cluster_name == cluster_name && !s.is_deleted)
            .map(|s| s.into())
            .collect())
    }

    pub async fn update(&self, app_id: &str, cluster_name: &str, namespace_name: &str, dto: NamespaceDTO) -> Result<(), anyhow::Error> {
        let existing = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let now = Utc::now().timestamp_millis();

        let stored = StoredNamespace {
            id: existing.id,
            app_id: existing.app_id,
            cluster_name: existing.cluster_name,
            namespace_name: existing.namespace_name,
            format: dto.format.unwrap_or(existing.format),
            is_public: dto.is_public.unwrap_or(existing.is_public),
            comment: dto.comment.or(existing.comment),
            is_deleted: existing.is_deleted,
            deleted_at: existing.deleted_at,
            data_change_created_by: existing.data_change_created_by,
            data_change_created_time: existing.data_change_created_time,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        self.persistence.update(stored).await?;
        Ok(())
    }

    pub async fn delete(&self, app_id: &str, cluster_name: &str, namespace_name: &str, _operator: &str) -> Result<(), anyhow::Error> {
        let existing = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        self.persistence.delete(existing.id).await?;
        Ok(())
    }
}

impl From<StoredNamespace> for NamespaceDTO {
    fn from(stored: StoredNamespace) -> Self {
        Self {
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            namespace_name: stored.namespace_name,
            format: Some(stored.format),
            is_public: Some(stored.is_public),
            comment: stored.comment,
            data_change_created_by: Some(stored.data_change_created_by),
            data_change_last_modified_by: stored.data_change_last_modified_by,
            data_change_created_time: Some(format_timestamp(stored.data_change_created_time)),
            data_change_last_time: stored.data_change_last_time.map(format_timestamp),
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}