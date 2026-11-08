//! Port of upstream `configservice/util/InstanceConfigAuditUtil`.
//!
//! Every successful client config fetch audits which release each instance
//! received:
//! 1. find-or-create an `Instance` row keyed (appId, clusterName, dataCenter, ip);
//! 2. upsert an `InstanceConfig` row keyed (instanceId, configAppId,
//!    configNamespaceName) with the delivered `releaseKey`.
//!
//! Two in-memory caches mirror upstream and keep write volume low:
//! - `instance_cache`   (1h TTL): identity -> instance id
//! - `release_key_cache`(1d TTL): instance+ns -> last audited releaseKey
//!
//! The HTTP layer constructs an `InstanceAuditService` per request, so the
//! caches and the spawn gate live in a single process-wide `SharedState` that
//! every instance points at. This is what makes release-key dedup effective
//! across requests. Detached audit tasks are additionally gated by a bounded
//! `Semaphore` so a burst of fetches cannot create unbounded tasks; when the
//! gate is saturated the audit event is dropped (logged), matching upstream's
//! bounded audit queue. Failures are logged, never propagated.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::api::dto::InstanceConfigDTO;
use crate::persistence::traits::{ApolloPersistenceService, InstancePersistence};

const INSTANCE_CACHE_TTL: Duration = Duration::from_secs(3600);
const RELEASE_KEY_CACHE_TTL: Duration = Duration::from_secs(24 * 3600);
/// Maximum number of audit tasks allowed to run at once. Detached spawns are
/// gated by a `Semaphore` of this size.
const MAX_CONCURRENT_AUDITS: usize = 32;

struct CachedEntry<T> {
    value: T,
    inserted: Instant,
}

impl<T> CachedEntry<T> {
    fn fresh(&self, ttl: Duration) -> bool {
        self.inserted.elapsed() < ttl
    }
}

/// State shared by every `InstanceAuditService` instance in the process.
///
/// The HTTP layer builds a fresh service per request, so keeping the caches
/// (and the spawn gate) here is what allows release-key dedup to survive
/// across requests and bounds the number of detached tasks.
struct SharedState {
    instance_cache: Mutex<HashMap<String, CachedEntry<i64>>>,
    release_key_cache: Mutex<HashMap<String, CachedEntry<String>>>,
    spawn_gate: Arc<Semaphore>,
}

impl SharedState {
    fn new(max_concurrent_audits: usize) -> Arc<Self> {
        Arc::new(Self {
            instance_cache: Mutex::new(HashMap::new()),
            release_key_cache: Mutex::new(HashMap::new()),
            spawn_gate: Arc::new(Semaphore::new(max_concurrent_audits)),
        })
    }

    /// Try to reserve one detached-task slot. Returns `None` when the gate is
    /// saturated; the returned permit is released (via `Drop`) when the task
    /// finishes, so the bound holds for the whole task lifetime.
    fn try_reserve(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.spawn_gate).try_acquire_owned().ok()
    }

    fn is_recently_audited(&self, rk_key: &str, release_key: &str) -> bool {
        let cache = self.release_key_cache.lock().unwrap();
        match cache.get(rk_key) {
            Some(hit) => hit.fresh(RELEASE_KEY_CACHE_TTL) && hit.value.as_str() == release_key,
            None => false,
        }
    }

    fn mark_audited(&self, rk_key: String, release_key: String) {
        self.release_key_cache.lock().unwrap().insert(
            rk_key,
            CachedEntry {
                value: release_key,
                inserted: Instant::now(),
            },
        );
    }
}

static GLOBAL_SHARED: OnceLock<Arc<SharedState>> = OnceLock::new();

fn global_shared() -> Arc<SharedState> {
    GLOBAL_SHARED
        .get_or_init(|| SharedState::new(MAX_CONCURRENT_AUDITS))
        .clone()
}

/// Represents the `InstanceAuditService` entity.
pub struct InstanceAuditService {
    persistence: Arc<dyn ApolloPersistenceService>,
    shared: Arc<SharedState>,
}

