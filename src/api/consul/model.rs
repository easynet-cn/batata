// Consul API data models
// These models match the Consul Agent API specification

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Service registration request
/// PUT /v1/agent/service/register
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentServiceRegistration {
    /// Service ID (optional, defaults to Name if not provided)
    #[serde(rename = "ID", default)]
    pub id: Option<String>,

    /// Service name (required)
    #[serde(rename = "Name")]
    pub name: String,

    /// Service tags for filtering and metadata
    #[serde(rename = "Tags", default)]
    pub tags: Option<Vec<String>>,

    /// Service address (optional, uses agent address if not provided)
    #[serde(rename = "Address", default)]
    pub address: Option<String>,

    /// Service port
    #[serde(rename = "Port", default)]
    pub port: Option<u16>,

    /// Service metadata key-value pairs
    #[serde(rename = "Meta", default)]
    pub meta: Option<HashMap<String, String>>,

    /// Enable tag override from external sources
    #[serde(rename = "EnableTagOverride", default)]
    pub enable_tag_override: Option<bool>,

    /// Service weights for load balancing
    #[serde(rename = "Weights", default)]
    pub weights: Option<Weights>,

    /// Single health check definition
    #[serde(rename = "Check", default)]
    pub check: Option<AgentServiceCheck>,

    /// Multiple health check definitions
    #[serde(rename = "Checks", default)]
    pub checks: Option<Vec<AgentServiceCheck>>,

    /// Namespace (Consul Enterprise, maps to Nacos namespace)
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
}

impl AgentServiceRegistration {
    /// Get the effective service ID
    pub fn service_id(&self) -> String {
        self.id.clone().unwrap_or_else(|| self.name.clone())
    }

    /// Get the effective address
    pub fn effective_address(&self) -> String {
        self.address
            .clone()
            .unwrap_or_else(|| "127.0.0.1".to_string())
    }

    /// Get the effective port
    pub fn effective_port(&self) -> u16 {
        self.port.unwrap_or(0)
    }

    /// Get the weight value
    pub fn weight(&self) -> f64 {
        self.weights
            .as_ref()
            .map(|w| w.passing as f64)
            .unwrap_or(1.0)
    }
}

/// Service weights for load balancing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Weights {
    /// Weight when service is passing health checks
    #[serde(rename = "Passing", default = "default_passing_weight")]
    pub passing: i32,

    /// Weight when service has warning health checks
    #[serde(rename = "Warning", default = "default_warning_weight")]
    pub warning: i32,
}

fn default_passing_weight() -> i32 {
    1
}

fn default_warning_weight() -> i32 {
    1
}

/// Health check definition for service registration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentServiceCheck {
    /// Check ID (optional, auto-generated if not provided)
    #[serde(rename = "CheckID", default)]
    pub check_id: Option<String>,

    /// Check name
    #[serde(rename = "Name", default)]
    pub name: Option<String>,

    /// TTL-based check duration (e.g., "30s")
    #[serde(rename = "TTL", default)]
    pub ttl: Option<String>,

    /// HTTP endpoint for HTTP checks
    #[serde(rename = "HTTP", default)]
    pub http: Option<String>,

    /// HTTP method (GET, POST, etc.)
    #[serde(rename = "Method", default)]
    pub method: Option<String>,

    /// HTTP headers
    #[serde(rename = "Header", default)]
    pub header: Option<HashMap<String, Vec<String>>>,

    /// TCP address for TCP checks
    #[serde(rename = "TCP", default)]
    pub tcp: Option<String>,

    /// gRPC endpoint for gRPC checks
    #[serde(rename = "GRPC", default)]
    pub grpc: Option<String>,

    /// Check interval (e.g., "10s")
    #[serde(rename = "Interval", default)]
    pub interval: Option<String>,

    /// Check timeout (e.g., "5s")
    #[serde(rename = "Timeout", default)]
    pub timeout: Option<String>,

    /// Deregister after critical for duration
    #[serde(rename = "DeregisterCriticalServiceAfter", default)]
    pub deregister_critical_service_after: Option<String>,

    /// Notes for the check
    #[serde(rename = "Notes", default)]
    pub notes: Option<String>,

    /// Initial status
    #[serde(rename = "Status", default)]
    pub status: Option<String>,
}

/// Agent service representation (response format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentService {
    /// Service ID
    #[serde(rename = "ID")]
    pub id: String,

    /// Service name
    #[serde(rename = "Service")]
    pub service: String,

    /// Service tags
    #[serde(rename = "Tags")]
    pub tags: Option<Vec<String>>,

    /// Service port
    #[serde(rename = "Port")]
    pub port: u16,

    /// Service address
    #[serde(rename = "Address")]
    pub address: String,

    /// Service metadata
    #[serde(rename = "Meta")]
    pub meta: Option<HashMap<String, String>>,

    /// Tag override enabled
    #[serde(rename = "EnableTagOverride")]
    pub enable_tag_override: bool,

    /// Service weights
    #[serde(rename = "Weights")]
    pub weights: Weights,

    /// Datacenter
    #[serde(rename = "Datacenter", skip_serializing_if = "Option::is_none")]
    pub datacenter: Option<String>,

    /// Namespace
    #[serde(rename = "Namespace", skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Full agent service response with checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentServiceWithChecks {
    /// Service information
    #[serde(flatten)]
    pub service: AgentService,

    /// Associated health checks
    #[serde(rename = "Checks", skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<AgentCheck>>,
}

