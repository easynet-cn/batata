//! Istio MCP Types
//!
//! This module defines types for Istio MCP (Mesh Configuration Protocol),
//! which is used to push configuration to Istio control plane.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// MCP resource type URLs for Istio
pub mod type_urls {
    /// ServiceEntry type URL
    pub const SERVICE_ENTRY: &str = "networking.istio.io/v1alpha3/ServiceEntry";
    /// VirtualService type URL
    pub const VIRTUAL_SERVICE: &str = "networking.istio.io/v1alpha3/VirtualService";
    /// DestinationRule type URL
    pub const DESTINATION_RULE: &str = "networking.istio.io/v1alpha3/DestinationRule";
    /// Gateway type URL
    pub const GATEWAY: &str = "networking.istio.io/v1alpha3/Gateway";
}

/// Istio resource type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IstioResourceType {
    /// ServiceEntry - external service definition
    ServiceEntry,
    /// VirtualService - traffic routing rules
    VirtualService,
    /// DestinationRule - traffic policy rules
    DestinationRule,
    /// Gateway - ingress/egress gateway
    Gateway,
}

impl IstioResourceType {
    /// Get the type URL for this resource type
    pub fn type_url(&self) -> &'static str {
        match self {
            IstioResourceType::ServiceEntry => type_urls::SERVICE_ENTRY,
            IstioResourceType::VirtualService => type_urls::VIRTUAL_SERVICE,
            IstioResourceType::DestinationRule => type_urls::DESTINATION_RULE,
            IstioResourceType::Gateway => type_urls::GATEWAY,
        }
    }

    /// Parse resource type from type URL
    pub fn from_type_url(url: &str) -> Option<Self> {
        match url {
            type_urls::SERVICE_ENTRY => Some(IstioResourceType::ServiceEntry),
            type_urls::VIRTUAL_SERVICE => Some(IstioResourceType::VirtualService),
            type_urls::DESTINATION_RULE => Some(IstioResourceType::DestinationRule),
            type_urls::GATEWAY => Some(IstioResourceType::Gateway),
            _ => None,
        }
    }
}

/// Istio ServiceEntry - represents an external service
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Resource metadata
    pub metadata: ResourceMetadata,
    /// ServiceEntry spec
    pub spec: ServiceEntrySpec,
}

impl ServiceEntry {
    /// Create a new ServiceEntry for an external HTTP service
    pub fn new_http(name: &str, hosts: Vec<String>, port: u32) -> Self {
        Self {
            metadata: ResourceMetadata {
                name: name.to_string(),
                namespace: "default".to_string(),
                ..Default::default()
            },
            spec: ServiceEntrySpec {
                hosts,
                ports: vec![Port {
                    number: port,
                    name: "http".to_string(),
                    protocol: Protocol::Http,
                    target_port: None,
                }],
                resolution: Resolution::Dns,
                location: Location::MeshExternal,
                ..Default::default()
            },
        }
    }

    /// Create a ServiceEntry with static endpoints
    pub fn with_endpoints(mut self, endpoints: Vec<WorkloadEntry>) -> Self {
        self.spec.endpoints = endpoints;
        self.spec.resolution = Resolution::Static;
        self
    }
}

/// Resource metadata (Kubernetes-style)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceMetadata {
    /// Resource name
    pub name: String,
    /// Namespace
    pub namespace: String,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Annotations
    pub annotations: HashMap<String, String>,
    /// Resource version
    pub resource_version: String,
}

/// ServiceEntry specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEntrySpec {
    /// Hosts associated with the service
    pub hosts: Vec<String>,
    /// Addresses (virtual IPs) for the service
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<String>,
    /// Ports exposed by the service
    pub ports: Vec<Port>,
    /// Service discovery mode
    pub resolution: Resolution,
    /// Service location (internal or external to mesh)
    pub location: Location,
    /// Endpoints for the service (when resolution is STATIC)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<WorkloadEntry>,
    /// Export configuration
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub export_to: Vec<String>,
    /// Subject alternate names for TLS
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subject_alt_names: Vec<String>,
}

/// Service port definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    /// Port number
    pub number: u32,
    /// Port name
    pub name: String,
    /// Protocol (HTTP, HTTPS, GRPC, TCP, etc.)
    pub protocol: Protocol,
    /// Target port (if different from number)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_port: Option<u32>,
}

/// Protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Protocol {
    /// HTTP protocol
    #[default]
    Http,
    /// HTTPS protocol
    Https,
    /// gRPC protocol
    Grpc,
    /// gRPC-Web protocol
    GrpcWeb,
    /// HTTP/2 protocol
    Http2,
    /// MongoDB protocol
    Mongo,
    /// MySQL protocol
    Mysql,
    /// Redis protocol
    Redis,
    /// TCP protocol
    Tcp,
    /// TLS protocol
    Tls,
    /// UDP protocol
    Udp,
}

/// Service discovery resolution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Resolution {
    /// No resolution (passthrough)
    None,
    /// Use STATIC endpoints
    Static,
    /// Use DNS resolution
    #[default]
    Dns,
    /// Use DNS round-robin resolution
    DnsRoundRobin,
}

