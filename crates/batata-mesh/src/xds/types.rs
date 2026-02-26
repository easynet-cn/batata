//! xDS Resource Types
//!
//! Native Rust types for xDS resources, providing a cleaner API
//! than the raw protobuf types.

use std::collections::HashMap;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

/// Node identifier for xDS clients
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Node {
    /// Unique node identifier
    pub id: String,
    /// Cluster the node belongs to
    pub cluster: String,
    /// Metadata for the node
    pub metadata: HashMap<String, String>,
    /// Locality information
    pub locality: Option<Locality>,
    /// User agent name
    pub user_agent_name: Option<String>,
    /// User agent version
    pub user_agent_version: Option<String>,
}

/// Locality - identifies where a service is running
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Locality {
    /// Region (e.g., "us-west-1")
    pub region: String,
    /// Zone within region (e.g., "us-west-1a")
    pub zone: String,
    /// Sub-zone (e.g., "rack-1")
    pub sub_zone: String,
}

impl Locality {
    /// Create a new locality
    pub fn new(region: impl Into<String>, zone: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            zone: zone.into(),
            sub_zone: String::new(),
        }
    }

    /// Create with sub-zone
    pub fn with_sub_zone(mut self, sub_zone: impl Into<String>) -> Self {
        self.sub_zone = sub_zone.into();
        self
    }

    /// Check if locality matches another (for filtering)
    pub fn matches(&self, other: &Locality) -> bool {
        // Empty fields match anything
        (self.region.is_empty() || self.region == other.region)
            && (self.zone.is_empty() || self.zone == other.zone)
            && (self.sub_zone.is_empty() || self.sub_zone == other.sub_zone)
    }
}

/// Endpoint - represents a single service endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// Endpoint address
    pub address: SocketAddr,
    /// Health status
    pub health_status: HealthStatus,
    /// Load balancing weight (1-128)
    pub weight: u32,
    /// Metadata for the endpoint
    pub metadata: HashMap<String, String>,
    /// Priority level (0 = highest)
    pub priority: u32,
}

impl Endpoint {
    /// Create a new healthy endpoint
    pub fn new(address: SocketAddr) -> Self {
        Self {
            address,
            health_status: HealthStatus::Healthy,
            weight: 100,
            metadata: HashMap::new(),
            priority: 0,
        }
    }

    /// Set the weight
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight.clamp(1, 128);
        self
    }

    /// Set the health status
    pub fn with_health_status(mut self, status: HealthStatus) -> Self {
        self.health_status = status;
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
}

/// Health status for endpoints
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Health status unknown
    #[default]
    Unknown,
    /// Endpoint is healthy
    Healthy,
    /// Endpoint is unhealthy
    Unhealthy,
    /// Endpoint is draining (graceful shutdown)
    Draining,
    /// Health check timed out
    Timeout,
    /// Endpoint is degraded
    Degraded,
}

impl HealthStatus {
    /// Check if the status indicates the endpoint can receive traffic
    pub fn is_available(&self) -> bool {
        matches!(self, HealthStatus::Healthy | HealthStatus::Degraded)
    }
}

/// Cluster load assignment - endpoints for a cluster
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterLoadAssignment {
    /// Cluster name
    pub cluster_name: String,
    /// Endpoints grouped by locality
    pub endpoints: Vec<LocalityEndpoints>,
}

impl ClusterLoadAssignment {
    /// Create a new cluster load assignment
    pub fn new(cluster_name: impl Into<String>) -> Self {
        Self {
            cluster_name: cluster_name.into(),
            endpoints: Vec::new(),
        }
    }

    /// Add endpoints for a locality
    pub fn add_locality(&mut self, locality: Locality, endpoints: Vec<Endpoint>, weight: u32) {
        self.endpoints.push(LocalityEndpoints {
            locality,
            endpoints,
            weight,
            priority: 0,
        });
    }

    /// Get all endpoints (flattened)
    pub fn all_endpoints(&self) -> impl Iterator<Item = &Endpoint> {
        self.endpoints.iter().flat_map(|le| le.endpoints.iter())
    }

    /// Get healthy endpoint count
    pub fn healthy_count(&self) -> usize {
        self.all_endpoints()
            .filter(|e| e.health_status.is_available())
            .count()
    }

    /// Get total endpoint count
    pub fn total_count(&self) -> usize {
        self.all_endpoints().count()
    }
}

