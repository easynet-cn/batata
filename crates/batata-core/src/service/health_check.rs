// Cluster member health check service
// Periodically checks the health of cluster members and updates their state

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use batata_api::{
    grpc::{Metadata, Payload, request_client::RequestClient},
    model::{Member, NodeState},
    remote::model::{HealthCheckRequest, RequestTrait},
};
use prost_types::Any;

use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};

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
    /// Interval for reporting local member info to peers.
    /// Similar to Nacos MemberInfoReportTask (50s).
    /// Set to 0 to disable member reporting.
    pub member_report_interval: Duration,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(5),
            check_timeout: Duration::from_secs(3),
            max_fail_count: 3,
            suspicious_threshold: 1,
            member_report_interval: Duration::from_secs(5),
        }
    }
}

impl HealthCheckConfig {
    /// Create from application Configuration
    pub fn from_configuration(config: &crate::model::Configuration) -> Self {
        Self {
            check_interval: Duration::from_millis(config.cluster_health_check_interval_ms()),
            check_timeout: Duration::from_millis(config.cluster_health_check_timeout_ms()),
            max_fail_count: config.cluster_health_check_max_fail_count(),
            suspicious_threshold: config.cluster_health_check_suspicious_threshold(),
            member_report_interval: Duration::from_millis(
                config.cluster_member_report_interval_ms(),
            ),
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

/// Lock-free per-member health state tracked with atomics.
///
/// This is stored alongside `MemberHealthStatus` and used by the health-check
/// loop to implement state transitions:
///   UP -> SUSPICIOUS  (after `suspicious_threshold` consecutive failures)
///   SUSPICIOUS -> DOWN  (after `max_fail_count` consecutive failures)
///   DOWN/SUSPICIOUS -> UP  (after 1 success)
pub struct MemberHealthState {
    /// Consecutive health check failures (reset to 0 on success)
    pub consecutive_failures: AtomicU32,
    /// Timestamp (millis since epoch) of the last health check attempt
    pub last_check_time: AtomicU64,
}

impl MemberHealthState {
    pub fn new() -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            last_check_time: AtomicU64::new(0),
        }
    }

    /// Record a successful health check, resetting consecutive failures
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.last_check_time.store(
            chrono::Utc::now().timestamp_millis() as u64,
            Ordering::Relaxed,
        );
    }

    /// Record a failed health check, incrementing the consecutive failure counter.
    /// Returns the new consecutive failure count.
    pub fn record_failure(&self) -> u32 {
        self.last_check_time.store(
            chrono::Utc::now().timestamp_millis() as u64,
            Ordering::Relaxed,
        );
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Determine the appropriate node state based on current consecutive failures
    pub fn determine_state(&self, config: &HealthCheckConfig) -> NodeState {
        let failures = self.consecutive_failures.load(Ordering::Relaxed);
        if failures >= config.max_fail_count as u32 {
            NodeState::Down
        } else if failures >= config.suspicious_threshold as u32 {
            NodeState::Suspicious
        } else {
            NodeState::Up
        }
    }
}

impl Default for MemberHealthState {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check service for cluster members
pub struct MemberHealthChecker {
    config: HealthCheckConfig,
    members: Arc<DashMap<String, Member>>,
    health_status: Arc<DashMap<String, MemberHealthStatus>>,
    /// Lock-free per-member health state for atomic consecutive failure tracking
    health_states: Arc<DashMap<String, Arc<MemberHealthState>>>,
    /// Circuit breakers per member address for resilient health checks
    circuit_breakers: Arc<DashMap<String, CircuitBreaker>>,
    /// Running flag using AtomicBool for lock-free access
    running: Arc<AtomicBool>,
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
            health_states: Arc::new(DashMap::new()),
            circuit_breakers: Arc::new(DashMap::new()),
            running: Arc::new(AtomicBool::new(false)),
            local_address,
        }
    }

    /// Start the health check service
    pub async fn start(&self) {
        // Use compare_exchange to atomically check and set running flag
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // Already running
        }

