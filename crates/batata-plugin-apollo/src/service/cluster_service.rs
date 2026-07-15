use std::sync::Arc;

use crate::api::dto::ClusterDTO;
use crate::persistence::shared::StoredCluster;
use crate::persistence::traits::{ApolloPersistenceService, ClusterPersistence};
use chrono::Utc;

pub struct ClusterService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ClusterService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, app_id: &str, dto: ClusterDTO) -> Result<ClusterDTO, anyhow::Error> {
        let existing = self.persistence.get(app_id, &dto.name).await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("Cluster already exists: {}", dto.name));
        }

        let now = Utc::now().timestamp_millis();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let stored = StoredCluster {
            id: 0,
            name: dto.name.clone(),
            app_id: app_id.to_string(),
            parent_cluster_id: dto.parent_cluster_id.unwrap_or(0),
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        let created = self.persistence.create(stored).await?;
        Ok(self.stored_to_dto(&created))
    }

    pub async fn get(&self, app_id: &str, cluster_name: &str) -> Result<Option<ClusterDTO>, anyhow::Error> {
        let stored = self.persistence.get(app_id, cluster_name).await?;
        Ok(stored.map(|s| self.stored_to_dto(&s)))
    }

    pub async fn list(&self, app_id: &str) -> Result<Vec<ClusterDTO>, anyhow::Error> {
        let stored_list = self.persistence.list(app_id).await?;
        Ok(stored_list.iter().map(|s| self.stored_to_dto(s)).collect())
    }

    pub async fn update(&self, app_id: &str, cluster_name: &str, dto: ClusterDTO) -> Result<(), anyhow::Error> {
        let existing = self.persistence.get(app_id, cluster_name).await?
            .ok_or_else(|| anyhow::anyhow!("Cluster not found: {}", cluster_name))?;

        let now = Utc::now().timestamp_millis();
        let updated = StoredCluster {
            id: existing.id,
            name: existing.name,
            app_id: existing.app_id,
            parent_cluster_id: dto.parent_cluster_id.unwrap_or(existing.parent_cluster_id),
            is_deleted: existing.is_deleted,
            deleted_at: existing.deleted_at,
            data_change_created_by: existing.data_change_created_by,
            data_change_created_time: existing.data_change_created_time,
            data_change_last_modified_by: dto.data_change_last_modified_by,
            data_change_last_time: Some(now),
        };

        self.persistence.update(updated).await?;
        Ok(())
    }

    pub async fn delete(&self, app_id: &str, cluster_name: &str, _operator: &str) -> Result<(), anyhow::Error> {
        self.persistence.delete(app_id, cluster_name).await?;
        Ok(())
    }

    fn stored_to_dto(&self, stored: &StoredCluster) -> ClusterDTO {
        ClusterDTO {
            name: stored.name.clone(),
            app_id: stored.app_id.clone(),
            parent_cluster_id: Some(stored.parent_cluster_id),
            comment: None,
            data_change_created_by: Some(stored.data_change_created_by.clone()),
            data_change_last_modified_by: stored.data_change_last_modified_by.clone(),
            data_change_created_time: Some(chrono::DateTime::from_timestamp_millis(stored.data_change_created_time)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string())
                .unwrap_or_default()),
            data_change_last_time: stored.data_change_last_time.and_then(|t| {
                chrono::DateTime::from_timestamp_millis(t)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string())
            }),
        }
    }
}