/// Endpoints for a specific locality
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalityEndpoints {
    /// Locality for these endpoints
    pub locality: Locality,
    /// Endpoints in this locality
    pub endpoints: Vec<Endpoint>,
    /// Load balancing weight for this locality
    pub weight: u32,
    /// Priority level (0 = highest)
    pub priority: u32,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Cluster name
    pub name: String,
    /// Discovery type
    pub discovery_type: DiscoveryType,
    /// Load balancing policy
    pub lb_policy: LbPolicy,
    /// Connect timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Health check configuration
    pub health_checks: Vec<HealthCheckConfig>,
    /// Circuit breaker configuration
    pub circuit_breakers: Option<CircuitBreakerConfig>,
    /// TLS configuration
    pub tls_context: Option<TlsContext>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Default for Cluster {
    fn default() -> Self {
        Self {
            name: String::new(),
            discovery_type: DiscoveryType::Eds,
            lb_policy: LbPolicy::RoundRobin,
            connect_timeout_ms: 5000,
            health_checks: Vec::new(),
            circuit_breakers: None,
            tls_context: None,
            metadata: HashMap::new(),
        }
    }
}

impl Cluster {
    /// Create a new cluster with EDS discovery
    pub fn new_eds(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            discovery_type: DiscoveryType::Eds,
            ..Default::default()
        }
    }

    /// Create a new cluster with static endpoints
    pub fn new_static(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            discovery_type: DiscoveryType::Static,
            ..Default::default()
        }
    }
}

/// Cluster discovery type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DiscoveryType {
    /// Static endpoints
    Static,
    /// Strict DNS resolution
    StrictDns,
    /// Logical DNS resolution
    LogicalDns,
    /// EDS (Endpoint Discovery Service)
    #[default]
    Eds,
    /// Original destination
    OriginalDst,
}

/// Load balancing policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LbPolicy {
    /// Round robin
    #[default]
    RoundRobin,
    /// Least connections
    LeastRequest,
    /// Ring hash (consistent hashing)
    RingHash,
    /// Random
    Random,
    /// Maglev consistent hashing
    Maglev,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check timeout in milliseconds
    pub timeout_ms: u64,
    /// Interval between health checks in milliseconds
    pub interval_ms: u64,
    /// Number of unhealthy checks before marking unhealthy
    pub unhealthy_threshold: u32,
    /// Number of healthy checks before marking healthy
    pub healthy_threshold: u32,
    /// Health check type
    pub check_type: HealthCheckType,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            interval_ms: 10000,
            unhealthy_threshold: 3,
            healthy_threshold: 2,
            check_type: HealthCheckType::Tcp,
        }
    }
}

/// Health check type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum HealthCheckType {
    /// TCP health check
    #[default]
    Tcp,
    /// HTTP health check
    Http {
        path: String,
        expected_statuses: Vec<u32>,
    },
    /// gRPC health check
    Grpc { service_name: Option<String> },
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Maximum connections
    pub max_connections: u32,
    /// Maximum pending requests
    pub max_pending_requests: u32,
    /// Maximum requests
    pub max_requests: u32,
    /// Maximum retries
    pub max_retries: u32,
}

/// TLS context for upstream connections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsContext {
    /// SNI for TLS
    pub sni: Option<String>,
    /// ALPN protocols
    pub alpn_protocols: Vec<String>,
    /// Whether to validate server certificate
    pub validate_server: bool,
}

// =============================================================================
// Listener Discovery Service (LDS) Types
// =============================================================================

/// Listener configuration (LDS)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Listener {
    /// Listener name
    pub name: String,
    /// Address to bind to
    pub address: ListenerAddress,
    /// Filter chains
    pub filter_chains: Vec<FilterChain>,
    /// Default filter chain (for unmatched connections)
    pub default_filter_chain: Option<FilterChain>,
    /// Whether to use original destination
    pub use_original_dst: bool,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Listener {
    /// Create a new listener
    pub fn new(name: impl Into<String>, address: ListenerAddress) -> Self {
        Self {
            name: name.into(),
            address,
            ..Default::default()
        }
    }

    /// Add a filter chain
    pub fn with_filter_chain(mut self, chain: FilterChain) -> Self {
        self.filter_chains.push(chain);
        self
    }
}

/// Listener address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerAddress {
    /// IP address (0.0.0.0 for all interfaces)
    pub address: String,
    /// Port
    pub port: u16,
    /// Protocol (TCP/UDP)
    pub protocol: ListenerProtocol,
}

