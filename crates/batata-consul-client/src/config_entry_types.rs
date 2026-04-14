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
    pub const SERVICE_DEFAULTS: &str = "service-defaults";
    pub const PROXY_DEFAULTS: &str = "proxy-defaults";
    pub const SERVICE_ROUTER: &str = "service-router";
    pub const SERVICE_SPLITTER: &str = "service-splitter";
    pub const SERVICE_RESOLVER: &str = "service-resolver";
    pub const INGRESS_GATEWAY: &str = "ingress-gateway";
    pub const TERMINATING_GATEWAY: &str = "terminating-gateway";
    pub const MESH: &str = "mesh";
    pub const EXPORTED_SERVICES: &str = "exported-services";
    pub const SERVICE_INTENTIONS: &str = "service-intentions";
    pub const JWT_PROVIDER: &str = "jwt-provider";
    pub const SAMENESS_GROUP: &str = "sameness-group";
    pub const INLINE_CERTIFICATE: &str = "inline-certificate";
    pub const FILE_SYSTEM_CERTIFICATE: &str = "file-system-certificate";
    pub const API_GATEWAY: &str = "api-gateway";
    pub const BOUND_API_GATEWAY: &str = "bound-api-gateway";
    pub const HTTP_ROUTE: &str = "http-route";
    pub const TCP_ROUTE: &str = "tcp-route";
    pub const RATE_LIMIT_IP: &str = "control-plane-request-limit";
}

/// MeshGateway config block.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshGatewayConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
}

/// ExposeConfig used by service-defaults / proxy-defaults.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExposeConfig {
    #[serde(default)]
    pub checks: bool,
    #[serde(default)]
    pub paths: Vec<ExposePath>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExposePath {
    pub listener_port: u16,
    pub path: String,
    pub local_path_port: u16,
    pub protocol: String,
    #[serde(default)]
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
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(default)]
    pub transparent_proxy: TransparentProxyMeshConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_enabling_permissive_mutual_tls: Option<bool>,
    #[serde(default)]
    pub tls: MeshTLSConfig,
    #[serde(default)]
    pub http: MeshHTTPConfig,
    #[serde(default)]
    pub peering: PeeringMeshConfig,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

fn default_mesh_kind() -> String {
    kinds::MESH.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TransparentProxyMeshConfig {
    #[serde(default)]
    pub mesh_destinations_only: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshTLSConfig {
    #[serde(default)]
    pub incoming: Option<MeshDirectionalTLSConfig>,
    #[serde(default)]
    pub outgoing: Option<MeshDirectionalTLSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshDirectionalTLSConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls_min_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls_max_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cipher_suites: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MeshHTTPConfig {
    #[serde(default)]
    pub sanitize_x_forwarded_client_cert: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringMeshConfig {
    #[serde(default)]
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
    pub kind: String,
    /// Always equals the partition name in Enterprise; "default" in OSS.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    pub services: Vec<ExportedService>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

fn default_exports_kind() -> String {
    kinds::EXPORTED_SERVICES.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExportedService {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub consumers: Vec<ServiceConsumer>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceConsumer {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub peer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sameness_group: String,
}

// ---------------------------------------------------------------------------
// ingress-gateway / terminating-gateway (config_entry_gateways.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IngressGatewayConfigEntry {
    #[serde(rename = "Kind", default = "default_ingress_kind")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    pub tls: GatewayTLSConfig,
    pub listeners: Vec<IngressListener>,
    #[serde(default)]
    pub defaults: Option<IngressServiceConfig>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}
fn default_ingress_kind() -> String {
    kinds::INGRESS_GATEWAY.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GatewayTLSConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sds: Option<GatewayTLSSDSConfig>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls_min_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls_max_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cipher_suites: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GatewayTLSSDSConfig {
    pub cluster_name: String,
    pub cert_resource: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IngressListener {
    pub port: u16,
    pub protocol: String,
    pub services: Vec<IngressService>,
    #[serde(default)]
    pub tls: Option<GatewayTLSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IngressService {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_headers: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<GatewayServiceTLSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GatewayServiceTLSConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sds: Option<GatewayTLSSDSConfig>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IngressServiceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pending_requests: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_requests: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passive_health_check: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TerminatingGatewayConfigEntry {
    #[serde(rename = "Kind", default = "default_terminating_kind")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    pub services: Vec<LinkedService>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}
fn default_terminating_kind() -> String {
    kinds::TERMINATING_GATEWAY.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LinkedService {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub namespace: String,
    #[serde(rename = "CAFile", default, skip_serializing_if = "String::is_empty")]
    pub ca_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cert_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sni: String,
}

// ---------------------------------------------------------------------------
// sameness-group (config_entry_sameness_group.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamenessGroupConfigEntry {
    #[serde(rename = "Kind", default = "default_sg_kind")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default)]
    pub default_for_failover: bool,
    #[serde(default)]
    pub include_local: bool,
    pub members: Vec<SamenessGroupMember>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}
fn default_sg_kind() -> String {
    kinds::SAMENESS_GROUP.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SamenessGroupMember {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub peer: String,
}

// ---------------------------------------------------------------------------
// jwt-provider (config_entry_jwt_provider.go)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JWTProviderConfigEntry {
    #[serde(rename = "Kind", default = "default_jwt_kind")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audiences: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_web_key_set: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarding: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_skew_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_config: Option<serde_json::Value>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
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
pub struct ServiceIntentionsConfigEntry {
    #[serde(rename = "Kind", default = "default_intentions_kind")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<IntentionSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jwt: Option<serde_json::Value>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}
fn default_intentions_kind() -> String {
    kinds::SERVICE_INTENTIONS.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionSource {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub namespace: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partition: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub peer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub action: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sameness_group: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub precedence_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub precedence: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub legacy_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// service-defaults and proxy-defaults (config_entry.go core)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceDefaultsConfigEntry {
    #[serde(rename = "Kind", default = "default_svc_defaults_kind")]
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_proxy: Option<serde_json::Value>,
    #[serde(default)]
    pub mesh_gateway: MeshGatewayConfig,
    #[serde(default)]
    pub expose: ExposeConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_sni: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_inbound_connections: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_connect_timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_request_timeout_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_inbound_connections: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envoy_extensions: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}
fn default_svc_defaults_kind() -> String {
    kinds::SERVICE_DEFAULTS.to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProxyDefaultsConfigEntry {
    #[serde(rename = "Kind", default = "default_proxy_defaults_kind")]
    pub kind: String,
    /// Must be "global".
    #[serde(default = "default_proxy_defaults_name")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_proxy: Option<serde_json::Value>,
    #[serde(default)]
    pub mesh_gateway: MeshGatewayConfig,
    #[serde(default)]
    pub expose: ExposeConfig,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}
fn default_proxy_defaults_kind() -> String {
    kinds::PROXY_DEFAULTS.to_string()
}
fn default_proxy_defaults_name() -> String {
    "global".to_string()
}
