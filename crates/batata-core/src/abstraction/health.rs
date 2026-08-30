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
    /// The `service_id` field.
    pub service_id: String,
    /// The `total_checks` field.
    pub total_checks: u32,
    /// The `passing` field.
    pub passing: u32,
    /// The `warning` field.
    pub warning: u32,
    /// The `critical` field.
    pub critical: u32,
    /// The `overall_status` field.
    pub overall_status: HealthStatus,
}

impl ServiceHealthSummary {
    /// Returns `true` if healthy.
    pub fn is_healthy(&self) -> bool {
        self.critical == 0
    }

    /// Computes the overall status.
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
    /// The `instance_id` field.
    pub instance_id: String,
    /// The `light_beat_enabled` field.
    pub light_beat_enabled: bool,
    /// The `client_beat_interval` field.
    pub client_beat_interval: i64,
}

/// Heartbeat status
#[derive(Debug, Clone)]
pub struct HeartbeatStatus {
    /// The `instance_id` field.
    pub instance_id: String,
    /// The `last_heartbeat` field.
    pub last_heartbeat: i64,
    /// The `healthy` field.
    pub healthy: bool,
    /// The `auto_enabled` field.
    pub auto_enabled: bool,
    /// The `interval_ms` field.
    pub interval_ms: u64,
}

/// Health error types
#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    #[error("Check not found: {0}")]
    /// The `CheckNotFound` variant.
    CheckNotFound(String),

    #[error("Service not found: {0}")]
    /// The `ServiceNotFound` variant.
    ServiceNotFound(String),

    #[error("Instance not found: {0}")]
    /// The `InstanceNotFound` variant.
    InstanceNotFound(String),

    #[error("Invalid check configuration: {0}")]
    /// The `InvalidCheck` variant.
    InvalidCheck(String),

    #[error("Check type not supported: {0:?}")]
    /// The `UnsupportedCheckType` variant.
    UnsupportedCheckType(HealthCheckType),

    #[error("Heartbeat timeout")]
    /// The `HeartbeatTimeout` variant.
    HeartbeatTimeout,

    #[error("Internal error: {0}")]
    /// The `InternalError` variant.
    InternalError(String),
}

impl HealthError {
    /// Status Code.
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
