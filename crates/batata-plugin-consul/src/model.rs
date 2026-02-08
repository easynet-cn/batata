// Consul API data models
// These models match the Consul Agent API specification

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Consul-compatible boolean query parameter deserialization.
/// In Consul, query parameters like `?recurse` (key-present with empty value) mean `true`.
/// Standard serde can't parse empty string "" as bool.
pub mod consul_bool {
    use serde::{self, Deserialize, Deserializer};

    /// Deserialize Option<bool> where empty string means Some(true) (Consul convention)
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Accept either bool or string
        let opt: Option<serde_json::Value> = Option::deserialize(deserializer)?;
        match opt {
            None => Ok(None),
            Some(serde_json::Value::Bool(b)) => Ok(Some(b)),
            Some(serde_json::Value::String(ref s)) if s.is_empty() => Ok(Some(true)),
            Some(serde_json::Value::String(ref s)) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(Some(true)),
                "false" | "0" | "no" => Ok(Some(false)),
                _ => Ok(Some(true)), // key-present = true
            },
            Some(serde_json::Value::Null) => Ok(None),
            _ => Ok(Some(true)),
        }
    }
}

/// Consul-compatible integer query parameter deserialization.
/// Handles empty strings gracefully by returning None.
pub mod consul_u64 {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<serde_json::Value> = Option::deserialize(deserializer)?;
        match opt {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::Number(n)) => Ok(n.as_u64()),
            Some(serde_json::Value::String(ref s)) if s.is_empty() => Ok(None),
            Some(serde_json::Value::String(ref s)) => Ok(s.parse().ok()),
            _ => Ok(None),
        }
    }
}

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

    /// Service kind (e.g., "connect-proxy", "mesh-gateway")
    #[serde(rename = "Kind", default)]
    pub kind: Option<String>,

    /// Proxy configuration for connect-proxy services
    #[serde(rename = "Proxy", default)]
    pub proxy: Option<serde_json::Value>,

    /// Connect configuration for mesh-enabled services
    #[serde(rename = "Connect", default)]
    pub connect: Option<serde_json::Value>,

    /// Tagged addresses for the service
    #[serde(rename = "TaggedAddresses", default)]
    pub tagged_addresses: Option<serde_json::Value>,

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

    /// Service kind (e.g., "connect-proxy", "mesh-gateway")
    #[serde(rename = "Kind", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Proxy configuration
    #[serde(rename = "Proxy", skip_serializing_if = "Option::is_none")]
    pub proxy: Option<serde_json::Value>,

    /// Connect configuration
    #[serde(rename = "Connect", skip_serializing_if = "Option::is_none")]
    pub connect: Option<serde_json::Value>,

    /// Tagged addresses
    #[serde(rename = "TaggedAddresses", skip_serializing_if = "Option::is_none")]
    pub tagged_addresses: Option<serde_json::Value>,

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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckRegistration {
    #[serde(rename = "Name")]
    pub name: String,

    #[serde(rename = "CheckID", alias = "ID", default)]
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
    #[serde(default, deserialize_with = "consul_bool::deserialize")]
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

use batata_api::naming::model::Instance as NacosInstance;

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
        if let Some(ref kind) = reg.kind {
            metadata.insert("consul_kind".to_string(), kind.clone());
        }
        if let Some(ref proxy) = reg.proxy {
            metadata.insert(
                "consul_proxy".to_string(),
                serde_json::to_string(proxy).unwrap_or_default(),
            );
        }
        if let Some(ref connect) = reg.connect {
            metadata.insert(
                "consul_connect".to_string(),
                serde_json::to_string(connect).unwrap_or_default(),
            );
        }
        if let Some(ref tagged_addresses) = reg.tagged_addresses {
            metadata.insert(
                "consul_tagged_addresses".to_string(),
                serde_json::to_string(tagged_addresses).unwrap_or_default(),
            );
        }
        // Store warning weight in metadata
        if let Some(ref weights) = reg.weights {
            metadata.insert(
                "consul_warning_weight".to_string(),
                weights.warning.to_string(),
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

        // Extract Kind, Proxy, Connect, TaggedAddresses from metadata
        let kind = instance.metadata.get("consul_kind").cloned();
        let proxy = instance
            .metadata
            .get("consul_proxy")
            .and_then(|s| serde_json::from_str(s).ok());
        let connect = instance
            .metadata
            .get("consul_connect")
            .and_then(|s| serde_json::from_str(s).ok());
        let tagged_addresses = instance
            .metadata
            .get("consul_tagged_addresses")
            .and_then(|s| serde_json::from_str(s).ok());

        // Filter out Consul-specific metadata keys
        let meta: HashMap<String, String> = instance
            .metadata
            .iter()
            .filter(|(k, _)| !k.starts_with("consul_") && k.as_str() != "enable_tag_override")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Round-trip weight correctly: store as i32 with proper rounding
        let weight = instance.weight.round() as i32;
        let weight = if weight < 1 { 1 } else { weight };

        AgentService {
            id: instance.instance_id.clone(),
            service: instance.service_name.clone(),
            tags,
            port: instance.port as u16,
            address: instance.ip.clone(),
            meta: if meta.is_empty() { None } else { Some(meta) },
            enable_tag_override,
            weights: Weights {
                passing: weight,
                warning: instance
                    .metadata
                    .get("consul_warning_weight")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1),
            },
            datacenter: None,
            kind,
            proxy,
            connect,
            tagged_addresses,
            namespace: None,
        }
    }
}

