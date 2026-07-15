use std::sync::Arc;

use crate::api::dto::AccessKeyDTO;
use crate::persistence::shared::StoredAccessKey;
use crate::persistence::traits::{ApolloPersistenceService, AccessKeyPersistence};
use chrono::Utc;

pub struct AccessKeyService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AccessKeyService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn create(&self, app_id: &str, operator: &str) -> Result<AccessKeyDTO, anyhow::Error> {
        let secret = Self::generate_secret();
        let now = Utc::now().timestamp_millis();

        let stored = StoredAccessKey {
            id: 0,
            app_id: app_id.to_string(),
            secret,
            mode: 0,
            is_enabled: true,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = self.persistence.create(stored).await?;
        Ok(created.into())
    }

    pub async fn list_by_app(&self, app_id: &str) -> Result<Vec<AccessKeyDTO>, anyhow::Error> {
        let stored_list = self.persistence.get_by_app(app_id).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    pub async fn delete(&self, _app_id: &str, id: i32, _operator: &str) -> Result<(), anyhow::Error> {
        self.persistence.delete(id).await?;
        Ok(())
    }

    fn generate_secret() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{:x}", now)
    }
}

impl From<StoredAccessKey> for AccessKeyDTO {
    fn from(stored: StoredAccessKey) -> Self {
        Self {
            id: Some(stored.id),
            app_id: stored.app_id,
            secret: stored.secret,
            mode: stored.mode,
            is_enabled: stored.is_enabled,
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