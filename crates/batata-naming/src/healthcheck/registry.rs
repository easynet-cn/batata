//! Unified health check registry
//!
//! InstanceCheckRegistry provides a single source of truth for all health checks.
//! It:
//! - Stores check configs and runtime statuses
//! - Indexes checks by instance and service for fast lookup
//! - Aggregates multiple checks per instance into a single healthy/unhealthy decision
//! - Immediately syncs health status changes via HealthCheckResultHandler

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use tracing::{debug, info, warn};

use batata_plugin::HealthCheckResultHandler;

/// Tri-state health check status
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CheckStatus {
    /// Healthy
    Passing,
    /// Degraded but still healthy
    Warning,
    /// Unhealthy
    Critical,
}

impl CheckStatus {
    /// String representation of the status
    pub fn as_str(&self) -> &str {
        match self {
            Self::Passing => "passing",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    /// Whether this status is considered healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Passing | Self::Warning)
    }
}

/// Health check type
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CheckType {
    /// No active health check
    None,
    /// TCP connection test
    Tcp,
    /// HTTP GET request
    Http,
    /// TTL-based passive check
    Ttl,
    /// gRPC health protocol
    Grpc,
    /// MySQL/Database health check
    Mysql,
}

impl CheckType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "",
            Self::Tcp => "tcp",
            Self::Http => "http",
            Self::Ttl => "ttl",
            Self::Grpc => "grpc",
            Self::Mysql => "mysql",
        }
    }

    /// Parse from string
    pub fn from_str_value(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "tcp" => Self::Tcp,
            "http" => Self::Http,
            "ttl" => Self::Ttl,
            "grpc" => Self::Grpc,
            "mysql" => Self::Mysql,
            "none" | "" => Self::None,
            _ => Self::None,
        }
    }

    /// Whether this check type requires active (outbound) checking
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Tcp | Self::Http | Self::Grpc | Self::Mysql)
    }
}

/// Check configuration (static after registration)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstanceCheckConfig {
    /// Unique check ID
    pub check_id: String,
    /// Human-readable name
    pub name: String,
    /// Check protocol type
    pub check_type: CheckType,
    // Location coordinates
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
    pub ip: String,
    pub port: i32,
    pub cluster_name: String,
    // Active check params
    pub http_url: Option<String>,
    pub tcp_addr: Option<String>,
    pub grpc_addr: Option<String>,
    /// Database connection URL for database health check (MySQL/PostgreSQL/SQLite)
    pub db_url: Option<String>,
    pub interval: Duration,
    pub timeout: Duration,
    // TTL params
    pub ttl: Option<Duration>,
    // Thresholds
    /// Consecutive successes before transitioning to Passing (default: 0 = immediate)
    pub success_before_passing: u32,
    /// Consecutive failures before transitioning to Critical (default: 0 = immediate; Nacos uses 3)
    pub failures_before_critical: u32,
    // Auto-deregistration
    pub deregister_critical_after: Option<Duration>,
    /// Initial status when check is registered
    pub initial_status: CheckStatus,
    /// Notes / maintenance reason
    pub notes: String,
    /// Service tags (carried from service registration for health check responses)
    #[cfg_attr(test, allow(dead_code))]
    pub service_tags: Vec<String>,
}

/// Check runtime status (updated frequently)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstanceCheckStatus {
    /// Current check status
    pub status: CheckStatus,
    /// Last check output message
    pub output: String,
    /// Timestamp in milliseconds
    pub last_updated: i64,
    /// Consecutive successes
    pub consecutive_successes: u32,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Timestamp when entered Critical state (for auto-deregistration)
    pub critical_since: Option<i64>,
    /// Last response time in milliseconds
    pub last_response_time_ms: u64,
}

impl InstanceCheckStatus {
    /// Create a new status with the given initial state
    fn new(initial_status: CheckStatus) -> Self {
        let now = current_timestamp_ms();
        let critical_since = if initial_status == CheckStatus::Critical {
            Some(now)
        } else {
            None
        };
        Self {
            status: initial_status,
            output: String::new(),
            last_updated: now,
            consecutive_successes: 0,
            consecutive_failures: 0,
            critical_since,
            last_response_time_ms: 0,
        }
    }
}

