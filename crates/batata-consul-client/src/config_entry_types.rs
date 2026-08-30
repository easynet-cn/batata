//! Strongly-typed config entry structs.
//!
//! Maps each Consul config entry Kind to its own Rust struct. Wire-compatible
//! with the corresponding Go SDK files under `api/config_entry_*.go`.
//!
//! All types use `#[serde(rename_all = "PascalCase")]` to match Consul's
//! JSON field casing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Enum of all supported config entry Kind values.
///
/// Matches Consul Go SDK constants (`api.ServiceDefaults`, `api.ProxyDefaults`,
/// etc.) so users can reference them without string literals.
pub mod kinds {
/// The config-entry kind for `service-defaults`.
    pub const SERVICE_DEFAULTS: &str = "service-defaults";
/// The config-entry kind for `proxy-defaults`.
    pub const PROXY_DEFAULTS: &str = "proxy-defaults";
/// The config-entry kind for `service-router`.
    pub const SERVICE_ROUTER: &str = "service-router";
/// The config-entry kind for `service-splitter`.
    pub const SERVICE_SPLITTER: &str = "service-splitter";
/// The config-entry kind for `service-resolver`.
    pub const SERVICE_RESOLVER: &str = "service-resolver";
/// The config-entry kind for `ingress-gateway`.
    pub const INGRESS_GATEWAY: &str = "ingress-gateway";
/// The config-entry kind for `terminating-gateway`.
    pub const TERMINATING_GATEWAY: &str = "terminating-gateway";
/// The config-entry kind for `mesh`.
    pub const MESH: &str = "mesh";
/// The config-entry kind for `exported-services`.
    pub const EXPORTED_SERVICES: &str = "exported-services";
/// The config-entry kind for `service-intentions`.
    pub const SERVICE_INTENTIONS: &str = "service-intentions";
/// The config-entry kind for `jwt-provider`.
    pub const JWT_PROVIDER: &str = "jwt-provider";
/// The config-entry kind for `sameness-group`.
    pub const SAMENESS_GROUP: &str = "sameness-group";
/// The config-entry kind for `inline-certificate`.
    pub const INLINE_CERTIFICATE: &str = "inline-certificate";
/// The config-entry kind for `file-system-certificate`.
    pub const FILE_SYSTEM_CERTIFICATE: &str = "file-system-certificate";
/// The config-entry kind for `api-gateway`.
    pub const API_GATEWAY: &str = "api-gateway";
/// The config-entry kind for `bound-api-gateway`.
    pub const BOUND_API_GATEWAY: &str = "bound-api-gateway";
/// The config-entry kind for `http-route`.
    pub const HTTP_ROUTE: &str = "http-route";
/// The config-entry kind for `tcp-route`.
    pub const TCP_ROUTE: &str = "tcp-route";
/// The config-entry kind for `control-plane-request-limit`.
    pub const RATE_LIMIT_IP: &str = "control-plane-request-limit";
}

/// MeshGateway config block.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshGatewayConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The mesh gateway mode.
    pub mode: String,
}

/// ExposeConfig used by service-defaults / proxy-defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExposeConfig {
    #[serde(default)]
    /// Whether to expose health check ports.
    pub checks: bool,
    #[serde(default)]
    /// Paths exposed through the proxy.
    pub paths: Vec<ExposePath>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `ExposePath`.
pub struct ExposePath {
    /// The port the listener is exposed on.
    pub listener_port: u16,
    /// The path to expose.
    pub path: String,
    /// The local port the path maps to.
    pub local_path_port: u16,
    /// The protocol used on the exposed path.
    pub protocol: String,
    #[serde(default)]
    /// Whether the path was parsed from a health check.
    pub parsed_from_check: bool,
}

// ---------------------------------------------------------------------------
// mesh (config_entry_mesh.go)
// ---------------------------------------------------------------------------

