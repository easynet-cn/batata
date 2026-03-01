//! Health check task for individual instances
//!
//! This module provides the health check task that manages
//! health checking for a single instance, including:
//! - Adaptive check interval (based on response time)
//! - Failure threshold tracking
//! - Health status updates

use super::config::HealthCheckConfig;
use super::processor::{HealthCheckProcessor, HealthCheckResult, HealthCheckType};
use crate::model::Instance;
use crate::service::{ClusterConfig, NamingService};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Lower bound for normalized check time (2 seconds, matches Nacos)
const LOWER_CHECK_RT: u64 = 2000;

/// Upper random bound for initial check time (5 seconds, matches Nacos)
const UPPER_RANDOM_CHECK_RT: u64 = 5000;

/// Health check task for a single instance
#[derive(Clone)]
pub struct HealthCheckTask {
    /// Instance to check
    instance: Instance,

    /// Service namespace
    namespace: String,

    /// Service group name
    group_name: String,

    /// Service name
    service_name: String,

    /// Cluster configuration
    cluster_config: ClusterConfig,

    /// Health check configuration
    config: Arc<HealthCheckConfig>,

    /// Naming service for updating health status
    naming_service: Arc<NamingService>,

    /// Task ID (unique identifier)
    task_id: String,

    /// Normalized check interval (adaptive)
    check_rt_normalized: Duration,

    /// Best check time observed
    check_rt_best: Duration,

    /// Worst check time observed
    check_rt_worst: Duration,

    /// Last check time
    check_rt_last: Duration,

    /// Second to last check time
    check_rt_last_last: Duration,

    /// Task start time
    start_time: i64,

    /// Consecutive failure count
    consecutive_failures: u32,

    /// Current health status
    healthy: bool,

    /// Check type
    check_type: HealthCheckType,
}

impl std::fmt::Debug for HealthCheckTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthCheckTask")
            .field("task_id", &self.task_id)
            .field("instance", &self.instance)
            .field("healthy", &self.healthy)
            .field("consecutive_failures", &self.consecutive_failures)
            .field("check_type", &self.check_type)
            .finish()
    }
}

impl HealthCheckTask {
    /// Create a new health check task
    pub fn new(
        instance: Instance,
        namespace: String,
        group_name: String,
        service_name: String,
        cluster_config: ClusterConfig,
        config: Arc<HealthCheckConfig>,
        naming_service: Arc<NamingService>,
    ) -> Self {
        let task_id = format!(
            "{}:{}:{}",
            instance.ip, instance.port, instance.cluster_name
        );
        let check_type = HealthCheckType::from_str(&cluster_config.health_check_type);

        let check_rt_normalized = Self::init_check_interval(&config, &check_type);

        Self {
            instance,
            namespace,
            group_name,
            service_name,
            cluster_config,
            config,
            naming_service,
            task_id,
            check_rt_normalized,
            check_rt_best: Duration::from_secs(9999),
            check_rt_worst: Duration::ZERO,
            check_rt_last: Duration::ZERO,
            check_rt_last_last: Duration::ZERO,
            start_time: chrono::Utc::now().timestamp_millis(),
            consecutive_failures: 0,
            healthy: true,
            check_type,
        }
    }

    /// Initialize check interval with random delay (matches Nacos)
    fn init_check_interval(config: &HealthCheckConfig, check_type: &HealthCheckType) -> Duration {
        let max = match check_type {
            HealthCheckType::Tcp => config.tcp_health_params.max,
            HealthCheckType::Http => config.http_health_params.max,
            HealthCheckType::None | HealthCheckType::Ttl | HealthCheckType::Grpc => {
                return Duration::from_secs(5);
            }
        };

        let random_delay = fastrand::u64(0..=UPPER_RANDOM_CHECK_RT);
        let random_upper = random_delay.min(max);
        Duration::from_millis(LOWER_CHECK_RT + random_upper)
    }

    /// Get task ID
    pub fn get_task_id(&self) -> &str {
        &self.task_id
    }

    /// Get normalized check interval (adaptive)
    pub fn get_check_rt_normalized(&self) -> Duration {
        self.check_rt_normalized
    }

    /// Execute the health check
    pub async fn do_check<P>(&mut self, processor: &P) -> HealthCheckResult
    where
        P: HealthCheckProcessor,
    {
        let start_time = std::time::Instant::now();

        // Perform health check
        let result = processor.check(&self.instance, &self.cluster_config).await;

        let response_time = start_time.elapsed();

        // Update check time statistics
        self.update_check_time_stats(response_time);

        // Process result
        self.process_result(result.clone(), response_time).await;

        result
    }

    /// Update check time statistics (matches Nacos logic)
    fn update_check_time_stats(&mut self, response_time: Duration) {
        self.check_rt_last_last = self.check_rt_last;
        self.check_rt_last = response_time;

        if response_time < self.check_rt_best {
            self.check_rt_best = response_time;
        }

        if response_time > self.check_rt_worst {
            self.check_rt_worst = response_time;
        }
    }

