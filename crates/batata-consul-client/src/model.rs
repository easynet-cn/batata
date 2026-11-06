//! Shared request and response data models.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// --- Query/Write Options & Meta ---

/// Options for read (GET) requests, supporting blocking queries
#[derive(Clone, Debug, Default)]
pub struct QueryOptions {
    /// Datacenter to query
    pub datacenter: String,
    /// ACL token override for this request
    pub token: String,
    /// Blocking query: wait index from previous response
    pub wait_index: u64,
    /// Blocking query: max wait time (e.g., "5m", "30s")
    pub wait_time: Option<std::time::Duration>,
    /// Filter expression
    pub filter: String,
    /// Namespace (Enterprise)
    pub namespace: String,
    /// Partition (Enterprise)
    pub partition: String,
    /// Consistency mode: "", "consistent", "stale"
    pub require_consistent: bool,
    /// Whether stale reads are allowed.
    pub allow_stale: bool,
    /// Near node for sorting (e.g., "_agent")
    pub near: String,
}

/// Metadata returned from read (GET) requests
#[derive(Clone, Debug, Default)]
pub struct QueryMeta {
    /// Index for blocking queries
    pub last_index: u64,
    /// Time in ms since last contact with leader
    pub last_contact: u64,
    /// Whether the cluster has a known leader
    pub known_leader: bool,
    /// Whether the response was served from cache
    pub cache_hit: bool,
    /// Age of cached response in seconds
    pub cache_age: u64,
}

/// Options for write (PUT/DELETE) requests
#[derive(Clone, Debug, Default)]
pub struct WriteOptions {
    /// Datacenter to write to.
    pub datacenter: String,
    /// ACL token override for this request.
    pub token: String,
    /// Namespace (Enterprise).
    pub namespace: String,
    /// Partition (Enterprise).
    pub partition: String,
}

/// Metadata returned from write requests
#[derive(Clone, Debug, Default)]
pub struct WriteMeta {
    /// Duration of the request
    pub request_time: std::time::Duration,
}

// --- KV ---

/// A KV pair from the Consul KV store
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct KVPair {
    /// The key of the KV pair.
    pub key: String,
    /// The index at which the pair was created.
    pub create_index: u64,
    /// The index at which the pair was last modified.
    pub modify_index: u64,
    /// The index of the lock held on this pair, if any.
    pub lock_index: u64,
    /// User-defined flags attached to the pair.
    pub flags: u64,
    /// Base64-encoded value
    pub value: Option<String>,
    /// The session ID holding the lock, if any.
    pub session: Option<String>,
    #[serde(default)]
    /// The namespace of the pair (Enterprise).
    pub namespace: Option<String>,
    #[serde(default)]
    /// The admin partition of the pair (Enterprise).
    pub partition: Option<String>,
}

impl KVPair {
    /// Get the decoded value bytes
    pub fn value_bytes(&self) -> Option<Vec<u8>> {
        use base64::Engine;
        self.value
            .as_ref()
            .and_then(|v| base64::engine::general_purpose::STANDARD.decode(v).ok())
    }

    /// Get the decoded value as UTF-8 string
    pub fn value_str(&self) -> Option<String> {
        self.value_bytes().and_then(|b| String::from_utf8(b).ok())
    }
}

// --- Agent ---

/// A service registered with the local agent
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AgentService {
    #[serde(rename = "ID")]
    /// The service ID; defaults to the service name.
    pub id: String,
    /// The service name.
    pub service: String,
    #[serde(default)]
    /// Tags attached to the service.
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    /// Arbitrary key/value metadata.
    pub meta: Option<HashMap<String, String>>,
    /// The port the service listens on.
    pub port: u16,
    /// The address of the service.
    pub address: String,
    #[serde(default)]
    /// The weight of the service.
    pub weights: Option<AgentWeights>,
    /// Whether tag updates from the catalog override local tags.
    pub enable_tag_override: bool,
    #[serde(default)]
    /// The hash of the service content.
    pub content_hash: Option<String>,
    #[serde(default)]
    /// The datacenter of the service.
    pub datacenter: Option<String>,
    #[serde(default)]
    /// The namespace of the service (Enterprise).
    pub namespace: Option<String>,
    #[serde(default)]
    /// The admin partition of the service (Enterprise).
    pub partition: Option<String>,
}

/// Represents a `None`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AgentWeights {
    /// The weight used when the service check is passing.
    pub passing: i32,
    /// The weight used when the service check is warning.
    pub warning: i32,
}

/// Service registration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AgentServiceRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    /// The optional service ID; defaults to the service name.
    pub id: Option<String>,
    /// The service name.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Tags attached to the service.
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The port the service listens on.
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The address of the service.
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Arbitrary key/value metadata.
    pub meta: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether tag updates from the catalog override local tags.
    pub enable_tag_override: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A single health check for the service.
    pub check: Option<AgentServiceCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Multiple health checks for the service.
    pub checks: Option<Vec<AgentServiceCheck>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The weight of the service.
    pub weights: Option<AgentWeights>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The namespace of the service (Enterprise).
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The admin partition of the service (Enterprise).
    pub partition: Option<String>,
}

/// Health check definition for service registration
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentServiceCheck {
    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    /// The check ID; defaults to the check name.
    pub check_id: Option<String>,
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    /// The name of the check.
    pub name: Option<String>,
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    /// The HTTP endpoint to probe.
    pub http: Option<String>,
    #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
    /// The TCP address to probe.
    pub tcp: Option<String>,
    #[serde(rename = "GRPC", skip_serializing_if = "Option::is_none")]
    /// The gRPC endpoint to probe.
    pub grpc: Option<String>,
    #[serde(rename = "Interval", skip_serializing_if = "Option::is_none")]
    /// The interval between probes (e.g. "10s").
    pub interval: Option<String>,
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    /// The probe timeout (e.g. "5s").
    pub timeout: Option<String>,
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    /// The TTL for TTL-based checks.
    pub ttl: Option<String>,
    #[serde(
        rename = "DeregisterCriticalServiceAfter",
        skip_serializing_if = "Option::is_none"
    )]
    /// Auto-deregister the service this long after a critical check.
    pub deregister_critical_service_after: Option<String>,
    #[serde(rename = "Status", skip_serializing_if = "Option::is_none")]
    /// The initial check status.
    pub status: Option<String>,
    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    /// Human-readable notes about the check.
    pub notes: Option<String>,
    #[serde(rename = "TLSSkipVerify", skip_serializing_if = "Option::is_none")]
    /// Whether to skip TLS verification for HTTPS checks.
    pub tls_skip_verify: Option<bool>,
    #[serde(rename = "GRPCUseTLS", skip_serializing_if = "Option::is_none")]
    /// Whether the gRPC check uses TLS.
    pub grpc_use_tls: Option<bool>,
}