        info!("Starting member health checker with circuit breaker protection");

        let members = self.members.clone();
        let health_status = self.health_status.clone();
        let health_states = self.health_states.clone();
        let circuit_breakers = self.circuit_breakers.clone();
        let running = self.running.clone();
        let config = self.config.clone();
        let local_address = self.local_address.clone();

        // Start the member report task (round-robin reports to peers)
        if config.member_report_interval.as_millis() > 0 {
            let report_members = self.members.clone();
            let report_running = self.running.clone();
            let report_config = config.clone();
            let report_local = self.local_address.clone();
            tokio::spawn(async move {
                Self::member_report_loop(
                    report_members,
                    report_running,
                    report_config,
                    report_local,
                )
                .await;
            });
        }

        tokio::spawn(async move {
            Self::health_check_loop(
                members,
                health_status,
                health_states,
                circuit_breakers,
                running,
                config,
                local_address,
            )
            .await;
        });
    }

    /// Stop the health check service
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("Stopped member health checker");
    }

    /// Main health check loop
    async fn health_check_loop(
        members: Arc<DashMap<String, Member>>,
        health_status: Arc<DashMap<String, MemberHealthStatus>>,
        health_states: Arc<DashMap<String, Arc<MemberHealthState>>>,
        circuit_breakers: Arc<DashMap<String, CircuitBreaker>>,
        running: Arc<AtomicBool>,
        config: HealthCheckConfig,
        local_address: String,
    ) {
        loop {
            // Lock-free check of running flag
            if !running.load(Ordering::SeqCst) {
                break;
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
                let health_states = health_states.clone();
                let circuit_breakers = circuit_breakers.clone();
                let members = members.clone();
                let config = config.clone();

                let handle = tokio::spawn(async move {
                    Self::check_member(
                        &address,
                        &health_status,
                        &health_states,
                        &circuit_breakers,
                        &members,
                        &config,
                    )
                    .await;
                });
                handles.push(handle);
            }

            // Wait for all checks to complete, log any task errors
            for handle in handles {
                if let Err(e) = handle.await {
                    warn!("Health check task failed: {}", e);
                }
            }

            tokio::time::sleep(config.check_interval).await;
        }
    }

    /// Check a single member's health with circuit breaker protection
    ///
    /// State transitions based on consecutive failures:
    ///   UP -> SUSPICIOUS  (after `suspicious_threshold` consecutive failures)
    ///   SUSPICIOUS -> DOWN  (after `max_fail_count` consecutive failures)
    ///   DOWN/SUSPICIOUS -> UP  (after 1 success)
    async fn check_member(
        address: &str,
        health_status: &Arc<DashMap<String, MemberHealthStatus>>,
        health_states: &Arc<DashMap<String, Arc<MemberHealthState>>>,
        circuit_breakers: &Arc<DashMap<String, CircuitBreaker>>,
        members: &Arc<DashMap<String, Member>>,
        config: &HealthCheckConfig,
    ) {
        // Initialize health status if not exists
        if !health_status.contains_key(address) {
            health_status.insert(
                address.to_string(),
                MemberHealthStatus::new(address.to_string()),
            );
        }

        // Get or create atomic health state for this member
        let state = health_states
            .entry(address.to_string())
            .or_insert_with(|| Arc::new(MemberHealthState::new()))
            .clone();

        // Get or create circuit breaker for this member
        let cb_config = CircuitBreakerConfig {
            failure_threshold: config.max_fail_count as u32,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 2,
            failure_window: Duration::from_secs(120),
        };
        let cb = circuit_breakers
            .entry(address.to_string())
            .or_insert_with(|| CircuitBreaker::with_config(cb_config));
        if !cb.allow_request() {
            debug!(
                "Circuit breaker open for member: {}, skipping health check",
                address
            );
            // Still update status to reflect circuit is open
            if let Some(mut status) = health_status.get_mut(address) {
                status.last_check_time = chrono::Utc::now().timestamp_millis();
            }
            return;
        }
        drop(cb); // Release the lock before async operation

        let check_result = Self::perform_health_check(address, config).await;

        // Update circuit breaker
        if let Some(cb) = circuit_breakers.get(address) {
            if check_result {
                cb.record_success();
            } else {
                cb.record_failure();
                if cb.state() == CircuitState::Open {
                    warn!(
                        "Circuit breaker opened for member: {}, will skip checks for recovery period",
                        address
                    );
                }
            }
        }

        // Update atomic health state and derive new node state
        let new_state = if check_result {
            state.record_success();
            NodeState::Up
        } else {
            state.record_failure();
            state.determine_state(config)
        };

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
                let consecutive = state.consecutive_failures.load(Ordering::Relaxed);
                warn!(
                    "Health check failed for member: {}, fail_count: {}, consecutive: {}, state: {:?}",
                    address, status.fail_count, consecutive, new_state
                );

                // Update member state using the atomic-derived state
                if let Some(mut member) = members.get_mut(address) {
                    member.state = new_state;
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
        let endpoint = match Channel::from_shared(grpc_address.clone()) {
            Ok(ep) => ep,
            Err(e) => {
                debug!("Invalid gRPC address {}: {}", grpc_address, e);
                return false;
            }
        };

        let channel = match tokio::time::timeout(config.check_timeout, endpoint.connect()).await {
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

    /// Member info report loop — reports local member metadata to one peer per cycle
    /// (round-robin), similar to Nacos MemberInfoReportTask.
    async fn member_report_loop(
        members: Arc<DashMap<String, Member>>,
        running: Arc<AtomicBool>,
        config: HealthCheckConfig,
        local_address: String,
    ) {
        let mut cursor: usize = 0;
        info!(
            "Starting member report task (interval: {}ms)",
            config.member_report_interval.as_millis()
        );

        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            tokio::time::sleep(config.member_report_interval).await;

            let peers: Vec<String> = members
                .iter()
                .filter(|e| e.key() != &local_address)
                .map(|e| e.key().clone())
                .collect();

            if peers.is_empty() {
                continue;
            }

            // Round-robin: report to one peer per cycle
            cursor %= peers.len();
            let target = &peers[cursor];
            cursor = cursor.wrapping_add(1);

            // Build and send MemberReportRequest
            if let Err(e) = Self::send_member_report(target, &local_address, &config).await {
                debug!("Member report to {} failed: {}", target, e);
            }
        }
    }

    /// Send a member report to a single peer via cluster gRPC.
    async fn send_member_report(
        target: &str,
        local_address: &str,
        config: &HealthCheckConfig,
    ) -> Result<(), String> {
        use batata_api::model::MemberBuilder;
        use batata_api::remote::model::RequestTrait;

        let parts: Vec<&str> = target.split(':').collect();
        if parts.len() != 2 {
            return Err("invalid address".to_string());
        }
        let main_port: u16 = parts[1].parse().map_err(|e| format!("{}", e))?;
        let grpc_address = format!("http://{}:{}", parts[0], main_port + 1001);

        let channel = tokio::time::timeout(
            config.check_timeout,
            Channel::from_shared(grpc_address.clone())
                .map_err(|e| format!("{}", e))?
                .connect(),
        )
        .await
        .map_err(|_| "connect timeout".to_string())?
        .map_err(|e| format!("connect: {}", e))?;

        let mut client = RequestClient::new(channel);

        // Build local member info from address
        let local_parts: Vec<&str> = local_address.split(':').collect();
        let (local_ip, local_port) = if local_parts.len() == 2 {
            (
                local_parts[0].to_string(),
                local_parts[1].parse::<u16>().unwrap_or(8848),
            )
        } else {
            (local_address.to_string(), 8848)
        };

        let local_member = MemberBuilder::new(local_ip, local_port).build();

        let request = batata_api::remote::model::MemberReportRequest {
            node: Some(local_member),
            ..Default::default()
        };

        let metadata = Metadata {
            r#type: request.request_type().to_string(),
            ..Default::default()
        };
        let body = request.body();
        let payload = Payload {
            metadata: Some(metadata),
            body: Some(Any {
                type_url: String::default(),
                value: body,
            }),
        };

        tokio::time::timeout(config.check_timeout, client.request(payload))
            .await
            .map_err(|_| "request timeout".to_string())?
            .map_err(|e| format!("request: {}", e))?;

        debug!("Member report sent to {}", target);
        Ok(())
    }

    /// Get health status for a member
    pub fn get_health_status(&self, address: &str) -> Option<MemberHealthStatus> {
        self.health_status.get(address).map(|e| e.clone())
    }

    /// Get all health statuses
    pub fn get_all_health_status(&self) -> Vec<MemberHealthStatus> {
        self.health_status
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Check if a member is healthy
    pub fn is_member_healthy(&self, address: &str) -> bool {
        self.health_status
            .get(address)
            .map(|e| matches!(e.state, NodeState::Up))
            .unwrap_or(false)
    }

    /// Get circuit breaker state for a member
    pub fn get_circuit_breaker_state(&self, address: &str) -> Option<CircuitState> {
        self.circuit_breakers.get(address).map(|cb| cb.state())
    }

    /// Get all circuit breaker states for monitoring
    pub fn get_all_circuit_breaker_states(&self) -> Vec<(String, CircuitState, u32)> {
        self.circuit_breakers
            .iter()
            .map(|e| (e.key().clone(), e.state(), e.failure_count()))
            .collect()
    }

    /// Reset circuit breaker for a specific member (for manual recovery)
    pub fn reset_circuit_breaker(&self, address: &str) {
        if let Some(cb) = self.circuit_breakers.get(address) {
            cb.reset();
            info!("Circuit breaker manually reset for member: {}", address);
        }
    }

    /// Get the atomic health state for a member (consecutive failures, last check time)
    pub fn get_health_state(&self, address: &str) -> Option<Arc<MemberHealthState>> {
        self.health_states.get(address).map(|e| e.value().clone())
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

    #[test]
    fn test_member_health_state_consecutive_failures() {
        let state = MemberHealthState::new();
        assert_eq!(state.consecutive_failures.load(Ordering::Relaxed), 0);

        let count = state.record_failure();
        assert_eq!(count, 1);
        assert_eq!(state.consecutive_failures.load(Ordering::Relaxed), 1);

        let count = state.record_failure();
        assert_eq!(count, 2);

        // Success resets consecutive failures
        state.record_success();
        assert_eq!(state.consecutive_failures.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_member_health_state_transitions() {
        let config = HealthCheckConfig {
            suspicious_threshold: 2,
            max_fail_count: 4,
            ..HealthCheckConfig::default()
        };

        let state = MemberHealthState::new();

        // Initially UP
        assert!(matches!(state.determine_state(&config), NodeState::Up));

        // 1 failure -> still UP
        state.record_failure();
        assert!(matches!(state.determine_state(&config), NodeState::Up));

        // 2 failures -> SUSPICIOUS
        state.record_failure();
        assert!(matches!(
            state.determine_state(&config),
            NodeState::Suspicious
        ));

        // 4 failures -> DOWN
        state.record_failure();
        state.record_failure();
        assert!(matches!(state.determine_state(&config), NodeState::Down));

        // 1 success -> back to UP
        state.record_success();
        assert!(matches!(state.determine_state(&config), NodeState::Up));
    }

    #[test]
    fn test_member_health_state_last_check_time() {
        let state = MemberHealthState::new();
        assert_eq!(state.last_check_time.load(Ordering::Relaxed), 0);

        state.record_failure();
        let t1 = state.last_check_time.load(Ordering::Relaxed);
        assert!(t1 > 0);

        state.record_success();
        let t2 = state.last_check_time.load(Ordering::Relaxed);
        assert!(t2 >= t1);
    }
}
