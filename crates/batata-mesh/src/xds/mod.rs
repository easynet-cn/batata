//! xDS Protocol Types and Definitions
//!
//! This module contains the xDS protocol types generated from proto files
//! and additional Rust types for working with xDS resources.

pub mod encode;
pub mod generated;
pub mod rbac;
pub mod types;

/// xDS type URLs for resource identification
pub mod type_urls {
    /// Endpoint Discovery Service type URL
    pub const ENDPOINT_TYPE: &str =
        "type.googleapis.com/envoy.config.endpoint.v3.ClusterLoadAssignment";
    /// Cluster Discovery Service type URL
    pub const CLUSTER_TYPE: &str = "type.googleapis.com/envoy.config.cluster.v3.Cluster";
    /// Listener Discovery Service type URL
    pub const LISTENER_TYPE: &str = "type.googleapis.com/envoy.config.listener.v3.Listener";
    /// Route Discovery Service type URL
    pub const ROUTE_TYPE: &str = "type.googleapis.com/envoy.config.route.v3.RouteConfiguration";
    /// Secret Discovery Service type URL
    pub const SECRET_TYPE: &str =
        "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.Secret";
    /// Runtime Discovery Service type URL
    pub const RUNTIME_TYPE: &str = "type.googleapis.com/envoy.service.runtime.v3.Runtime";
    /// HTTP connection manager network filter (`Listener.Filter.typed_config`).
    pub const HCM_TYPE: &str = "type.googleapis.com/envoy.extensions.filters.network.http_connection_manager.v3.HttpConnectionManager";
    /// TCP proxy network filter (`Listener.Filter.typed_config`).
    pub const TCP_PROXY_TYPE: &str =
        "type.googleapis.com/envoy.extensions.filters.network.tcp_proxy.v3.TcpProxy";
    /// Terminal HTTP router filter (`HttpConnectionManager.HttpFilter.typed_config`).
    pub const ROUTER_FILTER_TYPE: &str =
        "type.googleapis.com/envoy.extensions.filters.http.router.v3.Router";
    /// Network RBAC filter (`Listener.Filter.typed_config`) for L4 intentions.
    pub const NETWORK_RBAC_TYPE: &str =
        "type.googleapis.com/envoy.extensions.filters.network.rbac.v3.RBAC";
    /// HTTP RBAC filter (`HttpConnectionManager.HttpFilter.typed_config`) for L7.
    pub const HTTP_RBAC_TYPE: &str =
        "type.googleapis.com/envoy.extensions.filters.http.rbac.v3.RBAC";
    /// Envoy transport socket extension name used for TLS.
    pub const TLS_TRANSPORT_SOCKET_NAME: &str = "envoy.transport_sockets.tls";
    /// Downstream TLS context (`TransportSocket.typed_config`).
    pub const DOWNSTREAM_TLS_CONTEXT_TYPE: &str =
        "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.DownstreamTlsContext";
    /// Upstream TLS context (`TransportSocket.typed_config`).
    pub const UPSTREAM_TLS_CONTEXT_TYPE: &str =
        "type.googleapis.com/envoy.extensions.transport_sockets.tls.v3.UpstreamTlsContext";
}

/// Resource type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// Endpoint resources (EDS)
    Endpoint,
    /// Cluster resources (CDS)
    Cluster,
    /// Listener resources (LDS)
    Listener,
    /// Route resources (RDS)
    Route,
    /// Secret resources (SDS)
    Secret,
    /// Runtime resources
    Runtime,
}

impl ResourceType {
    /// Get the type URL for this resource type
    pub fn type_url(&self) -> &'static str {
        match self {
            ResourceType::Endpoint => type_urls::ENDPOINT_TYPE,
            ResourceType::Cluster => type_urls::CLUSTER_TYPE,
            ResourceType::Listener => type_urls::LISTENER_TYPE,
            ResourceType::Route => type_urls::ROUTE_TYPE,
            ResourceType::Secret => type_urls::SECRET_TYPE,
            ResourceType::Runtime => type_urls::RUNTIME_TYPE,
        }
    }

    /// Parse resource type from type URL
    pub fn from_type_url(url: &str) -> Option<Self> {
        match url {
            type_urls::ENDPOINT_TYPE => Some(ResourceType::Endpoint),
            type_urls::CLUSTER_TYPE => Some(ResourceType::Cluster),
            type_urls::LISTENER_TYPE => Some(ResourceType::Listener),
            type_urls::ROUTE_TYPE => Some(ResourceType::Route),
            type_urls::SECRET_TYPE => Some(ResourceType::Secret),
            type_urls::RUNTIME_TYPE => Some(ResourceType::Runtime),
            _ => None,
        }
    }

    /// Get the short name for this resource type
    pub fn short_name(&self) -> &'static str {
        match self {
            ResourceType::Endpoint => "EDS",
            ResourceType::Cluster => "CDS",
            ResourceType::Listener => "LDS",
            ResourceType::Route => "RDS",
            ResourceType::Secret => "SDS",
            ResourceType::Runtime => "RTDS",
        }
    }
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}