impl Default for ListenerAddress {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 0,
            protocol: ListenerProtocol::Tcp,
        }
    }
}

impl ListenerAddress {
    /// Create a TCP listener address
    pub fn tcp(address: impl Into<String>, port: u16) -> Self {
        Self {
            address: address.into(),
            port,
            protocol: ListenerProtocol::Tcp,
        }
    }

    /// Create a UDP listener address
    pub fn udp(address: impl Into<String>, port: u16) -> Self {
        Self {
            address: address.into(),
            port,
            protocol: ListenerProtocol::Udp,
        }
    }
}

/// Listener protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ListenerProtocol {
    #[default]
    Tcp,
    Udp,
}

/// Filter chain for processing connections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterChain {
    /// Filter chain name
    pub name: String,
    /// Match criteria for this filter chain
    pub filter_chain_match: Option<FilterChainMatch>,
    /// Filters to apply
    pub filters: Vec<NetworkFilter>,
    /// Transport socket (TLS configuration)
    pub transport_socket: Option<TransportSocket>,
}

impl FilterChain {
    /// Create a new filter chain
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Add a filter
    pub fn with_filter(mut self, filter: NetworkFilter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Set filter chain match
    pub fn with_match(mut self, filter_match: FilterChainMatch) -> Self {
        self.filter_chain_match = Some(filter_match);
        self
    }
}

/// Filter chain match criteria
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterChainMatch {
    /// Destination port ranges
    pub destination_port: Option<u16>,
    /// Server names (SNI)
    pub server_names: Vec<String>,
    /// Transport protocol (e.g., "tls", "raw_buffer")
    pub transport_protocol: Option<String>,
    /// Application protocols (ALPN)
    pub application_protocols: Vec<String>,
    /// Source IP prefixes
    pub source_prefix_ranges: Vec<CidrRange>,
    /// Destination IP prefixes
    pub prefix_ranges: Vec<CidrRange>,
}

/// CIDR range for IP matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CidrRange {
    /// IP address prefix
    pub address_prefix: String,
    /// Prefix length
    pub prefix_len: u32,
}

impl CidrRange {
    /// Create a new CIDR range
    pub fn new(address_prefix: impl Into<String>, prefix_len: u32) -> Self {
        Self {
            address_prefix: address_prefix.into(),
            prefix_len,
        }
    }
}

/// Network filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkFilter {
    /// Filter name
    pub name: String,
    /// Filter type
    pub filter_type: NetworkFilterType,
}

impl NetworkFilter {
    /// Create an HTTP connection manager filter
    pub fn http_connection_manager(route_config_name: impl Into<String>) -> Self {
        Self {
            name: "envoy.filters.network.http_connection_manager".to_string(),
            filter_type: NetworkFilterType::HttpConnectionManager {
                route_config_name: route_config_name.into(),
                codec_type: HttpCodecType::Auto,
                stat_prefix: "ingress_http".to_string(),
            },
        }
    }

    /// Create a TCP proxy filter
    pub fn tcp_proxy(cluster: impl Into<String>) -> Self {
        Self {
            name: "envoy.filters.network.tcp_proxy".to_string(),
            filter_type: NetworkFilterType::TcpProxy {
                stat_prefix: "tcp_proxy".to_string(),
                cluster: cluster.into(),
            },
        }
    }
}

/// Network filter types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkFilterType {
    /// HTTP connection manager
    HttpConnectionManager {
        /// Route configuration name (for RDS)
        route_config_name: String,
        /// HTTP codec type
        codec_type: HttpCodecType,
        /// Stat prefix
        stat_prefix: String,
    },
    /// TCP proxy
    TcpProxy {
        /// Stat prefix
        stat_prefix: String,
        /// Cluster name
        cluster: String,
    },
    /// Generic filter configuration
    Generic {
        /// Filter configuration (JSON)
        config: String,
    },
}

/// HTTP codec type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HttpCodecType {
    #[default]
    Auto,
    Http1,
    Http2,
    Http3,
}

/// Transport socket configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportSocket {
    /// Transport socket name
    pub name: String,
    /// TLS context
    pub tls_context: Option<DownstreamTlsContext>,
}

/// Downstream TLS context for listeners
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownstreamTlsContext {
    /// Require client certificate
    pub require_client_certificate: bool,
    /// Common TLS context
    pub common_tls_context: CommonTlsContext,
}

