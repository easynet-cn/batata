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

#[cfg(test)]
mod tests {
    use super::super::registry::*;
    use super::*;
    use std::time::Duration;

    fn create_test_components() -> (Arc<NamingService>, Arc<InstanceCheckRegistry>) {
        let naming_service = Arc::new(NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::new(naming_service.clone()));
        (naming_service, registry)
    }

    fn create_check_config(
        check_id: &str,
        deregister_after: Option<Duration>,
    ) -> InstanceCheckConfig {
        InstanceCheckConfig {
            check_id: check_id.to_string(),
            name: format!("Check {}", check_id),
            check_type: CheckType::Tcp,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-svc".to_string(),
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: None,
            grpc_addr: None,
            db_url: None,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            ttl: None,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: deregister_after,
            origin: CheckOrigin::ConsulService,
            initial_status: CheckStatus::Passing,
            consul_service_id: None,
            notes: String::new(),
        }
    }

    #[test]
    fn test_reap_skips_checks_without_deregister_config() {
        let (naming_service, registry) = create_test_components();
        let monitor = DeregisterMonitor::new(registry.clone(), naming_service.clone(), 30);

        // Register instance
        let instance = Instance {
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-svc".to_string(),
            healthy: true,
            enabled: true,
            ephemeral: false,
            ..Default::default()
        };
        naming_service.register_instance("public", "DEFAULT_GROUP", "test-svc", instance);

        // Register check WITHOUT deregister_critical_after
        let config = create_check_config("no-deregister", None);
        registry.register_check(config);

        // Make it critical
        registry.ttl_update(
            "no-deregister",
            CheckStatus::Critical,
            Some("failed".to_string()),
        );

        // Reap should skip it
        monitor.reap_critical_instances();

        // Instance should still exist
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(
            instances.len(),
            1,
            "Instance should not be deregistered without deregister config"
        );
    }

    #[test]
    fn test_reap_deregisters_after_threshold() {
        let (naming_service, registry) = create_test_components();
        let monitor = DeregisterMonitor::new(registry.clone(), naming_service.clone(), 30);

        // Register instance
        let instance = Instance {
            ip: "10.0.0.2".to_string(),
            port: 9090,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-svc".to_string(),
            healthy: true,
            enabled: true,
            ephemeral: false,
            ..Default::default()
        };
        naming_service.register_instance("public", "DEFAULT_GROUP", "test-svc", instance);

        // Register check with very short deregister threshold (1ms)
        let config = create_check_config("auto-deregister", Some(Duration::from_millis(1)));
        let mut config = config;
        config.ip = "10.0.0.2".to_string();
        config.port = 9090;
        config.initial_status = CheckStatus::Critical;
        registry.register_check(config);

        // Make it critical (sets critical_since)
        registry.ttl_update(
            "auto-deregister",
            CheckStatus::Critical,
            Some("failed".to_string()),
        );

        // Wait for threshold
        std::thread::sleep(Duration::from_millis(10));

        // Reap should deregister the instance
        monitor.reap_critical_instances();

        // Instance should be gone
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(
            instances.len(),
            0,
            "Instance should be deregistered after critical threshold"
        );

        // Check should also be removed
        assert!(
            registry.get_check("auto-deregister").is_none(),
            "Check should be removed after deregistration"
        );
    }

    #[test]
    fn test_reap_preserves_recently_critical() {
        let (naming_service, registry) = create_test_components();
        let monitor = DeregisterMonitor::new(registry.clone(), naming_service.clone(), 30);

        // Register instance
        let instance = Instance {
            ip: "10.0.0.3".to_string(),
            port: 7070,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-svc".to_string(),
            healthy: true,
            enabled: true,
            ephemeral: false,
            ..Default::default()
        };
        naming_service.register_instance("public", "DEFAULT_GROUP", "test-svc", instance);

        // Register check with long deregister threshold (1 hour)
        let mut config = create_check_config("recent-critical", Some(Duration::from_secs(3600)));
        config.ip = "10.0.0.3".to_string();
        config.port = 7070;
        registry.register_check(config);

        // Make it critical
        registry.ttl_update(
            "recent-critical",
            CheckStatus::Critical,
            Some("just failed".to_string()),
        );

        // Reap immediately — should NOT deregister (threshold not reached)
        monitor.reap_critical_instances();

        // Instance should still exist
        let instances =
            naming_service.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(
            instances.len(),
            1,
            "Recently critical instance should not be deregistered"
        );
    }
}