/// A check registered with the agent
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AgentCheck {
    /// The node the check runs on.
    pub node: String,
    #[serde(rename = "CheckID")]
    /// The check ID.
    pub check_id: String,
    /// The check name.
    pub name: String,
    /// The check status: passing, warning or critical.
    pub status: String,
    /// Human-readable notes about the check.
    pub notes: String,
    /// The output of the last check run.
    pub output: String,
    #[serde(rename = "ServiceID")]
    /// The ID of the associated service, if any.
    pub service_id: String,
    /// The name of the associated service, if any.
    pub service_name: String,
    #[serde(rename = "Type", default)]
    /// The check type.
    pub check_type: Option<String>,
    #[serde(default)]
    /// The namespace of the check (Enterprise).
    pub namespace: Option<String>,
    #[serde(default)]
    /// The admin partition of the check (Enterprise).
    pub partition: Option<String>,
    /// The index at which the check was created.
    pub create_index: u64,
    /// The index at which the check was last modified.
    pub modify_index: u64,
}

/// A member of the Consul cluster (serf)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AgentMember {
    /// The name of the member node.
    pub name: String,
    /// The address of the member node.
    pub addr: String,
    /// The port of the member node.
    pub port: u16,
    #[serde(default)]
    /// Tags describing the member.
    pub tags: Option<HashMap<String, String>>,
    /// The serf status of the member.
    pub status: i32,
    /// The minimum supported protocol version.
    pub protocol_min: u8,
    /// The maximum supported protocol version.
    pub protocol_max: u8,
    /// The currently used protocol version.
    pub protocol_cur: u8,
    /// The minimum delegate protocol version.
    pub delegate_min: u8,
    /// The maximum delegate protocol version.
    pub delegate_max: u8,
    /// The current delegate protocol version.
    pub delegate_cur: u8,
}

/// Health check registration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentCheckRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    /// The check ID; defaults to the check name.
    pub id: Option<String>,
    #[serde(rename = "Name")]
    /// The name of the check.
    pub name: String,
    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    /// Human-readable notes about the check.
    pub notes: Option<String>,
    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    /// The ID of the service this check is attached to.
    pub service_id: Option<String>,
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    /// The HTTP endpoint to probe.
    pub http: Option<String>,
    #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
    /// The TCP address to probe.
    pub tcp: Option<String>,
    #[serde(rename = "GRPC", skip_serializing_if = "Option::is_none")]
    /// The gRPC endpoint to probe.
    pub grpc: Option<String>,
    #[serde(rename = "Interval", skip_serializing_if = "Option::is_none")]
    /// The interval between probes (e.g. "10s").
    pub interval: Option<String>,
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    /// The probe timeout (e.g. "5s").
    pub timeout: Option<String>,
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    /// The TTL for TTL-based checks.
    pub ttl: Option<String>,
    #[serde(
        rename = "DeregisterCriticalServiceAfter",
        skip_serializing_if = "Option::is_none"
    )]
    /// Auto-deregister the service this long after a critical check.
    pub deregister_critical_service_after: Option<String>,
    #[serde(rename = "Status", skip_serializing_if = "Option::is_none")]
    /// The initial check status.
    pub status: Option<String>,
    #[serde(rename = "TLSSkipVerify", skip_serializing_if = "Option::is_none")]
    /// Whether to skip TLS verification for HTTPS checks.
    pub tls_skip_verify: Option<bool>,
    #[serde(rename = "GRPCUseTLS", skip_serializing_if = "Option::is_none")]
    /// Whether the gRPC check uses TLS.
    pub grpc_use_tls: Option<bool>,
}

/// TTL check update payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct AgentCheckUpdate {
    /// The new check status.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The output to attach to the check.
    pub output: Option<String>,
}

// --- Health ---

/// A health check entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct HealthCheck {
    /// The node the check runs on.
    pub node: String,
    #[serde(rename = "CheckID")]
    /// The check ID.
    pub check_id: String,
    /// The check name.
    pub name: String,
    /// The check status: passing, warning or critical.
    pub status: String,
    /// Human-readable notes about the check.
    pub notes: String,
    /// The output of the last check run.
    pub output: String,
    #[serde(rename = "ServiceID")]
    /// The ID of the associated service, if any.
    pub service_id: String,
    /// The name of the associated service, if any.
    pub service_name: String,
    #[serde(default)]
    /// Tags of the associated service.
    pub service_tags: Option<Vec<String>>,
    #[serde(rename = "Type", default)]
    /// The check type.
    pub check_type: Option<String>,
    #[serde(default)]
    /// The namespace of the check (Enterprise).
    pub namespace: Option<String>,
    #[serde(default)]
    /// The admin partition of the check (Enterprise).
    pub partition: Option<String>,
    /// The index at which the check was created.
    pub create_index: u64,
    /// The index at which the check was last modified.
    pub modify_index: u64,
}

/// A service entry with node and health checks (from /v1/health/service)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceEntry {
    /// The node hosting the service.
    pub node: Node,
    /// The service registered on the node.
    pub service: AgentService,
    /// Health checks associated with the node/service.
    pub checks: Vec<HealthCheck>,
}

// --- Catalog ---

/// Node info
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Node {
    #[serde(rename = "ID", default)]
    /// The node ID.
    pub id: Option<String>,
    /// The node name.
    pub node: String,
    /// The node address.
    pub address: String,
    #[serde(default)]
    /// The datacenter of the node.
    pub datacenter: Option<String>,
    #[serde(default)]
    /// Addresses tagged by type (LAN, WAN, etc.).
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(default)]
    /// Arbitrary key/value metadata.
    pub meta: Option<HashMap<String, String>>,
    #[serde(default)]
    /// The index at which the node was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the node was last modified.
    pub modify_index: u64,
}

