// Health check abstraction layer
// Provides unified interface for Nacos heartbeat and Consul health checks

use async_trait::async_trait;

use super::types::{HealthCheck, HealthCheckType, HealthStatus};

/// Health check management abstraction
#[async_trait]
pub trait HealthCheckManager: Send + Sync {
    /// Register a health check
    async fn register_check(&self, check: HealthCheck) -> Result<(), HealthError>;

    /// Deregister a health check
    async fn deregister_check(&self, check_id: &str) -> Result<(), HealthError>;

    /// Update check status (for TTL checks)
    async fn update_check_status(
        &self,
        check_id: &str,
        status: HealthStatus,
        output: Option<String>,
    ) -> Result<(), HealthError>;

    /// Pass a TTL check (shorthand for update_check_status with Passing)
    async fn pass_check(&self, check_id: &str, note: Option<String>) -> Result<(), HealthError> {
        self.update_check_status(check_id, HealthStatus::Passing, note)
            .await
    }

    /// Warn a TTL check
    async fn warn_check(&self, check_id: &str, note: Option<String>) -> Result<(), HealthError> {
        self.update_check_status(check_id, HealthStatus::Warning, note)
            .await
    }

    /// Fail a TTL check
    async fn fail_check(&self, check_id: &str, note: Option<String>) -> Result<(), HealthError> {
        self.update_check_status(check_id, HealthStatus::Critical, note)
            .await
    }

    /// Get all checks for a service
    async fn get_service_checks(&self, service_id: &str) -> Result<Vec<HealthCheck>, HealthError>;

    /// Get check by ID
    async fn get_check(&self, check_id: &str) -> Result<Option<HealthCheck>, HealthError>;

    /// Get health status summary for a service
    async fn get_service_health(
        &self,
        service_id: &str,
    ) -> Result<ServiceHealthSummary, HealthError>;
}

/// Service health summary
#[derive(Debug, Clone)]
pub struct ServiceHealthSummary {
    pub service_id: String,
    pub total_checks: u32,
    pub passing: u32,
    pub warning: u32,
    pub critical: u32,
    pub overall_status: HealthStatus,
}

impl ServiceHealthSummary {
    pub fn is_healthy(&self) -> bool {
        self.critical == 0
    }

    pub fn calculate_overall_status(&mut self) {
        if self.critical > 0 {
            self.overall_status = HealthStatus::Critical;
        } else if self.warning > 0 {
            self.overall_status = HealthStatus::Warning;
        } else if self.passing > 0 {
            self.overall_status = HealthStatus::Passing;
        } else {
            self.overall_status = HealthStatus::Unknown;
        }
    }
}

/// Heartbeat management (Nacos-style)
#[async_trait]
pub trait HeartbeatManager: Send + Sync {
    /// Send heartbeat for an instance
    async fn heartbeat(&self, instance_id: &str) -> Result<HeartbeatResponse, HealthError>;

    /// Start automatic heartbeat for an instance
    async fn start_auto_heartbeat(
        &self,
        instance_id: &str,
        interval_ms: u64,
    ) -> Result<(), HealthError>;

    /// Stop automatic heartbeat
    async fn stop_auto_heartbeat(&self, instance_id: &str) -> Result<(), HealthError>;

    /// Get heartbeat status
    async fn get_heartbeat_status(
        &self,
        instance_id: &str,
    ) -> Result<Option<HeartbeatStatus>, HealthError>;
}

/// Heartbeat response
#[derive(Debug, Clone)]
pub struct HeartbeatResponse {
    pub instance_id: String,
    pub light_beat_enabled: bool,
    pub client_beat_interval: i64,
}

/// Heartbeat status
#[derive(Debug, Clone)]
pub struct HeartbeatStatus {
    pub instance_id: String,
    pub last_heartbeat: i64,
    pub healthy: bool,
    pub auto_enabled: bool,
    pub interval_ms: u64,
}

/// Health error types
#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    #[error("Check not found: {0}")]
    CheckNotFound(String),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Invalid check configuration: {0}")]
    InvalidCheck(String),

    #[error("Check type not supported: {0:?}")]
    UnsupportedCheckType(HealthCheckType),

    #[error("Heartbeat timeout")]
    HeartbeatTimeout,

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl HealthError {
    pub fn status_code(&self) -> u16 {
        match self {
            HealthError::CheckNotFound(_) => 404,
            HealthError::ServiceNotFound(_) => 404,
            HealthError::InstanceNotFound(_) => 404,
            HealthError::InvalidCheck(_) => 400,
            HealthError::UnsupportedCheckType(_) => 400,
            HealthError::HeartbeatTimeout => 408,
            HealthError::InternalError(_) => 500,
        }
    }
}

// ============================================================================
// Consul health check types
// ============================================================================

pub mod consul {
    use serde::{Deserialize, Serialize};

    /// Consul Agent Check
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentCheck {
        #[serde(rename = "CheckID")]
        pub check_id: String,
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Status")]
        pub status: String,
        #[serde(rename = "Notes")]
        pub notes: String,
        #[serde(rename = "Output")]
        pub output: String,
        #[serde(rename = "ServiceID")]
        pub service_id: String,
        #[serde(rename = "ServiceName")]
        pub service_name: String,
        #[serde(rename = "Type")]
        pub check_type: String,
    }

    /// Consul Check Definition for registration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CheckDefinition {
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
        pub check_id: Option<String>,
        #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
        pub service_id: Option<String>,
        #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
        pub ttl: Option<String>,
        #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
        pub http: Option<String>,
        #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
        pub tcp: Option<String>,
        #[serde(rename = "GRPC", skip_serializing_if = "Option::is_none")]
        pub grpc: Option<String>,
        #[serde(rename = "Interval", skip_serializing_if = "Option::is_none")]
        pub interval: Option<String>,
        #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
        pub timeout: Option<String>,
        #[serde(
            rename = "DeregisterCriticalServiceAfter",
            skip_serializing_if = "Option::is_none"
        )]
        pub deregister_critical_service_after: Option<String>,
    }

    impl From<super::HealthCheck> for CheckDefinition {
        fn from(check: super::HealthCheck) -> Self {
            let (ttl, http, tcp, grpc, interval) = match check.check_type {
                super::HealthCheckType::Ttl => (
                    Some(format!("{}s", check.interval_secs)),
                    None,
                    None,
                    None,
                    None,
                ),
                super::HealthCheckType::Http => (
                    None,
                    check.http_endpoint,
                    None,
                    None,
                    Some(format!("{}s", check.interval_secs)),
                ),
                super::HealthCheckType::Tcp => (
                    None,
                    None,
                    check.tcp_address,
                    None,
                    Some(format!("{}s", check.interval_secs)),
                ),
                super::HealthCheckType::Grpc => (
                    None,
                    None,
                    None,
                    check.tcp_address, // gRPC uses same field
                    Some(format!("{}s", check.interval_secs)),
                ),
                _ => (None, None, None, None, None),
            };

            CheckDefinition {
                name: check.name,
                check_id: Some(check.id),
                service_id: Some(check.service_id),
                ttl,
                http,
                tcp,
                grpc,
                interval,
                timeout: Some(format!("{}s", check.timeout_secs)),
                deregister_critical_service_after: Some("30s".to_string()),
            }
        }
    }
}
