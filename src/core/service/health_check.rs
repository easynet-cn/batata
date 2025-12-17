// Cluster member health check service
// Periodically checks the health of cluster members and updates their state

use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use crate::api::{
    grpc::{request_client::RequestClient, Metadata, Payload},
    model::{Member, NodeState},
    remote::model::{HealthCheckRequest, RequestTrait},
};
use prost_types::Any;

/// Health check configuration
#[derive(Clone, Debug)]
pub struct HealthCheckConfig {
    /// Interval between health checks
    pub check_interval: Duration,
    /// Timeout for health check requests
    pub check_timeout: Duration,
    /// Number of failed checks before marking node as DOWN
    pub max_fail_count: i32,
    /// Number of failed checks before marking node as SUSPICIOUS
    pub suspicious_threshold: i32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(5),
            check_timeout: Duration::from_secs(3),
            max_fail_count: 3,
            suspicious_threshold: 1,
        }
    }
}

/// Member health status tracker
#[derive(Clone, Debug)]
pub struct MemberHealthStatus {
    pub address: String,
    pub fail_count: i32,
    pub last_check_time: i64,
    pub last_success_time: i64,
    pub state: NodeState,
}

impl MemberHealthStatus {
    pub fn new(address: String) -> Self {
        Self {
            address,
            fail_count: 0,
            last_check_time: 0,
            last_success_time: chrono::Utc::now().timestamp_millis(),
            state: NodeState::Up,
        }
    }

    pub fn record_success(&mut self) {
        self.fail_count = 0;
        self.last_success_time = chrono::Utc::now().timestamp_millis();
        self.state = NodeState::Up;
    }

    pub fn record_failure(&mut self, config: &HealthCheckConfig) {
        self.fail_count += 1;
        if self.fail_count >= config.max_fail_count {
            self.state = NodeState::Down;
        } else if self.fail_count >= config.suspicious_threshold {
            self.state = NodeState::Suspicious;
        }
    }
}

/// Health check service for cluster members
pub struct MemberHealthChecker {
    config: HealthCheckConfig,
    members: Arc<DashMap<String, Member>>,
    health_status: Arc<DashMap<String, MemberHealthStatus>>,
    running: Arc<RwLock<bool>>,
    local_address: String,
}

impl MemberHealthChecker {
    pub fn new(
        members: Arc<DashMap<String, Member>>,
        local_address: String,
        config: HealthCheckConfig,
    ) -> Self {
        Self {
            config,
            members,
            health_status: Arc::new(DashMap::new()),
            running: Arc::new(RwLock::new(false)),
            local_address,
        }
    }

