//! Port of upstream `configservice/util/InstanceConfigAuditUtil`.
//!
//! Every successful client config fetch audits which release each instance
//! received:
//! 1. find-or-create an `Instance` row keyed (appId, clusterName, dataCenter, ip);
//! 2. upsert an `InstanceConfig` row keyed (instanceId, configAppId,
//!    configNamespaceName) with the delivered `releaseKey`.
//!
//! Two bounded in-memory caches mirror upstream and keep write volume low:
//! - `instance_cache`   (1h TTL): identity → instance id
//! - `release_key_cache`(1d TTL): instance+ns → last audited releaseKey
//! Writes run on detached tokio tasks; failures are logged, never propagated.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::api::dto::InstanceConfigDTO;
use crate::persistence::traits::{ApolloPersistenceService, InstancePersistence};

const INSTANCE_CACHE_TTL: Duration = Duration::from_secs(3600);
const RELEASE_KEY_CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
/// Upstream `instance.config.audit.max.size` default queue bound.
#[allow(dead_code)]
const MAX_PENDING: usize = 10_000;

struct CachedEntry<T> {
    value: T,
    inserted: Instant,
}

impl<T> CachedEntry<T> {
    fn fresh(&self, ttl: Duration) -> bool {
        self.inserted.elapsed() < ttl
    }
}

pub struct InstanceAuditService {
    persistence: Arc<dyn ApolloPersistenceService>,
    instance_cache: std::sync::Mutex<HashMap<String, CachedEntry<i32>>>,
    release_key_cache: std::sync::Mutex<HashMap<String, CachedEntry<String>>>,
}

impl InstanceAuditService {
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self {
            persistence,
            instance_cache: std::sync::Mutex::new(HashMap::new()),
            release_key_cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Fire-and-forget audit for one delivered release (upstream queues the
    /// work; we spawn a task instead — same observable behaviour).
    pub fn audit_async(
        &self,
        client_app_id: &str,
        cluster_name: &str,
        data_center: &str,
        client_ip: &str,
        config_app_id: &str,
        config_namespace_name: &str,
        release_key: String,
    ) {
        let persistence = self.persistence.clone();
        let service = Self::new(persistence.clone());
        let client_app_id = client_app_id.to_string();
        let cluster_name = cluster_name.to_string();
        let data_center = data_center.to_string();
        let client_ip = client_ip.to_string();
        let config_app_id = config_app_id.to_string();
        let config_namespace_name = config_namespace_name.to_string();
        tokio::spawn(async move {
            if client_ip.is_empty() {
                return; // upstream skips empty ips too
            }
            if let Err(e) = service
                .audit(
                    &client_app_id,
                    &cluster_name,
                    &data_center,
                    &client_ip,
                    &config_app_id,
                    &config_namespace_name,
                    release_key,
                )
                .await
            {
                tracing::warn!("instance audit failed: {}", e);
            }
        });
    }

    async fn audit(
        &self,
        client_app_id: &str,
        cluster_name: &str,
        data_center: &str,
        client_ip: &str,
        config_app_id: &str,
        config_namespace_name: &str,
        release_key: String,
    ) -> anyhow::Result<()> {
        // 1) resolve or create the instance row.
        let instance_id = self
            .find_or_create_instance(client_app_id, cluster_name, data_center, client_ip)
            .await?;

        // 2) skip redundant writes when the same release was already audited.
        let rk_key = format!("{}+{}+{}", instance_id, config_app_id, config_namespace_name);
        if let Some(hit) = self.release_key_cache.lock().unwrap().get(&rk_key) {
            if hit.fresh(RELEASE_KEY_CACHE_TTL) && hit.value == release_key {
                return Ok(());
            }
        }

        self.persistence
            .create_or_update_instance_config(InstanceConfigDTO {
                id: None,
                instance_id,
                config_app_id: Some(config_app_id.to_string()),
                namespace_name: config_namespace_name.to_string(),
                cluster_name: cluster_name.to_string(),
                release_key: release_key.clone(),
                configurations: None,
                data_change_created_by: Some("apollo-client".to_string()),
                data_change_created_time: None,
                data_change_last_time: None,
            })
            .await?;

        self.release_key_cache.lock().unwrap().insert(
            rk_key,
            CachedEntry { value: release_key, inserted: Instant::now() },
        );
        Ok(())
    }

    async fn find_or_create_instance(
        &self,
        app_id: &str,
        cluster_name: &str,
        data_center: &str,
        ip: &str,
    ) -> anyhow::Result<i32> {
        let cache_key = format!("{}|{}|{}|{}", app_id, cluster_name, ip, data_center);
        if let Some(hit) = self.instance_cache.lock().unwrap().get(&cache_key) {
            if hit.fresh(INSTANCE_CACHE_TTL) {
                return Ok(hit.value);
            }
        }

        let existing = InstancePersistence::get_by_app(&self.persistence, app_id, Some(cluster_name))
            .await?
            .into_iter()
            .find(|i| i.ip == ip && i.data_center == data_center);

        let instance_id = match existing {
            Some(i) => i.id,
            None => {
                let now = chrono::Utc::now().timestamp_millis();
                let created = InstancePersistence::upsert(
                    &self.persistence,
                    crate::persistence::shared::StoredInstance {
                        id: 0,
                        app_id: app_id.to_string(),
                        cluster_name: cluster_name.to_string(),
                        data_center: data_center.to_string(),
                        ip: ip.to_string(),
                        data_change_created_time: now,
                        data_change_last_time: Some(now),
                    },
                )
                .await?;
                created.id
            }
        };

        self.instance_cache.lock().unwrap().insert(
            cache_key,
            CachedEntry { value: instance_id, inserted: Instant::now() },
        );
        Ok(instance_id)
    }
}
