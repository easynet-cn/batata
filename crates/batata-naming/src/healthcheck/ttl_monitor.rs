//! TTL Monitor - background task that scans all TTL checks for expiration
//!
//! Runs every second and marks expired TTL checks as Critical.

use std::sync::Arc;

use tracing::{debug, warn};

use super::registry::{CheckStatus, CheckType, InstanceCheckRegistry};
use super::ttl_processor::TtlHealthCheckProcessor;

/// Background monitor that detects expired TTL checks
pub struct TtlMonitor {
    registry: Arc<InstanceCheckRegistry>,
}

impl TtlMonitor {
    /// Create a new TTL monitor
    pub fn new(registry: Arc<InstanceCheckRegistry>) -> Self {
        Self { registry }
    }

    /// Start the monitor loop (runs forever)
    pub async fn start(&self) {
        tracing::info!("TTL monitor started");
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            interval.tick().await;
            self.scan_expired_ttl_checks();
        }
    }

    /// Scan all checks and mark expired TTL checks as Critical
    fn scan_expired_ttl_checks(&self) {
        let all_checks = self.registry.get_all_checks();

        for (config, status) in all_checks {
            // Only process TTL checks
            if config.check_type != CheckType::Ttl {
                continue;
            }

            // Skip checks already in Critical state
            if status.status == CheckStatus::Critical {
                continue;
            }

            // Check if TTL has expired
            if let Some(ttl) = config.ttl
                && TtlHealthCheckProcessor::check_expiration(&status, ttl)
            {
                warn!(
                    "TTL expired for check {}: last_updated={}ms ago, ttl={}ms",
                    config.check_id,
                    current_timestamp_ms() - status.last_updated,
                    ttl.as_millis()
                );
                self.registry.ttl_update(
                    &config.check_id,
                    CheckStatus::Critical,
                    Some("TTL expired".to_string()),
                );
                debug!(
                    "Marked check {} as Critical due to TTL expiration",
                    config.check_id
                );
            }
        }
    }
}

fn current_timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