/// A service from the catalog
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogService {
    #[serde(rename = "ID", default)]
    /// The node ID.
    pub id: Option<String>,
    /// The node hosting the service.
    pub node: String,
    /// The node address.
    pub address: String,
    #[serde(default)]
    /// The datacenter of the node.
    pub datacenter: Option<String>,
    #[serde(default)]
    /// Addresses tagged by type (LAN, WAN, etc.).
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(default)]
    /// Arbitrary key/value metadata for the node.
    pub node_meta: Option<HashMap<String, String>>,
    #[serde(rename = "ServiceID")]
    /// The service ID.
    pub service_id: String,
    /// The service name.
    pub service_name: String,
    /// The address of the service.
    pub service_address: String,
    #[serde(default)]
    /// Tags attached to the service.
    pub service_tags: Option<Vec<String>>,
    #[serde(default)]
    /// Arbitrary key/value metadata for the service.
    pub service_meta: Option<HashMap<String, String>>,
    /// The port the service listens on.
    pub service_port: u16,
    #[serde(default)]
    /// The weight of the service.
    pub service_weights: Option<AgentWeights>,
    /// Whether tag override is enabled for the service.
    pub service_enable_tag_override: bool,
    #[serde(default)]
    /// The namespace of the service (Enterprise).
    pub namespace: Option<String>,
    #[serde(default)]
    /// The admin partition of the service (Enterprise).
    pub partition: Option<String>,
    /// The index at which the entry was created.
    pub create_index: u64,
    /// The index at which the entry was last modified.
    pub modify_index: u64,
}

/// Catalog node with its services
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogNode {
    /// The node.
    pub node: Option<Node>,
    #[serde(default)]
    /// Services registered on the node, keyed by service ID.
    pub services: Option<HashMap<String, AgentService>>,
}

/// Catalog registration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    /// The node ID.
    pub id: Option<String>,
    /// The node name.
    pub node: String,
    /// The node address.
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The datacenter to register in.
    pub datacenter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Addresses tagged by type (LAN, WAN, etc.).
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Arbitrary key/value metadata for the node.
    pub node_meta: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The service to register.
    pub service: Option<AgentService>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A single health check to register.
    pub check: Option<HealthCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Multiple health checks to register.
    pub checks: Option<Vec<HealthCheck>>,
}

/// Catalog deregistration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct CatalogDeregistration {
    /// The node name.
    pub node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The node address.
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The datacenter to deregister from.
    pub datacenter: Option<String>,
    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    /// The service ID to deregister.
    pub service_id: Option<String>,
    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    /// The check ID to deregister.
    pub check_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The namespace of the entry (Enterprise).
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The admin partition of the entry (Enterprise).
    pub partition: Option<String>,
}

// --- Session ---

/// A session entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct SessionEntry {
    #[serde(rename = "ID", default)]
    /// The session ID.
    pub id: Option<String>,
    #[serde(default)]
    /// The human-readable session name.
    pub name: Option<String>,
    #[serde(default)]
    /// The node the session is bound to.
    pub node: Option<String>,
    #[serde(default)]
    /// The lock delay applied when a session is invalidated.
    pub lock_delay: Option<u64>,
    #[serde(default)]
    /// The invalidation behavior: `release` or `delete`.
    pub behavior: Option<String>,
    #[serde(rename = "TTL", default)]
    /// The session TTL (e.g. "30s").
    pub ttl: Option<String>,
    #[serde(default)]
    /// Health checks associated with the session.
    pub checks: Option<Vec<String>>,
    #[serde(default)]
    /// Node-level checks associated with the session.
    pub node_checks: Option<Vec<String>>,
    #[serde(default)]
    /// Service-level checks associated with the session.
    pub service_checks: Option<Vec<ServiceCheck>>,
    #[serde(default)]
    /// The namespace of the session (Enterprise).
    pub namespace: Option<String>,
    #[serde(default)]
    /// The admin partition of the session (Enterprise).
    pub partition: Option<String>,
    #[serde(default)]
    /// The index at which the session was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the session was last modified.
    pub modify_index: u64,
}

/// Represents a `None`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ServiceCheck {
    #[serde(rename = "ID")]
    /// The check ID.
    pub id: String,
    #[serde(default)]
    /// The namespace of the check (Enterprise).
    pub namespace: Option<String>,
}

// --- Event ---

/// A user event
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct UserEvent {
    #[serde(rename = "ID", default)]
    /// The event ID.
    pub id: Option<String>,
    /// The event name.
    pub name: String,
    #[serde(default)]
    /// The base64-encoded event payload.
    pub payload: Option<String>,
    #[serde(default)]
    /// The node filter applied when firing the event.
    pub node_filter: Option<String>,
    #[serde(default)]
    /// The service filter applied when firing the event.
    pub service_filter: Option<String>,
    #[serde(default)]
    /// The tag filter applied when firing the event.
    pub tag_filter: Option<String>,
    #[serde(default)]
    /// The event version.
    pub version: u32,
    #[serde(default)]
    /// The Lamport time of the event.
    pub l_time: u64,
}

// --- ACL ---

