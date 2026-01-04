// Service Discovery abstraction layer
// Provides unified interface for Nacos and Consul service registration/discovery

use async_trait::async_trait;
use std::collections::HashMap;

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

// ============================================================================
// Nacos-specific conversion helpers
// ============================================================================

/// Convert from Nacos Instance to unified ServiceInstance
pub mod nacos {
    use super::*;
    use crate::api::naming::model::Instance as NacosInstance;

    impl From<NacosInstance> for ServiceInstance {
        fn from(nacos: NacosInstance) -> Self {
            ServiceInstance {
                id: nacos.instance_id,
                service: nacos.service_name,
                namespace: String::new(), // Set by caller
                address: nacos.ip,
                port: nacos.port as u16,
                weight: nacos.weight,
                healthy: nacos.healthy,
                enabled: nacos.enabled,
                ephemeral: nacos.ephemeral,
                tags: Vec::new(),
                metadata: nacos.metadata,
                cluster: nacos.cluster_name,
                group: String::new(), // Set by caller
            }
        }
    }

    impl From<ServiceInstance> for NacosInstance {
        fn from(instance: ServiceInstance) -> Self {
            NacosInstance {
                instance_id: instance.id,
                ip: instance.address,
                port: instance.port as i32,
                weight: instance.weight,
                healthy: instance.healthy,
                enabled: instance.enabled,
                ephemeral: instance.ephemeral,
                cluster_name: instance.cluster,
                service_name: instance.service,
                metadata: instance.metadata,
                instance_heart_beat_interval: 5000,
                instance_heart_beat_time_out: 15000,
                ip_delete_timeout: 30000,
                instance_id_generator: "simple".to_string(),
            }
        }
    }
}

// ============================================================================
// Consul-specific conversion helpers
// ============================================================================

/// Consul-specific types and conversions
pub mod consul {
    use super::*;

