use std::sync::Arc;

use crate::api::dto::CommitDTO;
use crate::persistence::shared::StoredCommit;
use crate::persistence::traits::{ApolloPersistenceService, CommitPersistence};
use chrono::Utc;

pub struct CommitService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl CommitService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, dto: CommitDTO) -> Result<CommitDTO, anyhow::Error> {
        let now = Utc::now().timestamp_millis();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let stored = StoredCommit {
            id: 0,
            change_sets: dto.change_sets.clone(),
            app_id: dto.app_id.clone(),
            cluster_name: dto.cluster_name.clone(),
            namespace_name: dto.namespace_name.clone(),
            comment: dto.comment.clone(),
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: created_by,
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = self.persistence.create(stored).await?;
        Ok(created.into())
    }

    pub async fn get(&self, id: i32) -> Result<Option<CommitDTO>, anyhow::Error> {
        let stored = self.persistence.get_by_id(id).await?;
        Ok(stored.map(|s| s.into()))
    }

    pub async fn list(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Vec<CommitDTO>, anyhow::Error> {
        let stored_list = self.persistence.list_by_namespace(app_id, cluster_name, namespace_name).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    pub async fn delete(&self, id: i32, _operator: &str) -> Result<(), anyhow::Error> {
        let stored = self.persistence.get_by_id(id).await?
            .ok_or_else(|| anyhow::anyhow!("Commit not found: {}", id))?;

        let now = Utc::now().timestamp_millis();
        let updated_stored = StoredCommit {
            id: stored.id,
            change_sets: stored.change_sets,
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            namespace_name: stored.namespace_name,
            comment: stored.comment,
            is_deleted: true,
            deleted_at: now,
            data_change_created_by: stored.data_change_created_by,
            data_change_created_time: stored.data_change_created_time,
            data_change_last_modified_by: Some(_operator.to_string()),
            data_change_last_time: Some(now),
        };

        <dyn CommitPersistence>::update(&self.persistence, updated_stored).await?;
        Ok(())
    }
}

impl From<StoredCommit> for CommitDTO {
    fn from(stored: StoredCommit) -> Self {
        Self {
            id: Some(stored.id),
            change_sets: stored.change_sets,
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            namespace_name: stored.namespace_name,
            comment: stored.comment,
            data_change_created_by: Some(stored.data_change_created_by),
            data_change_created_time: Some(format_timestamp(stored.data_change_created_time)),
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}