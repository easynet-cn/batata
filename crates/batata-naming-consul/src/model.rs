//! Consul naming domain models
//!
//! These models represent Consul's native service discovery semantics.
//! NO Nacos concepts (namespace, group, cluster, ephemeral, protection threshold).
//! All fields are native Consul types — no JSON-stuffed metadata workarounds.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Service Registration (Agent model)
// ============================================================================

/// A Consul agent service registration
///
/// This is the payload for `PUT /v1/agent/service/register`.
/// Unlike Nacos, Consul services are always server-managed (no heartbeat-based lifecycle).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentServiceRegistration {
    /// Service ID (unique per agent, defaults to service name)
    #[serde(rename = "ID", default)]
    pub id: Option<String>,
    /// Service name (used for DNS and catalog lookup)
    #[serde(rename = "Name")]
    pub name: String,
    /// Tags for classification and filtering
    #[serde(rename = "Tags", default)]
    pub tags: Vec<String>,
    /// Service port
    #[serde(rename = "Port", default)]
    pub port: u16,
    /// Service address (overrides agent address)
    #[serde(rename = "Address", default)]
    pub address: String,
    /// Service metadata key-value pairs
    #[serde(rename = "Meta", default)]
    pub meta: HashMap<String, String>,
    /// Whether tag override is enabled for anti-entropy
    #[serde(rename = "EnableTagOverride", default)]
    pub enable_tag_override: bool,
    /// Service weights for DNS load balancing
    #[serde(rename = "Weights")]
    pub weights: Option<Weights>,
    /// Service kind (e.g., connect-proxy, mesh-gateway)
    #[serde(rename = "Kind", default)]
    pub kind: ServiceKind,
    /// Proxy configuration (for connect-proxy kind)
    #[serde(rename = "Proxy", skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Connect configuration (for service mesh)
    #[serde(rename = "Connect", skip_serializing_if = "Option::is_none")]
    pub connect: Option<ConnectConfig>,
    /// Tagged addresses (lan, wan)
    #[serde(rename = "TaggedAddresses", default)]
    pub tagged_addresses: HashMap<String, ServiceAddress>,
    /// Single health check
    #[serde(rename = "Check", skip_serializing_if = "Option::is_none")]
    pub check: Option<CheckDefinition>,
    /// Multiple health checks
    #[serde(rename = "Checks", skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<CheckDefinition>>,
}

/// Service kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ServiceKind {
    /// Regular service
    #[default]
    #[serde(rename = "")]
    Typical,
    /// Connect proxy
    #[serde(rename = "connect-proxy")]
    ConnectProxy,
    /// Mesh gateway
    #[serde(rename = "mesh-gateway")]
    MeshGateway,
    /// Terminating gateway
    #[serde(rename = "terminating-gateway")]
    TerminatingGateway,
    /// Ingress gateway
    #[serde(rename = "ingress-gateway")]
    IngressGateway,
}

/// Service weights for DNS SRV records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weights {
    /// Weight when all health checks pass
    #[serde(rename = "Passing")]
    pub passing: i32,
    /// Weight when some health checks warn
    #[serde(rename = "Warning")]
    pub warning: i32,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            passing: 1,
            warning: 1,
        }
    }
}

/// Tagged service address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAddress {
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Port")]
    pub port: u16,
}

/// Proxy configuration for connect-proxy services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(rename = "DestinationServiceName", default)]
    pub destination_service_name: String,
    #[serde(rename = "DestinationServiceID", default)]
    pub destination_service_id: String,
    #[serde(rename = "LocalServiceAddress", default)]
    pub local_service_address: String,
    #[serde(rename = "LocalServicePort", default)]
    pub local_service_port: u16,
    #[serde(rename = "Upstreams", default)]
    pub upstreams: Vec<Upstream>,
}

/// Upstream service definition for proxies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    #[serde(rename = "DestinationName")]
    pub destination_name: String,
    #[serde(rename = "LocalBindPort")]
    pub local_bind_port: u16,
    #[serde(rename = "Datacenter", default)]
    pub datacenter: String,
}

/// Connect configuration for service mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectConfig {
    #[serde(rename = "Native", default)]
    pub native: bool,
    #[serde(rename = "SidecarService", skip_serializing_if = "Option::is_none")]
    pub sidecar_service: Option<Box<AgentServiceRegistration>>,
}

// ============================================================================
// Agent Service (runtime state)
// ============================================================================