/// ACL Token
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLToken {
    #[serde(rename = "AccessorID", default)]
    /// The accessor ID used to reference the token.
    pub accessor_id: String,
    #[serde(rename = "SecretID", skip_serializing_if = "Option::is_none")]
    /// The secret ID used to authenticate with the token.
    pub secret_id: Option<String>,
    #[serde(rename = "Description", default)]
    /// A human-readable description of the token.
    pub description: String,
    #[serde(rename = "Policies", default, skip_serializing_if = "Option::is_none")]
    /// Policies linked to the token.
    pub policies: Option<Vec<ACLTokenPolicyLink>>,
    #[serde(rename = "Roles", default, skip_serializing_if = "Option::is_none")]
    /// Roles linked to the token.
    pub roles: Option<Vec<ACLTokenRoleLink>>,
    #[serde(
        rename = "ServiceIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Service identities granted to the token.
    pub service_identities: Option<Vec<ACLServiceIdentity>>,
    #[serde(
        rename = "NodeIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Node identities granted to the token.
    pub node_identities: Option<Vec<ACLNodeIdentity>>,
    #[serde(
        rename = "TemplatedPolicies",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Templated policies applied to the token.
    pub templated_policies: Option<Vec<ACLTemplatedPolicy>>,
    #[serde(rename = "Local", default)]
    /// Whether the token is local to the current datacenter.
    pub local: bool,
    #[serde(
        rename = "AuthMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The auth method used to create the token.
    pub auth_method: Option<String>,
    #[serde(
        rename = "ExpirationTTL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The TTL after which the token expires.
    pub expiration_ttl: Option<String>,
    #[serde(
        rename = "ExpirationTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The absolute time at which the token expires.
    pub expiration_time: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The time the token was created.
    pub create_time: Option<String>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    /// The hash of the token contents.
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the token was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the token was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the token (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the token (Enterprise).
    pub partition: Option<String>,
}

/// ACL Token list entry (lighter than full ACLToken)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTokenListEntry {
    #[serde(rename = "AccessorID")]
    /// The accessor ID used to reference the token.
    pub accessor_id: String,
    #[serde(rename = "SecretID", skip_serializing_if = "Option::is_none")]
    /// The secret ID used to authenticate with the token.
    pub secret_id: Option<String>,
    #[serde(rename = "Description", default)]
    /// A human-readable description of the token.
    pub description: String,
    #[serde(rename = "Policies", default, skip_serializing_if = "Option::is_none")]
    /// Policies linked to the token.
    pub policies: Option<Vec<ACLTokenPolicyLink>>,
    #[serde(rename = "Roles", default, skip_serializing_if = "Option::is_none")]
    /// Roles linked to the token.
    pub roles: Option<Vec<ACLTokenRoleLink>>,
    #[serde(rename = "Local", default)]
    /// Whether the token is local to the current datacenter.
    pub local: bool,
    #[serde(
        rename = "AuthMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The auth method used to create the token.
    pub auth_method: Option<String>,
    #[serde(
        rename = "ExpirationTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The absolute time at which the token expires.
    pub expiration_time: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The time the token was created.
    pub create_time: Option<String>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    /// The hash of the token contents.
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the token was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the token was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the token (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the token (Enterprise).
    pub partition: Option<String>,
}

/// Link to a policy in a token
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLTokenPolicyLink {
    #[serde(rename = "ID", default)]
    /// The policy ID.
    pub id: String,
    #[serde(default)]
    /// The policy name.
    pub name: String,
}

/// Link to a role in a token
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLTokenRoleLink {
    #[serde(rename = "ID", default)]
    /// The role ID.
    pub id: String,
    #[serde(default)]
    /// The role name.
    pub name: String,
}

/// Generic ACL link used by Namespace/Partition ACL defaults.
/// Matches Consul's `api.ACLLink`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLLink {
    #[serde(rename = "ID", default)]
    /// The linked object ID.
    pub id: String,
    #[serde(default)]
    /// The linked object name.
    pub name: String,
}

/// Legacy v1 ACL entry (Consul &lt; 1.4). Retained for backward compatibility.
///
/// Matches Consul's `api.ACLEntry`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ACLEntry {
    #[serde(default)]
    /// The index at which the entry was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the entry was last modified.
    pub modify_index: u64,
    #[serde(rename = "ID", default)]
    /// The ACL entry ID.
    pub id: String,
    #[serde(default)]
    /// The ACL entry name.
    pub name: String,
    /// Legacy type: "client" or "management".
    #[serde(rename = "Type", default)]
    pub acl_type: String,
    #[serde(default)]
    /// The legacy ACL rules in HCL format.
    pub rules: String,
}

/// Wire envelope for legacy ACL create response `{"ID": "xxx"}`.
#[derive(Deserialize)]
pub(crate) struct LegacyACLCreateResp {
    #[serde(rename = "ID", default)]
    pub id: String,
}

/// Service identity for ACL
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLServiceIdentity {
    #[serde(rename = "ServiceName")]
    /// The name of the service the identity applies to.
    pub service_name: String,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Datacenters the identity is valid in.
    pub datacenters: Option<Vec<String>>,
}

/// Node identity for ACL
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLNodeIdentity {
    /// The name of the node the identity applies to.
    pub node_name: String,
    /// The datacenter the identity is valid in.
    pub datacenter: String,
}

/// Templated policy reference
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTemplatedPolicy {
    #[serde(rename = "TemplateName")]
    /// The name of the policy template.
    pub template_name: String,
    #[serde(
        rename = "TemplateVariables",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Variables used to render the template.
    pub template_variables: Option<ACLTemplatedPolicyVariables>,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Datacenters the templated policy applies to.
    pub datacenters: Option<Vec<String>>,
}

/// Variables for templated policies
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLTemplatedPolicyVariables {
    /// The name variable used when rendering the template.
    pub name: String,
}

/// Templated policy response from server
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLTemplatedPolicyResponse {
    /// The name of the policy template.
    pub template_name: String,
    #[serde(default)]
    /// The JSON schema describing the template variables.
    pub schema: String,
    #[serde(default)]
    /// The raw policy template.
    pub template: String,
    #[serde(default)]
    /// A human-readable description of the template.
    pub description: String,
}

