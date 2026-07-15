use std::sync::Arc;

use crate::api::dto::InstanceDTO;
use crate::persistence::shared::StoredInstance;
use crate::persistence::traits::{ApolloPersistenceService, InstancePersistence};
use chrono::Utc;

pub struct InstanceService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl InstanceService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    pub async fn register(&self, dto: InstanceDTO) -> Result<InstanceDTO, anyhow::Error> {
        let now = Utc::now().timestamp_millis();

        let stored = StoredInstance {
            id: 0,
            app_id: dto.app_id.clone(),
            cluster_name: dto.cluster_name.clone(),
            data_center: dto.data_center.clone(),
            ip: dto.ip.clone(),
            data_change_created_time: now,
            data_change_last_time: Some(now),
        };

        let created = self.persistence.upsert(stored).await?;
        Ok(created.into())
    }

    pub async fn get(&self, app_id: &str, cluster_name: &str, ip: &str, data_center: &str) -> Result<Option<InstanceDTO>, anyhow::Error> {
        let instances = self.persistence.get_by_app(app_id, Some(cluster_name)).await?;
        let found = instances.into_iter()
            .find(|i| i.ip == ip && i.data_center == data_center);
        Ok(found.map(|s| s.into()))
    }

    pub async fn list_by_app_cluster(&self, app_id: &str, cluster_name: &str) -> Result<Vec<InstanceDTO>, anyhow::Error> {
        let stored_list = self.persistence.get_by_app(app_id, Some(cluster_name)).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    pub async fn heartbeat(&self, app_id: &str, cluster_name: &str, ip: &str, data_center: &str) -> Result<(), anyhow::Error> {
        let now = Utc::now().timestamp_millis();

        let stored = StoredInstance {
            id: 0,
            app_id: app_id.to_string(),
            cluster_name: cluster_name.to_string(),
            data_center: data_center.to_string(),
            ip: ip.to_string(),
            data_change_created_time: now,
            data_change_last_time: Some(now),
        };

        self.persistence.upsert(stored).await?;
        Ok(())
    }
}

impl From<StoredInstance> for InstanceDTO {
    fn from(stored: StoredInstance) -> Self {
        Self {
            id: Some(stored.id),
            app_id: stored.app_id,
            cluster_name: stored.cluster_name,
            data_center: stored.data_center,
            ip: stored.ip,
            data_change_created_time: Some(format_timestamp(stored.data_change_created_time)),
            data_change_last_time: stored.data_change_last_time.map(|ts| format_timestamp(ts)),
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ts)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.f+00:00")
        .to_string()
}