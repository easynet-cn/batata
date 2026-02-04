//! Instance health checker service
//!
//! This module provides TCP and HTTP health checks for service instances
//! based on cluster configuration.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use tracing::{debug, info, warn};

use crate::{
    model::Instance,
    service::{ClusterConfig, NamingService},
};

/// Health check configuration
#[derive(Clone, Debug)]
pub struct InstanceHealthCheckConfig {
    /// Interval between health checks (default: 5 seconds)
    pub check_interval: Duration,
    /// Timeout for health check operations (default: 3 seconds)
    pub check_timeout: Duration,
    /// Number of consecutive failures before marking unhealthy (default: 3)
    pub unhealthy_threshold: u32,
    /// Number of consecutive successes before marking healthy (default: 2)
    pub healthy_threshold: u32,
}

impl Default for InstanceHealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(5),
            check_timeout: Duration::from_secs(3),
            unhealthy_threshold: 3,
            healthy_threshold: 2,
        }
    }
}

/// Health check status for an instance
#[derive(Clone, Debug)]
pub struct InstanceHealthStatus {
    /// Instance key (ip#port#cluster)
    pub instance_key: String,
    /// Current health status
    pub healthy: bool,
    /// Consecutive failure count
    pub fail_count: u32,
    /// Consecutive success count
    pub success_count: u32,
    /// Last check timestamp
    pub last_check_time: i64,
    /// Last success timestamp
    pub last_success_time: Option<i64>,
    /// Last failure message
    pub last_failure_message: Option<String>,
}

impl InstanceHealthStatus {
    fn new(instance_key: String) -> Self {
        Self {
            instance_key,
            healthy: true,
            fail_count: 0,
            success_count: 0,
            last_check_time: chrono::Utc::now().timestamp_millis(),
            last_success_time: Some(chrono::Utc::now().timestamp_millis()),
            last_failure_message: None,
        }
    }
}

/// Health check result
#[derive(Debug)]
pub struct HealthCheckResult {
    pub success: bool,
    pub message: Option<String>,
    pub response_time_ms: u64,
}

/// Instance health checker service
pub struct InstanceHealthChecker {
    config: InstanceHealthCheckConfig,
    naming_service: Arc<NamingService>,
    health_status: Arc<DashMap<String, InstanceHealthStatus>>,
    running: Arc<AtomicBool>,
}

impl InstanceHealthChecker {
    /// Create a new instance health checker
    pub fn new(naming_service: Arc<NamingService>, config: InstanceHealthCheckConfig) -> Self {
        Self {
            config,
            naming_service,
            health_status: Arc::new(DashMap::new()),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the health checker background task
    pub fn start(&self) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        info!("Starting instance health checker");

        let running = self.running.clone();
        let config = self.config.clone();
        let naming_service = self.naming_service.clone();
        let health_status = self.health_status.clone();

        tokio::spawn(async move {
            Self::health_check_loop(running, config, naming_service, health_status).await;
        });
    }

    /// Stop the health checker
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("Stopped instance health checker");
    }