/// Service location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Location {
    /// Service is inside the mesh
    MeshInternal,
    /// Service is outside the mesh
    #[default]
    MeshExternal,
}

/// Workload entry (endpoint) for ServiceEntry
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadEntry {
    /// IP address of the endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Port mappings
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub ports: HashMap<String, u32>,
    /// Labels for the endpoint
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub labels: HashMap<String, String>,
    /// Network this endpoint belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    /// Locality (region/zone/subzone)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locality: Option<String>,
    /// Weight (for load balancing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
    /// Service account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account: Option<String>,
}

/// Istio VirtualService - traffic routing configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VirtualService {
    /// Resource metadata
    pub metadata: ResourceMetadata,
    /// VirtualService spec
    pub spec: VirtualServiceSpec,
}

/// VirtualService specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualServiceSpec {
    /// Hosts this virtual service applies to
    pub hosts: Vec<String>,
    /// Gateways this virtual service is attached to
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gateways: Vec<String>,
    /// HTTP routes
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub http: Vec<HttpRoute>,
    /// TCP routes
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tcp: Vec<TcpRoute>,
    /// TLS routes
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tls: Vec<TlsRoute>,
    /// Export configuration
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub export_to: Vec<String>,
}

/// HTTP route definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRoute {
    /// Route name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Match conditions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub r#match: Vec<HttpMatchRequest>,
    /// Route destinations
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub route: Vec<HttpRouteDestination>,
    /// Redirect configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect: Option<HttpRedirect>,
    /// Timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    /// Retries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<HttpRetry>,
    /// Fault injection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fault: Option<HttpFaultInjection>,
}

/// HTTP match request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpMatchRequest {
    /// URI match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<StringMatch>,
    /// Scheme match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<StringMatch>,
    /// Method match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<StringMatch>,
    /// Authority match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<StringMatch>,
    /// Header matches
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, StringMatch>,
    /// Source labels
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub source_labels: HashMap<String, String>,
    /// Source namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_namespace: Option<String>,
    /// Ignore URI case
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_uri_case: Option<bool>,
}

/// String match type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StringMatch {
    /// Exact match
    Exact(String),
    /// Prefix match
    Prefix(String),
    /// Regex match
    Regex(String),
}

impl Default for StringMatch {
    fn default() -> Self {
        StringMatch::Prefix("/".to_string())
    }
}

/// HTTP route destination
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRouteDestination {
    /// Destination
    pub destination: Destination,
    /// Weight (for traffic splitting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
    /// Headers to add/modify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
}

/// Destination specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    /// Service host
    pub host: String,
    /// Service subset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subset: Option<String>,
    /// Service port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<PortSelector>,
}

/// Port selector
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortSelector {
    /// Port number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
}

/// Headers modification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Headers {
    /// Request headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<HeaderOperations>,
    /// Response headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<HeaderOperations>,
}

/// Header operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderOperations {
    /// Headers to set
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub set: HashMap<String, String>,
    /// Headers to add
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub add: HashMap<String, String>,
    /// Headers to remove
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

/// HTTP redirect
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRedirect {
    /// URI to redirect to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Authority to redirect to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// Redirect code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_code: Option<u32>,
}

/// HTTP retry policy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRetry {
    /// Number of retries
    pub attempts: i32,
    /// Per-try timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_try_timeout: Option<String>,
    /// Retry on conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_on: Option<String>,
}

/// HTTP fault injection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpFaultInjection {
    /// Delay fault
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<Delay>,
    /// Abort fault
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abort: Option<Abort>,
}

/// Delay fault injection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delay {
    /// Fixed delay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_delay: Option<String>,
    /// Percentage of requests to delay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<Percent>,
}

/// Abort fault injection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Abort {
    /// HTTP status code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<i32>,
    /// Percentage of requests to abort
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<Percent>,
}

/// Percentage value
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Percent {
    /// Value (0-100)
    pub value: f64,
}

/// TCP route
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpRoute {
    /// Match conditions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub r#match: Vec<L4MatchAttributes>,
    /// Route destinations
    pub route: Vec<RouteDestination>,
}

/// L4 match attributes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L4MatchAttributes {
    /// Destination subnets
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub destination_subnets: Vec<String>,
    /// Port number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// Source subnet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_subnet: Option<String>,
    /// Source labels
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub source_labels: HashMap<String, String>,
}

/// Route destination (for TCP/TLS)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDestination {
    /// Destination
    pub destination: Destination,
    /// Weight
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<i32>,
}

/// TLS route
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsRoute {
    /// Match conditions
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub r#match: Vec<TlsMatchAttributes>,
    /// Route destinations
    pub route: Vec<RouteDestination>,
}

/// TLS match attributes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsMatchAttributes {
    /// SNI hosts
    pub sni_hosts: Vec<String>,
    /// Destination subnets
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub destination_subnets: Vec<String>,
    /// Port number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// Source labels
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub source_labels: HashMap<String, String>,
}

/// Istio DestinationRule - traffic policy configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DestinationRule {
    /// Resource metadata
    pub metadata: ResourceMetadata,
    /// DestinationRule spec
    pub spec: DestinationRuleSpec,
}