/// Build an instance key from location coordinates
pub fn build_instance_key(
    namespace: &str,
    group_name: &str,
    service_name: &str,
    ip: &str,
    port: i32,
    cluster_name: &str,
) -> String {
    format!(
        "{}#{}#{}#{}#{}#{}",
        namespace, group_name, service_name, ip, port, cluster_name
    )
}

/// Build a service key from location coordinates
pub fn build_check_service_key(namespace: &str, group_name: &str, service_name: &str) -> String {
    format!("{}#{}#{}", namespace, group_name, service_name)
}

/// Unified health check registry
///
/// Central store for all health check configurations and their runtime statuses.
/// Indexes checks by instance and service for fast lookup, aggregates multiple
/// checks per instance into a single healthy/unhealthy decision, and syncs
/// health status changes via the result handler.
pub struct InstanceCheckRegistry {
    // Primary storage: check_key → config/status
    configs: DashMap<String, InstanceCheckConfig>,
    statuses: DashMap<String, InstanceCheckStatus>,

    // Index: instance_key → [check_key]
    instance_checks: DashMap<String, Vec<String>>,

    // Index: service_key → {instance_key}
    service_instances: DashMap<String, HashSet<String>>,

    // Result handler — called when health status changes.
    result_handler: Arc<dyn HealthCheckResultHandler>,
}

impl InstanceCheckRegistry {
    /// Create a new registry with the given result handler.
    pub fn new(result_handler: Arc<dyn HealthCheckResultHandler>) -> Self {
        Self {
            configs: DashMap::new(),
            statuses: DashMap::new(),
            instance_checks: DashMap::new(),
            service_instances: DashMap::new(),
            result_handler,
        }
    }

    /// Convenience constructor for the core Batata naming service.
    ///
    /// Creates a `CoreResultHandler` wrapping the given NamingServiceProvider,
    /// so health status changes are synced to the NamingService.
    pub fn with_naming_service(
        naming_service: Arc<dyn batata_api::naming::NamingServiceProvider>,
    ) -> Self {
        let handler: Arc<dyn HealthCheckResultHandler> = Arc::new(
            super::result_handler::CoreResultHandler::new(naming_service),
        );
        Self::new(handler)
    }

    /// Register a new health check. Returns the check key.
    pub fn register_check(&self, config: InstanceCheckConfig) -> String {
        let check_key = config.check_id.clone();
        let instance_key = build_instance_key(
            &config.namespace,
            &config.group_name,
            &config.service_name,
            &config.ip,
            config.port,
            &config.cluster_name,
        );
        let service_key =
            build_check_service_key(&config.namespace, &config.group_name, &config.service_name);

        // Initialize status
        let status = InstanceCheckStatus::new(config.initial_status.clone());
        self.statuses.insert(check_key.clone(), status);

        // Update instance_checks index
        self.instance_checks
            .entry(instance_key.clone())
            .or_default()
            .push(check_key.clone());

        // Update service_instances index
        self.service_instances
            .entry(service_key)
            .or_default()
            .insert(instance_key);

        // Store config
        self.configs.insert(check_key.clone(), config);

        info!("Registered health check: {}", check_key);
        check_key
    }

    /// Deregister a health check by check_key
    pub fn deregister_check(&self, check_key: &str) {
        if let Some((_, config)) = self.configs.remove(check_key) {
            self.statuses.remove(check_key);

            // Remove from instance_checks index
            let instance_key = build_instance_key(
                &config.namespace,
                &config.group_name,
                &config.service_name,
                &config.ip,
                config.port,
                &config.cluster_name,
            );
            if let Some(mut checks) = self.instance_checks.get_mut(&instance_key) {
                checks.retain(|k| k != check_key);
                if checks.is_empty() {
                    drop(checks);
                    self.instance_checks.remove(&instance_key);
                    // Also clean up service_instances index
                    let service_key = build_check_service_key(
                        &config.namespace,
                        &config.group_name,
                        &config.service_name,
                    );
                    if let Some(mut instances) = self.service_instances.get_mut(&service_key) {
                        instances.remove(&instance_key);
                        if instances.is_empty() {
                            drop(instances);
                            self.service_instances.remove(&service_key);
                        }
                    }
                }
            }

            debug!("Deregistered health check: {}", check_key);
        }
    }

