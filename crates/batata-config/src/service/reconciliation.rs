//! Config cache reconciliation service for cluster mode
//!
//! Periodically reloads config MD5 checksums from the persistence layer
//! to detect and fix cache drift caused by missed cluster broadcasts.
//! This is the safety net that ensures eventual consistency of the config
//! cache across cluster nodes.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use super::cache::ConfigCacheService;
use super::notifier::ConfigChangeNotifier;

/// Configuration for the reconciliation task
#[derive(Debug, Clone)]
pub struct ReconciliationConfig {
    /// Interval between reconciliation cycles
    pub interval: Duration,
    /// Whether reconciliation is enabled
    pub enabled: bool,
}

impl Default for ReconciliationConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            enabled: true,
        }
    }
}

/// Starts the config cache reconciliation background task.
///
/// This task periodically compares the in-memory config cache with the
/// persistence layer. If any configs have drifted (MD5 mismatch), it
/// updates the cache and notifies subscribers.
///
/// Returns a shutdown sender — drop it or send `()` to stop the task.
pub fn start_reconciliation_task(
    config: ReconciliationConfig,
    cache: Arc<ConfigCacheService>,
    persistence: Arc<dyn batata_persistence::traits::PersistenceService>,
    notifier: Arc<ConfigChangeNotifier>,
    shutdown: watch::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !config.enabled {
            info!("Config reconciliation task is disabled");
            return;
        }

        info!(
            "Config reconciliation task started (interval={}s)",
            config.interval.as_secs()
        );

        let mut shutdown = shutdown;
        let mut interval = tokio::time::interval(config.interval);
        // Skip the first immediate tick
        interval.tick().await;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = reconcile_once(&cache, persistence.as_ref(), &notifier).await {
                        error!("Config reconciliation cycle failed: {}", e);
                    }
                }
                _ = shutdown.changed() => {
                    info!("Config reconciliation task shutting down");
                    return;
                }
            }
        }
    })
}

/// Run a single reconciliation cycle.
///
/// Compares the cache MD5 digests against the persistence layer and
/// updates any entries that have drifted.
async fn reconcile_once(
    cache: &ConfigCacheService,
    persistence: &dyn batata_persistence::traits::PersistenceService,
    notifier: &ConfigChangeNotifier,
) -> Result<(), String> {
    let digests = cache.export_md5_digest();

    if digests.is_empty() {
        debug!("Config reconciliation: cache is empty, skipping");
        return Ok(());
    }

    let mut drift_count = 0u64;

    for (key, cached_md5, _cached_ts) in &digests {
        // Parse key back to components: "{tenant}+{group}+{dataId}"
        let parts: Vec<&str> = key.splitn(3, '+').collect();
        if parts.len() != 3 {
            warn!("Config reconciliation: invalid cache key format: {}", key);
            continue;
        }
        let (tenant, group, data_id) = (parts[0], parts[1], parts[2]);

        // Look up the config in persistence
        match persistence.config_find_one(data_id, group, tenant).await {
            Ok(Some(config)) => {
                let db_md5 = if config.md5.is_empty() {
                    format!("{:x}", md5::compute(config.content.as_bytes()))
                } else {
                    config.md5.clone()
                };

                if &db_md5 != cached_md5 {
                    debug!(
                        "Config drift detected: {}/{}/{} cache_md5={} db_md5={}",
                        tenant, group, data_id, cached_md5, db_md5
                    );

                    // Update the cache
                    let changed = cache.dump(
                        tenant,
                        group,
                        data_id,
                        &config.content,
                        config.modified_time,
                        &config.config_type,
                        &config.encrypted_data_key,
                    );

                    if changed {
                        // Notify local subscribers about the corrected config
                        notifier.notify_change(tenant, group, data_id);
                        drift_count += 1;
                    }
                }
            }
            Ok(None) => {
                // Config deleted from DB but still in cache — remove it
                debug!(
                    "Config reconciliation: removing stale cache entry {}/{}/{}",
                    tenant, group, data_id
                );
                cache.remove(tenant, group, data_id);
                notifier.notify_change(tenant, group, data_id);
                drift_count += 1;
            }
            Err(e) => {
                warn!(
                    "Config reconciliation: failed to query {}/{}/{}: {}",
                    tenant, group, data_id, e
                );
            }
        }
    }

    if drift_count > 0 {
        info!(
            "Config reconciliation: corrected {} drifted entries out of {} total",
            drift_count,
            digests.len()
        );
    } else {
        debug!(
            "Config reconciliation: all {} entries consistent",
            digests.len()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconciliation_config_defaults() {
        let config = ReconciliationConfig::default();
        assert_eq!(config.interval, Duration::from_secs(30));
        assert!(config.enabled);
    }
}
