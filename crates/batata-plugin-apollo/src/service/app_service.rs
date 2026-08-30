use std::sync::Arc;

use crate::api::dto::AppDTO;
use crate::persistence::shared::{StoredApp, StoredCluster, StoredNamespace};
use crate::persistence::traits::{ApolloPersistenceService, AppPersistence, ClusterPersistence, NamespacePersistence};
use chrono::Utc;

/// Represents the `AppService` entity.
pub struct AppService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AppService {
    /// Creates a new `AppService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Upstream adminservice createApp bootstraps the default Cluster row and
    /// the `application` Namespace so clients can consume immediately.
    pub async fn bootstrap_default_namespace(&self, app_id: &str, operator: &str) -> Result<(), anyhow::Error> {
        let now = Utc::now().timestamp_millis();
        if ClusterPersistence::get(&self.persistence, app_id, "default").await?.is_none() {
            ClusterPersistence::create(
                &self.persistence,
                StoredCluster {
                    id: 0,
                    name: "default".into(),
                    app_id: app_id.to_string(),
                    parent_cluster_id: 0,
                    is_deleted: false,
                    deleted_at: 0,
                    data_change_created_by: operator.to_string(),
                    data_change_created_time: now,
                    data_change_last_modified_by: None,
                    data_change_last_time: Some(now),
                },
            )
            .await?;
        }
        if self
            .persistence
            .get_by_app_cluster(app_id, "default", "application")
            .await?
            .is_none()
        {
            NamespacePersistence::create(
                &self.persistence,
                StoredNamespace {
                    id: 0,
                    app_id: app_id.to_string(),
                    cluster_name: "default".into(),
                    namespace_name: "application".into(),
                    format: "properties".into(),
                    is_public: false,
                    comment: None,
                    is_deleted: false,
                    deleted_at: 0,
                    data_change_created_by: operator.to_string(),
                    data_change_created_time: now,
                    data_change_last_modified_by: None,
                    data_change_last_time: Some(now),
                },
            )
            .await?;
        }
        Ok(())
    }

    /// Performs the `create` operation.
    pub async fn create(&self, dto: AppDTO) -> Result<AppDTO, anyhow::Error> {
        let existing = AppPersistence::get(&*self.persistence, &dto.app_id).await?;

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

        let created = AppPersistence::create(&*self.persistence, stored).await?;
        // Upstream: app creation bootstraps default cluster + application ns.
        let operator = dto.data_change_created_by.clone().unwrap_or_else(|| "apollo".into());
        self.bootstrap_default_namespace(&dto.app_id, &operator).await?;
        Ok(created.into())
    }

    /// Performs the `get` operation.
    pub async fn get(&self, app_id: &str) -> Result<Option<AppDTO>, anyhow::Error> {
        let stored = AppPersistence::get(&*self.persistence, app_id).await?;
        Ok(stored.map(|s| s.into()))
    }

    /// Performs the `list` operation.
    pub async fn list(&self) -> Result<Vec<AppDTO>, anyhow::Error> {
        let stored_list = AppPersistence::list(&*self.persistence).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    /// Returns the requested value.
    pub async fn get_by_ids(&self, app_ids: &[String]) -> Result<Vec<AppDTO>, anyhow::Error> {
        let stored_list = self.persistence.get_by_ids(app_ids).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    /// Performs the `update` operation.
    pub async fn update(&self, app_id: &str, dto: AppDTO) -> Result<(), anyhow::Error> {
        let existing = AppPersistence::get(&*self.persistence, app_id).await?
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

        AppPersistence::update(&*self.persistence, stored).await?;
        Ok(())
    }

    /// Performs the `delete` operation.
    pub async fn delete(&self, app_id: &str, _operator: &str) -> Result<(), anyhow::Error> {
        AppPersistence::delete(&*self.persistence, app_id).await?;
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