// ============================================================================
// Agent Core Models
// ============================================================================

/// Agent self response - GET /v1/agent/self
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSelf {
    #[serde(rename = "Config")]
    pub config: AgentConfig,
    #[serde(rename = "Coord", skip_serializing_if = "Option::is_none")]
    pub coord: Option<Coordinate>,
    #[serde(rename = "Member")]
    pub member: AgentMember,
    #[serde(rename = "Meta")]
    pub meta: HashMap<String, String>,
    #[serde(rename = "Stats")]
    pub stats: AgentStats,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
    #[serde(rename = "NodeName")]
    pub node_name: String,
    #[serde(rename = "NodeID")]
    pub node_id: String,
    #[serde(rename = "Server")]
    pub server: bool,
    #[serde(rename = "Revision")]
    pub revision: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "PrimaryDatacenter")]
    pub primary_datacenter: String,
}

/// Network coordinate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinate {
    #[serde(rename = "Adjustment")]
    pub adjustment: f64,
    #[serde(rename = "Error")]
    pub error: f64,
    #[serde(rename = "Height")]
    pub height: f64,
    #[serde(rename = "Vec")]
    pub vec: Vec<f64>,
}

/// Agent statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent: HashMap<String, String>,
    pub runtime: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raft: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serf_lan: Option<HashMap<String, String>>,
}

/// Cluster member information - GET /v1/agent/members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMember {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Addr")]
    pub addr: String,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Tags")]
    pub tags: HashMap<String, String>,
    #[serde(rename = "Status")]
    pub status: i32,
    #[serde(rename = "ProtocolMin")]
    pub protocol_min: u8,
    #[serde(rename = "ProtocolMax")]
    pub protocol_max: u8,
    #[serde(rename = "ProtocolCur")]
    pub protocol_cur: u8,
    #[serde(rename = "DelegateMin")]
    pub delegate_min: u8,
    #[serde(rename = "DelegateMax")]
    pub delegate_max: u8,
    #[serde(rename = "DelegateCur")]
    pub delegate_cur: u8,
}

impl Default for AgentMember {
    fn default() -> Self {
        Self {
            name: "batata-node".to_string(),
            addr: "127.0.0.1".to_string(),
            port: 8301,
            tags: HashMap::new(),
            status: 1, // 1 = alive
            protocol_min: 1,
            protocol_max: 5,
            protocol_cur: 2,
            delegate_min: 2,
            delegate_max: 5,
            delegate_cur: 4,
        }
    }
}