/// ACL Policy
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLPolicy {
    #[serde(rename = "ID", default)]
    /// The policy ID.
    pub id: String,
    #[serde(rename = "Name")]
    /// The policy name.
    pub name: String,
    #[serde(rename = "Description", default)]
    /// A human-readable description of the policy.
    pub description: String,
    #[serde(rename = "Rules", default)]
    /// The policy rules in HCL or JSON format.
    pub rules: String,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Datacenters the policy applies to.
    pub datacenters: Option<Vec<String>>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    /// The hash of the policy contents.
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the policy was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the policy was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the policy (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the policy (Enterprise).
    pub partition: Option<String>,
}

/// ACL Policy list entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLPolicyListEntry {
    #[serde(rename = "ID")]
    /// The policy ID.
    pub id: String,
    #[serde(rename = "Name")]
    /// The policy name.
    pub name: String,
    #[serde(rename = "Description", default)]
    /// A human-readable description of the policy.
    pub description: String,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Datacenters the policy applies to.
    pub datacenters: Option<Vec<String>>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    /// The hash of the policy contents.
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the policy was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the policy was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the policy (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the policy (Enterprise).
    pub partition: Option<String>,
}

/// ACL Role
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLRole {
    #[serde(rename = "ID", default)]
    /// The role ID.
    pub id: String,
    #[serde(rename = "Name")]
    /// The role name.
    pub name: String,
    #[serde(rename = "Description", default)]
    /// A human-readable description of the role.
    pub description: String,
    #[serde(rename = "Policies", default, skip_serializing_if = "Option::is_none")]
    /// Policies linked to the role.
    pub policies: Option<Vec<ACLTokenPolicyLink>>,
    #[serde(
        rename = "ServiceIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Service identities granted to the role.
    pub service_identities: Option<Vec<ACLServiceIdentity>>,
    #[serde(
        rename = "NodeIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Node identities granted to the role.
    pub node_identities: Option<Vec<ACLNodeIdentity>>,
    #[serde(
        rename = "TemplatedPolicies",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Templated policies applied to the role.
    pub templated_policies: Option<Vec<ACLTemplatedPolicy>>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    /// The hash of the role contents.
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the role was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the role was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the role (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the role (Enterprise).
    pub partition: Option<String>,
}

/// ACL Auth Method
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLAuthMethod {
    #[serde(rename = "Name")]
    /// The auth method name.
    pub name: String,
    #[serde(rename = "Type")]
    /// The auth method type (e.g. `kubernetes`, `jwt`, `oidc`).
    pub method_type: String,
    #[serde(
        rename = "DisplayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// A display name for the auth method.
    pub display_name: Option<String>,
    #[serde(
        rename = "Description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// A human-readable description of the auth method.
    pub description: Option<String>,
    #[serde(
        rename = "MaxTokenTTL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The maximum TTL for tokens created by this method.
    pub max_token_ttl: Option<String>,
    #[serde(
        rename = "TokenLocality",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Whether created tokens are local or global.
    pub token_locality: Option<String>,
    #[serde(rename = "Config", default, skip_serializing_if = "Option::is_none")]
    /// Type-specific configuration for the auth method.
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the method was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the method was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the method (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the method (Enterprise).
    pub partition: Option<String>,
}

/// ACL Auth Method list entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLAuthMethodListEntry {
    #[serde(rename = "Name")]
    /// The auth method name.
    pub name: String,
    #[serde(rename = "Type")]
    /// The auth method type (e.g. `kubernetes`, `jwt`, `oidc`).
    pub method_type: String,
    #[serde(
        rename = "DisplayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// A display name for the auth method.
    pub display_name: Option<String>,
    #[serde(
        rename = "Description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// A human-readable description of the auth method.
    pub description: Option<String>,
    #[serde(
        rename = "MaxTokenTTL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The maximum TTL for tokens created by this method.
    pub max_token_ttl: Option<String>,
    #[serde(
        rename = "TokenLocality",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Whether created tokens are local or global.
    pub token_locality: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the method was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the method was last modified.
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the method (Enterprise).
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the method (Enterprise).
    pub partition: Option<String>,
}

/// ACL Binding Rule
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLBindingRule {
    #[serde(rename = "ID", default)]
    /// The binding rule ID.
    pub id: String,
    #[serde(default)]
    /// A human-readable description of the rule.
    pub description: String,
    /// The auth method this rule applies to.
    pub auth_method: String,
    #[serde(default)]
    /// The selector expression used to match logins.
    pub selector: String,
    /// What to bind: `service`, `role` or `templated-policy`.
    pub bind_type: String,
    /// The name of the role or policy to bind.
    pub bind_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Variables used when rendering a templated policy.
    pub bind_vars: Option<ACLTemplatedPolicyVariables>,
    #[serde(default)]
    /// The index at which the rule was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the rule was last modified.
    pub modify_index: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The namespace of the rule (Enterprise).
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the rule (Enterprise).
    pub partition: Option<String>,
}

/// ACL Login parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLLoginParams {
    /// The auth method to log in with.
    pub auth_method: String,
    /// The bearer token presented to the auth method.
    pub bearer_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Additional metadata passed to the auth method.
    pub meta: Option<HashMap<String, String>>,
}

/// ACL OIDC Auth URL parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLOIDCAuthURLParams {
    /// The OIDC auth method to use.
    pub auth_method: String,
    #[serde(rename = "RedirectURI")]
    /// The URI to redirect to after authentication.
    pub redirect_uri: String,
    /// A client-generated nonce used to bind the callback.
    pub client_nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Additional metadata passed to the auth method.
    pub meta: Option<HashMap<String, String>>,
}

/// ACL OIDC Callback parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct ACLOIDCCallbackParams {
    /// The OIDC auth method to complete.
    pub auth_method: String,
    /// The state parameter returned by the OIDC provider.
    pub state: String,
    /// The authorization code returned by the OIDC provider.
    pub code: String,
    /// The nonce originally supplied when generating the auth URL.
    pub client_nonce: String,
}

/// ACL Replication status
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLReplicationStatus {
    #[serde(rename = "Enabled")]
    /// Whether ACL replication is enabled.
    pub enabled: bool,
    #[serde(rename = "Running")]
    /// Whether ACL replication is currently running.
    pub running: bool,
    #[serde(rename = "SourceDatacenter", default)]
    /// The datacenter tokens are replicated from.
    pub source_datacenter: String,
    #[serde(rename = "ReplicationType", default)]
    /// The replication type: `tokens`, `policies` or `roles`.
    pub replication_type: String,
    #[serde(rename = "ReplicatedIndex", default)]
    /// The last replicated index.
    pub replicated_index: u64,
    #[serde(rename = "ReplicatedRoleIndex", default)]
    /// The last replicated role index.
    pub replicated_role_index: u64,
    #[serde(rename = "ReplicatedTokenIndex", default)]
    /// The last replicated token index.
    pub replicated_token_index: u64,
    #[serde(
        rename = "LastSuccess",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The time of the last successful replication.
    pub last_success: Option<String>,
    #[serde(rename = "LastError", default, skip_serializing_if = "Option::is_none")]
    /// The time of the last replication error.
    pub last_error: Option<String>,
    #[serde(rename = "LastErrorMessage", default)]
    /// The message describing the last replication error.
    pub last_error_message: String,
}

// === Peering Types ===

/// A peering connection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Peering {
    #[serde(default, rename = "ID")]
    /// The peering ID.
    pub id: String,
    /// The peering name.
    pub name: String,
    #[serde(default)]
    /// The peering state: `initial`, `active`, `failing` or `deleting`.
    pub state: String,
    #[serde(default)]
    /// The admin partition of the peering (Enterprise).
    pub partition: String,
    #[serde(default)]
    /// Arbitrary key/value metadata.
    pub meta: HashMap<String, String>,
    #[serde(default)]
    /// The addresses of the peer's servers.
    pub peer_server_addresses: Vec<String>,
    #[serde(default)]
    /// The index at which the peering was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the peering was last modified.
    pub modify_index: u64,
}

/// Response containing a peering token
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringToken {
    /// The generated peering token.
    pub peering_token: String,
}

/// Request to generate a peering token
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringGenerateTokenRequest {
    /// The name of the peer to generate a token for.
    pub peer_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the peer (Enterprise).
    pub partition: Option<String>,
    #[serde(default)]
    /// Arbitrary key/value metadata for the token.
    pub meta: HashMap<String, String>,
}