impl InstanceAuditService {
    /// Creates a new `InstanceAuditService`.
    ///
    /// All instances share one `SharedState`, so constructing a service per
    /// request does not reset the dedup caches or the spawn gate.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self {
            persistence,
            shared: global_shared(),
        }
    }

    /// Fire-and-forget audit for one delivered release (upstream queues the
    /// work; we spawn a task instead - same observable behaviour).
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
        // Bound the number of concurrent detached tasks. Under a burst we drop
        // the excess event (matching upstream's bounded audit queue) instead of
        // spawning without limit.
        let permit = match self.shared.try_reserve() {
            Some(p) => p,
            None => {
                tracing::warn!("instance audit concurrency limit reached; dropping audit event");
                return;
            }
        };

        // Reuse the shared caches instead of building a fresh `Self` whose
        // empty caches would defeat release-key dedup on every delivery.
        let persistence = Arc::clone(&self.persistence);
        let shared = Arc::clone(&self.shared);
        let client_app_id = client_app_id.to_string();
        let cluster_name = cluster_name.to_string();
        let data_center = data_center.to_string();
        let client_ip = client_ip.to_string();
        let config_app_id = config_app_id.to_string();
        let config_namespace_name = config_namespace_name.to_string();
        tokio::spawn(async move {
            // Held for the whole task; released on drop (including early
            // returns) so a slot is always recycled.
            let _permit = permit;
            if client_ip.is_empty() {
                return; // upstream skips empty ips too
            }
            let service = InstanceAuditService {
                persistence,
                shared,
            };
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
        let rk_key = format!(
            "{}+{}+{}",
            instance_id, config_app_id, config_namespace_name
        );
        if self.shared.is_recently_audited(&rk_key, &release_key) {
            return Ok(());
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

        self.shared.mark_audited(rk_key, release_key);
        Ok(())
    }

    async fn find_or_create_instance(
        &self,
        app_id: &str,
        cluster_name: &str,
        data_center: &str,
        ip: &str,
    ) -> anyhow::Result<i64> {
        let cache_key = format!("{}|{}|{}|{}", app_id, cluster_name, ip, data_center);
        {
            let cache = self.shared.instance_cache.lock().unwrap();
            if let Some(hit) = cache.get(&cache_key)
                && hit.fresh(INSTANCE_CACHE_TTL)
            {
                return Ok(hit.value);
            }
        }

        let existing =
            InstancePersistence::get_by_app(&self.persistence, app_id, Some(cluster_name))
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

        self.shared.instance_cache.lock().unwrap().insert(
            cache_key,
            CachedEntry {
                value: instance_id,
                inserted: Instant::now(),
            },
        );
        Ok(instance_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_gate_is_bounded() {
        let shared = SharedState::new(2);
        assert_eq!(shared.spawn_gate.available_permits(), 2);

        // Success path: slots are granted until the bound is reached.
        let first = shared.try_reserve().expect("first slot granted");
        let second = shared.try_reserve().expect("second slot granted");

        // Guard: once saturated, further spawns must be rejected.
        assert!(
            shared.try_reserve().is_none(),
            "gate must reject when all permits are held"
        );

        // Permits are returned as tasks finish, so the pool recovers.
        drop(first);
        assert_eq!(shared.spawn_gate.available_permits(), 1);
        assert!(shared.try_reserve().is_some(), "slot recycled after drop");
        drop(second);
    }

    #[test]
    fn spawn_gate_uses_configured_max() {
        assert_eq!(MAX_CONCURRENT_AUDITS, 32);
        let shared = SharedState::new(MAX_CONCURRENT_AUDITS);
        assert_eq!(
            shared.spawn_gate.available_permits(),
            MAX_CONCURRENT_AUDITS,
            "fresh state must expose exactly the configured bound"
        );
    }

    #[test]
    fn global_shared_state_is_reused_across_instances() {
        // The HTTP layer calls `new()` per request; all of them must point at
        // the same state or the dedup caches reset on every delivery.
        let a = global_shared();
        let b = global_shared();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn release_key_cache_deduplicates_within_ttl() {
        let shared = SharedState::new(MAX_CONCURRENT_AUDITS);
        let key = "7+app+namespace".to_string();

        // Success path: nothing audited yet, so a delivery is not deduped.
        assert!(!shared.is_recently_audited(&key, "release-1"));

        shared.mark_audited(key.clone(), "release-1".to_string());

        // Guard: the same release is recognised and the redundant write skipped.
        assert!(shared.is_recently_audited(&key, "release-1"));
        // A different release must still be written.
        assert!(!shared.is_recently_audited(&key, "release-2"));
        // An unrelated key must not be deduped.
        assert!(!shared.is_recently_audited("other", "release-1"));
    }
}
