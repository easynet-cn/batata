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
        assert!(!unhealthy_checker.running.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!expired_checker.running.load(std::sync::atomic::Ordering::SeqCst));
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
}