/// Request to establish a peering connection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringEstablishRequest {
    /// The name of the peer to establish.
    pub peer_name: String,
    /// The token generated by the peer cluster.
    pub peering_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The admin partition of the peer (Enterprise).
    pub partition: Option<String>,
    #[serde(default)]
    /// Arbitrary key/value metadata for the peering.
    pub meta: HashMap<String, String>,
}

/// ACL Token filter options for listing
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTokenFilterOptions {
    #[serde(
        rename = "AuthMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Filter tokens created by this auth method.
    pub auth_method: Option<String>,
    #[serde(rename = "Policy", default, skip_serializing_if = "Option::is_none")]
    /// Filter tokens linked to this policy.
    pub policy: Option<String>,
    #[serde(rename = "Role", default, skip_serializing_if = "Option::is_none")]
    /// Filter tokens linked to this role.
    pub role: Option<String>,
    #[serde(
        rename = "ServiceName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Filter tokens with a service identity for this service.
    pub service_name: Option<String>,
}

// === Operator Types ===

/// Raft configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftConfiguration {
    #[serde(default)]
    /// The servers participating in the Raft cluster.
    pub servers: Vec<RaftServer>,
    #[serde(default)]
    /// The Raft index of this configuration.
    pub index: u64,
}

/// A server in the Raft configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftServer {
    #[serde(rename = "ID")]
    /// The server ID.
    pub id: String,
    /// The node name of the server.
    pub node: String,
    /// The address of the server.
    pub address: String,
    /// Whether the server is the current leader.
    pub leader: bool,
    /// Whether the server is a voting member.
    pub voter: bool,
    #[serde(default)]
    /// The Raft protocol version used by the server.
    pub protocol_version: String,
}

/// Autopilot configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotConfiguration {
    #[serde(default)]
    /// Whether failed servers are automatically removed.
    pub cleanup_dead_servers: bool,
    #[serde(default)]
    /// How long a server can go without contact before being flagged.
    pub last_contact_threshold: String,
    #[serde(default)]
    /// The maximum number of trailing logs a server may have.
    pub max_trailing_logs: u64,
    #[serde(default)]
    /// The minimum number of servers required for quorum.
    pub min_quorum: u64,
    #[serde(default)]
    /// How long a server must be stable before being promoted to voter.
    pub server_stabilization_time: String,
    #[serde(default)]
    /// The node meta tag used to group servers into redundancy zones.
    pub redundancy_zone_tag: String,
    #[serde(default)]
    /// Whether autopilot upgrade migrations are disabled.
    pub disable_upgrade_migration: bool,
    #[serde(default)]
    /// The node meta tag containing the desired Consul version.
    pub upgrade_version_tag: String,
    #[serde(default)]
    /// The index at which the configuration was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the configuration was last modified.
    pub modify_index: u64,
}

/// Autopilot health status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotHealth {
    /// Whether the cluster is healthy.
    pub healthy: bool,
    #[serde(default)]
    /// The number of server failures the cluster can tolerate.
    pub failure_tolerance: i32,
    #[serde(default)]
    /// The health of individual servers.
    pub servers: Vec<ServerHealth>,
}

/// Health status of a single server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServerHealth {
    #[serde(rename = "ID")]
    /// The server ID.
    pub id: String,
    /// The server name.
    pub name: String,
    /// The server address.
    pub address: String,
    #[serde(default)]
    /// The serf (gossip) status of the server.
    pub serf_status: String,
    /// Whether the server is healthy.
    pub healthy: bool,
    #[serde(default)]
    /// The Consul version running on the server.
    pub version: String,
    /// Whether the server is the current leader.
    pub leader: bool,
    /// Whether the server is a voting member.
    pub voter: bool,
}

/// Keyring response from the operator keyring API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponse {
    #[serde(default)]
    /// Whether the keyring is for the WAN pool.
    pub wan: bool,
    /// The datacenter the keyring belongs to.
    pub datacenter: String,
    #[serde(default)]
    /// The network segment the keyring belongs to.
    pub segment: String,
    #[serde(default)]
    /// The keys in use, mapped to the number of nodes holding each.
    pub keys: HashMap<String, i32>,
    #[serde(default)]
    /// The primary keys, mapped to the number of nodes holding each.
    pub primary_keys: HashMap<String, i32>,
    #[serde(default)]
    /// The total number of nodes in the keyring scope.
    pub num_nodes: i32,
}

// === Connect Types ===

/// Connect CA root certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARoots {
    #[serde(rename = "ActiveRootID")]
    /// The ID of the currently active root certificate.
    pub active_root_id: String,
    #[serde(default)]
    /// All known root certificates.
    pub roots: Vec<CARoot>,
}

/// A single CA root certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARoot {
    #[serde(rename = "ID")]
    /// The root certificate ID.
    pub id: String,
    /// The common name of the root certificate.
    pub name: String,
    #[serde(default)]
    /// The PEM-encoded root certificate.
    pub root_cert: String,
    /// Whether this root is currently active.
    pub active: bool,
    #[serde(default)]
    /// The index at which the root was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the root was last modified.
    pub modify_index: u64,
}

/// Connect CA configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CAConfig {
    #[serde(default)]
    /// The CA provider (e.g. `consul`, `vault`, `aws-pca`).
    pub provider: String,
    #[serde(default)]
    /// Provider-specific configuration.
    pub config: HashMap<String, serde_json::Value>,
    #[serde(default)]
    /// Provider-reported state information.
    pub state: HashMap<String, String>,
    #[serde(default)]
    /// Whether to force a rotation without cross-signing.
    pub force_without_cross_signing: bool,
    #[serde(default)]
    /// The index at which the config was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the config was last modified.
    pub modify_index: u64,
}

/// A service mesh intention
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Intention {
    #[serde(default, rename = "ID")]
    /// The intention ID.
    pub id: String,
    #[serde(default)]
    /// The source service name.
    pub source_name: String,
    #[serde(default)]
    /// The destination service name.
    pub destination_name: String,
    #[serde(default)]
    /// The namespace of the source service (Enterprise).
    pub source_namespace: String,
    #[serde(default)]
    /// The namespace of the destination service (Enterprise).
    pub destination_namespace: String,
    #[serde(default)]
    /// The admin partition of the source service (Enterprise).
    pub source_partition: String,
    #[serde(default)]
    /// The admin partition of the destination service (Enterprise).
    pub destination_partition: String,
    #[serde(default)]
    /// The action: `allow` or `deny`.
    pub action: String,
    #[serde(default)]
    /// A human-readable description of the intention.
    pub description: String,
    #[serde(default)]
    /// Arbitrary key/value metadata.
    pub meta: HashMap<String, String>,
    #[serde(default)]
    /// The precedence used to resolve conflicting intentions.
    pub precedence: i32,
    #[serde(default)]
    /// The index at which the intention was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the intention was last modified.
    pub modify_index: u64,
}

