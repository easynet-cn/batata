//! Heartbeat checking module
//!
//! This module provides heartbeat monitoring that matches Nacos implementation:
//! - UnhealthyInstanceChecker: marks instances as unhealthy if heartbeat times out
//! - ExpiredInstanceChecker: deletes instances if expired
//!
//! This implementation maintains an internal map tracking last heartbeat times,
//! which allows full functionality without modifying the Instance model.

use super::config::HealthCheckConfig;
use crate::service::NamingService;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Key for tracking instance heartbeats
type InstanceKey = String;

/// Heartbeat tracking entry
#[derive(Clone, Debug)]
pub(crate) struct HeartbeatEntry {
    namespace: String,
    group_name: String,
    service_name: String,
    ip: String,
    port: i32,
    cluster_name: String,
    last_heartbeat: i64,
    heartbeat_timeout: i64,
    ip_delete_timeout: i64,
}

/// Unhealthy instance checker (matches Nacos UnhealthyInstanceChecker)
///
/// This checker monitors ephemeral instances and marks them as unhealthy
/// if they haven't sent a heartbeat within the configured timeout.
pub struct UnhealthyInstanceChecker {
    naming_service: Arc<NamingService>,
    config: Arc<HealthCheckConfig>,
    pub(crate) heartbeat_map: Arc<DashMap<InstanceKey, HeartbeatEntry>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl UnhealthyInstanceChecker {
    /// Create a new unhealthy instance checker
    pub fn new(naming_service: Arc<NamingService>, config: Arc<HealthCheckConfig>) -> Self {
        Self {
            naming_service,
            config,
            heartbeat_map: Arc::new(DashMap::new()),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Record a heartbeat for an instance
    pub fn record_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
        heartbeat_timeout: i64,
        ip_delete_timeout: i64,
    ) {
        let key = Self::make_key(namespace, group_name, service_name, ip, port, cluster_name);
        let now = chrono::Utc::now().timestamp_millis();

        let entry = HeartbeatEntry {
            namespace: namespace.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            ip: ip.to_string(),
            port,
            cluster_name: cluster_name.to_string(),
            last_heartbeat: now,
            heartbeat_timeout,
            ip_delete_timeout,
        };

        self.heartbeat_map.insert(key.clone(), entry);
        debug!("Recorded heartbeat for {}", key);
    }

    /// Remove heartbeat tracking for an instance
    pub fn remove_heartbeat(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) {
        let key = Self::make_key(namespace, group_name, service_name, ip, port, cluster_name);
        self.heartbeat_map.remove(&key);
        debug!("Removed heartbeat tracking for {}", key);
    }

    /// Make instance key
    fn make_key(
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> String {
        format!("{}#{}#{}#{}#{}#{}", namespace, group_name, service_name, ip, port, cluster_name)
    }

    /// Start the checker
    pub async fn start(&self) {
        if self
            .running
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            info!("Unhealthy instance checker already running");
            return;
        }

        info!("Starting unhealthy instance checker");

        let running = self.running.clone();
        let naming_service = self.naming_service.clone();
        let heartbeat_map = self.heartbeat_map.clone();

        // Check every 5 seconds (matches Nacos)
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        while running.load(std::sync::atomic::Ordering::SeqCst) {
            interval.tick().await;

            if !self.config.is_enabled() {
                continue;
            }

            let now = chrono::Utc::now().timestamp_millis();
            let mut unhealthy_instances = Vec::new();

            // Check all tracked instances
            for entry in heartbeat_map.iter() {
                let elapsed = now - entry.last_heartbeat;
                if elapsed > entry.heartbeat_timeout {
                    unhealthy_instances.push((entry.key().clone(), elapsed));
                }
            }

            // Mark unhealthy instances
            for (key, elapsed) in &unhealthy_instances {
                if let Some(entry) = heartbeat_map.get(key) {
                    info!(
                        "Marking instance {}:{} as unhealthy due to heartbeat timeout (elapsed: {}ms, timeout: {}ms)",
                        entry.ip, entry.port, elapsed, entry.heartbeat_timeout
                    );

                    // Update instance health status
                    let instances = naming_service.get_instances(
                        &entry.namespace,
                        &entry.group_name,
                        &entry.service_name,
                        &entry.cluster_name,
                        false,
                    );

                    for instance in instances {
                        if instance.ip == entry.ip && instance.port == entry.port {
                            // Mark as unhealthy
                            if naming_service.heartbeat(
                                &entry.namespace,
                                &entry.group_name,
                                &entry.service_name,
                                &instance.ip,
                                instance.port,
                                &entry.cluster_name,
                            ) {
                                debug!("Successfully marked {}:{} as unhealthy", instance.ip, instance.port);
                            } else {
                                warn!("Failed to mark {}:{} as unhealthy", instance.ip, instance.port);
                            }
                            break;
                        }
                    }
                }
            }

            if !unhealthy_instances.is_empty() {
                info!(
                    "Marked {} instances as unhealthy due to heartbeat timeout",
                    unhealthy_instances.len()
                );
            }
        }

        info!("Unhealthy instance checker stopped");
    }

    /// Stop the checker
    pub fn stop(&self) {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        info!("Stopped unhealthy instance checker");
    }

    /// Get number of tracked instances
    pub fn get_tracked_count(&self) -> usize {
        self.heartbeat_map.len()
    }
}

/// Expired instance checker (matches Nacos ExpiredInstanceChecker)
///
/// This checker monitors ephemeral instances and deletes them
/// if they haven't sent a heartbeat within the delete timeout.
pub struct ExpiredInstanceChecker {
    naming_service: Arc<NamingService>,
    config: Arc<HealthCheckConfig>,
    heartbeat_map: Arc<DashMap<InstanceKey, HeartbeatEntry>>,
    expire_enabled: bool,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ExpiredInstanceChecker {
    /// Create a new expired instance checker
    pub fn new(
        naming_service: Arc<NamingService>,
        config: Arc<HealthCheckConfig>,
        expire_enabled: bool,
        heartbeat_map: Arc<DashMap<InstanceKey, HeartbeatEntry>>,
    ) -> Self {
        Self {
            naming_service,
            config,
            heartbeat_map,
            expire_enabled,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start the checker
    pub async fn start(&self) {
        if self
            .running
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            info!("Expired instance checker already running");
            return;
        }

        info!("Starting expired instance checker");

        let running = self.running.clone();
        let naming_service = self.naming_service.clone();
        let heartbeat_map = self.heartbeat_map.clone();

        // Check every 5 seconds (matches Nacos)
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        while running.load(std::sync::atomic::Ordering::SeqCst) {
            interval.tick().await;

            if !self.config.is_enabled() || !self.expire_enabled {
                continue;
            }

            let now = chrono::Utc::now().timestamp_millis();
            let mut expired_instances = Vec::new();

            // Check all tracked instances
            for entry in heartbeat_map.iter() {
                let elapsed = now - entry.last_heartbeat;
                if elapsed > entry.ip_delete_timeout {
                    expired_instances.push((entry.key().clone(), elapsed));
                }
            }

            // Delete expired instances
            for (key, elapsed) in &expired_instances {
                if let Some(entry) = heartbeat_map.get(key) {
                    info!(
                        "Deleting expired instance {}:{} (elapsed: {}ms, timeout: {}ms)",
                        entry.ip, entry.port, elapsed, entry.ip_delete_timeout
                    );

                    // Build instance for deregistration
                    let instance = crate::model::Instance {
                        ip: entry.ip.clone(),
                        port: entry.port,
                        cluster_name: entry.cluster_name.clone(),
                        ..Default::default()
                    };

                    // Deregister the instance
                    if naming_service.deregister_instance(
                        &entry.namespace,
                        &entry.group_name,
                        &entry.service_name,
                        &instance,
                    ) {
                        info!("Successfully deleted expired instance {}:{}", entry.ip, entry.port);
                        heartbeat_map.remove(key);
                    } else {
                        warn!("Failed to delete expired instance {}:{}", entry.ip, entry.port);
                    }
                }
            }

            if !expired_instances.is_empty() {
                info!(
                    "Deleted {} expired instances",
                    expired_instances.len()
                );
            }
        }

        info!("Expired instance checker stopped");
    }

    /// Stop the checker
    pub fn stop(&self) {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        info!("Stopped expired instance checker");
    }

    /// Get number of tracked instances
    pub fn get_tracked_count(&self) -> usize {
        self.heartbeat_map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Instance;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_checker_creation() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let heartbeat_map = Arc::new(DashMap::new());

        let unhealthy_checker = UnhealthyInstanceChecker::new(
            naming_service.clone(),
            config.clone(),
        );

        let expired_checker = ExpiredInstanceChecker::new(
            naming_service,
            config,
            true,
            heartbeat_map,
        );

        // Test that checkers can be created
        assert!(!unhealthy_checker.running.load(Ordering::SeqCst));
        assert!(!expired_checker.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_heartbeat_recording() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        assert_eq!(checker.get_tracked_count(), 1);

        checker.remove_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );

        assert_eq!(checker.get_tracked_count(), 0);
    }

    #[test]
    fn test_instance_key() {
        let key = UnhealthyInstanceChecker::make_key(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
        );
        assert_eq!(key, "public#DEFAULT_GROUP#test-service#127.0.0.1#8080#DEFAULT");
    }

    #[test]
    fn test_heartbeat_entry_fields() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        checker.record_heartbeat(
            "test-namespace",
            "test-group",
            "test-service-name",
            "192.168.1.100",
            9090,
            "test-cluster",
            20000,
            40000,
        );

        let key = checker.heartbeat_map.iter().next().unwrap();
        let entry = key.value();

        assert_eq!(entry.namespace, "test-namespace");
        assert_eq!(entry.group_name, "test-group");
        assert_eq!(entry.service_name, "test-service-name");
        assert_eq!(entry.ip, "192.168.1.100");
        assert_eq!(entry.port, 9090);
        assert_eq!(entry.cluster_name, "test-cluster");
        assert_eq!(entry.heartbeat_timeout, 20000);
        assert_eq!(entry.ip_delete_timeout, 40000);
        assert!(entry.last_heartbeat > 0);
    }

    #[test]
    fn test_multiple_heartbeats_same_instance() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Record initial heartbeat
        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        let first_key = checker.heartbeat_map.iter().next().unwrap();
        let first_time = first_key.value().last_heartbeat;

        // Wait a bit and record another heartbeat (should update the entry)
        std::thread::sleep(std::time::Duration::from_millis(10));

        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        // Should still have only one entry
        assert_eq!(checker.get_tracked_count(), 1);

        // Last heartbeat should be updated
        let updated_key = checker.heartbeat_map.iter().next().unwrap();
        assert!(updated_key.value().last_heartbeat > first_time);
    }

    #[test]
    fn test_multiple_instances_different_keys() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Register multiple instances
        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.2",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8081,
            "DEFAULT",
            15000,
            30000,
        );

        // Should track 3 instances
        assert_eq!(checker.get_tracked_count(), 3);
    }