/// Common TLS context (shared between upstream and downstream)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommonTlsContext {
    /// TLS minimum version
    pub tls_minimum_version: TlsVersion,
    /// TLS maximum version
    pub tls_maximum_version: TlsVersion,
    /// ALPN protocols
    pub alpn_protocols: Vec<String>,
    /// TLS certificates
    pub tls_certificates: Vec<TlsCertificate>,
}

/// TLS version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TlsVersion {
    /// TLS 1.0
    Tls10,
    /// TLS 1.1
    Tls11,
    /// TLS 1.2
    #[default]
    Tls12,
    /// TLS 1.3
    Tls13,
}

/// TLS certificate
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsCertificate {
    /// Certificate chain (PEM)
    pub certificate_chain: String,
    /// Private key (PEM)
    pub private_key: String,
}

// =============================================================================
// Route Discovery Service (RDS) Types
// =============================================================================

/// Route configuration (RDS)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteConfiguration {
    /// Route configuration name
    pub name: String,
    /// Virtual hosts
    pub virtual_hosts: Vec<VirtualHost>,
    /// Internal only routes
    pub internal_only_headers: Vec<String>,
}

impl RouteConfiguration {
    /// Create a new route configuration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Add a virtual host
    pub fn with_virtual_host(mut self, vhost: VirtualHost) -> Self {
        self.virtual_hosts.push(vhost);
        self
    }
}

/// Virtual host for routing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VirtualHost {
    /// Virtual host name
    pub name: String,
    /// Domains to match (e.g., ["*.example.com", "example.com"])
    pub domains: Vec<String>,
    /// Routes
    pub routes: Vec<Route>,
    /// Virtual host level rate limits
    pub rate_limits: Vec<RateLimitConfig>,
    /// Retry policy
    pub retry_policy: Option<RetryPolicy>,
    /// Request headers to add
    pub request_headers_to_add: Vec<HeaderValueOption>,
    /// Response headers to add
    pub response_headers_to_add: Vec<HeaderValueOption>,
}

impl VirtualHost {
    /// Create a new virtual host
    pub fn new(name: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            name: name.into(),
            domains,
            ..Default::default()
        }
    }

    /// Add a route
    pub fn with_route(mut self, route: Route) -> Self {
        self.routes.push(route);
        self
    }
}

/// Route definition
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Route {
    /// Route name
    pub name: String,
    /// Match criteria
    pub match_config: RouteMatch,
    /// Route action
    pub action: RouteAction,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Route {
    /// Create a new route
    pub fn new(name: impl Into<String>, match_config: RouteMatch, action: RouteAction) -> Self {
        Self {
            name: name.into(),
            match_config,
            action,
            ..Default::default()
        }
    }
}

/// Route match criteria
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteMatch {
    /// Path match type
    pub path_match: PathMatch,
    /// Header matchers
    pub headers: Vec<HeaderMatcher>,
    /// Query parameter matchers
    pub query_parameters: Vec<QueryParameterMatcher>,
    /// Fraction of traffic to match (for canary)
    pub runtime_fraction: Option<f64>,
}

impl RouteMatch {
    /// Create a prefix match
    pub fn prefix(prefix: impl Into<String>) -> Self {
        Self {
            path_match: PathMatch::Prefix(prefix.into()),
            ..Default::default()
        }
    }

    /// Create an exact path match
    pub fn path(path: impl Into<String>) -> Self {
        Self {
            path_match: PathMatch::Path(path.into()),
            ..Default::default()
        }
    }

    /// Create a regex path match
    pub fn regex(regex: impl Into<String>) -> Self {
        Self {
            path_match: PathMatch::Regex(regex.into()),
            ..Default::default()
        }
    }

    /// Add a header matcher
    pub fn with_header(mut self, matcher: HeaderMatcher) -> Self {
        self.headers.push(matcher);
        self
    }
}

/// Path match type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathMatch {
    /// Prefix match
    Prefix(String),
    /// Exact path match
    Path(String),
    /// Regex match
    Regex(String),
    /// Path match with path template
    PathTemplate(String),
}

impl Default for PathMatch {
    fn default() -> Self {
        PathMatch::Prefix("/".to_string())
    }
}

/// Header matcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderMatcher {
    /// Header name
    pub name: String,
    /// Match type
    pub match_type: HeaderMatchType,
    /// Invert match
    pub invert_match: bool,
}