    /// Deregister all checks for a given instance
    pub fn deregister_all_instance_checks(&self, instance_key: &str) {
        if let Some((_, check_keys)) = self.instance_checks.remove(instance_key) {
            for check_key in &check_keys {
                if let Some((_, _config)) = self.configs.remove(check_key) {
                    self.statuses.remove(check_key);
                }
            }

            // Clean up service_instances index - find the service key from any removed config
            // We need to iterate check_keys since configs are already removed
            // Use the instance_key format: namespace#group#service#ip#port#cluster
            let parts: Vec<&str> = instance_key.splitn(6, '#').collect();
            if parts.len() >= 3 {
                let service_key = format!("{}#{}#{}", parts[0], parts[1], parts[2]);
                if let Some(mut instances) = self.service_instances.get_mut(&service_key) {
                    instances.remove(instance_key);
                    if instances.is_empty() {
                        drop(instances);
                        self.service_instances.remove(&service_key);
                    }
                }
            }

            debug!(
                "Deregistered {} checks for instance: {}",
                check_keys.len(),
                instance_key
            );
        }
    }

    /// Update check result from an active check execution.
    /// Handles threshold counting and aggregation + sync.
    pub fn update_check_result(
        &self,
        check_key: &str,
        success: bool,
        output: String,
        response_time_ms: u64,
    ) {
        let config = match self.configs.get(check_key) {
            Some(c) => c.clone(),
            None => return,
        };

        if let Some(mut status) = self.statuses.get_mut(check_key) {
            let now = current_timestamp_ms();
            status.last_updated = now;
            status.last_response_time_ms = response_time_ms;
            status.output = output;

            if success {
                status.consecutive_successes += 1;
                status.consecutive_failures = 0;

                // Apply threshold: only transition to Passing after enough successes
                if status.consecutive_successes > config.success_before_passing
                    && (status.status == CheckStatus::Critical
                        || status.status == CheckStatus::Warning)
                {
                    status.status = CheckStatus::Passing;
                    status.critical_since = None;
                    debug!(
                        "Check {} transitioned to Passing after {} successes",
                        check_key, status.consecutive_successes
                    );
                }
            } else {
                status.consecutive_failures += 1;
                status.consecutive_successes = 0;

                // Apply threshold: only transition to Critical after enough failures
                if status.consecutive_failures > config.failures_before_critical
                    && status.status != CheckStatus::Critical
                {
                    status.status = CheckStatus::Critical;
                    status.critical_since = Some(now);
                    debug!(
                        "Check {} transitioned to Critical after {} failures",
                        check_key, status.consecutive_failures
                    );
                    // Notify handler of individual check failure (e.g., Consul session invalidation)
                    self.result_handler.on_check_critical(check_key);
                }
            }
        }

        // Aggregate and sync to NamingService
        self.aggregate_and_sync(&config);
    }

    /// Update check status from a TTL update (manual pass/warn/fail).
    /// No thresholds — immediate status change.
    pub fn ttl_update(&self, check_key: &str, status: CheckStatus, output: Option<String>) {
        let config = match self.configs.get(check_key) {
            Some(c) => c.clone(),
            None => {
                warn!("TTL update for unknown check: {}", check_key);
                return;
            }
        };

        if let Some(mut check_status) = self.statuses.get_mut(check_key) {
            let now = current_timestamp_ms();
            check_status.last_updated = now;
            if let Some(output) = output {
                check_status.output = output;
            }

            let was_critical = check_status.status == CheckStatus::Critical;
            check_status.status = status.clone();

            match status {
                CheckStatus::Critical => {
                    check_status.consecutive_failures += 1;
                    check_status.consecutive_successes = 0;
                    if !was_critical {
                        check_status.critical_since = Some(now);
                        // Notify handler of individual check failure
                        self.result_handler.on_check_critical(check_key);
                    }
                }
                CheckStatus::Passing => {
                    check_status.consecutive_successes += 1;
                    check_status.consecutive_failures = 0;
                    check_status.critical_since = None;
                }
                CheckStatus::Warning => {
                    check_status.consecutive_successes += 1;
                    check_status.consecutive_failures = 0;
                    check_status.critical_since = None;
                }
            }
        }

        // Aggregate and sync to NamingService
        self.aggregate_and_sync(&config);
    }