/// DestinationRule specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationRuleSpec {
    /// Target host
    pub host: String,
    /// Traffic policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traffic_policy: Option<TrafficPolicy>,
    /// Subsets
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subsets: Vec<Subset>,
    /// Export configuration
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub export_to: Vec<String>,
}

/// Traffic policy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficPolicy {
    /// Connection pool settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_pool: Option<ConnectionPoolSettings>,
    /// Load balancer settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_balancer: Option<LoadBalancerSettings>,
    /// Outlier detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlier_detection: Option<OutlierDetection>,
    /// TLS settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<ClientTlsSettings>,
}

/// Connection pool settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionPoolSettings {
    /// TCP settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp: Option<TcpSettings>,
    /// HTTP settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpSettings>,
}

/// TCP connection pool settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpSettings {
    /// Maximum connections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<i32>,
    /// Connection timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_timeout: Option<String>,
}

/// HTTP connection pool settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpSettings {
    /// HTTP/1.1 max pending requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h2_upgrade_policy: Option<String>,
    /// HTTP/2 max requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http1_max_pending_requests: Option<i32>,
    /// HTTP/2 max requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http2_max_requests: Option<i32>,
    /// Max requests per connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_requests_per_connection: Option<i32>,
    /// Max retries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<i32>,
}

/// Load balancer settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancerSettings {
    /// Simple load balancing mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simple: Option<SimpleLb>,
    /// Consistent hash settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consistent_hash: Option<ConsistentHashLb>,
}

/// Simple load balancing algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SimpleLb {
    /// Unspecified
    Unspecified,
    /// Round robin
    #[default]
    RoundRobin,
    /// Least connections
    LeastConn,
    /// Random
    Random,
    /// Passthrough
    Passthrough,
}

/// Consistent hash load balancing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistentHashLb {
    /// Hash based on HTTP header
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_header_name: Option<String>,
    /// Hash based on cookie
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_cookie: Option<HttpCookie>,
    /// Use source IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_source_ip: Option<bool>,
    /// Minimum ring size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_ring_size: Option<u64>,
}

/// HTTP cookie for consistent hashing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpCookie {
    /// Cookie name
    pub name: String,
    /// Cookie path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Cookie TTL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

/// Outlier detection settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlierDetection {
    /// Consecutive errors before ejection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consecutive_errors: Option<i32>,
    /// Consecutive gateway errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consecutive_gateway_errors: Option<i32>,
    /// Consecutive 5xx errors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consecutive_5xx_errors: Option<i32>,
    /// Interval between analysis sweeps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Base ejection time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_ejection_time: Option<String>,
    /// Max ejection percent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_ejection_percent: Option<i32>,
    /// Minimum healthy percent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_health_percent: Option<i32>,
}

/// Client TLS settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTlsSettings {
    /// TLS mode
    pub mode: TlsMode,
    /// Client certificate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_certificate: Option<String>,
    /// Private key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    /// CA certificates
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ca_certificates: Option<String>,
    /// SNI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    /// Subject alt names
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subject_alt_names: Vec<String>,
}

/// TLS mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TlsMode {
    /// Do not use TLS
    #[default]
    Disable,
    /// Use TLS (simple)
    Simple,
    /// Use mutual TLS
    Mutual,
    /// Istio mutual TLS
    IstioMutual,
}

/// Subset definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subset {
    /// Subset name
    pub name: String,
    /// Label selector
    pub labels: HashMap<String, String>,
    /// Traffic policy for this subset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traffic_policy: Option<TrafficPolicy>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_entry_creation() {
        let se =
            ServiceEntry::new_http("my-service", vec!["my-service.example.com".to_string()], 80);

        assert_eq!(se.metadata.name, "my-service");
        assert_eq!(se.spec.hosts, vec!["my-service.example.com"]);
        assert_eq!(se.spec.ports[0].number, 80);
        assert_eq!(se.spec.resolution, Resolution::Dns);
    }

    #[test]
    fn test_service_entry_with_endpoints() {
        let endpoint = WorkloadEntry {
            address: Some("192.168.1.1".to_string()),
            ports: {
                let mut m = HashMap::new();
                m.insert("http".to_string(), 8080);
                m
            },
            ..Default::default()
        };

        let se =
            ServiceEntry::new_http("my-service", vec!["my-service.example.com".to_string()], 80)
                .with_endpoints(vec![endpoint]);

        assert_eq!(se.spec.resolution, Resolution::Static);
        assert_eq!(se.spec.endpoints.len(), 1);
    }

    #[test]
    fn test_istio_resource_type() {
        assert_eq!(
            IstioResourceType::from_type_url(type_urls::SERVICE_ENTRY),
            Some(IstioResourceType::ServiceEntry)
        );
        assert_eq!(
            IstioResourceType::ServiceEntry.type_url(),
            type_urls::SERVICE_ENTRY
        );
    }

    #[test]
    fn test_service_entry_serialization() {
        let se = ServiceEntry::new_http("test", vec!["test.example.com".to_string()], 80);
        let json = serde_json::to_string(&se).unwrap();
        assert!(json.contains("test.example.com"));
    }
}
