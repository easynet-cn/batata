use std::sync::Arc;

use crate::api::dto::AppDTO;
use crate::persistence::shared::StoredApp;
use crate::persistence::traits::{ApolloPersistenceService, AppPersistence};
use chrono::Utc;

pub struct AppService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AppService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, dto: AppDTO) -> Result<AppDTO, anyhow::Error> {
        let existing = self.persistence.get(&dto.app_id).await?;

        if existing.is_some() {
            return Err(anyhow::anyhow!("App already exists: {}", dto.app_id));
        }

        let now = Utc::now().timestamp_millis();
        let created_by = dto.data_change_created_by.clone().unwrap_or_default();

        let stored = StoredApp {
            app_id: dto.app_id.clone(),
            name: dto.name.clone(),
            org_id: dto.org_id.clone(),
            org_name: dto.org_name.clone(),
            owner_name: dto.owner_name.clone(),
            owner_email: dto.owner_email.clone(),
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

    pub async fn get(&self, app_id: &str) -> Result<Option<AppDTO>, anyhow::Error> {
        let stored = self.persistence.get(app_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    pub async fn list(&self) -> Result<Vec<AppDTO>, anyhow::Error> {
        let stored_list = self.persistence.list().await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    pub async fn get_by_ids(&self, app_ids: &[String]) -> Result<Vec<AppDTO>, anyhow::Error> {
        let stored_list = self.persistence.get_by_ids(app_ids).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    pub async fn update(&self, app_id: &str, dto: AppDTO) -> Result<(), anyhow::Error> {
        let existing = self.persistence.get(app_id).await?
            .ok_or_else(|| anyhow::anyhow!("App not found: {}", app_id))?;

        let now = Utc::now().timestamp_millis();

        let stored = StoredApp {
            app_id: app_id.to_string(),
            name: if dto.name.is_empty() { existing.name } else { dto.name },
            org_id: if dto.org_id.is_empty() { existing.org_id } else { dto.org_id },
            org_name: if dto.org_name.is_empty() { existing.org_name } else { dto.org_name },
            owner_name: if dto.owner_name.is_empty() { existing.owner_name } else { dto.owner_name },
            owner_email: if dto.owner_email.is_empty() { existing.owner_email } else { dto.owner_email },
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

    pub async fn delete(&self, app_id: &str, operator: &str) -> Result<(), anyhow::Error> {
        self.persistence.delete(app_id).await?;
        Ok(())
    }
}

impl From<StoredApp> for AppDTO {
    fn from(stored: StoredApp) -> Self {
        Self {
            app_id: stored.app_id,
            name: stored.name,
            org_id: stored.org_id,
            org_name: stored.org_name,
            owner_name: stored.owner_name,
            owner_email: stored.owner_email,
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