/// Result of an intention authorization check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionCheck {
    /// Whether the connection is allowed.
    pub allowed: bool,
}

// === Transaction Types ===

/// A single transaction operation. At least one of the fields should be set.
///
/// Wire-compatible with Consul Go SDK's `api.TxnOp`, which supports KV,
/// Node, Service, and Check operations in a single atomic transaction.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnOp {
    #[serde(rename = "KV", default, skip_serializing_if = "Option::is_none")]
    /// A KV operation.
    pub kv: Option<TxnKVOp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A node operation.
    pub node: Option<TxnNodeOp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A service operation.
    pub service: Option<TxnServiceOp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A check operation.
    pub check: Option<TxnCheckOp>,
}

/// A KV operation within a transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnKVOp {
    /// The operation to perform (e.g. `set`, `get`, `delete`).
    pub verb: String,
    /// The KV key the operation targets.
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The base64-encoded value to write.
    pub value: Option<String>,
    #[serde(default)]
    /// User-defined flags attached to the pair.
    pub flags: u64,
    #[serde(default)]
    /// The modify index used for check-and-set operations.
    pub index: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The session used to lock the pair.
    pub session: Option<String>,
}

/// A Node operation within a transaction.
///
/// Valid verbs: "set", "cas", "get", "delete", "delete-cas".
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnNodeOp {
    /// The operation to perform (e.g. `set`, `get`, `delete`).
    pub verb: String,
    /// The node the operation targets.
    pub node: TxnNode,
}

/// Node entry used in Txn Node operations. Matches Consul `api.Node`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TxnNode {
    #[serde(rename = "ID", default)]
    /// The node ID.
    pub id: String,
    #[serde(rename = "Node", default)]
    /// The node name.
    pub node: String,
    #[serde(rename = "Address", default)]
    /// The node address.
    pub address: String,
    #[serde(
        rename = "Datacenter",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// The datacenter of the node.
    pub datacenter: Option<String>,
    #[serde(
        rename = "TaggedAddresses",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Addresses tagged by type (LAN, WAN, etc.).
    pub tagged_addresses: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "Meta", default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary key/value metadata.
    pub meta: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "CreateIndex", default)]
    /// The index at which the node was created.
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    /// The index at which the node was last modified.
    pub modify_index: u64,
}

/// A Service operation within a transaction.
///
/// Valid verbs: "set", "cas", "get", "delete", "delete-cas".
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnServiceOp {
    /// The operation to perform (e.g. `set`, `get`, `delete`).
    pub verb: String,
    /// Node on which the service runs.
    pub node: String,
    /// The service the operation targets.
    pub service: TxnService,
}

/// Service entry used in Txn Service operations. Matches Consul
/// `api.AgentService` minus agent-only fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnService {
    #[serde(rename = "ID", default)]
    /// The service ID; defaults to the service name.
    pub id: String,
    #[serde(default)]
    /// The service name.
    pub service: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Tags attached to the service.
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    /// The address of the service.
    pub address: String,
    #[serde(default)]
    /// The port the service listens on.
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Arbitrary key/value metadata.
    pub meta: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    /// The index at which the service was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the service was last modified.
    pub modify_index: u64,
}

/// A Check operation within a transaction.
///
/// Valid verbs: "set", "cas", "get", "delete", "delete-cas".
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnCheckOp {
    /// The operation to perform (e.g. `set`, `get`, `delete`).
    pub verb: String,
    /// The check the operation targets.
    pub check: TxnCheck,
}

/// Health check entry used in Txn Check operations.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnCheck {
    #[serde(default)]
    /// The node the check runs on.
    pub node: String,
    #[serde(rename = "CheckID", default)]
    /// The check ID.
    pub check_id: String,
    #[serde(default)]
    /// The check name.
    pub name: String,
    #[serde(default)]
    /// The check status: passing, warning or critical.
    pub status: String,
    #[serde(default)]
    /// Human-readable notes about the check.
    pub notes: String,
    #[serde(default)]
    /// The output of the last check run.
    pub output: String,
    #[serde(rename = "ServiceID", default)]
    /// The ID of the associated service, if any.
    pub service_id: String,
    #[serde(default)]
    /// The name of the associated service, if any.
    pub service_name: String,
    #[serde(default)]
    /// The index at which the check was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the check was last modified.
    pub modify_index: u64,
}

/// Response from a transaction execution
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct TxnResponse {
    #[serde(default)]
    /// The results of the successfully applied operations.
    pub results: Vec<TxnResult>,
    #[serde(default)]
    /// The errors reported for failed operations.
    pub errors: Vec<TxnError>,
}

/// A single result entry from a transaction. Exactly one of the fields
/// will be populated per original op.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TxnResult {
    #[serde(rename = "KV", default, skip_serializing_if = "Option::is_none")]
    /// The KV pair returned by the operation.
    pub kv: Option<KVPair>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The node returned by the operation.
    pub node: Option<TxnNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The service returned by the operation.
    pub service: Option<TxnService>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The check returned by the operation.
    pub check: Option<TxnCheck>,
}

/// A single error entry from a transaction
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct TxnError {
    #[serde(default)]
    /// The index of the operation that failed.
    pub op_index: u64,
    #[serde(default)]
    /// A description of the failure.
    pub what: String,
}

// === Coordinate Types ===

/// WAN coordinate information for a datacenter
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DatacenterCoordinate {
    /// The datacenter name.
    pub datacenter: String,
    #[serde(rename = "AreaID", default)]
    /// The network area ID.
    pub area_id: String,
    #[serde(default)]
    /// The node coordinates within this datacenter.
    pub coordinates: Vec<NodeCoordinate>,
}

/// LAN coordinate information for a node
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct NodeCoordinate {
    /// The node name.
    pub node: String,
    #[serde(default)]
    /// The network segment of the node.
    pub segment: String,
    /// The network coordinate of the node.
    pub coord: Coordinate,
    #[serde(default)]
    /// The index at which the coordinate was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the coordinate was last modified.
    pub modify_index: u64,
}