/// Agent health check representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCheck {
    /// Check ID
    #[serde(rename = "CheckID")]
    pub check_id: String,

    /// Check name
    #[serde(rename = "Name")]
    pub name: String,

    /// Check status (passing, warning, critical)
    #[serde(rename = "Status")]
    pub status: String,

    /// Check notes
    #[serde(rename = "Notes")]
    pub notes: String,

    /// Check output
    #[serde(rename = "Output")]
    pub output: String,

    /// Associated service ID
    #[serde(rename = "ServiceID")]
    pub service_id: String,

    /// Associated service name
    #[serde(rename = "ServiceName")]
    pub service_name: String,

    /// Check type
    #[serde(rename = "Type")]
    pub check_type: String,
}

/// Maintenance mode request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceRequest {
    /// Enable or disable maintenance mode
    #[serde(default)]
    pub enable: bool,

    /// Reason for maintenance
    #[serde(default)]
    pub reason: Option<String>,
}

/// Query parameters for service endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServiceQueryParams {
    /// Filter by namespace
    pub ns: Option<String>,

    /// Filter string (Consul filtering syntax)
    pub filter: Option<String>,
}

/// Generic Consul API response
#[derive(Debug, Clone, Serialize)]
pub struct ConsulResponse<T> {
    #[serde(flatten)]
    pub data: T,
}

/// Consul API error response
#[derive(Debug, Clone, Serialize)]
pub struct ConsulError {
    pub error: String,
}

impl ConsulError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
        }
    }
}

// ============================================================================
// Health Check Models
// ============================================================================

/// Node information in health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
    #[serde(rename = "TaggedAddresses", skip_serializing_if = "Option::is_none")]
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(rename = "Meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            node: "batata-node".to_string(),
            address: "127.0.0.1".to_string(),
            datacenter: "dc1".to_string(),
            tagged_addresses: None,
            meta: None,
        }
    }
}

/// Health check registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRegistration {
    #[serde(rename = "Name")]
    pub name: String,

    #[serde(rename = "CheckID", default)]
    pub check_id: Option<String>,

    #[serde(rename = "ServiceID", default)]
    pub service_id: Option<String>,

    #[serde(rename = "Notes", default)]
    pub notes: Option<String>,

    #[serde(rename = "TTL", default)]
    pub ttl: Option<String>,

    #[serde(rename = "HTTP", default)]
    pub http: Option<String>,

    #[serde(rename = "Method", default)]
    pub method: Option<String>,

    #[serde(rename = "Header", default)]
    pub header: Option<HashMap<String, Vec<String>>>,

    #[serde(rename = "TCP", default)]
    pub tcp: Option<String>,

    #[serde(rename = "GRPC", default)]
    pub grpc: Option<String>,

    #[serde(rename = "Interval", default)]
    pub interval: Option<String>,

    #[serde(rename = "Timeout", default)]
    pub timeout: Option<String>,

    #[serde(rename = "DeregisterCriticalServiceAfter", default)]
    pub deregister_critical_service_after: Option<String>,

    #[serde(rename = "Status", default)]
    pub status: Option<String>,
}

impl CheckRegistration {
    /// Get the effective check ID
    pub fn effective_check_id(&self) -> String {
        self.check_id.clone().unwrap_or_else(|| {
            if let Some(ref service_id) = self.service_id {
                format!("service:{}", service_id)
            } else {
                format!("check:{}", self.name)
            }
        })
    }

    /// Determine the check type
    pub fn check_type(&self) -> &'static str {
        if self.ttl.is_some() {
            "ttl"
        } else if self.http.is_some() {
            "http"
        } else if self.tcp.is_some() {
            "tcp"
        } else if self.grpc.is_some() {
            "grpc"
        } else {
            "ttl" // default
        }
    }
}

/// Health check status update
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CheckStatusUpdate {
    #[serde(rename = "Status", default)]
    pub status: Option<String>,
    #[serde(rename = "Output", default)]
    pub output: Option<String>,
}

/// Health check information in responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    #[serde(rename = "Node")]
    pub node: String,

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

    #[serde(rename = "ServiceTags", skip_serializing_if = "Option::is_none")]
    pub service_tags: Option<Vec<String>>,

    #[serde(rename = "Type")]
    pub check_type: String,

    #[serde(rename = "CreateIndex", skip_serializing_if = "Option::is_none")]
    pub create_index: Option<u64>,

    #[serde(rename = "ModifyIndex", skip_serializing_if = "Option::is_none")]
    pub modify_index: Option<u64>,
}