impl HeaderMatcher {
    /// Create an exact match
    pub fn exact(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            match_type: HeaderMatchType::Exact(value.into()),
            invert_match: false,
        }
    }

    /// Create a presence match
    pub fn present(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            match_type: HeaderMatchType::Present,
            invert_match: false,
        }
    }
}

/// Header match type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeaderMatchType {
    /// Exact value match
    Exact(String),
    /// Prefix match
    Prefix(String),
    /// Suffix match
    Suffix(String),
    /// Regex match
    Regex(String),
    /// Header presence check
    Present,
    /// Range match (for numeric headers)
    Range { start: i64, end: i64 },
}

/// Query parameter matcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParameterMatcher {
    /// Parameter name
    pub name: String,
    /// Match type
    pub match_type: QueryParamMatchType,
}

/// Query parameter match type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryParamMatchType {
    /// Exact value match
    Exact(String),
    /// Regex match
    Regex(String),
    /// Presence check
    Present,
}

/// Route action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteAction {
    /// Route to a cluster
    Route(RouteDestination),
    /// Redirect
    Redirect(RedirectAction),
    /// Direct response
    DirectResponse(DirectResponseAction),
}

impl Default for RouteAction {
    fn default() -> Self {
        RouteAction::Route(RouteDestination::default())
    }
}

/// Route destination
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteDestination {
    /// Single cluster destination
    pub cluster: Option<String>,
    /// Weighted clusters
    pub weighted_clusters: Vec<WeightedCluster>,
    /// Timeout
    pub timeout_ms: Option<u64>,
    /// Retry policy
    pub retry_policy: Option<RetryPolicy>,
    /// Host rewrite
    pub host_rewrite: Option<String>,
    /// Prefix rewrite
    pub prefix_rewrite: Option<String>,
}

impl RouteDestination {
    /// Create a destination to a single cluster
    pub fn cluster(name: impl Into<String>) -> Self {
        Self {
            cluster: Some(name.into()),
            ..Default::default()
        }
    }

    /// Create a destination with weighted clusters
    pub fn weighted(clusters: Vec<WeightedCluster>) -> Self {
        Self {
            weighted_clusters: clusters,
            ..Default::default()
        }
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set prefix rewrite
    pub fn with_prefix_rewrite(mut self, prefix: impl Into<String>) -> Self {
        self.prefix_rewrite = Some(prefix.into());
        self
    }
}

/// Weighted cluster for traffic splitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedCluster {
    /// Cluster name
    pub name: String,
    /// Weight (0-100)
    pub weight: u32,
    /// Request headers to add
    pub request_headers_to_add: Vec<HeaderValueOption>,
}

impl WeightedCluster {
    /// Create a new weighted cluster
    pub fn new(name: impl Into<String>, weight: u32) -> Self {
        Self {
            name: name.into(),
            weight,
            request_headers_to_add: Vec::new(),
        }
    }
}

/// Redirect action
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RedirectAction {
    /// Scheme to redirect to (http/https)
    pub scheme_redirect: Option<String>,
    /// Host to redirect to
    pub host_redirect: Option<String>,
    /// Port to redirect to
    pub port_redirect: Option<u16>,
    /// Path to redirect to
    pub path_redirect: Option<String>,
    /// Prefix to rewrite
    pub prefix_rewrite: Option<String>,
    /// Response code (301, 302, 303, 307, 308)
    pub response_code: u16,
    /// Strip query string
    pub strip_query: bool,
}

impl RedirectAction {
    /// Create a redirect to HTTPS
    pub fn to_https() -> Self {
        Self {
            scheme_redirect: Some("https".to_string()),
            response_code: 301,
            ..Default::default()
        }
    }

    /// Create a redirect to a different host
    pub fn to_host(host: impl Into<String>) -> Self {
        Self {
            host_redirect: Some(host.into()),
            response_code: 301,
            ..Default::default()
        }
    }
}

/// Direct response action
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectResponseAction {
    /// HTTP status code
    pub status: u32,
    /// Response body
    pub body: Option<String>,
}

impl DirectResponseAction {
    /// Create a direct response
    pub fn new(status: u32, body: Option<String>) -> Self {
        Self { status, body }
    }
}

/// Retry policy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Retry on conditions
    pub retry_on: Vec<String>,
    /// Number of retries
    pub num_retries: u32,
    /// Per-try timeout in milliseconds
    pub per_try_timeout_ms: Option<u64>,
    /// Retry priority levels
    pub retry_priority: Option<String>,
}

