use std::sync::Arc;

use crate::persistence::shared::StoredNamespaceLock;
use crate::persistence::traits::{ApolloPersistenceService, NamespaceLockPersistence};
use chrono::Utc;

pub struct NamespaceLockService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl NamespaceLockService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn lock(&self, app_id: &str, cluster_name: &str, namespace_name: &str, locked_by: &str) -> Result<(), anyhow::Error> {
        let existing = self.persistence.get(app_id, cluster_name, namespace_name).await?;

        if let Some(lock) = existing {
            if lock.locked_by != locked_by {
                return Err(anyhow::anyhow!("Namespace is locked by {}", lock.locked_by));
            }
            return Ok(());
        }

        let now = Utc::now().timestamp_millis();
        let stored = StoredNamespaceLock {
            id: 0,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            locked_by: locked_by.to_string(),
            locked_at: now,
            data_change_created_by: locked_by.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        self.persistence.lock(stored).await?;
        Ok(())
    }

    pub async fn unlock(&self, app_id: &str, cluster_name: &str, namespace_name: &str, locked_by: &str) -> Result<(), anyhow::Error> {
        let lock = self.persistence.get(app_id, cluster_name, namespace_name).await?;

        if let Some(lock_model) = lock {
            if lock_model.locked_by != locked_by {
                return Err(anyhow::anyhow!("Namespace is locked by {}", lock_model.locked_by));
            }
        }

        self.persistence.unlock(app_id, cluster_name, namespace_name).await?;
        Ok(())
    }

    pub async fn get_lock(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Option<StoredNamespaceLock>, anyhow::Error> {
        self.persistence.get(app_id, cluster_name, namespace_name).await
    }

    pub async fn is_locked(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<bool, anyhow::Error> {
        self.persistence.is_locked(app_id, cluster_name, namespace_name).await
    }
}