    /// Process health check result and update status
    async fn process_result(&mut self, result: HealthCheckResult, _response_time: Duration) {
        let _previous_healthy = self.healthy;
        let failure_threshold = self.config.get_failure_threshold();

        if result.success {
            // Health check passed
            self.consecutive_failures = 0;

            // If previously unhealthy, mark as healthy
            if !self.healthy {
                self.healthy = true;
                info!(
                    "Instance {}:{} became healthy after check",
                    self.instance.ip, self.instance.port
                );

                // Update naming service
                self.update_naming_service_health(true).await;
            }

            // Adjust check interval for success (adaptive, matches Nacos)
            self.adjust_interval_success();
        } else {
            // Health check failed
            self.consecutive_failures += 1;

            // Check if threshold reached
            if self.consecutive_failures >= failure_threshold && self.healthy {
                self.healthy = false;
                warn!(
                    "Instance {}:{} became unhealthy after {} consecutive failures: {:?}",
                    self.instance.ip, self.instance.port, self.consecutive_failures, result.message
                );

                // Update naming service
                self.update_naming_service_health(false).await;
            } else if self.consecutive_failures < failure_threshold {
                debug!(
                    "Instance {}:{} health check failed ({}/{}): {:?}",
                    self.instance.ip,
                    self.instance.port,
                    self.consecutive_failures,
                    failure_threshold,
                    result.message
                );
            }

            // Adjust check interval for failure (adaptive, matches Nacos)
            self.adjust_interval_failure();
        }

        // Update check interval normalization
        self.normalize_check_interval();
    }

    /// Adjust check interval on success (matches Nacos adaptive logic)
    fn adjust_interval_success(&mut self) {
        let factor = match self.check_type {
            HealthCheckType::Tcp => self.config.get_tcp_factor(),
            HealthCheckType::Http => self.config.get_http_factor(),
            HealthCheckType::None | HealthCheckType::Ttl | HealthCheckType::Grpc => return,
        };

        // Speed up checks when healthy (multiply by factor)
        let new_interval = (self.check_rt_normalized.as_millis() as f64 * factor) as u64;
        self.check_rt_normalized = Duration::from_millis(new_interval);
    }

    /// Adjust check interval on failure (matches Nacos adaptive logic)
    fn adjust_interval_failure(&mut self) {
        let factor = match self.check_type {
            HealthCheckType::Tcp => self.config.get_tcp_factor(),
            HealthCheckType::Http => self.config.get_http_factor(),
            HealthCheckType::None | HealthCheckType::Ttl | HealthCheckType::Grpc => return,
        };

        // Slow down checks when failing (increase interval)
        let current = self.check_rt_normalized.as_millis() as f64;
        let max = match self.check_type {
            HealthCheckType::Tcp => self.config.tcp_health_params.max as f64,
            HealthCheckType::Http => self.config.http_health_params.max as f64,
            HealthCheckType::None | HealthCheckType::Ttl | HealthCheckType::Grpc => 5000.0,
        };

        let new_interval = (current * (1.0 - factor) + factor * max) as u64;
        self.check_rt_normalized = Duration::from_millis(new_interval);
    }

    /// Normalize check interval to stay within bounds (matches Nacos)
    fn normalize_check_interval(&mut self) {
        let (min, max) = match self.check_type {
            HealthCheckType::Tcp => (
                self.config.tcp_health_params.min,
                self.config.tcp_health_params.max,
            ),
            HealthCheckType::Http => (
                self.config.http_health_params.min,
                self.config.http_health_params.max,
            ),
            HealthCheckType::None | HealthCheckType::Ttl | HealthCheckType::Grpc => (2000, 5000),
        };

        let current = self.check_rt_normalized.as_millis() as u64;
        let normalized = current.clamp(min, max);
        self.check_rt_normalized = Duration::from_millis(normalized);
    }

    /// Update health status in naming service
    async fn update_naming_service_health(&self, healthy: bool) {
        // Use heartbeat to update health status
        if self.naming_service.heartbeat(
            &self.namespace,
            &self.group_name,
            &self.service_name,
            &self.instance.ip,
            self.instance.port,
            &self.instance.cluster_name,
        ) {
            debug!(
                "Updated instance {}:{} health status to {} in naming service",
                self.instance.ip, self.instance.port, healthy
            );
        } else {
            warn!(
                "Failed to update instance {}:{} health status to {} in naming service",
                self.instance.ip, self.instance.port, healthy
            );
        }
    }

    /// Get current health status
    pub fn is_healthy(&self) -> bool {
        self.healthy
    }

    /// Get consecutive failure count
    pub fn get_consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Get task start time
    pub fn get_start_time(&self) -> i64 {
        self.start_time
    }

    /// Get check type
    pub fn get_check_type(&self) -> &HealthCheckType {
        &self.check_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_format() {
        let instance = Instance {
            ip: "127.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        };
        let config = Arc::new(HealthCheckConfig::default());
        let cluster_config = ClusterConfig::default();
        let naming_service = Arc::new(NamingService::default());

        let task = HealthCheckTask::new(
            instance.clone(),
            "public".to_string(),
            "DEFAULT_GROUP".to_string(),
            "test-service".to_string(),
            cluster_config,
            config,
            naming_service,
        );

        assert_eq!(task.get_task_id(), "127.0.0.1:8080:DEFAULT");
    }

    #[test]
    fn test_task_initial_state() {
        let instance = Instance::default();
        let config = Arc::new(HealthCheckConfig::default());
        let cluster_config = ClusterConfig::default();
        let naming_service = Arc::new(NamingService::default());

        let task = HealthCheckTask::new(
            instance,
            "public".to_string(),
            "DEFAULT_GROUP".to_string(),
            "test-service".to_string(),
            cluster_config,
            config,
            naming_service,
        );

        assert!(task.is_healthy());
        assert_eq!(task.get_consecutive_failures(), 0);
        assert!(task.get_start_time() > 0);
    }
}