/// Agent host info - GET /v1/agent/host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHostInfo {
    #[serde(rename = "Memory")]
    pub memory: HostMemory,
    #[serde(rename = "CPU")]
    pub cpu: Vec<HostCPU>,
    #[serde(rename = "Disk")]
    pub disk: HostDisk,
    #[serde(rename = "Host")]
    pub host: HostInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMemory {
    #[serde(rename = "Total")]
    pub total: u64,
    #[serde(rename = "Available")]
    pub available: u64,
    #[serde(rename = "Used")]
    pub used: u64,
    #[serde(rename = "UsedPercent")]
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCPU {
    #[serde(rename = "CPU")]
    pub cpu: i32,
    #[serde(rename = "VendorID")]
    pub vendor_id: String,
    #[serde(rename = "Family")]
    pub family: String,
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "PhysicalID")]
    pub physical_id: String,
    #[serde(rename = "CoreID")]
    pub core_id: String,
    #[serde(rename = "Cores")]
    pub cores: i32,
    #[serde(rename = "Mhz")]
    pub mhz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostDisk {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Total")]
    pub total: u64,
    #[serde(rename = "Free")]
    pub free: u64,
    #[serde(rename = "Used")]
    pub used: u64,
    #[serde(rename = "UsedPercent")]
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    #[serde(rename = "Hostname")]
    pub hostname: String,
    #[serde(rename = "OS")]
    pub os: String,
    #[serde(rename = "Platform")]
    pub platform: String,
    #[serde(rename = "PlatformVersion")]
    pub platform_version: String,
    #[serde(rename = "KernelVersion")]
    pub kernel_version: String,
}

/// Agent version - GET /v1/agent/version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVersion {
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Revision")]
    pub revision: String,
    #[serde(rename = "Prerelease")]
    pub prerelease: String,
    #[serde(rename = "HumanVersion")]
    pub human_version: String,
    #[serde(rename = "BuildDate")]
    pub build_date: String,
    #[serde(rename = "FIPS")]
    pub fips: String,
}

/// Agent maintenance mode request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentMaintenanceRequest {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

// ============================================================================
// Session Models
// ============================================================================

/// Session info - for Session API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "LockDelay")]
    pub lock_delay: u64,
    #[serde(rename = "Behavior")]
    pub behavior: String,
    #[serde(rename = "TTL")]
    pub ttl: String,
    #[serde(rename = "NodeChecks", skip_serializing_if = "Option::is_none")]
    pub node_checks: Option<Vec<String>>,
    #[serde(rename = "ServiceChecks", skip_serializing_if = "Option::is_none")]
    pub service_checks: Option<Vec<String>>,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// Session create request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionCreateRequest {
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "Node", default)]
    pub node: Option<String>,
    #[serde(rename = "LockDelay", default)]
    pub lock_delay: Option<String>,
    #[serde(rename = "Behavior", default)]
    pub behavior: Option<String>,
    #[serde(rename = "TTL", default)]
    pub ttl: Option<String>,
    #[serde(rename = "NodeChecks", default)]
    pub node_checks: Option<Vec<String>>,
    #[serde(rename = "ServiceChecks", default)]
    pub service_checks: Option<Vec<String>>,
}

/// Session create response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreateResponse {
    #[serde(rename = "ID")]
    pub id: String,
}

// ============================================================================
// Status Models
// ============================================================================

/// Query params for agent members
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AgentMembersParams {
    /// WAN members only
    #[serde(default, deserialize_with = "consul_bool::deserialize")]
    pub wan: Option<bool>,
    /// Segment filter
    pub segment: Option<String>,
}

// ============================================================================
// Event Models
// ============================================================================

/// User event - for Event API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEvent {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Payload")]
    pub payload: Option<String>,
    #[serde(rename = "NodeFilter")]
    pub node_filter: String,
    #[serde(rename = "ServiceFilter")]
    pub service_filter: String,
    #[serde(rename = "TagFilter")]
    pub tag_filter: String,
    #[serde(rename = "Version")]
    pub version: u64,
    #[serde(rename = "LTime")]
    pub ltime: u64,
}

/// Event fire request body
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventFireRequest {
    #[serde(default)]
    pub payload: Option<String>,
}

/// Query params for event fire
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventFireParams {
    /// Filter by node name (regex)
    pub node: Option<String>,
    /// Filter by service name (regex)
    pub service: Option<String>,
    /// Filter by service tag (regex)
    pub tag: Option<String>,
    /// Datacenter
    pub dc: Option<String>,
}

