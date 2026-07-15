use std::sync::Arc;
use std::collections::HashMap;

use crate::api::dto::{ReleaseDTO, ReleaseHistoryDTO};
use crate::persistence::shared::StoredRelease;
use crate::persistence::traits::{ApolloPersistenceService, ReleasePersistence, ItemPersistence, NamespacePersistence};
use chrono::Utc;

pub struct ReleaseService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl ReleaseService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn publish(&self, app_id: &str, cluster_name: &str, namespace_name: &str, 
        release_name: &str, release_comment: Option<String>, operator: &str, 
        _is_emergency_publish: bool) -> Result<ReleaseDTO, anyhow::Error> {

        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, namespace.id).await?;

        let mut configurations: HashMap<String, String> = HashMap::new();
        for item in items {
            configurations.insert(item.key, item.value);
        }

        let configurations_json = serde_json::to_string(&configurations)?;

        let now = Utc::now().timestamp_millis();
        let release_id = now;
        let release_key = format!("{}+{}+{}+{}", app_id, cluster_name, namespace_name, release_id);

        let stored = StoredRelease {
            id: 0,
            release_key: release_key.clone(),
            name: release_name.to_string(),
            comment: release_comment,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            configurations: configurations_json,
            release_id: Some(release_id),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = <dyn ReleasePersistence>::create(&self.persistence, stored).await?;
        Ok(created.into())
    }

    pub async fn get_latest_active(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Option<ReleaseDTO>, anyhow::Error> {
        let stored = <dyn ReleasePersistence>::get_latest(&self.persistence, app_id, cluster_name, namespace_name).await?;
        Ok(stored.map(|s| s.into()))
    }

    pub async fn get_configurations(&self, app_id: &str, cluster_name: &str, namespace_name: &str) -> Result<Option<HashMap<String, String>>, anyhow::Error> {
        let release = self.get_latest_active(app_id, cluster_name, namespace_name).await?;

        match release {
            Some(r) => {
                let configs: HashMap<String, String> = serde_json::from_str(&r.configurations.unwrap_or_default())?;
                Ok(Some(configs))
            }
            None => Ok(None),
        }
    }

    pub async fn find_active_releases(&self, app_id: &str, cluster_name: &str, namespace_name: &str, _page: u64, _size: u64) -> Result<(Vec<ReleaseDTO>, u64), anyhow::Error> {
        let stored_list = <dyn ReleasePersistence>::list_by_namespace(&self.persistence, app_id, cluster_name, namespace_name).await?;
        let active_releases: Vec<_> = stored_list.into_iter()
            .filter(|s| !s.is_deleted && !s.is_abandoned)
            .map(|s| s.into())
            .collect();
        
        let count = active_releases.len() as u64;
        Ok((active_releases, count))
    }

    pub async fn get_by_id(&self, release_id: i32) -> Result<Option<ReleaseDTO>, anyhow::Error> {
        let stored = <dyn ReleasePersistence>::get_by_id(&self.persistence, release_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    pub async fn get_gray_release(&self, release_id: i64) -> Result<Option<ReleaseDTO>, anyhow::Error> {
        let stored = <dyn ReleasePersistence>::get_by_release_id(&self.persistence, release_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    pub async fn rollback(&self, app_id: &str, cluster_name: &str, namespace_name: &str, release_id: i32, operator: &str) -> Result<ReleaseDTO, anyhow::Error> {
        let target_release = <dyn ReleasePersistence>::get_by_id(&self.persistence, release_id).await?
            .ok_or_else(|| anyhow::anyhow!("Release not found: {}", release_id))?;

        let now = Utc::now().timestamp_millis();
        let new_release_id = now;
        let release_key = format!("{}+{}+{}+{}+rollback", app_id, cluster_name, namespace_name, new_release_id);

        let stored = StoredRelease {
            id: 0,
            release_key: release_key,
            name: format!("rollback-{}", target_release.name),
            comment: Some(format!("Rollback to release {}", release_id)),
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            configurations: target_release.configurations.clone(),
            release_id: Some(new_release_id),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = <dyn ReleasePersistence>::create(&self.persistence, stored).await?;
        Ok(created.into())
    }

    pub async fn merge_branch_and_release(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        _branch_name: &str,
        release_name: &str,
        release_comment: Option<String>,
        operator: &str,
        _is_emergency_publish: bool,
        change_sets: crate::api::dto::ItemChangeSets,
    ) -> Result<ReleaseDTO, anyhow::Error> {
        let item_set_service = crate::service::ItemSetService::new(self.persistence.clone());
        item_set_service.update_set(app_id, cluster_name, namespace_name, change_sets).await?;

        let namespace = self.persistence.get_by_app_cluster(app_id, cluster_name, namespace_name).await?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name))?;

        let items = <dyn ItemPersistence>::list_by_namespace(&self.persistence, namespace.id).await?;

        let mut configurations: HashMap<String, String> = HashMap::new();
        for item in items {
            configurations.insert(item.key, item.value);
        }
        let configurations_json = serde_json::to_string(&configurations)?;

        let now = Utc::now().timestamp_millis();
        let release_id = now;
        let release_key = format!("{}+{}+{}+{}+merge", app_id, cluster_name, namespace_name, release_id);

        let stored = StoredRelease {
            id: 0,
            release_key: release_key.clone(),
            name: release_name.to_string(),
            comment: release_comment,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            namespace_name: namespace_name.to_string(),
            configurations: configurations_json,
            release_id: Some(release_id),
            is_abandoned: false,
            is_deleted: false,
            deleted_at: 0,
            data_change_created_by: operator.to_string(),
            data_change_created_time: now,
            data_change_last_modified_by: None,
            data_change_last_time: None,
        };

        let created = <dyn ReleasePersistence>::create(&self.persistence, stored).await?;
        Ok(created.into())
    }

    pub async fn find_release_history(&self, app_id: &str, cluster_name: &str, namespace_name: &str, _page: u64, _size: u64) -> Result<(Vec<ReleaseHistoryDTO>, u64), anyhow::Error> {
        Ok((vec![], 0))
    }
}

impl From<StoredRelease> for ReleaseDTO {
    fn from(stored: StoredRelease) -> Self {
        Self {
            id: Some(stored.id),
            release_key: stored.release_key,
            name: stored.name,
            comment: stored.comment,
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            namespace_name: stored.namespace_name,
            configurations: Some(stored.configurations),
            release_id: stored.release_id,
            is_abandoned: Some(stored.is_abandoned),
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