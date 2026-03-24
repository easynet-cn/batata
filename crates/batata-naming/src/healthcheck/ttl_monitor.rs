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

#[cfg(test)]
mod tests {
    use super::super::registry::*;
    use super::*;
    use crate::service::NamingService;
    use std::time::Duration;

    fn create_registry() -> Arc<InstanceCheckRegistry> {
        let naming_service = Arc::new(NamingService::new());
        Arc::new(InstanceCheckRegistry::new(naming_service))
    }

    fn create_ttl_check_config(check_id: &str, ttl_secs: u64) -> InstanceCheckConfig {
        InstanceCheckConfig {
            check_id: check_id.to_string(),
            name: format!("TTL check {}", check_id),
            check_type: CheckType::Ttl,
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
            ttl: Some(Duration::from_secs(ttl_secs)),
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            origin: CheckOrigin::ConsulService,
            initial_status: CheckStatus::Passing,
            consul_service_id: None,
            notes: String::new(),
        }
    }

    #[test]
    fn test_ttl_monitor_skips_non_ttl_checks() {
        let registry = create_registry();
        let monitor = TtlMonitor::new(registry.clone());

        // Register a TCP check (not TTL)
        let mut config = create_ttl_check_config("tcp-check", 5);
        config.check_type = CheckType::Tcp;
        registry.register_check(config);

        // Scan should not affect TCP checks
        monitor.scan_expired_ttl_checks();

        let (_, status) = registry.get_check("tcp-check").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "TCP check should not be affected by TTL monitor"
        );
    }

    #[test]
    fn test_ttl_monitor_skips_already_critical() {
        let registry = create_registry();
        let monitor = TtlMonitor::new(registry.clone());

        // Register a TTL check and make it critical
        let config = create_ttl_check_config("ttl-critical", 1);
        registry.register_check(config);
        registry.ttl_update(
            "ttl-critical",
            CheckStatus::Critical,
            Some("already critical".to_string()),
        );

        // Scan should skip already-critical checks (no double update)
        monitor.scan_expired_ttl_checks();

        let (_, status) = registry.get_check("ttl-critical").unwrap();
        assert_eq!(status.status, CheckStatus::Critical);
        // Output should remain the original, not "TTL expired"
        assert_eq!(status.output, "already critical");
    }

    #[test]
    fn test_ttl_monitor_detects_expired_check() {
        let registry = create_registry();
        let monitor = TtlMonitor::new(registry.clone());

        // Register a TTL check with very short TTL (1ms)
        let config = create_ttl_check_config("ttl-expire", 0);
        // TTL of 0 seconds = immediate expiration
        let mut config = config;
        config.ttl = Some(Duration::from_millis(1));
        registry.register_check(config);

        // Wait to ensure TTL expires
        std::thread::sleep(Duration::from_millis(10));

        // Scan should detect the expired check
        monitor.scan_expired_ttl_checks();

        let (_, status) = registry.get_check("ttl-expire").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Critical,
            "Expired TTL check should be marked Critical"
        );
        assert!(
            status.output.contains("TTL expired"),
            "Output should mention TTL expired"
        );
    }

    #[test]
    fn test_ttl_monitor_preserves_fresh_check() {
        let registry = create_registry();
        let monitor = TtlMonitor::new(registry.clone());

        // Register a TTL check with long TTL
        let config = create_ttl_check_config("ttl-fresh", 3600);
        registry.register_check(config);

        // Scan should not affect fresh checks
        monitor.scan_expired_ttl_checks();

        let (_, status) = registry.get_check("ttl-fresh").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "Fresh TTL check should remain Passing"
        );
    }

    #[test]
    fn test_ttl_monitor_renewed_check_stays_passing() {
        let registry = create_registry();
        let monitor = TtlMonitor::new(registry.clone());

        // Register a TTL check with 2s TTL
        let config = create_ttl_check_config("ttl-renewed", 2);
        registry.register_check(config);

        // Renew it
        registry.ttl_update(
            "ttl-renewed",
            CheckStatus::Passing,
            Some("renewed".to_string()),
        );

        // Scan should not expire it
        monitor.scan_expired_ttl_checks();

        let (_, status) = registry.get_check("ttl-renewed").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "Renewed check should stay Passing"
        );
    }
}
