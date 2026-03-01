//! Deregister Monitor - background task that reaps instances stuck in Critical state
//!
//! Scans all checks periodically and deregisters instances that have been Critical
//! longer than their configured deregister_critical_after duration.

use std::sync::Arc;

use tracing::{info, warn};

use super::registry::{CheckStatus, InstanceCheckRegistry};
use crate::model::Instance;
use crate::service::NamingService;

/// Background monitor that auto-deregisters instances in prolonged Critical state
pub struct DeregisterMonitor {
    registry: Arc<InstanceCheckRegistry>,
    naming_service: Arc<NamingService>,
    interval_secs: u64,
}

impl DeregisterMonitor {
    /// Create a new deregister monitor
    pub fn new(
        registry: Arc<InstanceCheckRegistry>,
        naming_service: Arc<NamingService>,
        interval_secs: u64,
    ) -> Self {
        Self {
            registry,
            naming_service,
            interval_secs,
        }
    }

    /// Start the monitor loop (runs forever)
    pub async fn start(&self) {
        info!(
            "Deregister monitor started with interval: {}s",
            self.interval_secs
        );
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(self.interval_secs));

        loop {
            interval.tick().await;
            self.reap_critical_instances();
        }
    }

    /// Scan for instances that should be auto-deregistered
    fn reap_critical_instances(&self) {
        let critical_checks = self.registry.get_checks_by_status(&CheckStatus::Critical);
        let now = current_timestamp_ms();

        for (config, status) in critical_checks {
            // Only process checks with deregister_critical_after configured
            let deregister_after = match config.deregister_critical_after {
                Some(d) => d,
                None => continue,
            };

            // Check if the instance has been Critical long enough
            let critical_since = match status.critical_since {
                Some(t) => t,
                None => continue,
            };

            let critical_duration_ms = now - critical_since;
            let threshold_ms = deregister_after.as_millis() as i64;

            if critical_duration_ms >= threshold_ms {
                warn!(
                    "Auto-deregistering instance for check {}: critical for {}ms (threshold: {}ms)",
                    config.check_id, critical_duration_ms, threshold_ms
                );

                // Deregister instance from NamingService
                let instance = Instance {
                    ip: config.ip.clone(),
                    port: config.port,
                    cluster_name: config.cluster_name.clone(),
                    ephemeral: false, // Consul instances are persistent
                    ..Default::default()
                };
                let _ = self.naming_service.deregister_instance(
                    &config.namespace,
                    &config.group_name,
                    &config.service_name,
                    &instance,
                );

                // Deregister all checks for this instance
                let instance_key = super::registry::build_instance_key(
                    &config.namespace,
                    &config.group_name,
                    &config.service_name,
                    &config.ip,
                    config.port,
                    &config.cluster_name,
                );
                self.registry.deregister_all_instance_checks(&instance_key);

                info!(
                    "Auto-deregistered instance {}:{} from {}/{}/{}",
                    config.ip,
                    config.port,
                    config.namespace,
                    config.group_name,
                    config.service_name
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
