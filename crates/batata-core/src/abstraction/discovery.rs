// Service Discovery abstraction layer

use async_trait::async_trait;

use super::types::{
    ChangeEvent, HealthStatus, PagedResult, ServiceDefinition, ServiceInstance, ServiceQuery,
};

/// Service discovery abstraction trait
/// Implementations: NacosServiceDiscovery, ConsulServiceDiscovery
#[async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Register a service instance
    async fn register(&self, instance: ServiceInstance) -> Result<(), DiscoveryError>;

    /// Deregister a service instance
    async fn deregister(&self, instance_id: &str) -> Result<(), DiscoveryError>;

    /// Update an existing instance (re-registration with same ID)
    async fn update(&self, instance: ServiceInstance) -> Result<(), DiscoveryError> {
        // Default implementation: deregister then register
        self.deregister(&instance.id).await?;
        self.register(instance).await
    }

    /// Get all instances for a service
    async fn get_instances(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<Vec<ServiceInstance>, DiscoveryError>;

    /// Get instances with query filters
    async fn query_instances(
        &self,
        query: ServiceQuery,
    ) -> Result<PagedResult<ServiceInstance>, DiscoveryError>;

    /// Get service definition (including all instances)
    async fn get_service(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<Option<ServiceDefinition>, DiscoveryError>;

    /// List all services
    async fn list_services(
        &self,
        query: ServiceQuery,
    ) -> Result<PagedResult<ServiceDefinition>, DiscoveryError>;

    /// Send heartbeat for an instance (TTL-based health check)
    async fn heartbeat(&self, instance_id: &str) -> Result<(), DiscoveryError>;

    /// Update instance health status
    async fn set_health_status(
        &self,
        instance_id: &str,
        status: HealthStatus,
    ) -> Result<(), DiscoveryError>;

    /// Get instance by ID
    async fn get_instance(
        &self,
        instance_id: &str,
    ) -> Result<Option<ServiceInstance>, DiscoveryError>;
}

/// Subscription management for service changes
#[async_trait]
pub trait ServiceSubscription: Send + Sync {
    /// Subscribe to service changes
    async fn subscribe(
        &self,
        subscriber_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<(), DiscoveryError>;

    /// Unsubscribe from service changes
    async fn unsubscribe(
        &self,
        subscriber_id: &str,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<(), DiscoveryError>;

    /// Get all subscribers for a service
    async fn get_subscribers(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
    ) -> Result<Vec<String>, DiscoveryError>;

    /// Clear all subscriptions for a subscriber
    async fn clear_subscriber(&self, subscriber_id: &str) -> Result<(), DiscoveryError>;

    /// Notify subscribers of a change (implementation detail)
    async fn notify_change(&self, event: ChangeEvent) -> Result<(), DiscoveryError>;
}

/// Batch operations for efficiency
#[async_trait]
pub trait BatchServiceDiscovery: ServiceDiscovery {
    /// Register multiple instances at once
    async fn batch_register(
        &self,
        instances: Vec<ServiceInstance>,
    ) -> Result<Vec<Result<(), DiscoveryError>>, DiscoveryError>;

    /// Deregister multiple instances at once
    async fn batch_deregister(
        &self,
        instance_ids: Vec<String>,
    ) -> Result<Vec<Result<(), DiscoveryError>>, DiscoveryError>;
}

/// Service discovery error types
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Instance not found: {0}")]
    InstanceNotFound(String),

    #[error("Instance already exists: {0}")]
    InstanceAlreadyExists(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl DiscoveryError {
    /// Get HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            DiscoveryError::ServiceNotFound(_) => 404,
            DiscoveryError::InstanceNotFound(_) => 404,
            DiscoveryError::InstanceAlreadyExists(_) => 409,
            DiscoveryError::InvalidRequest(_) => 400,
            DiscoveryError::NamespaceNotFound(_) => 404,
            DiscoveryError::PermissionDenied(_) => 403,
            DiscoveryError::RateLimitExceeded => 429,
            DiscoveryError::StorageError(_) => 500,
            DiscoveryError::NetworkError(_) => 502,
            DiscoveryError::InternalError(_) => 500,
        }
    }
}