    /// Get all checks for a given instance
    pub fn get_instance_checks(
        &self,
        instance_key: &str,
    ) -> Vec<(InstanceCheckConfig, InstanceCheckStatus)> {
        let check_keys = match self.instance_checks.get(instance_key) {
            Some(keys) => keys.clone(),
            None => return Vec::new(),
        };

        let mut result = Vec::new();
        for key in &check_keys {
            if let Some(config) = self.configs.get(key)
                && let Some(status) = self.statuses.get(key)
            {
                result.push((config.clone(), status.clone()));
            }
        }
        result
    }

    /// Get all registered checks
    pub fn get_all_checks(&self) -> Vec<(InstanceCheckConfig, InstanceCheckStatus)> {
        // Collect keys first to release configs lock before accessing statuses.
        // This prevents holding locks on two DashMaps simultaneously.
        let keys: Vec<String> = self.configs.iter().map(|e| e.key().clone()).collect();
        // --- configs lock released ---

        let mut result = Vec::new();
        for key in &keys {
            if let Some(config) = self.configs.get(key)
                && let Some(status) = self.statuses.get(key)
            {
                result.push((config.clone(), status.clone()));
            }
        }
        result
    }

    /// Get checks filtered by status
    pub fn get_checks_by_status(
        &self,
        status: &CheckStatus,
    ) -> Vec<(InstanceCheckConfig, InstanceCheckStatus)> {
        // Collect matching keys first to release statuses lock before accessing configs.
        // This prevents holding locks on two DashMaps simultaneously and avoids
        // reverse lock ordering (statuses→configs) vs get_all_checks (configs→statuses).
        let matching_keys: Vec<(String, InstanceCheckStatus)> = self
            .statuses
            .iter()
            .filter(|e| &e.value().status == status)
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();
        // --- statuses lock released ---

        let mut result = Vec::new();
        for (key, check_status) in matching_keys {
            if let Some(config) = self.configs.get(&key) {
                result.push((config.clone(), check_status));
            }
        }
        result
    }

    /// Get a single check (config + status) by check_key
    pub fn get_check(&self, check_key: &str) -> Option<(InstanceCheckConfig, InstanceCheckStatus)> {
        let config = self.configs.get(check_key)?.clone();
        let status = self.statuses.get(check_key)?.clone();
        Some((config, status))
    }

    /// Get a single check config by check_key
    pub fn get_check_config(&self, check_key: &str) -> Option<InstanceCheckConfig> {
        self.configs.get(check_key).map(|c| c.clone())
    }

    /// Get a single check status by check_key
    pub fn get_check_status(&self, check_key: &str) -> Option<InstanceCheckStatus> {
        self.statuses.get(check_key).map(|s| s.clone())
    }

    /// Check if a check_key exists in the registry
    pub fn has_check(&self, check_key: &str) -> bool {
        self.configs.contains_key(check_key)
    }

    /// Get the number of registered checks
    pub fn check_count(&self) -> usize {
        self.configs.len()
    }

    /// Get all check keys for a given instance
    pub fn get_check_keys_for_instance(&self, instance_key: &str) -> Vec<String> {
        self.instance_checks
            .get(instance_key)
            .map(|keys| keys.clone())
            .unwrap_or_default()
    }

    /// Get the result handler reference
    pub fn result_handler(&self) -> &Arc<dyn HealthCheckResultHandler> {
        &self.result_handler
    }

    // --- Internal methods ---

    /// Aggregate all checks for an instance and sync to NamingService
    fn aggregate_and_sync(&self, config: &InstanceCheckConfig) {
        let instance_key = build_instance_key(
            &config.namespace,
            &config.group_name,
            &config.service_name,
            &config.ip,
            config.port,
            &config.cluster_name,
        );

        let healthy = self.aggregate_instance_health(&instance_key);

        // Sync via result handler
        self.result_handler.on_health_changed(
            &config.namespace,
            &config.group_name,
            &config.service_name,
            &config.ip,
            config.port,
            &config.cluster_name,
            healthy,
        );

        {
            debug!(
                "Synced instance health: {}:{}@{}/{}/{} healthy={}",
                config.ip,
                config.port,
                config.namespace,
                config.group_name,
                config.service_name,
                healthy,
            );
        }
    }

