// Health check abstraction layer
// Provides health check abstraction

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