    /// Main health check loop
    async fn health_check_loop(
        running: Arc<AtomicBool>,
        config: InstanceHealthCheckConfig,
        naming_service: Arc<NamingService>,
        health_status: Arc<DashMap<String, InstanceHealthStatus>>,
    ) {
        while running.load(Ordering::SeqCst) {
            // Get all services to check
            let service_keys = naming_service.get_all_service_keys();

            for service_key in service_keys {
                // Parse service key: namespace@@group@@service
                let parts: Vec<&str> = service_key.split("@@").collect();
                if parts.len() != 3 {
                    continue;
                }
                let (namespace, group_name, service_name) = (parts[0], parts[1], parts[2]);

                // Get all clusters for this service
                let cluster_configs =
                    naming_service.get_all_cluster_configs(namespace, group_name, service_name);

                // Get all instances for this service
                let instances =
                    naming_service.get_instances(namespace, group_name, service_name, "", false);

                // Group instances by cluster
                let mut instances_by_cluster: HashMap<String, Vec<Instance>> = HashMap::new();
                for instance in instances {
                    instances_by_cluster
                        .entry(instance.cluster_name.clone())
                        .or_default()
                        .push(instance);
                }

                // Check each cluster's instances
                for (cluster_name, instances) in instances_by_cluster {
                    let cluster_config = cluster_configs
                        .iter()
                        .find(|c| c.name == cluster_name)
                        .cloned()
                        .unwrap_or_else(|| ClusterConfig {
                            name: cluster_name.clone(),
                            ..Default::default()
                        });

                    // Skip if health check is disabled for this cluster
                    if cluster_config.health_check_type.to_uppercase() == "NONE" {
                        continue;
                    }

                    // Check each instance concurrently
                    let mut handles = Vec::new();
                    for instance in instances {
                        if !instance.enabled {
                            continue;
                        }

                        let service_key = service_key.clone();
                        let cluster_config = cluster_config.clone();
                        let config = config.clone();
                        let health_status = health_status.clone();
                        let naming_service = naming_service.clone();
                        let namespace = namespace.to_string();
                        let group_name = group_name.to_string();
                        let service_name = service_name.to_string();

                        let handle = tokio::spawn(async move {
                            Self::check_instance(
                                &service_key,
                                &namespace,
                                &group_name,
                                &service_name,
                                &instance,
                                &cluster_config,
                                &config,
                                &health_status,
                                &naming_service,
                            )
                            .await;
                        });
                        handles.push(handle);
                    }

                    // Wait for all instance checks to complete
                    for handle in handles {
                        if let Err(e) = handle.await {
                            warn!("Instance health check task error: {}", e);
                        }
                    }
                }
            }

            tokio::time::sleep(config.check_interval).await;
        }
    }

    /// Check a single instance's health
    async fn check_instance(
        service_key: &str,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: &Instance,
        cluster_config: &ClusterConfig,
        config: &InstanceHealthCheckConfig,
        health_status: &Arc<DashMap<String, InstanceHealthStatus>>,
        naming_service: &Arc<NamingService>,
    ) {
        let instance_key = format!(
            "{}#{}#{}#{}",
            service_key, instance.ip, instance.port, instance.cluster_name
        );

        // Initialize health status if not exists
        if !health_status.contains_key(&instance_key) {
            health_status.insert(
                instance_key.clone(),
                InstanceHealthStatus::new(instance_key.clone()),
            );
        }

        // Determine check port
        let check_port = if cluster_config.use_instance_port || cluster_config.check_port <= 0 {
            instance.port
        } else {
            cluster_config.check_port
        };

        // Perform health check based on type
        let result = match cluster_config.health_check_type.to_uppercase().as_str() {
            "TCP" => Self::tcp_health_check(&instance.ip, check_port, config.check_timeout).await,
            "HTTP" => {
                let path = cluster_config
                    .metadata
                    .get("health_check_path")
                    .map(|s| s.as_str())
                    .unwrap_or("/health");
                Self::http_health_check(&instance.ip, check_port, path, config.check_timeout).await
            }
            _ => {
                // Unknown or disabled check type, assume healthy
                return;
            }
        };

        // Update health status
        if let Some(mut status) = health_status.get_mut(&instance_key) {
            let now = chrono::Utc::now().timestamp_millis();
            status.last_check_time = now;

            let previous_healthy = status.healthy;

            if result.success {
                status.success_count += 1;
                status.fail_count = 0;
                status.last_success_time = Some(now);
                status.last_failure_message = None;

                if status.success_count >= config.healthy_threshold && !status.healthy {
                    status.healthy = true;
                    info!(
                        "Instance {}:{} became healthy after {} successful checks",
                        instance.ip, instance.port, status.success_count
                    );
                }
            } else {
                status.fail_count += 1;
                status.success_count = 0;
                status.last_failure_message = result.message;

                if status.fail_count >= config.unhealthy_threshold && status.healthy {
                    status.healthy = false;
                    warn!(
                        "Instance {}:{} became unhealthy after {} failed checks: {:?}",
                        instance.ip, instance.port, status.fail_count, status.last_failure_message
                    );
                }
            }

            // Update the naming service if health status changed
            if status.healthy != previous_healthy {
                // Update instance health in naming service
                if naming_service.heartbeat(
                    namespace,
                    group_name,
                    service_name,
                    &instance.ip,
                    instance.port,
                    &instance.cluster_name,
                ) {
                    debug!(
                        "Updated instance {}:{} health status to {}",
                        instance.ip, instance.port, status.healthy
                    );
                }
            }
        }
    }