    /// Aggregate instance health: healthy = !any_check_critical
    fn aggregate_instance_health(&self, instance_key: &str) -> bool {
        let check_keys = match self.instance_checks.get(instance_key) {
            Some(keys) => keys.clone(),
            None => return true, // No checks = default healthy
        };

        if check_keys.is_empty() {
            return true; // No checks = default healthy
        }

        for key in &check_keys {
            if let Some(status) = self.statuses.get(key)
                && status.status == CheckStatus::Critical
            {
                return false;
            }
        }

        true
    }

    // ============================================================================
    // Snapshot / Restore for persistence
    // ============================================================================

    /// Snapshot all check configs for persistence. Returns serializable data.
    ///
    /// Only configs are snapshotted — runtime status is transient and will be
    /// re-established by active check execution after restore.
    pub fn snapshot_configs(&self) -> Vec<InstanceCheckConfig> {
        self.configs.iter().map(|e| e.value().clone()).collect()
    }

    /// Restore check configs from persisted data.
    ///
    /// Registers each config into the registry with its initial status.
    /// Active checks should be scheduled after restore by the caller.
    pub fn restore_configs(&self, configs: Vec<InstanceCheckConfig>) {
        let count = configs.len();
        for config in configs {
            self.register_check(config);
        }
        info!("Restored {} health check configs from snapshot", count);
    }
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_naming_service() -> Arc<crate::service::NamingService> {
        Arc::new(crate::service::NamingService::new())
    }

    fn make_config(
        check_id: &str,
        check_type: CheckType,
        initial_status: CheckStatus,
    ) -> InstanceCheckConfig {
        InstanceCheckConfig {
            check_id: check_id.to_string(),
            name: check_id.to_string(),
            check_type,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            ip: "127.0.0.1".to_string(),
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
            deregister_critical_after: None,
            initial_status,
            notes: String::new(),
            service_tags: vec![],
        }
    }

    #[test]
    fn test_register_and_deregister_check() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let config = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        let key = registry.register_check(config);
        assert_eq!(key, "check-1");
        assert_eq!(registry.check_count(), 1);