/// Query params for event list
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventListParams {
    /// Filter by event name
    pub name: Option<String>,
    /// Filter by node (regex)
    pub node: Option<String>,
    /// Filter by service (regex)
    pub service: Option<String>,
    /// Filter by tag (regex)
    pub tag: Option<String>,
}

// ============================================================================
// Prepared Query Models
// ============================================================================

/// Prepared query definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQuery {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Session", skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(rename = "Token", skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(rename = "Service")]
    pub service: PreparedQueryService,
    #[serde(rename = "DNS", skip_serializing_if = "Option::is_none")]
    pub dns: Option<PreparedQueryDNS>,
    #[serde(rename = "Template", skip_serializing_if = "Option::is_none")]
    pub template: Option<PreparedQueryTemplate>,
    #[serde(rename = "CreateIndex", skip_serializing_if = "Option::is_none")]
    pub create_index: Option<u64>,
    #[serde(rename = "ModifyIndex", skip_serializing_if = "Option::is_none")]
    pub modify_index: Option<u64>,
}

/// Prepared query service definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryService {
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "Failover", skip_serializing_if = "Option::is_none")]
    pub failover: Option<PreparedQueryFailover>,
    #[serde(rename = "OnlyPassing", default)]
    pub only_passing: bool,
    #[serde(rename = "Near", skip_serializing_if = "Option::is_none")]
    pub near: Option<String>,
    #[serde(rename = "Tags", skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(rename = "NodeMeta", skip_serializing_if = "Option::is_none")]
    pub node_meta: Option<HashMap<String, String>>,
    #[serde(rename = "ServiceMeta", skip_serializing_if = "Option::is_none")]
    pub service_meta: Option<HashMap<String, String>>,
}

/// Prepared query failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryFailover {
    #[serde(rename = "NearestN", skip_serializing_if = "Option::is_none")]
    pub nearest_n: Option<i32>,
    #[serde(rename = "Datacenters", skip_serializing_if = "Option::is_none")]
    pub datacenters: Option<Vec<String>>,
}

/// Prepared query DNS options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryDNS {
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

/// Prepared query template options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryTemplate {
    #[serde(rename = "Type")]
    pub template_type: String,
    #[serde(rename = "Regexp", skip_serializing_if = "Option::is_none")]
    pub regexp: Option<String>,
    #[serde(rename = "RemoveEmptyTags", default)]
    pub remove_empty_tags: bool,
}

/// Prepared query create request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryCreateRequest {
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "Session", default)]
    pub session: Option<String>,
    #[serde(rename = "Token", default)]
    pub token: Option<String>,
    #[serde(rename = "Service")]
    pub service: PreparedQueryService,
    #[serde(rename = "DNS", default)]
    pub dns: Option<PreparedQueryDNS>,
    #[serde(rename = "Template", default)]
    pub template: Option<PreparedQueryTemplate>,
}

/// Prepared query create response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryCreateResponse {
    #[serde(rename = "ID")]
    pub id: String,
}

/// Prepared query execute result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryExecuteResult {
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "Nodes")]
    pub nodes: Vec<ServiceHealth>,
    #[serde(rename = "DNS")]
    pub dns: PreparedQueryDNS,
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
    #[serde(rename = "Failovers")]
    pub failovers: i32,
}

/// Prepared query explain result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedQueryExplainResult {
    #[serde(rename = "Query")]
    pub query: PreparedQuery,
}

/// Query params for prepared query
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PreparedQueryParams {
    /// Datacenter
    pub dc: Option<String>,
    /// Token
    pub token: Option<String>,
    /// Near node for sorting
    pub near: Option<String>,
    /// Limit results
    pub limit: Option<i32>,
}

// ============================================================================
// ACL Auth Method Models
// ============================================================================