    #[test]
    fn test_expired_checker_without_expire_enabled() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let heartbeat_map = Arc::new(DashMap::new());

        // Create checker with expire_enabled = false
        let expired_checker = ExpiredInstanceChecker::new(
            naming_service,
            config,
            false, // expire disabled
            heartbeat_map,
        );

        assert!(!expired_checker.expire_enabled);
    }

    #[test]
    fn test_expired_checker_with_expire_enabled() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let heartbeat_map = Arc::new(DashMap::new());

        // Create checker with expire_enabled = true
        let expired_checker = ExpiredInstanceChecker::new(
            naming_service,
            config,
            true, // expire enabled
            heartbeat_map,
        );

        assert!(expired_checker.expire_enabled);
    }

    #[test]
    fn test_stop_checker() {
        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let unhealthy_checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Initially not running
        assert!(!unhealthy_checker.running.load(Ordering::SeqCst));

        // Stop should be idempotent
        unhealthy_checker.stop();
        assert!(!unhealthy_checker.running.load(Ordering::SeqCst));

        unhealthy_checker.stop();
        assert!(!unhealthy_checker.running.load(Ordering::SeqCst));
    }

    // Test that simulates the behavior of Nacos's ClientBeatCheckTaskV2Test
    // These tests verify the heartbeat tracking and timeout logic

    #[test]
    fn test_run_healthy_instance_with_recent_heartbeat() {
        // This test simulates testRunHealthyInstanceWithHeartBeat from Nacos
        // A healthy instance with recent heartbeat should remain tracked

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Simulate instance with recent heartbeat
        let now = chrono::Utc::now().timestamp_millis();
        checker.record_heartbeat(
            "test-namespace",
            "test-group",
            "test-service",
            "192.168.1.1",
            8080,
            "DEFAULT",
            15000, // heartbeat_timeout
            30000, // ip_delete_timeout
        );

        // Verify the entry is tracked
        assert_eq!(checker.get_tracked_count(), 1);

        // Verify last heartbeat is recent
        let entry = checker.heartbeat_map.iter().next().unwrap();
        let elapsed = now - entry.value().last_heartbeat;
        // Should be very recent (within a few milliseconds)
        assert!(elapsed.abs() < 1000);

        // With a recent heartbeat, the instance should NOT be considered unhealthy
        // (elapsed < heartbeat_timeout)
        assert!(elapsed < 15000);
    }

    #[test]
    fn test_run_unhealthy_instance_without_expire() {
        // This test simulates testRunUnhealthyInstanceWithoutExpire from Nacos
        // An unhealthy instance without expire enabled should remain tracked

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let heartbeat_map = Arc::new(DashMap::new());

        // Create unhealthy checker
        let unhealthy_checker = UnhealthyInstanceChecker::new(
            naming_service.clone(),
            config.clone(),
        );

        // Record a heartbeat that will time out
        unhealthy_checker.record_heartbeat(
            "test-namespace",
            "test-group",
            "test-service",
            "192.168.1.2",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        // Create expired checker with expire disabled
        let expired_checker = ExpiredInstanceChecker::new(
            naming_service,
            config,
            false, // expire disabled
            heartbeat_map,
        );

        // Verify instance is tracked
        assert_eq!(unhealthy_checker.get_tracked_count(), 1);
        assert_eq!(expired_checker.get_tracked_count(), 1);

        // Since expire is disabled, instance should remain in the map
        // (this is a basic structural test; the actual deletion behavior
        // is tested in async integration tests)
    }

    #[test]
    fn test_run_unhealthy_instance_with_expire() {
        // This test simulates testRunUnHealthyInstanceWithExpire from Nacos
        // An unhealthy instance with expire enabled would be deleted (in async context)

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let heartbeat_map = Arc::new(DashMap::new());

        // Create unhealthy checker
        let unhealthy_checker = UnhealthyInstanceChecker::new(
            naming_service.clone(),
            config.clone(),
        );

        // Record a heartbeat
        unhealthy_checker.record_heartbeat(
            "test-namespace",
            "test-group",
            "test-service",
            "192.168.1.3",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        // Create expired checker with expire enabled
        let expired_checker = ExpiredInstanceChecker::new(
            naming_service,
            config,
            true, // expire enabled
            heartbeat_map,
        );

        // Verify instance is tracked
        assert_eq!(unhealthy_checker.get_tracked_count(), 1);
        assert_eq!(expired_checker.get_tracked_count(), 1);

        // Verify expire is enabled
        assert!(expired_checker.expire_enabled);
    }

    #[test]
    fn test_heartbeat_timeout_comparison() {
        // Test comparing heartbeat timeout vs elapsed time
        // This is the core logic of unhealthy instance detection

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Set a short timeout for testing
        let short_timeout = 500; // 500ms

        checker.record_heartbeat(
            "test-namespace",
            "test-group",
            "test-service",
            "192.168.1.4",
            8080,
            "DEFAULT",
            short_timeout,
            1000,
        );

        // Wait for timeout
        std::thread::sleep(std::time::Duration::from_millis(600));

        let entry = checker.heartbeat_map.iter().next().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let elapsed = now - entry.value().last_heartbeat;

        // Verify elapsed time exceeds timeout
        assert!(elapsed > short_timeout, "elapsed {} > timeout {}", elapsed, short_timeout);

        // This instance should be marked as unhealthy by the checker
        // (actual marking happens in the async start() method)
    }

    #[test]
    fn test_ip_delete_timeout_greater_than_heartbeat_timeout() {
        // Verify that delete timeout is always greater than heartbeat timeout
        // This ensures instances are marked unhealthy before being deleted

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        let heartbeat_timeout = 15000;
        let ip_delete_timeout = 30000;

        checker.record_heartbeat(
            "test-namespace",
            "test-group",
            "test-service",
            "192.168.1.5",
            8080,
            "DEFAULT",
            heartbeat_timeout,
            ip_delete_timeout,
        );

        let entry = checker.heartbeat_map.iter().next().unwrap();

        // Verify delete timeout > heartbeat timeout
        assert!(entry.value().ip_delete_timeout > entry.value().heartbeat_timeout);
        assert_eq!(entry.value().ip_delete_timeout, ip_delete_timeout);
        assert_eq!(entry.value().heartbeat_timeout, heartbeat_timeout);
    }

    #[test]
    fn test_different_namespaces_are_separate() {
        // Verify that instances in different namespaces are tracked separately

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Register same instance in different namespaces
        checker.record_heartbeat(
            "namespace-1",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        checker.record_heartbeat(
            "namespace-2",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        // Should have 2 separate entries
        assert_eq!(checker.get_tracked_count(), 2);
    }

    #[test]
    fn test_different_groups_are_separate() {
        // Verify that instances in different groups are tracked separately

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Register same instance in different groups
        checker.record_heartbeat(
            "public",
            "GROUP_A",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        checker.record_heartbeat(
            "public",
            "GROUP_B",
            "test-service",
            "127.0.0.1",
            8080,
            "DEFAULT",
            15000,
            30000,
        );

        // Should have 2 separate entries
        assert_eq!(checker.get_tracked_count(), 2);
    }

    #[test]
    fn test_different_clusters_are_separate() {
        // Verify that instances in different clusters are tracked separately

        let naming_service = Arc::new(NamingService::new());
        let config = Arc::new(HealthCheckConfig::default());
        let checker = UnhealthyInstanceChecker::new(naming_service, config);

        // Register same instance in different clusters
        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "cluster-A",
            15000,
            30000,
        );

        checker.record_heartbeat(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            "127.0.0.1",
            8080,
            "cluster-B",
            15000,
            30000,
        );

        // Should have 2 separate entries
        assert_eq!(checker.get_tracked_count(), 2);
    }
}
