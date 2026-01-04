// Common types shared across different registry implementations
// These types are registry-agnostic and can be mapped to/from Nacos, Consul, etc.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unified service instance representation
/// Maps to: Nacos Instance, Consul ServiceEntry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    /// Unique instance identifier
    pub id: String,
    /// Service name
    pub service: String,
    /// Namespace/Datacenter
    pub namespace: String,
    /// Instance IP address
    pub address: String,
    /// Instance port
    pub port: u16,
    /// Instance weight for load balancing
    pub weight: f64,
    /// Health status
    pub healthy: bool,
    /// Whether instance is enabled
    pub enabled: bool,
    /// Whether instance is ephemeral (Nacos) or not
    pub ephemeral: bool,
    /// Tags/Labels (Consul: tags, Nacos: metadata keys)
    pub tags: Vec<String>,
    /// Metadata/Meta (key-value pairs)
    pub metadata: HashMap<String, String>,
    /// Cluster name (Nacos) or Datacenter (Consul)
    pub cluster: String,
    /// Group name (Nacos specific, empty for Consul)
    pub group: String,
}

impl ServiceInstance {
    /// Create instance key for storage
    pub fn instance_key(&self) -> String {
        format!("{}:{}:{}", self.service, self.address, self.port)
    }

    /// Create service key for grouping
    pub fn service_key(&self) -> String {
        if self.group.is_empty() {
            format!("{}@@{}", self.namespace, self.service)
        } else {
            format!("{}@@{}@@{}", self.namespace, self.group, self.service)
        }
    }
}

impl Default for ServiceInstance {
    fn default() -> Self {
        Self {
            id: String::new(),
            service: String::new(),
            namespace: "public".to_string(),
            address: String::new(),
            port: 0,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            tags: Vec::new(),
            metadata: HashMap::new(),
            cluster: "DEFAULT".to_string(),
            group: "DEFAULT_GROUP".to_string(),
        }
    }
}

/// Unified service definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    /// Service name
    pub name: String,
    /// Namespace/Datacenter
    pub namespace: String,
    /// Group (Nacos specific)
    pub group: String,
    /// Service-level metadata
    pub metadata: HashMap<String, String>,
    /// Protection threshold (Nacos specific)
    pub protect_threshold: f64,
    /// Service instances
    pub instances: Vec<ServiceInstance>,
}

impl Default for ServiceDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            namespace: "public".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            metadata: HashMap::new(),
            protect_threshold: 0.0,
            instances: Vec::new(),
        }
    }
}

/// Unified configuration item
/// Maps to: Nacos ConfigInfo, Consul KV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItem {
    /// Configuration key (Nacos: dataId, Consul: key)
    pub key: String,
    /// Group (Nacos specific, empty for Consul)
    pub group: String,
    /// Namespace (Nacos: tenant, Consul: datacenter)
    pub namespace: String,
    /// Configuration content/value
    pub content: String,
    /// Content type (json, yaml, properties, text)
    pub content_type: String,
    /// MD5 hash for change detection
    pub md5: String,
    /// Version/ModifyIndex
    pub version: u64,
    /// Application name
    pub app_name: String,
    /// Description
    pub description: String,
    /// Tags/Labels
    pub tags: Vec<String>,
    /// Creation timestamp
    pub created_at: i64,
    /// Last modified timestamp
    pub modified_at: i64,
}

impl ConfigItem {
    /// Create config key for storage
    pub fn config_key(&self) -> String {
        if self.group.is_empty() {
            format!("{}@@{}", self.namespace, self.key)
        } else {
            format!("{}@@{}@@{}", self.namespace, self.group, self.key)
        }
    }

    /// Calculate MD5 from content
    pub fn calculate_md5(&self) -> String {
        format!("{:x}", md5::compute(&self.content))
    }
}

impl Default for ConfigItem {
    fn default() -> Self {
        Self {
            key: String::new(),
            group: "DEFAULT_GROUP".to_string(),
            namespace: "public".to_string(),
            content: String::new(),
            content_type: "text".to_string(),
            md5: String::new(),
            version: 0,
            app_name: String::new(),
            description: String::new(),
            tags: Vec::new(),
            created_at: 0,
            modified_at: 0,
        }
    }
}

/// Health check definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Check ID
    pub id: String,
    /// Check name
    pub name: String,
    /// Service ID this check belongs to
    pub service_id: String,
    /// Check type
    pub check_type: HealthCheckType,
    /// Check interval in seconds
    pub interval_secs: u64,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// HTTP endpoint (for HTTP checks)
    pub http_endpoint: Option<String>,
    /// TCP address (for TCP checks)
    pub tcp_address: Option<String>,
    /// Script to run (for script checks)
    pub script: Option<String>,
    /// Current status
    pub status: HealthStatus,
    /// Last check output
    pub output: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthCheckType {
    /// TTL-based (client heartbeat)
    Ttl,
    /// HTTP endpoint check
    Http,
    /// TCP connection check
    Tcp,
    /// Script/command check
    Script,
    /// gRPC health check
    Grpc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HealthStatus {
    Passing,
    Warning,
    Critical,
    #[default]
    Unknown,
}

/// Query options for service discovery
#[derive(Debug, Clone, Default)]
pub struct ServiceQuery {
    /// Namespace/Datacenter filter
    pub namespace: Option<String>,
    /// Group filter (Nacos specific)
    pub group: Option<String>,
    /// Service name filter (supports wildcards)
    pub service: Option<String>,
    /// Cluster filter
    pub cluster: Option<String>,
    /// Tag filter
    pub tags: Vec<String>,
    /// Only return healthy instances
    pub healthy_only: bool,
    /// Only return enabled instances
    pub enabled_only: bool,
    /// Pagination: page number (1-based)
    pub page: Option<u32>,
    /// Pagination: page size
    pub page_size: Option<u32>,
}

/// Query options for configuration
#[derive(Debug, Clone, Default)]
pub struct ConfigQuery {
    /// Namespace filter
    pub namespace: Option<String>,
    /// Group filter (Nacos specific)
    pub group: Option<String>,
    /// Key/DataId filter (supports wildcards)
    pub key: Option<String>,
    /// Application name filter
    pub app_name: Option<String>,
    /// Content pattern search
    pub content_pattern: Option<String>,
    /// Tag filter
    pub tags: Vec<String>,
    /// Pagination: page number (1-based)
    pub page: Option<u32>,
    /// Pagination: page size
    pub page_size: Option<u32>,
    /// Include deleted items
    pub include_deleted: bool,
}

/// Paginated result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

impl<T> PagedResult<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: 0,
            page: 1,
            page_size: 10,
        }
    }

    pub fn new(items: Vec<T>, total: u64, page: u32, page_size: u32) -> Self {
        Self {
            items,
            total,
            page,
            page_size,
        }
    }
}

/// Change event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeEvent {
    /// Service instance changed
    ServiceChanged {
        namespace: String,
        group: String,
        service: String,
        instances: Vec<ServiceInstance>,
    },
    /// Configuration changed
    ConfigChanged {
        namespace: String,
        group: String,
        key: String,
        content: Option<String>,
        change_type: ChangeType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}

/// Registry type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryType {
    Nacos,
    Consul,
}

impl std::fmt::Display for RegistryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryType::Nacos => write!(f, "nacos"),
            RegistryType::Consul => write!(f, "consul"),
        }
    }
}