impl RetryPolicy {
    /// Create a default retry policy
    pub fn default_policy() -> Self {
        Self {
            retry_on: vec!["5xx".to_string(), "connect-failure".to_string()],
            num_retries: 3,
            per_try_timeout_ms: Some(5000),
            retry_priority: None,
        }
    }
}

/// Rate limit configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Rate limit stage (0-10)
    pub stage: u32,
    /// Disable local rate limiting
    pub disable_key: Option<String>,
    /// Rate limit actions
    pub actions: Vec<RateLimitAction>,
}

/// Rate limit action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitAction {
    /// Source cluster
    SourceCluster,
    /// Destination cluster
    DestinationCluster,
    /// Request headers
    RequestHeaders {
        header_name: String,
        descriptor_key: String,
    },
    /// Remote address
    RemoteAddress,
    /// Generic key
    GenericKey { descriptor_value: String },
}

/// Header value option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderValueOption {
    /// Header name
    pub header: String,
    /// Header value
    pub value: String,
    /// Append to existing header
    pub append: bool,
}

impl HeaderValueOption {
    /// Create a new header value option
    pub fn new(header: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            value: value.into(),
            append: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_locality_matches() {
        let us_west = Locality::new("us-west-1", "us-west-1a");
        let us_west_any = Locality::new("us-west-1", "");
        let any = Locality::default();

        assert!(us_west_any.matches(&us_west));
        assert!(any.matches(&us_west));
        assert!(!us_west.matches(&us_west_any));
    }

    #[test]
    fn test_endpoint_creation() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let endpoint = Endpoint::new(addr)
            .with_weight(50)
            .with_health_status(HealthStatus::Healthy);

        assert_eq!(endpoint.weight, 50);
        assert!(endpoint.health_status.is_available());
    }

    #[test]
    fn test_cluster_load_assignment() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut cla = ClusterLoadAssignment::new("test-cluster");

        cla.add_locality(
            Locality::new("us-west-1", "us-west-1a"),
            vec![Endpoint::new(addr)],
            100,
        );

        assert_eq!(cla.total_count(), 1);
        assert_eq!(cla.healthy_count(), 1);
    }

    #[test]
    fn test_listener_creation() {
        let listener = Listener::new("test-listener", ListenerAddress::tcp("0.0.0.0", 8080))
            .with_filter_chain(
                FilterChain::new("http-chain")
                    .with_filter(NetworkFilter::http_connection_manager("my-routes")),
            );

        assert_eq!(listener.name, "test-listener");
        assert_eq!(listener.address.port, 8080);
        assert_eq!(listener.filter_chains.len(), 1);
    }

    #[test]
    fn test_route_configuration() {
        let route_config = RouteConfiguration::new("my-routes").with_virtual_host(
            VirtualHost::new("api-vhost", vec!["api.example.com".to_string()]).with_route(
                Route::new(
                    "api-route",
                    RouteMatch::prefix("/api/"),
                    RouteAction::Route(RouteDestination::cluster("api-cluster")),
                ),
            ),
        );

        assert_eq!(route_config.name, "my-routes");
        assert_eq!(route_config.virtual_hosts.len(), 1);
        assert_eq!(route_config.virtual_hosts[0].routes.len(), 1);
    }

    #[test]
    fn test_weighted_clusters() {
        let destination = RouteDestination::weighted(vec![
            WeightedCluster::new("cluster-v1", 80),
            WeightedCluster::new("cluster-v2", 20),
        ]);

        assert_eq!(destination.weighted_clusters.len(), 2);
        assert_eq!(destination.weighted_clusters[0].weight, 80);
        assert_eq!(destination.weighted_clusters[1].weight, 20);
    }

    #[test]
    fn test_redirect_action() {
        let redirect = RedirectAction::to_https();
        assert_eq!(redirect.scheme_redirect, Some("https".to_string()));
        assert_eq!(redirect.response_code, 301);
    }

    #[test]
    fn test_route_match() {
        let match_prefix = RouteMatch::prefix("/api/v1");
        assert!(matches!(match_prefix.path_match, PathMatch::Prefix(_)));

        let match_exact = RouteMatch::path("/health");
        assert!(matches!(match_exact.path_match, PathMatch::Path(_)));

        let match_with_header =
            RouteMatch::prefix("/").with_header(HeaderMatcher::exact("x-version", "v2"));
        assert_eq!(match_with_header.headers.len(), 1);
    }
}