/// An agent service as returned by the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentService {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "Tags", default)]
    pub tags: Vec<String>,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Meta", default)]
    pub meta: HashMap<String, String>,
    #[serde(rename = "Weights")]
    pub weights: Weights,
    #[serde(rename = "EnableTagOverride")]
    pub enable_tag_override: bool,
    #[serde(rename = "Kind")]
    pub kind: ServiceKind,
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
}

// ============================================================================
// Catalog (cluster-wide view)
// ============================================================================

/// A catalog service entry (returned by catalog endpoints)
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
    #[serde(rename = "ServiceTags", default)]
    pub service_tags: Vec<String>,
    #[serde(rename = "ServiceMeta", default)]
    pub service_meta: HashMap<String, String>,
    #[serde(rename = "ServiceWeights")]
    pub service_weights: Weights,
    #[serde(rename = "ServiceKind")]
    pub service_kind: ServiceKind,
    #[serde(rename = "ServiceEnableTagOverride")]
    pub service_enable_tag_override: bool,
}

/// Node information in the catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogNode {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
    #[serde(rename = "Meta", default)]
    pub meta: HashMap<String, String>,
}

// ============================================================================
// Health Check
// ============================================================================

/// A health check definition for registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckDefinition {
    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    /// TTL-based check interval (e.g., "15s")
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    /// HTTP endpoint to check
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    /// HTTP method (default: GET)
    #[serde(rename = "Method", skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// HTTP headers
    #[serde(rename = "Header", skip_serializing_if = "Option::is_none")]
    pub header: Option<HashMap<String, Vec<String>>>,
    /// TCP address to check
    #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
    pub tcp: Option<String>,
    /// gRPC endpoint
    #[serde(rename = "GRPC", skip_serializing_if = "Option::is_none")]
    pub grpc: Option<String>,
    /// Whether to use TLS for gRPC
    #[serde(rename = "GRPCUseTLS", default)]
    pub grpc_use_tls: bool,
    /// Check interval (e.g., "10s")
    #[serde(rename = "Interval", skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Check timeout
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    /// Deregister after critical for this duration (e.g., "30s")
    #[serde(
        rename = "DeregisterCriticalServiceAfter",
        skip_serializing_if = "Option::is_none"
    )]
    pub deregister_critical_service_after: Option<String>,
    /// Initial check status
    #[serde(rename = "Status", skip_serializing_if = "Option::is_none")]
    pub status: Option<CheckStatus>,
    /// Notes
    #[serde(rename = "Notes", default)]
    pub notes: String,
}

/// Health check runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    #[serde(rename = "CheckID")]
    pub check_id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Status")]
    pub status: CheckStatus,
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

/// Consul check status — native tri-state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CheckStatus {
    #[serde(rename = "passing")]
    Passing,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "critical")]
    #[default]
    Critical,
}

impl CheckStatus {
    pub fn is_passing(&self) -> bool {
        matches!(self, CheckStatus::Passing)
    }

    pub fn is_healthy(&self) -> bool {
        matches!(self, CheckStatus::Passing | CheckStatus::Warning)
    }
}

/// Health service entry (returned by health endpoints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    #[serde(rename = "Node")]
    pub node: CatalogNode,
    #[serde(rename = "Service")]
    pub service: AgentService,
    #[serde(rename = "Checks")]
    pub checks: Vec<HealthCheck>,
}

// ============================================================================
// Query Parameters
// ============================================================================

/// Consul blocking query parameters
#[derive(Debug, Clone, Default)]
pub struct BlockingQuery {
    /// Wait index for blocking queries (long-polling)
    pub index: Option<u64>,
    /// Max wait time (e.g., "5m", default: "5m")
    pub wait: Option<String>,
    /// Datacenter to query
    pub dc: Option<String>,
    /// Filter expression
    pub filter: Option<String>,
    /// Namespace (enterprise feature)
    pub ns: Option<String>,
}

/// Consul service query parameters
#[derive(Debug, Clone, Default)]
pub struct ConsulServiceQuery {
    /// Datacenter
    pub dc: Option<String>,
    /// Tag filter
    pub tag: Option<String>,
    /// Node metadata filter
    pub node_meta: Option<String>,
    /// Only return passing checks
    pub passing: bool,
    /// Filter expression
    pub filter: Option<String>,
    /// Blocking query parameters
    pub blocking: BlockingQuery,
}
