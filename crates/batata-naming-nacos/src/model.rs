//! Nacos naming domain models
//!
//! These models are Nacos-specific and contain NO Consul concepts.
//! They represent the full fidelity of Nacos service discovery semantics.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Instance
// ============================================================================

/// A Nacos service instance
///
/// Identified by: `ip#port#clusterName#serviceName` within a namespace+group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NacosInstance {
    /// Unique instance identifier (format: "ip#port#clusterName#serviceName")
    pub instance_id: String,
    /// Instance IP address
    pub ip: String,
    /// Instance port
    pub port: i32,
    /// Load balancing weight, clamped to [0.0, 10000.0]
    pub weight: f64,
    /// Whether the instance is healthy
    pub healthy: bool,
    /// Whether the instance is enabled (admin control)
    pub enabled: bool,
    /// Ephemeral (heartbeat-based) vs Persistent (server-managed)
    pub ephemeral: bool,
    /// Cluster this instance belongs to (default: "DEFAULT")
    pub cluster_name: String,
    /// Fully qualified service name (group@@serviceName)
    pub service_name: String,
    /// Custom metadata key-value pairs
    pub metadata: HashMap<String, String>,
}

impl NacosInstance {
    /// Generate instance ID from components
    pub fn generate_id(ip: &str, port: i32, cluster: &str, service: &str) -> String {
        format!("{ip}#{port}#{cluster}#{service}")
    }

    /// Storage key within a service: "ip#port#clusterName"
    pub fn instance_key(&self) -> String {
        format!("{}#{}#{}", self.ip, self.port, self.cluster_name)
    }

    /// Get heartbeat interval from preserved metadata (default: 5000ms)
    pub fn heartbeat_interval_ms(&self) -> i64 {
        self.metadata
            .get("preserved.heart.beat.interval")
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000)
    }

    /// Get heartbeat timeout from preserved metadata (default: 15000ms)
    pub fn heartbeat_timeout_ms(&self) -> i64 {
        self.metadata
            .get("preserved.heart.beat.timeout")
            .and_then(|v| v.parse().ok())
            .unwrap_or(15000)
    }

    /// Get IP delete timeout from preserved metadata (default: 30000ms)
    pub fn ip_delete_timeout_ms(&self) -> i64 {
        self.metadata
            .get("preserved.ip.delete.timeout")
            .and_then(|v| v.parse().ok())
            .unwrap_or(30000)
    }
}

impl Default for NacosInstance {
    fn default() -> Self {
        Self {
            instance_id: String::new(),
            ip: String::new(),
            port: 0,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: String::new(),
            metadata: HashMap::new(),
        }
    }
}

// ============================================================================
// Service
// ============================================================================

/// A Nacos service definition
///
/// Identified by: `namespace@@group@@serviceName`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NacosService {
    /// Service name
    pub name: String,
    /// Group name (default: "DEFAULT_GROUP")
    pub group_name: String,
    /// Comma-separated cluster names
    pub clusters: String,
    /// Client-side cache TTL in milliseconds (default: 1000)
    pub cache_millis: i64,
    /// All instances of this service
    pub hosts: Vec<NacosInstance>,
    /// Last update timestamp
    pub last_ref_time: i64,
    /// Checksum for change detection
    pub checksum: String,
    /// Whether to return all IPs including unhealthy
    pub all_ips: bool,
    /// Whether protection threshold is currently triggered
    pub reach_protection_threshold: bool,
}

impl Default for NacosService {
    fn default() -> Self {
        Self {
            name: String::new(),
            group_name: "DEFAULT_GROUP".to_string(),
            clusters: String::new(),
            cache_millis: 1000,
            hosts: Vec::new(),
            last_ref_time: 0,
            checksum: String::new(),
            all_ips: false,
            reach_protection_threshold: false,
        }
    }
}

impl NacosService {
    /// Get only healthy and enabled hosts
    pub fn healthy_hosts(&self) -> Vec<&NacosInstance> {
        self.hosts
            .iter()
            .filter(|h| h.healthy && h.enabled)
            .collect()
    }

    /// Build service key: "namespace@@group@@service"
    pub fn service_key(namespace: &str, group: &str, service: &str) -> String {
        format!("{namespace}@@{group}@@{service}")
    }
}

// ============================================================================
// Service Metadata
// ============================================================================

/// Service-level metadata (Nacos-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Protection threshold [0.0, 1.0]
    ///
    /// When the ratio of healthy instances falls below this threshold,
    /// unhealthy instances are also returned to prevent cascading failures.
    pub protect_threshold: f32,
    /// Service-level metadata
    pub metadata: HashMap<String, String>,
    /// Selector type: "none", "label", "expression"
    pub selector_type: String,
    /// Selector expression for instance filtering
    pub selector_expression: String,
    /// Default ephemeral setting for new instances
    pub ephemeral: bool,
    /// Monotonic revision counter for change detection
    pub revision: u64,
}

impl Default for ServiceMetadata {
    fn default() -> Self {
        Self {
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            selector_type: "none".to_string(),
            selector_expression: String::new(),
            ephemeral: true,
            revision: 0,
        }
    }
}

// ============================================================================
// Cluster
// ============================================================================

/// Cluster configuration (Nacos-specific)
///
/// Each service can have multiple clusters, each with its own
/// health check configuration and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Cluster name
    pub name: String,
    /// Health check type for this cluster
    pub health_check_type: HealthCheckType,
    /// Health check port (0 = use instance port)
    pub health_check_port: u16,
    /// Whether to use instance port for health check
    pub use_instance_port: bool,
    /// Cluster-level metadata
    pub metadata: HashMap<String, String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            name: "DEFAULT".to_string(),
            health_check_type: HealthCheckType::None,
            health_check_port: 0,
            use_instance_port: true,
            metadata: HashMap::new(),
        }
    }
}

/// Nacos health check types (cluster-level)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HealthCheckType {
    /// No active health check (rely on client heartbeat for ephemeral)
    #[default]
    None,
    /// TCP connection check
    Tcp,
    /// HTTP endpoint check
    Http,
    /// MySQL connection check
    Mysql,
}

// ============================================================================
// Protection
// ============================================================================

/// Protection threshold information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionInfo {
    /// Configured threshold
    pub threshold: f32,
    /// Total instance count
    pub total_instances: usize,
    /// Healthy instance count
    pub healthy_instances: usize,
    /// Ratio of healthy instances
    pub healthy_ratio: f32,
    /// Whether protection is currently triggered
    pub triggered: bool,
}

// ============================================================================
// Query
// ============================================================================

/// Query parameters for Nacos service discovery
#[derive(Debug, Clone, Default)]
pub struct NacosServiceQuery {
    /// Namespace filter (default: "public")
    pub namespace: String,
    /// Group filter (supports exact match or wildcard)
    pub group: Option<String>,
    /// Service name filter (supports exact match or wildcard)
    pub service: Option<String>,
    /// Cluster filter (comma-separated)
    pub clusters: Option<String>,
    /// Only return healthy instances
    pub healthy_only: bool,
    /// Only return enabled instances
    pub enabled_only: bool,
    /// Page number (1-based)
    pub page: Option<u32>,
    /// Page size
    pub page_size: Option<u32>,
}

/// Query parameters for Nacos instance listing
#[derive(Debug, Clone)]
pub struct NacosInstanceQuery {
    pub namespace: String,
    pub group: String,
    pub service: String,
    pub clusters: Option<String>,
    pub healthy_only: bool,
    pub enabled_only: bool,
    pub ephemeral: Option<bool>,
}