impl Default for HealthCheck {
    fn default() -> Self {
        Self {
            node: "batata-node".to_string(),
            check_id: String::new(),
            name: String::new(),
            status: "passing".to_string(),
            notes: String::new(),
            output: String::new(),
            service_id: String::new(),
            service_name: String::new(),
            service_tags: None,
            check_type: "ttl".to_string(),
            create_index: None,
            modify_index: None,
        }
    }
}

/// Service health entry (response for /v1/health/service/:service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    #[serde(rename = "Node")]
    pub node: Node,

    #[serde(rename = "Service")]
    pub service: AgentService,

    #[serde(rename = "Checks")]
    pub checks: Vec<HealthCheck>,
}

/// Query parameters for health endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct HealthQueryParams {
    /// Only return passing instances
    pub passing: Option<bool>,

    /// Filter by tag
    pub tag: Option<String>,

    /// Datacenter
    pub dc: Option<String>,

    /// Namespace (Enterprise)
    pub ns: Option<String>,

    /// Filter expression
    pub filter: Option<String>,
}

/// Query parameters for check update endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CheckUpdateParams {
    /// Optional note/output
    pub note: Option<String>,
}

// ============================================================================
// Conversion implementations
// ============================================================================

use crate::api::naming::model::Instance as NacosInstance;

impl From<&AgentServiceRegistration> for NacosInstance {
    fn from(reg: &AgentServiceRegistration) -> Self {
        let mut metadata = reg.meta.clone().unwrap_or_default();

        // Store Consul-specific fields in metadata
        if let Some(ref tags) = reg.tags {
            metadata.insert(
                "consul_tags".to_string(),
                serde_json::to_string(tags).unwrap_or_default(),
            );
        }
        if let Some(enable_tag_override) = reg.enable_tag_override {
            metadata.insert(
                "enable_tag_override".to_string(),
                enable_tag_override.to_string(),
            );
        }

        NacosInstance {
            instance_id: reg.service_id(),
            ip: reg.effective_address(),
            port: reg.effective_port() as i32,
            weight: reg.weight(),
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: reg.name.clone(),
            metadata,
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: "simple".to_string(),
        }
    }
}

impl From<&NacosInstance> for AgentService {
    fn from(instance: &NacosInstance) -> Self {
        // Extract tags from metadata
        let tags = instance
            .metadata
            .get("consul_tags")
            .and_then(|s| serde_json::from_str(s).ok());

        // Extract enable_tag_override from metadata
        let enable_tag_override = instance
            .metadata
            .get("enable_tag_override")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        // Filter out Consul-specific metadata keys
        let meta: HashMap<String, String> = instance
            .metadata
            .iter()
            .filter(|(k, _)| !k.starts_with("consul_") && k.as_str() != "enable_tag_override")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        AgentService {
            id: instance.instance_id.clone(),
            service: instance.service_name.clone(),
            tags,
            port: instance.port as u16,
            address: instance.ip.clone(),
            meta: if meta.is_empty() { None } else { Some(meta) },
            enable_tag_override,
            weights: Weights {
                passing: instance.weight as i32,
                warning: 1,
            },
            datacenter: None,
            namespace: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_registration_deserialize() {
        let json = r#"{
            "ID": "redis1",
            "Name": "redis",
            "Tags": ["primary", "v1"],
            "Address": "127.0.0.1",
            "Port": 6379,
            "Meta": {
                "version": "6.0"
            },
            "Weights": {
                "Passing": 10,
                "Warning": 1
            }
        }"#;

        let reg: AgentServiceRegistration = serde_json::from_str(json).unwrap();
        assert_eq!(reg.service_id(), "redis1");
        assert_eq!(reg.name, "redis");
        assert_eq!(reg.effective_port(), 6379);
        assert_eq!(reg.weight(), 10.0);
    }

    #[test]
    fn test_service_registration_defaults() {
        let json = r#"{"Name": "test-service"}"#;

        let reg: AgentServiceRegistration = serde_json::from_str(json).unwrap();
        assert_eq!(reg.service_id(), "test-service"); // Defaults to name
        assert_eq!(reg.effective_address(), "127.0.0.1");
        assert_eq!(reg.effective_port(), 0);
        assert_eq!(reg.weight(), 1.0);
    }

    #[test]
    fn test_nacos_instance_conversion() {
        let reg = AgentServiceRegistration {
            id: Some("web-1".to_string()),
            name: "web".to_string(),
            tags: Some(vec!["http".to_string(), "api".to_string()]),
            address: Some("192.168.1.100".to_string()),
            port: Some(8080),
            meta: Some([("env".to_string(), "prod".to_string())].into()),
            enable_tag_override: Some(true),
            weights: Some(Weights {
                passing: 5,
                warning: 1,
            }),
            check: None,
            checks: None,
            namespace: None,
        };

        let nacos: NacosInstance = (&reg).into();
        assert_eq!(nacos.instance_id, "web-1");
        assert_eq!(nacos.service_name, "web");
        assert_eq!(nacos.ip, "192.168.1.100");
        assert_eq!(nacos.port, 8080);
        assert_eq!(nacos.weight, 5.0);
        assert!(nacos.metadata.contains_key("consul_tags"));
        assert!(nacos.metadata.contains_key("env"));
    }
}