/// ACL Auth Method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMethod {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub method_type: String,
    #[serde(rename = "DisplayName", skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(rename = "Description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "MaxTokenTTL", skip_serializing_if = "Option::is_none")]
    pub max_token_ttl: Option<String>,
    #[serde(rename = "TokenLocality", skip_serializing_if = "Option::is_none")]
    pub token_locality: Option<String>,
    #[serde(rename = "Config", skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "CreateIndex", skip_serializing_if = "Option::is_none")]
    pub create_index: Option<u64>,
    #[serde(rename = "ModifyIndex", skip_serializing_if = "Option::is_none")]
    pub modify_index: Option<u64>,
    #[serde(rename = "Namespace", skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Auth Method create/update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMethodRequest {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub method_type: String,
    #[serde(rename = "DisplayName", default)]
    pub display_name: Option<String>,
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    #[serde(rename = "MaxTokenTTL", default)]
    pub max_token_ttl: Option<String>,
    #[serde(rename = "TokenLocality", default)]
    pub token_locality: Option<String>,
    #[serde(rename = "Config", default)]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
}

// ============================================================================
// Metrics API Models
// ============================================================================

/// Consul-style metrics response
/// GET /v1/agent/metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    /// Timestamp of metrics collection
    #[serde(rename = "Timestamp")]
    pub timestamp: String,
    /// Gauge metrics (point-in-time values)
    #[serde(rename = "Gauges")]
    pub gauges: Vec<GaugeMetric>,
    /// Counter metrics (cumulative values)
    #[serde(rename = "Counters")]
    pub counters: Vec<CounterMetric>,
    /// Sample metrics (histogram/distribution)
    #[serde(rename = "Samples")]
    pub samples: Vec<SampleMetric>,
    /// Point metrics (timestamped values)
    #[serde(rename = "Points", default)]
    pub points: Vec<PointMetric>,
}

/// Gauge metric - represents a point-in-time value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeMetric {
    /// Metric name
    #[serde(rename = "Name")]
    pub name: String,
    /// Current value
    #[serde(rename = "Value")]
    pub value: f64,
    /// Labels/tags for the metric
    #[serde(rename = "Labels")]
    pub labels: HashMap<String, String>,
}

/// Counter metric - represents a cumulative value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterMetric {
    /// Metric name
    #[serde(rename = "Name")]
    pub name: String,
    /// Cumulative count
    #[serde(rename = "Count")]
    pub count: i64,
    /// Total sum of values
    #[serde(rename = "Sum")]
    pub sum: f64,
    /// Labels/tags for the metric
    #[serde(rename = "Labels")]
    pub labels: HashMap<String, String>,
}

/// Sample metric - represents a distribution/histogram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleMetric {
    /// Metric name
    #[serde(rename = "Name")]
    pub name: String,
    /// Number of samples
    #[serde(rename = "Count")]
    pub count: i64,
    /// Mean value
    #[serde(rename = "Mean")]
    pub mean: f64,
    /// Minimum value
    #[serde(rename = "Min")]
    pub min: f64,
    /// Maximum value
    #[serde(rename = "Max")]
    pub max: f64,
    /// Standard deviation
    #[serde(rename = "Stddev")]
    pub stddev: f64,
    /// Labels/tags for the metric
    #[serde(rename = "Labels")]
    pub labels: HashMap<String, String>,
}

/// Point metric - timestamped value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMetric {
    /// Metric name
    #[serde(rename = "Name")]
    pub name: String,
    /// Metric points with timestamps
    #[serde(rename = "Points")]
    pub points: Vec<(i64, f64)>, // (timestamp, value)
}

impl GaugeMetric {
    /// Create a new gauge metric with empty labels
    pub fn new(name: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            value,
            labels: HashMap::new(),
        }
    }

    /// Add a label to the metric
    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }
}

impl CounterMetric {
    /// Create a new counter metric with empty labels
    pub fn new(name: &str, count: i64, sum: f64) -> Self {
        Self {
            name: name.to_string(),
            count,
            sum,
            labels: HashMap::new(),
        }
    }
}

impl SampleMetric {
    /// Create a new sample metric with empty labels
    pub fn new(name: &str, count: i64, mean: f64, min: f64, max: f64, stddev: f64) -> Self {
        Self {
            name: name.to_string(),
            count,
            mean,
            min,
            max,
            stddev,
            labels: HashMap::new(),
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
            kind: None,
            proxy: None,
            connect: None,
            tagged_addresses: None,
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