    /// Consul Agent Service Registration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentServiceRegistration {
        #[serde(rename = "ID")]
        pub id: Option<String>,
        #[serde(rename = "Name")]
        pub name: String,
        #[serde(rename = "Tags")]
        pub tags: Option<Vec<String>>,
        #[serde(rename = "Port")]
        pub port: Option<u16>,
        #[serde(rename = "Address")]
        pub address: Option<String>,
        #[serde(rename = "Meta")]
        pub meta: Option<HashMap<String, String>>,
        #[serde(rename = "EnableTagOverride")]
        pub enable_tag_override: Option<bool>,
        #[serde(rename = "Weights")]
        pub weights: Option<Weights>,
        #[serde(rename = "Check")]
        pub check: Option<AgentServiceCheck>,
        #[serde(rename = "Checks")]
        pub checks: Option<Vec<AgentServiceCheck>>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Weights {
        #[serde(rename = "Passing")]
        pub passing: i32,
        #[serde(rename = "Warning")]
        pub warning: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentServiceCheck {
        #[serde(rename = "CheckID")]
        pub check_id: Option<String>,
        #[serde(rename = "Name")]
        pub name: Option<String>,
        #[serde(rename = "TTL")]
        pub ttl: Option<String>,
        #[serde(rename = "HTTP")]
        pub http: Option<String>,
        #[serde(rename = "TCP")]
        pub tcp: Option<String>,
        #[serde(rename = "Interval")]
        pub interval: Option<String>,
        #[serde(rename = "Timeout")]
        pub timeout: Option<String>,
        #[serde(rename = "DeregisterCriticalServiceAfter")]
        pub deregister_critical_service_after: Option<String>,
    }

    /// Consul Catalog Service
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CatalogService {
        #[serde(rename = "ID")]
        pub id: String,
        #[serde(rename = "Node")]
        pub node: String,
        #[serde(rename = "Address")]
        pub address: String,
        #[serde(rename = "Datacenter")]
        pub datacenter: String,
        #[serde(rename = "ServiceID")]
        pub service_id: String,
        #[serde(rename = "ServiceName")]
        pub service_name: String,
        #[serde(rename = "ServiceAddress")]
        pub service_address: String,
        #[serde(rename = "ServicePort")]
        pub service_port: u16,
        #[serde(rename = "ServiceTags")]
        pub service_tags: Option<Vec<String>>,
        #[serde(rename = "ServiceMeta")]
        pub service_meta: Option<HashMap<String, String>>,
        #[serde(rename = "ServiceWeights")]
        pub service_weights: Option<Weights>,
    }

    /// Consul Health Service Entry
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ServiceEntry {
        #[serde(rename = "Node")]
        pub node: NodeInfo,
        #[serde(rename = "Service")]
        pub service: AgentService,
        #[serde(rename = "Checks")]
        pub checks: Vec<HealthCheck>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NodeInfo {
        #[serde(rename = "ID")]
        pub id: String,
        #[serde(rename = "Node")]
        pub node: String,
        #[serde(rename = "Address")]
        pub address: String,
        #[serde(rename = "Datacenter")]
        pub datacenter: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AgentService {
        #[serde(rename = "ID")]
        pub id: String,
        #[serde(rename = "Service")]
        pub service: String,
        #[serde(rename = "Tags")]
        pub tags: Option<Vec<String>>,
        #[serde(rename = "Port")]
        pub port: u16,
        #[serde(rename = "Address")]
        pub address: String,
        #[serde(rename = "Meta")]
        pub meta: Option<HashMap<String, String>>,
        #[serde(rename = "Weights")]
        pub weights: Option<Weights>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HealthCheck {
        #[serde(rename = "CheckID")]
        pub check_id: String,
        #[serde(rename = "Status")]
        pub status: String,
        #[serde(rename = "Output")]
        pub output: String,
        #[serde(rename = "ServiceID")]
        pub service_id: String,
        #[serde(rename = "ServiceName")]
        pub service_name: String,
    }

    use serde::{Deserialize, Serialize};

    impl From<AgentServiceRegistration> for ServiceInstance {
        fn from(consul: AgentServiceRegistration) -> Self {
            let weight = consul
                .weights
                .as_ref()
                .map(|w| w.passing as f64)
                .unwrap_or(1.0);

            ServiceInstance {
                id: consul.id.unwrap_or_else(|| consul.name.clone()),
                service: consul.name,
                namespace: String::new(), // Consul uses datacenter, set by caller
                address: consul.address.unwrap_or_default(),
                port: consul.port.unwrap_or(0),
                weight,
                healthy: true, // Default, updated by health checks
                enabled: true,
                ephemeral: true,
                tags: consul.tags.unwrap_or_default(),
                metadata: consul.meta.unwrap_or_default(),
                cluster: "DEFAULT".to_string(),
                group: String::new(), // Consul doesn't have groups
            }
        }
    }

    impl From<ServiceInstance> for AgentServiceRegistration {
        fn from(instance: ServiceInstance) -> Self {
            let mut meta = instance.metadata;
            // Store group in meta for Nacos compatibility
            if !instance.group.is_empty() {
                meta.insert("nacos_group".to_string(), instance.group);
            }
            if !instance.cluster.is_empty() {
                meta.insert("nacos_cluster".to_string(), instance.cluster);
            }

            AgentServiceRegistration {
                id: Some(instance.id),
                name: instance.service,
                tags: if instance.tags.is_empty() {
                    None
                } else {
                    Some(instance.tags)
                },
                port: Some(instance.port),
                address: Some(instance.address),
                meta: if meta.is_empty() { None } else { Some(meta) },
                enable_tag_override: None,
                weights: Some(Weights {
                    passing: instance.weight as i32,
                    warning: 1,
                }),
                check: None,
                checks: None,
            }
        }
    }

    impl From<ServiceEntry> for ServiceInstance {
        fn from(entry: ServiceEntry) -> Self {
            let healthy = entry.checks.iter().all(|c| c.status == "passing");
            let mut metadata = entry.service.meta.unwrap_or_default();

            // Extract Nacos-specific fields from meta
            let group = metadata.remove("nacos_group").unwrap_or_default();
            let cluster = metadata
                .remove("nacos_cluster")
                .unwrap_or_else(|| "DEFAULT".to_string());

            ServiceInstance {
                id: entry.service.id,
                service: entry.service.service,
                namespace: entry.node.datacenter,
                address: if entry.service.address.is_empty() {
                    entry.node.address
                } else {
                    entry.service.address
                },
                port: entry.service.port,
                weight: entry
                    .service
                    .weights
                    .map(|w| w.passing as f64)
                    .unwrap_or(1.0),
                healthy,
                enabled: true,
                ephemeral: true,
                tags: entry.service.tags.unwrap_or_default(),
                metadata,
                cluster,
                group,
            }
        }
    }
}
