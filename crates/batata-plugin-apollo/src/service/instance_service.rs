use std::sync::Arc;

use crate::api::dto::{InstanceDTO, OpenInstanceDTO, OpenInstancePageDTO};
use crate::entity::apollo_instance_config;
use crate::persistence::shared::StoredInstance;
use crate::persistence::traits::{
    ApolloPersistenceService, InstanceConfigPersistence, InstancePersistence, ReleasePersistence,
};
use chrono::Utc;

/// Represents the `InstanceService` entity.
pub struct InstanceService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl InstanceService {
    /// Creates a new `InstanceService`.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Performs the `register` operation.
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

    /// Performs the `get` operation.
    pub async fn get(&self, app_id: &str, cluster_name: &str, ip: &str, data_center: &str) -> Result<Option<InstanceDTO>, anyhow::Error> {
        let instances = self.persistence.get_by_app(app_id, Some(cluster_name)).await?;
        let found = instances.into_iter()
            .find(|i| i.ip == ip && i.data_center == data_center);
        Ok(found.map(|s| s.into()))
    }

    /// Returns the requested value.
    pub async fn list_by_app_cluster(&self, app_id: &str, cluster_name: &str) -> Result<Vec<InstanceDTO>, anyhow::Error> {
        let stored_list = self.persistence.get_by_app(app_id, Some(cluster_name)).await?;
        Ok(stored_list.into_iter().map(|s| s.into()).collect())
    }

    /// Performs the `heartbeat` operation.
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

    /// Resolves a single `apollo_instance` by id (used to enrich an instance
    /// config with its `ip` / `data_center`).
    async fn resolve_instance(&self, instance_id: i64) -> Option<crate::persistence::shared::StoredInstance> {
        self.persistence.get_instance_by_id(instance_id).await.ok().flatten()
    }

    /// Builds an `OpenInstanceDTO` from an instance config joined with its
    /// instance. Upstream `OpenApiController` returns exactly this shape
    /// (`appId`, `instanceAppId`, `clusterName`, `namespaceName`, `dataCenter`,
    /// `ip`, `releaseKey`, `releaseId`).
    async fn to_open_instance(&self, cfg: apollo_instance_config::Model) -> OpenInstanceDTO {
        let instance = self.resolve_instance(cfg.instance_id).await;
        let (ip, data_center, instance_app_id) = match instance {
            Some(i) => (i.ip, i.data_center, i.app_id),
            None => (String::new(), String::new(), cfg.config_app_id.clone()),
        };
        OpenInstanceDTO {
            app_id: cfg.config_app_id.clone(),
            instance_app_id,
            cluster_name: cfg.cluster_name.clone(),
            namespace_name: cfg.namespace_name.clone(),
            data_center: Some(data_center),
            ip,
            release_key: Some(cfg.release_key.clone()),
            release_id: None,
            last_modified_time: cfg
                .data_change_last_time
                .map(|ts| format_timestamp(chrono::DateTime::from_naive_utc_and_offset(ts, chrono::Utc))),
        }
    }

    /// `GET /openapi/v1/.../instances` — paged list of instance configs of a
    /// namespace. Upstream default page size is 20.
    ///
    /// Upstream: `OpenApiController.listInstances` → `PageDTO<OpenInstanceDTO>`.
    pub async fn list_open_instances(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        instance_app_id: Option<&str>,
        page: u64,
        size: u64,
    ) -> Result<OpenInstancePageDTO, anyhow::Error> {
        let (content, total) = self
            .persistence
            .list_instance_config_paged(app_id, cluster_name, namespace_name, instance_app_id, page, size)
            .await?;
        let mut items = Vec::with_capacity(content.len());
        for cfg in content {
            items.push(self.to_open_instance(cfg).await);
        }
        Ok(OpenInstancePageDTO {
            content: items,
            page: page as i32,
            size: size as i32,
            total: total as i64,
        })
    }

    /// `GET /openapi/v1/.../instances/by-release` — instance configs whose
    /// release is in `release_ids`. Upstream throws 404 if any requested
    /// release does not exist (`findReleaseOrThrow`).
    pub async fn list_open_instances_by_release(
        &self,
        release_ids: &[i64],
    ) -> Result<Vec<OpenInstanceDTO>, anyhow::Error> {
        // Resolve every requested release id; 404 on the first missing one.
        let mut release_keys = Vec::with_capacity(release_ids.len());
        for rid in release_ids {
            let release = self.persistence.get_by_release_id(*rid).await?;
            match release {
                Some(r) => release_keys.push(r.release_key),
                None => anyhow::bail!("release not found: {}", rid),
            }
        }
        let configs = self.persistence.list_instance_config_by_release_keys(&release_keys).await?;
        let mut items = Vec::with_capacity(configs.len());
        for cfg in configs {
            items.push(self.to_open_instance(cfg).await);
        }
        Ok(items)
    }

    /// `GET /openapi/v1/.../instances/by-release-not-in` — instance configs of
    /// a namespace whose release is NOT in `release_ids`.
    pub async fn list_open_instances_by_release_not_in(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
        release_ids: &[i64],
    ) -> Result<Vec<OpenInstanceDTO>, anyhow::Error> {
        // Resolve every requested release id; 404 on the first missing one.
        let mut release_keys = Vec::with_capacity(release_ids.len());
        for rid in release_ids {
            let release = self.persistence.get_by_release_id(*rid).await?;
            match release {
                Some(r) => release_keys.push(r.release_key),
                None => anyhow::bail!("release not found: {}", rid),
            }
        }
        let configs = self
            .persistence
            .list_instance_config_by_release_keys_not_in(app_id, cluster_name, namespace_name, &release_keys)
            .await?;
        let mut items = Vec::with_capacity(configs.len());
        for cfg in configs {
            items.push(self.to_open_instance(cfg).await);
        }
        Ok(items)
    }

    /// `GET /openapi/v1/.../instances/by-namespace` — distinct instance count of
    /// a namespace. Upstream returns the count as a single number.
    pub async fn count_open_instances(
        &self,
        app_id: &str,
        cluster_name: &str,
        namespace_name: &str,
    ) -> Result<u64, anyhow::Error> {
        self.persistence.count_instance_config(app_id, cluster_name, namespace_name).await
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
            data_change_created_time: Some(format_timestamp_millis(stored.data_change_created_time)),
            data_change_last_time: stored.data_change_last_time.map(format_timestamp_millis),
        }
    }
}

fn format_timestamp(ts: chrono::DateTime<chrono::Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string()
}

/// Convert an epoch-millis timestamp (`i64`) into an ISO-8601 UTC string.
///
/// Used when materializing [`StoredInstance`], whose timestamps are stored as
/// epoch millis rather than [`chrono::DateTime`].
fn format_timestamp_millis(ts: i64) -> String {
    let dt = chrono::DateTime::from_timestamp_millis(ts).unwrap_or_else(chrono::Utc::now);
    format_timestamp(dt)
}