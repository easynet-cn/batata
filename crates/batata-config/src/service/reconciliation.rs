//! Config cache reconciliation service for cluster mode
//!
//! Periodically reloads config MD5 checksums from the persistence layer
//! to detect and fix cache drift caused by missed cluster broadcasts.
//! This is the safety net that ensures eventual consistency of the config
//! cache across cluster nodes.

use futures::StreamExt;
use md5::Digest;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use super::cache::ConfigCacheService;
use super::notifier::ConfigChangeNotifier;

/// Max concurrent full-content reads per reconciliation cycle. Bounds the
/// fan-out so a large cache cannot open an unbounded number of DB queries.
const MAX_CONCURRENT_RECONCILE_READS: usize = 32;

/// Drive an iterator of futures with at most `cap` in flight at once and
/// collect every result. Centralizes the bounded-concurrency guard so it is
/// unit-testable; `reconcile_once` uses it for its full-content reads.
async fn run_bounded<F, T>(futures: impl Iterator<Item = F>, cap: usize) -> Vec<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
{
    futures::stream::iter(futures)
        .buffer_unordered(cap)
        .collect()
        .await
}

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
                    if let Err(e) = reconcile_once(cache.clone(), persistence.clone(), notifier.clone()).await {
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
    cache: Arc<ConfigCacheService>,
    persistence: Arc<dyn batata_persistence::traits::PersistenceService>,
    notifier: Arc<ConfigChangeNotifier>,
) -> Result<(), String> {
    let digests = cache.export_md5_digest();

    if digests.is_empty() {
        debug!("Config reconciliation: cache is empty, skipping");
        return Ok(());
    }

    // Process cache entries with bounded concurrency instead of N sequential
    // full-content reads. Each key keeps its exact Ok(Some)/Ok(None)/Err
    // handling; a future returns true when it corrected/removed the entry.
    // Reconciliation needs the full content to re-dump drifted configs, and
    // `config_find_md5_batch`'s default is itself a sequential full-read loop,
    // so a bounded concurrent batch is used directly.
    // Own the entries up front so each spawned future is `'static` (the borrow
    // of `digests` would otherwise outlive the `Send` requirement of
    // `buffer_unordered`). `digests` is consumed here.
    let entries: Vec<(String, String, i64)> = digests.into_iter().collect();
    // Capture the count before `entries` is consumed by the iterator below.
    let total = entries.len();
    let drift_flags: Vec<bool> = run_bounded(
        entries.into_iter().map(|(key, cached_md5, _cached_ts)| {
            // All three inputs are `Arc`s owned by `reconcile_once`; clone each
            // per item (the closure runs once per digest) so every spawned
            // future is `'static` + `Send`. A `&dyn`/`&T` borrow would break
            // `buffer_unordered`.
            let persistence = Arc::clone(&persistence);
            let cache = Arc::clone(&cache);
            let notifier = Arc::clone(&notifier);
            async move {
                // Parse key back to components: "{tenant}+{group}+{dataId}"
                let parts: Vec<&str> = key.splitn(3, '+').collect();
                if parts.len() != 3 {
                    warn!("Config reconciliation: invalid cache key format: {}", key);
                    return false;
                }
                let (tenant, group, data_id) = (parts[0], parts[1], parts[2]);

                // Look up the config in persistence
                match persistence.config_find_one(data_id, group, tenant).await {
                    Ok(Some(config)) => {
                        let db_md5 = if config.md5.is_empty() {
                            const_hex::encode(md5::Md5::digest(config.content.as_bytes()))
                        } else {
                            config.md5.clone()
                        };

                        if db_md5 != cached_md5 {
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
                                return true;
                            }
                        }
                        false
                    }
                    Ok(None) => {
                        // Config deleted from DB but still in cache — remove it
                        debug!(
                            "Config reconciliation: removing stale cache entry {}/{}/{}",
                            tenant, group, data_id
                        );
                        cache.remove(tenant, group, data_id);
                        notifier.notify_change(tenant, group, data_id);
                        true
                    }
                    Err(e) => {
                        warn!(
                            "Config reconciliation: failed to query {}/{}/{}: {}",
                            tenant, group, data_id, e
                        );
                        false
                    }
                }
            }
        }),
        MAX_CONCURRENT_RECONCILE_READS,
    )
    .await;

    let drift_count = drift_flags.iter().filter(|d| **d).count() as u64;

    if drift_count > 0 {
        info!(
            "Config reconciliation: corrected {} drifted entries out of {} total",
            drift_count, total
        );
    } else {
        debug!("Config reconciliation: all {} entries consistent", total);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc as StdArc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_reconciliation_config_defaults() {
        let config = ReconciliationConfig::default();
        assert_eq!(config.interval, Duration::from_secs(30));
        assert!(config.enabled);
    }

    // Mutant-checkable for the bounded read fan-out (finding 20): with more
    // items than the cap, concurrency must never exceed
    // MAX_CONCURRENT_RECONCILE_READS, and must actually overlap (so a fully
    // sequential regression is caught too).
    #[tokio::test]
    async fn test_reconcile_reads_are_bounded_and_concurrent() {
        let active = StdArc::new(AtomicUsize::new(0));
        let peak = StdArc::new(AtomicUsize::new(0));
        let n = MAX_CONCURRENT_RECONCILE_READS * 3 + 1;

        // Exercise the SAME bounded-concurrency helper the production cycle
        // uses: reverting it to a sequential loop would drop the peak to 1.
        let results: Vec<()> = run_bounded(
            (0..n).map(|_| {
                let active = active.clone();
                let peak = peak.clone();
                async move {
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(cur, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                }
            }),
            MAX_CONCURRENT_RECONCILE_READS,
        )
        .await;

        assert_eq!(results.len(), n, "every cache entry must be processed");
        let observed_peak = peak.load(Ordering::SeqCst);
        assert!(
            observed_peak <= MAX_CONCURRENT_RECONCILE_READS,
            "concurrent reads must not exceed the cap"
        );
        assert!(
            observed_peak > 1,
            "reads must overlap; a sequential loop would keep the peak at 1"
        );
    }
}
