//! Empty service cleaner - reclaims services that hold no instances.
//!
//! This is the batata counterpart of Nacos `EmptyServiceAutoCleanerV2`: a service is
//! reclaimed only when no client has instances registered for it **and** it has stayed
//! empty past a grace period (Nacos: `EMPTY_SERVICE_EXPIRED_TIME`, 60s by default).
//!
//! Without this reaper every service ever created stays resident forever. Distro then
//! keeps re-syncing those empty services on every cycle, so background work grows
//! without bound and eventually starves the cluster.

use std::sync::Arc;

use tokio::time::Duration;
use tracing::info;

use batata_common::server_status::ServerStatusManager;

use super::NamingService;

/// Grace period before an empty service is reclaimed (milliseconds).
///
/// Mirrors the Nacos default: `GlobalConfig.getEmptyServiceExpiredTime()` falls back to
/// `EMPTY_SERVICE_EXPIRED_TIME = 60000L`.
pub const DEFAULT_EMPTY_SERVICE_EXPIRED_MS: i64 = 60_000;

/// Default scan interval (seconds).
const DEFAULT_INTERVAL_SECS: u64 = 60;

/// Background reaper for services that hold no instances.
pub struct EmptyServiceCleaner {
    naming_service: Arc<NamingService>,
    interval_secs: u64,
    expired_ms: i64,
    /// Optional lifecycle status handle. When present the reaper stays idle until the
    /// node is UP, mirroring Nacos `ServiceMetadataReadyInterceptor`, which skips health
    /// checks until the application has started.
    status: Option<Arc<ServerStatusManager>>,
}

impl EmptyServiceCleaner {
    /// Create a cleaner with Nacos defaults (60s interval, 60s expiry).
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self {
            naming_service,
            interval_secs: DEFAULT_INTERVAL_SECS,
            expired_ms: DEFAULT_EMPTY_SERVICE_EXPIRED_MS,
            status: None,
        }
    }

    /// Attach the server lifecycle status so the reaper stays idle until the node is UP.
    pub fn with_status(mut self, status: Arc<ServerStatusManager>) -> Self {
        self.status = Some(status);
        self
    }

    /// Create a cleaner with a custom scan interval and expiry.
    pub fn with_interval(
        naming_service: Arc<NamingService>,
        interval_secs: u64,
        expired_ms: i64,
    ) -> Self {
        Self {
            naming_service,
            interval_secs,
            expired_ms,
            status: None,
        }
    }

    /// Start the reaper loop (runs forever).
    pub async fn start(&self) {
        info!(
            "Empty service cleaner started (interval: {}s, expiry: {}ms)",
            self.interval_secs, self.expired_ms
        );
        let mut interval = tokio::time::interval(Duration::from_secs(self.interval_secs));

        loop {
            interval.tick().await;

            // While the node is not UP the instance state is still being seeded (data
            // warmup / Distro snapshot load). Reaping now could drop services that are
            // about to receive their instances, so stay idle until ready.
            if let Some(ref status) = self.status {
                if !status.is_up() {
                    continue;
                }
            }

            let removed = self.naming_service.clean_empty_services(self.expired_ms);
            if removed > 0 {
                info!("Empty service cleaner: removed {} empty services", removed);
            }
        }
    }
}