    /// Start the health check service
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            return;
        }
        *running = true;
        drop(running);

        info!("Starting member health checker");

        let members = self.members.clone();
        let health_status = self.health_status.clone();
        let running = self.running.clone();
        let config = self.config.clone();
        let local_address = self.local_address.clone();

        tokio::spawn(async move {
            Self::health_check_loop(members, health_status, running, config, local_address).await;
        });
    }

    /// Stop the health check service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped member health checker");
    }

    /// Main health check loop
    async fn health_check_loop(
        members: Arc<DashMap<String, Member>>,
        health_status: Arc<DashMap<String, MemberHealthStatus>>,
        running: Arc<RwLock<bool>>,
        config: HealthCheckConfig,
        local_address: String,
    ) {
        loop {
            {
                let is_running = running.read().await;
                if !*is_running {
                    break;
                }
            }

            // Get all members to check
            let member_addresses: Vec<String> = members
                .iter()
                .filter(|e| e.key() != &local_address)
                .map(|e| e.key().clone())
                .collect();

            // Check each member concurrently
            let mut handles = Vec::new();
            for address in member_addresses {
                let health_status = health_status.clone();
                let members = members.clone();
                let config = config.clone();

                let handle = tokio::spawn(async move {
                    Self::check_member(&address, &health_status, &members, &config).await;
                });
                handles.push(handle);
            }

            // Wait for all checks to complete
            for handle in handles {
                let _ = handle.await;
            }

            tokio::time::sleep(config.check_interval).await;
        }
    }

    /// Check a single member's health
    async fn check_member(
        address: &str,
        health_status: &Arc<DashMap<String, MemberHealthStatus>>,
        members: &Arc<DashMap<String, Member>>,
        config: &HealthCheckConfig,
    ) {
        // Initialize health status if not exists
        if !health_status.contains_key(address) {
            health_status.insert(address.to_string(), MemberHealthStatus::new(address.to_string()));
        }

        let check_result = Self::perform_health_check(address, config).await;

        // Update health status
        if let Some(mut status) = health_status.get_mut(address) {
            status.last_check_time = chrono::Utc::now().timestamp_millis();

            if check_result {
                status.record_success();
                debug!("Health check passed for member: {}", address);

                // Update member state
                if let Some(mut member) = members.get_mut(address) {
                    member.state = NodeState::Up;
                    member.fail_access_cnt = 0;
                }
            } else {
                status.record_failure(config);
                warn!(
                    "Health check failed for member: {}, fail_count: {}",
                    address, status.fail_count
                );

                // Update member state
                if let Some(mut member) = members.get_mut(address) {
                    member.state = status.state.clone();
                    member.fail_access_cnt = status.fail_count;
                }
            }
        }
    }

    /// Perform actual health check via gRPC
    async fn perform_health_check(address: &str, config: &HealthCheckConfig) -> bool {
        // Calculate cluster gRPC port (main_port + 1001)
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() != 2 {
            return false;
        }

        let ip = parts[0];
        let main_port: u16 = match parts[1].parse() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Cluster gRPC port is main_port + 1001
        let grpc_port = main_port + 1001;
        let grpc_address = format!("http://{}:{}", ip, grpc_port);

        // Create gRPC channel with timeout
        let channel = match tokio::time::timeout(
            config.check_timeout,
            Channel::from_shared(grpc_address.clone())
                .unwrap()
                .connect(),
        )
        .await
        {
            Ok(Ok(channel)) => channel,
            Ok(Err(e)) => {
                debug!("Failed to connect to {}: {}", grpc_address, e);
                return false;
            }
            Err(_) => {
                debug!("Connection timeout to {}", grpc_address);
                return false;
            }
        };

        let mut client = RequestClient::new(channel);

        // Create health check request
        let request = HealthCheckRequest::new();

        let metadata = Metadata {
            r#type: request.request_type().to_string(),
            ..Default::default()
        };

        // Create payload manually
        let body = request.body();
        let payload = Payload {
            metadata: Some(metadata),
            body: Some(Any {
                type_url: String::default(),
                value: body,
            }),
        };

        // Send health check request
        match tokio::time::timeout(config.check_timeout, client.request(payload)).await {
            Ok(Ok(response)) => {
                // Check response type
                if let Some(metadata) = response.into_inner().metadata {
                    metadata.r#type.contains("HealthCheckResponse")
                } else {
                    false
                }
            }
            Ok(Err(e)) => {
                debug!("Health check request failed for {}: {}", grpc_address, e);
                false
            }
            Err(_) => {
                debug!("Health check request timeout for {}", grpc_address);
                false
            }
        }
    }

    /// Get health status for a member
    pub fn get_health_status(&self, address: &str) -> Option<MemberHealthStatus> {
        self.health_status.get(address).map(|e| e.clone())
    }

    /// Get all health statuses
    pub fn get_all_health_status(&self) -> Vec<MemberHealthStatus> {
        self.health_status.iter().map(|e| e.value().clone()).collect()
    }

    /// Check if a member is healthy
    pub fn is_member_healthy(&self, address: &str) -> bool {
        self.health_status
            .get(address)
            .map(|e| matches!(e.state, NodeState::Up))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_health_status_success() {
        let config = HealthCheckConfig::default();
        let mut status = MemberHealthStatus::new("127.0.0.1:8848".to_string());

        status.record_failure(&config);
        assert_eq!(status.fail_count, 1);
        assert!(matches!(status.state, NodeState::Suspicious));

        status.record_success();
        assert_eq!(status.fail_count, 0);
        assert!(matches!(status.state, NodeState::Up));
    }

    #[test]
    fn test_member_health_status_down() {
        let config = HealthCheckConfig::default();
        let mut status = MemberHealthStatus::new("127.0.0.1:8848".to_string());

        for _ in 0..3 {
            status.record_failure(&config);
        }

        assert_eq!(status.fail_count, 3);
        assert!(matches!(status.state, NodeState::Down));
    }
}