        registry.deregister_check("check-1");
        assert_eq!(registry.check_count(), 0);
    }

    #[test]
    fn test_instance_with_two_checks_one_critical() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        // Register two checks for the same instance
        let config1 = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        let config2 = make_config("check-2", CheckType::Http, CheckStatus::Critical);
        registry.register_check(config1);
        registry.register_check(config2);

        // Instance should be unhealthy because one check is Critical
        let instance_key = build_instance_key(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );
        assert!(!registry.aggregate_instance_health(&instance_key));
    }

    #[test]
    fn test_instance_with_two_checks_passing_and_warning() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        // Register two checks for the same instance
        let config1 = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        let mut config2 = make_config("check-2", CheckType::Http, CheckStatus::Passing);
        config2.initial_status = CheckStatus::Warning;
        registry.register_check(config1);
        registry.register_check(config2);

        // Instance should be healthy (passing + warning = healthy)
        let instance_key = build_instance_key(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );
        assert!(registry.aggregate_instance_health(&instance_key));
    }

    #[test]
    fn test_update_check_result_immediate() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let config = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        registry.register_check(config);

        // Fail the check (threshold=0, immediate)
        registry.update_check_result("check-1", false, "Connection refused".to_string(), 50);

        let status = registry.get_check_status("check-1").unwrap();
        assert_eq!(status.status, CheckStatus::Critical);
        assert_eq!(status.consecutive_failures, 1);
        assert!(status.critical_since.is_some());
    }

    #[test]
    fn test_update_check_result_with_threshold() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let mut config = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        config.failures_before_critical = 3; // Need 4 failures (> 3)
        registry.register_check(config);

        // First 3 failures should not transition to Critical
        for i in 0..3 {
            registry.update_check_result("check-1", false, format!("fail {}", i), 50);
            let status = registry.get_check_status("check-1").unwrap();
            assert_eq!(
                status.status,
                CheckStatus::Passing,
                "should still be passing after {} failures",
                i + 1
            );
        }

        // 4th failure should transition to Critical
        registry.update_check_result("check-1", false, "fail 3".to_string(), 50);
        let status = registry.get_check_status("check-1").unwrap();
        assert_eq!(status.status, CheckStatus::Critical);
    }

    #[test]
    fn test_ttl_update() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let config = make_config("check-1", CheckType::Ttl, CheckStatus::Critical);
        registry.register_check(config);

        // TTL pass — immediate, no threshold
        registry.ttl_update("check-1", CheckStatus::Passing, Some("alive".to_string()));

        let status = registry.get_check_status("check-1").unwrap();
        assert_eq!(status.status, CheckStatus::Passing);
        assert_eq!(status.output, "alive");
        assert!(status.critical_since.is_none());
    }

    #[test]
    fn test_deregister_all_instance_checks() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let config1 = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        let config2 = make_config("check-2", CheckType::Http, CheckStatus::Passing);
        registry.register_check(config1);
        registry.register_check(config2);
        assert_eq!(registry.check_count(), 2);

        let instance_key = build_instance_key(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );
        registry.deregister_all_instance_checks(&instance_key);
        assert_eq!(registry.check_count(), 0);
    }

    #[test]
    fn test_get_all_checks() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let config1 = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        let mut config2 = make_config("check-2", CheckType::Http, CheckStatus::Critical);
        config2.ip = "10.0.0.1".to_string(); // Different instance
        registry.register_check(config1);
        registry.register_check(config2);

        let all = registry.get_all_checks();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_get_checks_by_status() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let config1 = make_config("check-1", CheckType::Tcp, CheckStatus::Passing);
        let config2 = make_config("check-2", CheckType::Http, CheckStatus::Critical);
        registry.register_check(config1);
        registry.register_check(config2);

        let critical = registry.get_checks_by_status(&CheckStatus::Critical);
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].0.check_id, "check-2");
    }

    #[test]
    fn test_check_status_healthy_mapping() {
        assert!(CheckStatus::Passing.is_healthy());
        assert!(CheckStatus::Warning.is_healthy());
        assert!(!CheckStatus::Critical.is_healthy());
    }

    #[test]
    fn test_check_type_active() {
        assert!(CheckType::Tcp.is_active());
        assert!(CheckType::Http.is_active());
        assert!(CheckType::Grpc.is_active());
        assert!(!CheckType::Ttl.is_active());
        assert!(!CheckType::None.is_active());
    }

    #[test]
    fn test_no_checks_default_healthy() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        // An instance with no checks should be considered healthy
        assert!(registry.aggregate_instance_health("nonexistent#instance"));
    }

    // ============== Concurrent Stress Tests ==============

    fn make_stress_config(check_id: &str, service_name: &str, ip: &str) -> InstanceCheckConfig {
        InstanceCheckConfig {
            check_id: check_id.to_string(),
            name: check_id.to_string(),
            check_type: CheckType::Ttl,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: service_name.to_string(),
            ip: ip.to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: None,
            grpc_addr: None,
            db_url: None,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            ttl: Some(Duration::from_secs(30)),
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            initial_status: CheckStatus::Passing,
            notes: String::new(),
            service_tags: vec![],
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_register_and_get_all_checks() {
        let ns = test_naming_service();
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(ns));
        let barrier = Arc::new(tokio::sync::Barrier::new(4));

        let mut handles = Vec::new();

        // 2 writers: register checks
        for i in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..30 {
                    let config = make_stress_config(
                        &format!("check-{}-{}", i, j),
                        &format!("svc-{}", i),
                        &format!("10.0.{}.{}", i, j),
                    );
                    registry.register_check(config);
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 readers: get all checks
        for _ in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..30 {
                    let _ = registry.get_all_checks();
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent register + get_all_checks timed out"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_update_status_and_get_by_status() {
        let ns = test_naming_service();
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(ns));

        // Pre-register checks
        for i in 0..20 {
            let config = make_stress_config(
                &format!("upd-check-{}", i),
                "upd-svc",
                &format!("10.1.0.{}", i),
            );
            registry.register_check(config);
        }

        let barrier = Arc::new(tokio::sync::Barrier::new(4));
        let mut handles = Vec::new();

        // 2 writers: update statuses
        for t in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for round in 0..20 {
                    let idx = (t * 10 + round) % 20;
                    let success = round % 2 != 0;
                    registry.update_check_result(
                        &format!("upd-check-{}", idx),
                        success,
                        format!("round-{}", round),
                        1,
                    );
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 readers: get by status
        for _ in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..20 {
                    let _ = registry.get_checks_by_status(&CheckStatus::Passing);
                    let _ = registry.get_checks_by_status(&CheckStatus::Critical);
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent update_status + get_by_status timed out"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_concurrent_register_deregister_and_queries() {
        let ns = test_naming_service();
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(ns));

        // Pre-register checks
        for i in 0..20 {
            let config = make_stress_config(
                &format!("cd-check-{}", i),
                "cd-svc",
                &format!("10.2.0.{}", i),
            );
            registry.register_check(config);
        }

        let barrier = Arc::new(tokio::sync::Barrier::new(6));
        let mut handles = Vec::new();

        // 2 deregister threads
        for t in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for j in 0..10 {
                    let idx = t * 10 + j;
                    registry.deregister_check(&format!("cd-check-{}", idx));
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 reader threads: get all checks + get by status
        for _ in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _ in 0..20 {
                    let _ = registry.get_all_checks();
                    let _ = registry.get_checks_by_status(&CheckStatus::Passing);
                    tokio::task::yield_now().await;
                }
            }));
        }

        // 2 aggregate health queries
        for _ in 0..2 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for i in 0..20 {
                    let instance_key = build_instance_key(
                        "public",
                        "DEFAULT_GROUP",
                        "cd-svc",
                        &format!("10.2.0.{}", i),
                        8080,
                        "DEFAULT",
                    );
                    let _ = registry.aggregate_instance_health(&instance_key);
                    tokio::task::yield_now().await;
                }
            }));
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            for h in handles {
                h.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Deadlock detected: concurrent register + deregister + queries timed out"
        );
    }

    #[test]
    fn test_snapshot_configs_returns_all_checks() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        registry.register_check(make_config(
            "snap-1",
            CheckType::Http,
            CheckStatus::Critical,
        ));
        registry.register_check(make_config("snap-2", CheckType::Tcp, CheckStatus::Passing));
        registry.register_check(make_config("snap-3", CheckType::Ttl, CheckStatus::Warning));

        let snapshot = registry.snapshot_configs();
        assert_eq!(snapshot.len(), 3, "Should snapshot all 3 check configs");

        let ids: Vec<&str> = snapshot.iter().map(|c| c.check_id.as_str()).collect();
        assert!(ids.contains(&"snap-1"));
        assert!(ids.contains(&"snap-2"));
        assert!(ids.contains(&"snap-3"));
    }

    #[test]
    fn test_restore_configs_repopulates_registry() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns.clone());

        // Register and snapshot
        registry.register_check(make_config(
            "restore-1",
            CheckType::Http,
            CheckStatus::Critical,
        ));
        registry.register_check(make_config(
            "restore-2",
            CheckType::Tcp,
            CheckStatus::Passing,
        ));
        let snapshot = registry.snapshot_configs();

        // Create a fresh registry and restore
        let registry2 = InstanceCheckRegistry::with_naming_service(ns);
        assert_eq!(registry2.check_count(), 0);

        registry2.restore_configs(snapshot);
        assert_eq!(registry2.check_count(), 2, "Should have restored 2 checks");

        // Verify individual checks
        let c1 = registry2.get_check_config("restore-1");
        assert!(c1.is_some(), "restore-1 should exist after restore");
        assert_eq!(c1.unwrap().check_type, CheckType::Http);

        let c2 = registry2.get_check_config("restore-2");
        assert!(c2.is_some(), "restore-2 should exist after restore");
        assert_eq!(c2.unwrap().check_type, CheckType::Tcp);
    }

    #[test]
    fn test_snapshot_configs_serializable() {
        let ns = test_naming_service();
        let registry = InstanceCheckRegistry::with_naming_service(ns);

        let mut config = make_config("serial-1", CheckType::Http, CheckStatus::Critical);
        config.http_url = Some("http://localhost:8080/health".into());
        config.deregister_critical_after = Some(Duration::from_secs(90));
        registry.register_check(config);

        let snapshot = registry.snapshot_configs();

        // Verify JSON serialization roundtrip
        let json = serde_json::to_string(&snapshot).expect("Should serialize");
        let deserialized: Vec<InstanceCheckConfig> =
            serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized[0].check_id, "serial-1");
        assert_eq!(
            deserialized[0].http_url,
            Some("http://localhost:8080/health".into())
        );
        assert_eq!(
            deserialized[0].deregister_critical_after,
            Some(Duration::from_secs(90))
        );
    }
}