/// Network coordinate vector
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Coordinate {
    #[serde(default)]
    /// The coordinate vector.
    pub vec: Vec<f64>,
    #[serde(default)]
    /// The estimated error of the coordinate.
    pub error: f64,
    #[serde(default)]
    /// The adjustment applied to the coordinate.
    pub adjustment: f64,
    #[serde(default)]
    /// The height of the coordinate.
    pub height: f64,
}

// === Prepared Query Types ===

/// A prepared query definition
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct PreparedQuery {
    #[serde(rename = "ID", default)]
    /// The prepared query ID.
    pub id: String,
    #[serde(default)]
    /// The prepared query name.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The ACL token used when executing the query.
    pub token: String,
    #[serde(default)]
    /// The service targeting configuration.
    pub service: QueryService,
    #[serde(rename = "DNS", default)]
    /// The DNS settings for the query.
    pub dns: QueryDNS,
    #[serde(default)]
    /// The index at which the query was created.
    pub create_index: u64,
    #[serde(default)]
    /// The index at which the query was last modified.
    pub modify_index: u64,
}

/// Service targeting configuration for a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct QueryService {
    #[serde(default)]
    /// The service the query targets.
    pub service: String,
    #[serde(default)]
    /// The node used to sort results by network distance.
    pub near: String,
    #[serde(default)]
    /// Tags used to filter the service instances.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Whether only passing instances are returned.
    pub only_passing: bool,
    #[serde(default)]
    /// The failover configuration for the query.
    pub failover: QueryFailover,
}

/// Failover configuration for a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct QueryFailover {
    #[serde(default)]
    /// The number of nearest instances to return during failover.
    pub nearest_n: i32,
    #[serde(default)]
    /// The datacenters to fail over to, in order.
    pub datacenters: Vec<String>,
}

/// DNS settings for a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QueryDNS {
    #[serde(rename = "TTL", default)]
    /// The TTL applied to DNS answers for the query.
    pub ttl: String,
}

/// Response from executing a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct PreparedQueryExecuteResponse {
    #[serde(default)]
    /// The service that was queried.
    pub service: String,
    #[serde(default)]
    /// The healthy instances returned by the query.
    pub nodes: Vec<ServiceEntry>,
    #[serde(rename = "DNS", default)]
    /// The DNS settings used for the query.
    pub dns: QueryDNS,
    #[serde(default)]
    /// The datacenter that served the query.
    pub datacenter: String,
    #[serde(default)]
    /// The number of failovers performed to serve the query.
    pub failovers: i32,
}

// Aggregated health states. Mirrors the `Health*` constants in
// consul/api/health.go:17-20; the agent health endpoints signal the outcome
// through both this string and the HTTP status code.
/// Service and all of its checks are passing.
pub const HEALTH_PASSING: &str = "passing";
/// At least one check is in the warning state.
pub const HEALTH_WARNING: &str = "warning";
/// At least one check is critical, or the service is unknown.
pub const HEALTH_CRITICAL: &str = "critical";
/// The node or service is in maintenance mode.
pub const HEALTH_MAINTENANCE: &str = "maintenance";

/// Consul's Go `encoding/json` emits capital-first field names verbatim.
/// The wire rule is declared once per DTO; only Go acronym spellings such as
/// `ID`, `HTTP` and `TTL` remain explicit field-level overrides.
#[cfg(test)]
mod consul_pascal_case_contract {
    use super::*;

    #[test]
    fn container_rule_emits_pascal_case_keys() {
        let pair = KVPair {
            key: "app/config".to_string(),
            create_index: 7,
            modify_index: 9,
            lock_index: 0,
            flags: 42,
            value: None,
            session: None,
            namespace: None,
            partition: None,
        };
        let value = serde_json::to_value(&pair).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(
            object.get("Key").and_then(|v| v.as_str()),
            Some("app/config")
        );
        assert_eq!(object.get("CreateIndex").and_then(|v| v.as_u64()), Some(7));
        assert_eq!(object.get("ModifyIndex").and_then(|v| v.as_u64()), Some(9));
        assert_eq!(object.get("Flags").and_then(|v| v.as_u64()), Some(42));
        assert!(!object.contains_key("key"));
        assert!(!object.contains_key("create_index"));
        assert!(!object.contains_key("createIndex"));
    }

    #[test]
    fn container_rule_requires_pascal_case_input() {
        let parsed: AgentService = serde_json::from_str(
            r#"{"ID":"svc-1","Service":"svc","Port":8080,"Address":"127.0.0.1","EnableTagOverride":true}"#,
        )
        .unwrap();
        assert_eq!(parsed.id, "svc-1");
        assert_eq!(parsed.service, "svc");
        assert!(parsed.enable_tag_override);

        let snake_case = serde_json::from_str::<AgentService>(
            r#"{"id":"svc-1","service":"svc","port":8080,"address":"127.0.0.1","enable_tag_override":true}"#,
        );
        assert!(snake_case.is_err(), "snake_case input must not be accepted");
    }

    #[test]
    fn go_acronym_overrides_survive_the_container_rule() {
        let service = AgentService {
            id: "svc-1".to_string(),
            service: "svc".to_string(),
            port: 8080,
            address: "127.0.0.1".to_string(),
            enable_tag_override: true,
            ..Default::default()
        };
        let value = serde_json::to_value(&service).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.get("ID").and_then(|v| v.as_str()), Some("svc-1"));
        assert!(!object.contains_key("Id"));
        assert!(!object.contains_key("id"));
        assert_eq!(
            object.get("EnableTagOverride").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn mixed_dto_keeps_its_explicit_renames() {
        // This DTO was deliberately left alone: not every field carries a
        // rename, so a container rule would change the remaining wire keys.
        let check = AgentServiceCheck {
            check_id: Some("chk-1".to_string()),
            http: Some("http://127.0.0.1/health".to_string()),
            ttl: Some("10s".to_string()),
            ..Default::default()
        };
        let value = serde_json::to_value(&check).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(
            object.get("CheckID").and_then(|v| v.as_str()),
            Some("chk-1")
        );
        assert_eq!(
            object.get("HTTP").and_then(|v| v.as_str()),
            Some("http://127.0.0.1/health")
        );
        assert_eq!(object.get("TTL").and_then(|v| v.as_str()), Some("10s"));
        assert!(!object.contains_key("check_id"));
        assert!(!object.contains_key("Http"));
    }
}