/// MeshConfigEntry — Kind "mesh". One per cluster.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshConfigEntry {
    #[serde(rename = "Kind", default = "default_mesh_kind")]
    /// The config entry kind (`mesh`).
    pub kind: String,
    /// The name of the mesh config entry.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the entry.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default)]
    /// The transparent proxy mesh configuration.
    pub transparent_proxy: TransparentProxyMeshConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Whether permissive mutual TLS can be enabled.
    pub allow_enabling_permissive_mutual_tls: Option<bool>,
    #[serde(default)]
    /// The mesh TLS configuration.
    pub tls: MeshTLSConfig,
    #[serde(default)]
    /// The mesh HTTP configuration.
    pub http: MeshHTTPConfig,
    #[serde(default)]
    /// The mesh peering configuration.
    pub peering: PeeringMeshConfig,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}

fn default_mesh_kind() -> String {
    kinds::MESH.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `TransparentProxyMeshConfig`.
pub struct TransparentProxyMeshConfig {
    #[serde(default)]
    /// Whether only mesh-registered destinations are proxied.
    pub mesh_destinations_only: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `MeshTLSConfig`.
pub struct MeshTLSConfig {
    #[serde(default)]
    /// TLS configuration for incoming traffic.
    pub incoming: Option<MeshDirectionalTLSConfig>,
    #[serde(default)]
    /// TLS configuration for outgoing traffic.
    pub outgoing: Option<MeshDirectionalTLSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `MeshDirectionalTLSConfig`.
pub struct MeshDirectionalTLSConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The minimum TLS version.
    pub tls_min_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The maximum TLS version.
    pub tls_max_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The allowed cipher suites.
    pub cipher_suites: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `MeshHTTPConfig`.
pub struct MeshHTTPConfig {
    #[serde(default)]
    /// Whether to sanitize the `X-Forwarded-Client-Cert` header.
    pub sanitize_x_forwarded_client_cert: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `PeeringMeshConfig`.
pub struct PeeringMeshConfig {
    #[serde(default)]
    /// Whether peering traffic flows through mesh gateways.
    pub peer_through_mesh_gateways: bool,
}

// ---------------------------------------------------------------------------
// exported-services (config_entry_exports.go)
// ---------------------------------------------------------------------------

/// ExportedServicesConfigEntry — Kind "exported-services".
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExportedServicesConfigEntry {
    #[serde(rename = "Kind", default = "default_exports_kind")]
    /// The config entry kind (`exported-services`).
    pub kind: String,
    /// Always equals the partition name in Enterprise; "default" in OSS.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the entry.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    /// The list of exported services.
    pub services: Vec<ExportedService>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}

fn default_exports_kind() -> String {
    kinds::EXPORTED_SERVICES.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `ExportedService`.
pub struct ExportedService {
    /// The name of the exported service.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the exported service.
    pub namespace: Option<String>,
    /// The consumers allowed to access the service.
    pub consumers: Vec<ServiceConsumer>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `ServiceConsumer`.
pub struct ServiceConsumer {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The consuming admin partition.
    pub partition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The consuming peer cluster.
    pub peer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The consuming sameness group.
    pub sameness_group: String,
}

// ---------------------------------------------------------------------------
// ingress-gateway / terminating-gateway (config_entry_gateways.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `IngressGatewayConfigEntry`.
pub struct IngressGatewayConfigEntry {
    #[serde(rename = "Kind", default = "default_ingress_kind")]
    /// The config entry kind (`ingress-gateway`).
    pub kind: String,
    /// The name of the ingress gateway.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the gateway.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the gateway.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    /// The TLS configuration of the gateway.
    pub tls: GatewayTLSConfig,
    /// The listeners exposed by the gateway.
    pub listeners: Vec<IngressListener>,
    #[serde(default)]
    /// Default settings applied to all services.
    pub defaults: Option<IngressServiceConfig>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_ingress_kind() -> String {
    kinds::INGRESS_GATEWAY.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `GatewayTLSConfig`.
pub struct GatewayTLSConfig {
    #[serde(default)]
    /// Whether TLS is enabled.
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The SDS (secret discovery service) configuration.
    pub sds: Option<GatewayTLSSDSConfig>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The minimum TLS version.
    pub tls_min_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The maximum TLS version.
    pub tls_max_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The allowed cipher suites.
    pub cipher_suites: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `GatewayTLSSDSConfig`.
pub struct GatewayTLSSDSConfig {
    /// The SDS cluster name.
    pub cluster_name: String,
    /// The certificate resource name.
    pub cert_resource: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `IngressListener`.
pub struct IngressListener {
    /// The listener port.
    pub port: u16,
    /// The listener protocol (`http`, `https` or `tcp`).
    pub protocol: String,
    /// The services served on this listener.
    pub services: Vec<IngressService>,
    #[serde(default)]
    /// The TLS configuration for this listener.
    pub tls: Option<GatewayTLSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `IngressService`.
pub struct IngressService {
    /// The name of the backend service.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the backend service.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The hostnames this service is routed to.
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Headers added to requests.
    pub request_headers: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Headers added to responses.
    pub response_headers: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The per-service TLS configuration.
    pub tls: Option<GatewayServiceTLSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `GatewayServiceTLSConfig`.
pub struct GatewayServiceTLSConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The SDS configuration for the service.
    pub sds: Option<GatewayTLSSDSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `IngressServiceConfig`.
pub struct IngressServiceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The maximum number of connections per backend.
    pub max_connections: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The maximum number of pending requests per backend.
    pub max_pending_requests: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The maximum number of concurrent requests per backend.
    pub max_concurrent_requests: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The passive health check configuration.
    pub passive_health_check: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `TerminatingGatewayConfigEntry`.
pub struct TerminatingGatewayConfigEntry {
    #[serde(rename = "Kind", default = "default_terminating_kind")]
    /// The config entry kind (`terminating-gateway`).
    pub kind: String,
    /// The name of the terminating gateway.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the gateway.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the gateway.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    /// The services linked through the gateway.
    pub services: Vec<LinkedService>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_terminating_kind() -> String {
    kinds::TERMINATING_GATEWAY.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `LinkedService`.
pub struct LinkedService {
    /// The name of the linked service.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The namespace of the linked service.
    pub namespace: String,
    #[serde(rename = "CAFile", default, skip_serializing_if = "String::is_empty")]
    /// The CA certificate file for TLS.
    pub ca_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The client certificate file for TLS.
    pub cert_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The client key file for TLS.
    pub key_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The SNI name used for TLS.
    pub sni: String,
}

// ---------------------------------------------------------------------------
// sameness-group (config_entry_sameness_group.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `SamenessGroupConfigEntry`.
pub struct SamenessGroupConfigEntry {
    #[serde(rename = "Kind", default = "default_sg_kind")]
    /// The config entry kind (`sameness-group`).
    pub kind: String,
    /// The name of the sameness group.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the entry.
    pub partition: Option<String>,
    #[serde(default)]
    /// Whether this group is the default for failover.
    pub default_for_failover: bool,
    #[serde(default)]
    /// Whether the local cluster is included in the group.
    pub include_local: bool,
    /// The members of the sameness group.
    pub members: Vec<SamenessGroupMember>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_sg_kind() -> String {
    kinds::SAMENESS_GROUP.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `SamenessGroupMember`.
pub struct SamenessGroupMember {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The member admin partition.
    pub partition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The member peer cluster.
    pub peer: String,
}

// ---------------------------------------------------------------------------
// jwt-provider (config_entry_jwt_provider.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `JWTProviderConfigEntry`.
pub struct JWTProviderConfigEntry {
    #[serde(rename = "Kind", default = "default_jwt_kind")]
    /// The config entry kind (`jwt-provider`).
    pub kind: String,
    /// The name of the JWT provider.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The issuer of the JWTs.
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The expected audiences.
    pub audiences: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The JSON web key set used for verification.
    pub json_web_key_set: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where tokens are located in the request.
    pub locations: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// JWT forwarding configuration.
    pub forwarding: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Allowed clock skew in seconds.
    pub clock_skew_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The verification cache configuration.
    pub cache_config: Option<serde_json::Value>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_jwt_kind() -> String {
    kinds::JWT_PROVIDER.to_string()
}

// ---------------------------------------------------------------------------
// service-intentions (config_entry_intentions.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `ServiceIntentionsConfigEntry`.
pub struct ServiceIntentionsConfigEntry {
    #[serde(rename = "Kind", default = "default_intentions_kind")]
    /// The config entry kind (`service-intentions`).
    pub kind: String,
    /// The name of the destination service.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the destination service.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the destination service.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The source services allowed to connect.
    pub sources: Vec<IntentionSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The JWT configuration for this intention.
    pub jwt: Option<serde_json::Value>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_intentions_kind() -> String {
    kinds::SERVICE_INTENTIONS.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `IntentionSource`.
pub struct IntentionSource {
    /// The name of the source service.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The namespace of the source service.
    pub namespace: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The admin partition of the source service.
    pub partition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The peer cluster of the source service.
    pub peer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The action allowed: `allow` or `deny`.
    pub action: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The sameness group of the source service.
    pub sameness_group: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The precedence type of the intention.
    pub precedence_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// L7 permissions for this intention.
    pub permissions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    /// The numeric precedence of the intention.
    pub precedence: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The legacy ID of the intention.
    pub legacy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A human-readable description of the intention.
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// service-defaults and proxy-defaults (config_entry.go core)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `ServiceDefaultsConfigEntry`.
pub struct ServiceDefaultsConfigEntry {
    #[serde(rename = "Kind", default = "default_svc_defaults_kind")]
    /// The config entry kind (`service-defaults`).
    pub kind: String,
    /// The name of the service.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the service.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the service.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The protocol used by the service.
    pub protocol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The proxy mode of the service.
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The transparent proxy configuration.
    pub transparent_proxy: Option<serde_json::Value>,
    #[serde(default)]
    /// The mesh gateway configuration.
    pub mesh_gateway: MeshGatewayConfig,
    #[serde(default)]
    /// The expose configuration.
    pub expose: ExposeConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The external SNI name for TLS.
    pub external_sni: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Upstream configuration overrides.
    pub upstream_config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The destination override.
    pub destination: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Maximum inbound connections.
    pub max_inbound_connections: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Local connect timeout in milliseconds.
    pub local_connect_timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Local request timeout in milliseconds.
    pub local_request_timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Inbound connection balancing policy.
    pub balance_inbound_connections: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Envoy extensions applied to the proxy.
    pub envoy_extensions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_svc_defaults_kind() -> String {
    kinds::SERVICE_DEFAULTS.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
/// Represents a `ProxyDefaultsConfigEntry`.
pub struct ProxyDefaultsConfigEntry {
    #[serde(rename = "Kind", default = "default_proxy_defaults_kind")]
    /// The config entry kind (`proxy-defaults`).
    pub kind: String,
    /// Must be "global".
    #[serde(default = "default_proxy_defaults_name")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the entry.
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the entry.
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary metadata attached to the entry.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The proxy configuration.
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The proxy mode.
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The transparent proxy configuration.
    pub transparent_proxy: Option<serde_json::Value>,
    #[serde(default)]
    /// The mesh gateway configuration.
    pub mesh_gateway: MeshGatewayConfig,
    #[serde(default)]
    /// The expose configuration.
    pub expose: ExposeConfig,
    #[serde(default)]
    /// The creation index of the entry.
    pub create_index: u64,
    #[serde(default)]
    /// The last modification index of the entry.
    pub modify_index: u64,
}
fn default_proxy_defaults_kind() -> String {
    kinds::PROXY_DEFAULTS.to_string()
}
fn default_proxy_defaults_name() -> String {
    "global".to_string()
}