    /// Perform TCP health check
    async fn tcp_health_check(
        ip: &str,
        port: i32,
        timeout_duration: Duration,
    ) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let addr_str = format!("{}:{}", ip, port);
        let addr: SocketAddr = match addr_str.parse() {
            Ok(a) => a,
            Err(e) => {
                return HealthCheckResult {
                    success: false,
                    message: Some(format!("Invalid address: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        match timeout(timeout_duration, TcpStream::connect(addr)).await {
            Ok(Ok(_stream)) => {
                debug!("TCP health check passed for {}:{}", ip, port);
                HealthCheckResult {
                    success: true,
                    message: None,
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            Ok(Err(e)) => {
                debug!("TCP health check failed for {}:{}: {}", ip, port, e);
                HealthCheckResult {
                    success: false,
                    message: Some(format!("Connection failed: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(_) => {
                debug!("TCP health check timeout for {}:{}", ip, port);
                HealthCheckResult {
                    success: false,
                    message: Some("Connection timeout".to_string()),
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
        }
    }

    /// Perform HTTP health check
    async fn http_health_check(
        ip: &str,
        port: i32,
        path: &str,
        timeout_duration: Duration,
    ) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let addr_str = format!("{}:{}", ip, port);
        let addr: SocketAddr = match addr_str.parse() {
            Ok(a) => a,
            Err(e) => {
                return HealthCheckResult {
                    success: false,
                    message: Some(format!("Invalid address: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Connect with timeout
        let stream = match timeout(timeout_duration, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                return HealthCheckResult {
                    success: false,
                    message: Some(format!("Connection failed: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
            Err(_) => {
                return HealthCheckResult {
                    success: false,
                    message: Some("Connection timeout".to_string()),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Send HTTP request
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
            path, ip, port
        );

        let (mut reader, mut writer) = stream.into_split();

        if let Err(e) = writer.write_all(request.as_bytes()).await {
            return HealthCheckResult {
                success: false,
                message: Some(format!("Failed to send request: {}", e)),
                response_time_ms: start.elapsed().as_millis() as u64,
            };
        }

        // Read response with timeout
        let mut response = vec![0u8; 1024];
        let remaining = timeout_duration.saturating_sub(start.elapsed());

        match timeout(remaining, reader.read(&mut response)).await {
            Ok(Ok(n)) if n > 0 => {
                let response_str = String::from_utf8_lossy(&response[..n]);

                // Check HTTP status code (2xx or 3xx is considered healthy)
                if let Some(status_line) = response_str.lines().next() {
                    if let Some(status_code) = status_line.split_whitespace().nth(1) {
                        if let Ok(code) = status_code.parse::<u16>() {
                            if (200..400).contains(&code) {
                                debug!(
                                    "HTTP health check passed for {}:{}{} with status {}",
                                    ip, port, path, code
                                );
                                return HealthCheckResult {
                                    success: true,
                                    message: None,
                                    response_time_ms: start.elapsed().as_millis() as u64,
                                };
                            } else {
                                return HealthCheckResult {
                                    success: false,
                                    message: Some(format!("HTTP status code: {}", code)),
                                    response_time_ms: start.elapsed().as_millis() as u64,
                                };
                            }
                        }
                    }
                }

                HealthCheckResult {
                    success: false,
                    message: Some("Invalid HTTP response".to_string()),
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            Ok(Ok(_)) => HealthCheckResult {
                success: false,
                message: Some("Empty response".to_string()),
                response_time_ms: start.elapsed().as_millis() as u64,
            },
            Ok(Err(e)) => HealthCheckResult {
                success: false,
                message: Some(format!("Failed to read response: {}", e)),
                response_time_ms: start.elapsed().as_millis() as u64,
            },
            Err(_) => HealthCheckResult {
                success: false,
                message: Some("Response timeout".to_string()),
                response_time_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    /// Get health status for an instance
    pub fn get_health_status(&self, instance_key: &str) -> Option<InstanceHealthStatus> {
        self.health_status.get(instance_key).map(|e| e.clone())
    }

    /// Get all health statuses
    pub fn get_all_health_status(&self) -> Vec<InstanceHealthStatus> {
        self.health_status
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Check if an instance is healthy
    pub fn is_instance_healthy(&self, instance_key: &str) -> bool {
        self.health_status
            .get(instance_key)
            .map(|e| e.healthy)
            .unwrap_or(true)
    }

    /// Force a health check for a specific instance
    pub async fn force_check(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        ip: &str,
        port: i32,
        cluster_name: &str,
    ) -> Option<HealthCheckResult> {
        let instances = self.naming_service.get_instances(
            namespace,
            group_name,
            service_name,
            cluster_name,
            false,
        );

        let instance = instances.iter().find(|i| i.ip == ip && i.port == port)?;

        let cluster_config = self
            .naming_service
            .get_cluster_config(namespace, group_name, service_name, cluster_name)
            .unwrap_or_default();

        let check_port = if cluster_config.use_instance_port || cluster_config.check_port <= 0 {
            instance.port
        } else {
            cluster_config.check_port
        };

        let result = match cluster_config.health_check_type.to_uppercase().as_str() {
            "TCP" => Self::tcp_health_check(ip, check_port, self.config.check_timeout).await,
            "HTTP" => {
                let path = cluster_config
                    .metadata
                    .get("health_check_path")
                    .map(|s| s.as_str())
                    .unwrap_or("/health");
                Self::http_health_check(ip, check_port, path, self.config.check_timeout).await
            }
            _ => HealthCheckResult {
                success: true,
                message: Some("Health check disabled".to_string()),
                response_time_ms: 0,
            },
        };

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = InstanceHealthCheckConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(5));
        assert_eq!(config.check_timeout, Duration::from_secs(3));
        assert_eq!(config.unhealthy_threshold, 3);
        assert_eq!(config.healthy_threshold, 2);
    }

    #[test]
    fn test_health_status_new() {
        let status = InstanceHealthStatus::new("test-instance".to_string());
        assert!(status.healthy);
        assert_eq!(status.fail_count, 0);
        assert_eq!(status.success_count, 0);
    }

    #[tokio::test]
    async fn test_tcp_health_check_invalid_address() {
        let result =
            InstanceHealthChecker::tcp_health_check("invalid", 8080, Duration::from_secs(1)).await;
        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_tcp_health_check_connection_refused() {
        // Use localhost with a port that's very unlikely to be listening
        let result =
            InstanceHealthChecker::tcp_health_check("127.0.0.1", 59999, Duration::from_millis(500))
                .await;
        // Should fail - either connection refused or timeout
        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_http_health_check_invalid_address() {
        let result = InstanceHealthChecker::http_health_check(
            "invalid",
            8080,
            "/health",
            Duration::from_secs(1),
        )
        .await;
        assert!(!result.success);
        assert!(result.message.is_some());
    }
}
