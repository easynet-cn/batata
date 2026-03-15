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
    pub datacenter: String,
    pub token: String,
    pub namespace: String,
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
pub struct KVPair {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
    #[serde(rename = "LockIndex")]
    pub lock_index: u64,
    #[serde(rename = "Flags")]
    pub flags: u64,
    /// Base64-encoded value
    #[serde(rename = "Value")]
    pub value: Option<String>,
    #[serde(rename = "Session")]
    pub session: Option<String>,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default)]
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
pub struct AgentService {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Service")]
    pub service: String,
    #[serde(rename = "Tags", default)]
    pub tags: Option<Vec<String>>,
    #[serde(rename = "Meta", default)]
    pub meta: Option<HashMap<String, String>>,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Weights", default)]
    pub weights: Option<AgentWeights>,
    #[serde(rename = "EnableTagOverride")]
    pub enable_tag_override: bool,
    #[serde(rename = "ContentHash", default)]
    pub content_hash: Option<String>,
    #[serde(rename = "Datacenter", default)]
    pub datacenter: Option<String>,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default)]
    pub partition: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentWeights {
    #[serde(rename = "Passing")]
    pub passing: i32,
    #[serde(rename = "Warning")]
    pub warning: i32,
}

/// Service registration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentServiceRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Tags", skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(rename = "Port", skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "Meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
    #[serde(rename = "EnableTagOverride", skip_serializing_if = "Option::is_none")]
    pub enable_tag_override: Option<bool>,
    #[serde(rename = "Check", skip_serializing_if = "Option::is_none")]
    pub check: Option<AgentServiceCheck>,
    #[serde(rename = "Checks", skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<AgentServiceCheck>>,
    #[serde(rename = "Weights", skip_serializing_if = "Option::is_none")]
    pub weights: Option<AgentWeights>,
    #[serde(rename = "Namespace", skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// Health check definition for service registration
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentServiceCheck {
    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
    pub tcp: Option<String>,
    #[serde(rename = "GRPC", skip_serializing_if = "Option::is_none")]
    pub grpc: Option<String>,
    #[serde(rename = "Interval", skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    #[serde(
        rename = "DeregisterCriticalServiceAfter",
        skip_serializing_if = "Option::is_none"
    )]
    pub deregister_critical_service_after: Option<String>,
    #[serde(rename = "Status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(rename = "TLSSkipVerify", skip_serializing_if = "Option::is_none")]
    pub tls_skip_verify: Option<bool>,
    #[serde(rename = "GRPCUseTLS", skip_serializing_if = "Option::is_none")]
    pub grpc_use_tls: Option<bool>,
}

/// A check registered with the agent
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentCheck {
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
    #[serde(rename = "Type", default)]
    pub check_type: Option<String>,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default)]
    pub partition: Option<String>,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// A member of the Consul cluster (serf)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentMember {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Addr")]
    pub addr: String,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Tags", default)]
    pub tags: Option<HashMap<String, String>>,
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

/// Health check registration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentCheckRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    #[serde(rename = "HTTP", skip_serializing_if = "Option::is_none")]
    pub http: Option<String>,
    #[serde(rename = "TCP", skip_serializing_if = "Option::is_none")]
    pub tcp: Option<String>,
    #[serde(rename = "GRPC", skip_serializing_if = "Option::is_none")]
    pub grpc: Option<String>,
    #[serde(rename = "Interval", skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(rename = "Timeout", skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    #[serde(rename = "TTL", skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
    #[serde(
        rename = "DeregisterCriticalServiceAfter",
        skip_serializing_if = "Option::is_none"
    )]
    pub deregister_critical_service_after: Option<String>,
    #[serde(rename = "Status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(rename = "TLSSkipVerify", skip_serializing_if = "Option::is_none")]
    pub tls_skip_verify: Option<bool>,
    #[serde(rename = "GRPCUseTLS", skip_serializing_if = "Option::is_none")]
    pub grpc_use_tls: Option<bool>,
}

/// TTL check update payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AgentCheckUpdate {
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Output", skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

// --- Health ---

/// A health check entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
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
    #[serde(rename = "ServiceTags", default)]
    pub service_tags: Option<Vec<String>>,
    #[serde(rename = "Type", default)]
    pub check_type: Option<String>,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default)]
    pub partition: Option<String>,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// A service entry with node and health checks (from /v1/health/service)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ServiceEntry {
    #[serde(rename = "Node")]
    pub node: Node,
    #[serde(rename = "Service")]
    pub service: AgentService,
    #[serde(rename = "Checks")]
    pub checks: Vec<HealthCheck>,
}

// --- Catalog ---

/// Node info
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Node {
    #[serde(rename = "ID", default)]
    pub id: Option<String>,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Datacenter", default)]
    pub datacenter: Option<String>,
    #[serde(rename = "TaggedAddresses", default)]
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(rename = "Meta", default)]
    pub meta: Option<HashMap<String, String>>,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// A service from the catalog
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CatalogService {
    #[serde(rename = "ID", default)]
    pub id: Option<String>,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Datacenter", default)]
    pub datacenter: Option<String>,
    #[serde(rename = "TaggedAddresses", default)]
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(rename = "NodeMeta", default)]
    pub node_meta: Option<HashMap<String, String>>,
    #[serde(rename = "ServiceID")]
    pub service_id: String,
    #[serde(rename = "ServiceName")]
    pub service_name: String,
    #[serde(rename = "ServiceAddress")]
    pub service_address: String,
    #[serde(rename = "ServiceTags", default)]
    pub service_tags: Option<Vec<String>>,
    #[serde(rename = "ServiceMeta", default)]
    pub service_meta: Option<HashMap<String, String>>,
    #[serde(rename = "ServicePort")]
    pub service_port: u16,
    #[serde(rename = "ServiceWeights", default)]
    pub service_weights: Option<AgentWeights>,
    #[serde(rename = "ServiceEnableTagOverride")]
    pub service_enable_tag_override: bool,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default)]
    pub partition: Option<String>,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// Catalog node with its services
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CatalogNode {
    #[serde(rename = "Node")]
    pub node: Option<Node>,
    #[serde(rename = "Services", default)]
    pub services: Option<HashMap<String, AgentService>>,
}

/// Catalog registration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CatalogRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "Datacenter", skip_serializing_if = "Option::is_none")]
    pub datacenter: Option<String>,
    #[serde(rename = "TaggedAddresses", skip_serializing_if = "Option::is_none")]
    pub tagged_addresses: Option<HashMap<String, String>>,
    #[serde(rename = "NodeMeta", skip_serializing_if = "Option::is_none")]
    pub node_meta: Option<HashMap<String, String>>,
    #[serde(rename = "Service", skip_serializing_if = "Option::is_none")]
    pub service: Option<AgentService>,
    #[serde(rename = "Check", skip_serializing_if = "Option::is_none")]
    pub check: Option<HealthCheck>,
    #[serde(rename = "Checks", skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<HealthCheck>>,
}

/// Catalog deregistration payload
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CatalogDeregistration {
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "Datacenter", skip_serializing_if = "Option::is_none")]
    pub datacenter: Option<String>,
    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,
    #[serde(rename = "Namespace", skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

// --- Session ---

/// A session entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SessionEntry {
    #[serde(rename = "ID", default)]
    pub id: Option<String>,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "Node", default)]
    pub node: Option<String>,
    #[serde(rename = "LockDelay", default)]
    pub lock_delay: Option<u64>,
    #[serde(rename = "Behavior", default)]
    pub behavior: Option<String>,
    #[serde(rename = "TTL", default)]
    pub ttl: Option<String>,
    #[serde(rename = "Checks", default)]
    pub checks: Option<Vec<String>>,
    #[serde(rename = "NodeChecks", default)]
    pub node_checks: Option<Vec<String>>,
    #[serde(rename = "ServiceChecks", default)]
    pub service_checks: Option<Vec<ServiceCheck>>,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default)]
    pub partition: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ServiceCheck {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Namespace", default)]
    pub namespace: Option<String>,
}

// --- Event ---

/// A user event
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct UserEvent {
    #[serde(rename = "ID", default)]
    pub id: Option<String>,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Payload", default)]
    pub payload: Option<String>,
    #[serde(rename = "NodeFilter", default)]
    pub node_filter: Option<String>,
    #[serde(rename = "ServiceFilter", default)]
    pub service_filter: Option<String>,
    #[serde(rename = "TagFilter", default)]
    pub tag_filter: Option<String>,
    #[serde(rename = "Version", default)]
    pub version: u32,
    #[serde(rename = "LTime", default)]
    pub l_time: u64,
}

// --- ACL ---

/// ACL Token
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLToken {
    #[serde(rename = "AccessorID", default)]
    pub accessor_id: String,
    #[serde(rename = "SecretID", skip_serializing_if = "Option::is_none")]
    pub secret_id: Option<String>,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "Policies", default, skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<ACLTokenPolicyLink>>,
    #[serde(rename = "Roles", default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<ACLTokenRoleLink>>,
    #[serde(
        rename = "ServiceIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub service_identities: Option<Vec<ACLServiceIdentity>>,
    #[serde(
        rename = "NodeIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub node_identities: Option<Vec<ACLNodeIdentity>>,
    #[serde(
        rename = "TemplatedPolicies",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub templated_policies: Option<Vec<ACLTemplatedPolicy>>,
    #[serde(rename = "Local", default)]
    pub local: bool,
    #[serde(
        rename = "AuthMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_method: Option<String>,
    #[serde(
        rename = "ExpirationTTL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expiration_ttl: Option<String>,
    #[serde(
        rename = "ExpirationTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expiration_time: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub create_time: Option<String>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Token list entry (lighter than full ACLToken)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTokenListEntry {
    #[serde(rename = "AccessorID")]
    pub accessor_id: String,
    #[serde(rename = "SecretID", skip_serializing_if = "Option::is_none")]
    pub secret_id: Option<String>,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "Policies", default, skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<ACLTokenPolicyLink>>,
    #[serde(rename = "Roles", default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<ACLTokenRoleLink>>,
    #[serde(rename = "Local", default)]
    pub local: bool,
    #[serde(
        rename = "AuthMethod",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub auth_method: Option<String>,
    #[serde(
        rename = "ExpirationTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expiration_time: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub create_time: Option<String>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// Link to a policy in a token
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTokenPolicyLink {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: String,
}

/// Link to a role in a token
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTokenRoleLink {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: String,
}

/// Service identity for ACL
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLServiceIdentity {
    #[serde(rename = "ServiceName")]
    pub service_name: String,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub datacenters: Option<Vec<String>>,
}

/// Node identity for ACL
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLNodeIdentity {
    #[serde(rename = "NodeName")]
    pub node_name: String,
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
}

/// Templated policy reference
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTemplatedPolicy {
    #[serde(rename = "TemplateName")]
    pub template_name: String,
    #[serde(
        rename = "TemplateVariables",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub template_variables: Option<ACLTemplatedPolicyVariables>,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub datacenters: Option<Vec<String>>,
}

/// Variables for templated policies
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTemplatedPolicyVariables {
    #[serde(rename = "Name")]
    pub name: String,
}

/// Templated policy response from server
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLTemplatedPolicyResponse {
    #[serde(rename = "TemplateName")]
    pub template_name: String,
    #[serde(rename = "Schema", default)]
    pub schema: String,
    #[serde(rename = "Template", default)]
    pub template: String,
    #[serde(rename = "Description", default)]
    pub description: String,
}

/// ACL Policy
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLPolicy {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "Rules", default)]
    pub rules: String,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub datacenters: Option<Vec<String>>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Policy list entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLPolicyListEntry {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(
        rename = "Datacenters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub datacenters: Option<Vec<String>>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Role
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLRole {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "Policies", default, skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<ACLTokenPolicyLink>>,
    #[serde(
        rename = "ServiceIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub service_identities: Option<Vec<ACLServiceIdentity>>,
    #[serde(
        rename = "NodeIdentities",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub node_identities: Option<Vec<ACLNodeIdentity>>,
    #[serde(
        rename = "TemplatedPolicies",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub templated_policies: Option<Vec<ACLTemplatedPolicy>>,
    #[serde(rename = "Hash", default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Auth Method
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLAuthMethod {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub method_type: String,
    #[serde(
        rename = "DisplayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub display_name: Option<String>,
    #[serde(
        rename = "Description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
    #[serde(
        rename = "MaxTokenTTL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_token_ttl: Option<String>,
    #[serde(
        rename = "TokenLocality",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub token_locality: Option<String>,
    #[serde(rename = "Config", default, skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Auth Method list entry
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLAuthMethodListEntry {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Type")]
    pub method_type: String,
    #[serde(
        rename = "DisplayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub display_name: Option<String>,
    #[serde(
        rename = "Description",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<String>,
    #[serde(
        rename = "MaxTokenTTL",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub max_token_ttl: Option<String>,
    #[serde(
        rename = "TokenLocality",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub token_locality: Option<String>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Binding Rule
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLBindingRule {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Description", default)]
    pub description: String,
    #[serde(rename = "AuthMethod")]
    pub auth_method: String,
    #[serde(rename = "Selector", default)]
    pub selector: String,
    #[serde(rename = "BindType")]
    pub bind_type: String,
    #[serde(rename = "BindName")]
    pub bind_name: String,
    #[serde(rename = "BindVars", default, skip_serializing_if = "Option::is_none")]
    pub bind_vars: Option<ACLTemplatedPolicyVariables>,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
    #[serde(rename = "Namespace", default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// ACL Login parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLLoginParams {
    #[serde(rename = "AuthMethod")]
    pub auth_method: String,
    #[serde(rename = "BearerToken")]
    pub bearer_token: String,
    #[serde(rename = "Meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
}

/// ACL OIDC Auth URL parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLOIDCAuthURLParams {
    #[serde(rename = "AuthMethod")]
    pub auth_method: String,
    #[serde(rename = "RedirectURI")]
    pub redirect_uri: String,
    #[serde(rename = "ClientNonce")]
    pub client_nonce: String,
    #[serde(rename = "Meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,
}

/// ACL OIDC Callback parameters
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLOIDCCallbackParams {
    #[serde(rename = "AuthMethod")]
    pub auth_method: String,
    #[serde(rename = "State")]
    pub state: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "ClientNonce")]
    pub client_nonce: String,
}

/// ACL Replication status
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ACLReplicationStatus {
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "Running")]
    pub running: bool,
    #[serde(rename = "SourceDatacenter", default)]
    pub source_datacenter: String,
    #[serde(rename = "ReplicationType", default)]
    pub replication_type: String,
    #[serde(rename = "ReplicatedIndex", default)]
    pub replicated_index: u64,
    #[serde(rename = "ReplicatedRoleIndex", default)]
    pub replicated_role_index: u64,
    #[serde(rename = "ReplicatedTokenIndex", default)]
    pub replicated_token_index: u64,
    #[serde(
        rename = "LastSuccess",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_success: Option<String>,
    #[serde(rename = "LastError", default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(rename = "LastErrorMessage", default)]
    pub last_error_message: String,
}

// === Peering Types ===

/// A peering connection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Peering {
    #[serde(default, rename = "ID")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub partition: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
    #[serde(default)]
    pub peer_server_addresses: Vec<String>,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

/// Response containing a peering token
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringToken {
    pub peering_token: String,
}

/// Request to generate a peering token
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringGenerateTokenRequest {
    pub peer_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

/// Request to establish a peering connection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PeeringEstablishRequest {
    pub peer_name: String,
    pub peering_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
    #[serde(default)]
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
    pub auth_method: Option<String>,
    #[serde(rename = "Policy", default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(rename = "Role", default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(
        rename = "ServiceName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub service_name: Option<String>,
}

// === Operator Types ===

/// Raft configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftConfiguration {
    #[serde(default)]
    pub servers: Vec<RaftServer>,
    #[serde(default)]
    pub index: u64,
}

/// A server in the Raft configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RaftServer {
    #[serde(rename = "ID")]
    pub id: String,
    pub node: String,
    pub address: String,
    pub leader: bool,
    pub voter: bool,
    #[serde(default)]
    pub protocol_version: String,
}

/// Autopilot configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotConfiguration {
    #[serde(default)]
    pub cleanup_dead_servers: bool,
    #[serde(default)]
    pub last_contact_threshold: String,
    #[serde(default)]
    pub max_trailing_logs: u64,
    #[serde(default)]
    pub min_quorum: u64,
    #[serde(default)]
    pub server_stabilization_time: String,
    #[serde(default)]
    pub redundancy_zone_tag: String,
    #[serde(default)]
    pub disable_upgrade_migration: bool,
    #[serde(default)]
    pub upgrade_version_tag: String,
    #[serde(default, rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(default, rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// Autopilot health status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AutopilotHealth {
    pub healthy: bool,
    #[serde(default)]
    pub failure_tolerance: i32,
    #[serde(default)]
    pub servers: Vec<ServerHealth>,
}

/// Health status of a single server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServerHealth {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    pub address: String,
    #[serde(default)]
    pub serf_status: String,
    pub healthy: bool,
    #[serde(default)]
    pub version: String,
    pub leader: bool,
    pub voter: bool,
}

/// Keyring response from the operator keyring API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeyringResponse {
    #[serde(default)]
    pub wan: bool,
    pub datacenter: String,
    #[serde(default)]
    pub segment: String,
    #[serde(default)]
    pub keys: HashMap<String, i32>,
    #[serde(default)]
    pub primary_keys: HashMap<String, i32>,
    #[serde(default)]
    pub num_nodes: i32,
}

// === Connect Types ===

/// Connect CA root certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARoots {
    #[serde(rename = "ActiveRootID")]
    pub active_root_id: String,
    #[serde(default)]
    pub roots: Vec<CARoot>,
}

/// A single CA root certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CARoot {
    #[serde(rename = "ID")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub root_cert: String,
    pub active: bool,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

/// Connect CA configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CAConfig {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub state: HashMap<String, String>,
    #[serde(default)]
    pub force_without_cross_signing: bool,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

/// A service mesh intention
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Intention {
    #[serde(default, rename = "ID")]
    pub id: String,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub destination_name: String,
    #[serde(default)]
    pub source_namespace: String,
    #[serde(default)]
    pub destination_namespace: String,
    #[serde(default)]
    pub source_partition: String,
    #[serde(default)]
    pub destination_partition: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub meta: HashMap<String, String>,
    #[serde(default)]
    pub precedence: i32,
    #[serde(default)]
    pub create_index: u64,
    #[serde(default)]
    pub modify_index: u64,
}

/// Result of an intention authorization check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntentionCheck {
    pub allowed: bool,
}

// === Transaction Types ===

/// A single transaction operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxnOp {
    #[serde(rename = "KV", skip_serializing_if = "Option::is_none")]
    pub kv: Option<TxnKVOp>,
}

/// A KV operation within a transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxnKVOp {
    #[serde(rename = "Verb")]
    pub verb: String,
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value", default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(rename = "Index", default)]
    pub index: u64,
    #[serde(rename = "Session", default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Response from a transaction execution
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TxnResponse {
    #[serde(rename = "Results", default)]
    pub results: Vec<TxnResult>,
    #[serde(rename = "Errors", default)]
    pub errors: Vec<TxnError>,
}

/// A single result entry from a transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxnResult {
    #[serde(rename = "KV", skip_serializing_if = "Option::is_none")]
    pub kv: Option<KVPair>,
}

/// A single error entry from a transaction
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TxnError {
    #[serde(rename = "OpIndex", default)]
    pub op_index: u64,
    #[serde(rename = "What", default)]
    pub what: String,
}

// === Coordinate Types ===

/// WAN coordinate information for a datacenter
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatacenterCoordinate {
    #[serde(rename = "Datacenter")]
    pub datacenter: String,
    #[serde(rename = "AreaID", default)]
    pub area_id: String,
    #[serde(rename = "Coordinates", default)]
    pub coordinates: Vec<NodeCoordinate>,
}

/// LAN coordinate information for a node
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct NodeCoordinate {
    #[serde(rename = "Node")]
    pub node: String,
    #[serde(rename = "Segment", default)]
    pub segment: String,
    #[serde(rename = "Coord")]
    pub coord: Coordinate,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
}

/// Network coordinate vector
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Coordinate {
    #[serde(rename = "Vec", default)]
    pub vec: Vec<f64>,
    #[serde(rename = "Error", default)]
    pub error: f64,
    #[serde(rename = "Adjustment", default)]
    pub adjustment: f64,
    #[serde(rename = "Height", default)]
    pub height: f64,
}

// === Prepared Query Types ===

/// A prepared query definition
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PreparedQuery {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Token", default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(rename = "Service", default)]
    pub service: QueryService,
    #[serde(rename = "DNS", default)]
    pub dns: QueryDNS,
    #[serde(rename = "CreateIndex", default)]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex", default)]
    pub modify_index: u64,
}

/// Service targeting configuration for a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QueryService {
    #[serde(rename = "Service", default)]
    pub service: String,
    #[serde(rename = "Near", default)]
    pub near: String,
    #[serde(rename = "Tags", default)]
    pub tags: Vec<String>,
    #[serde(rename = "OnlyPassing", default)]
    pub only_passing: bool,
    #[serde(rename = "Failover", default)]
    pub failover: QueryFailover,
}

/// Failover configuration for a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QueryFailover {
    #[serde(rename = "NearestN", default)]
    pub nearest_n: i32,
    #[serde(rename = "Datacenters", default)]
    pub datacenters: Vec<String>,
}

/// DNS settings for a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct QueryDNS {
    #[serde(rename = "TTL", default)]
    pub ttl: String,
}

/// Response from executing a prepared query
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PreparedQueryExecuteResponse {
    #[serde(rename = "Service", default)]
    pub service: String,
    #[serde(rename = "Nodes", default)]
    pub nodes: Vec<ServiceEntry>,
    #[serde(rename = "DNS", default)]
    pub dns: QueryDNS,
    #[serde(rename = "Datacenter", default)]
    pub datacenter: String,
    #[serde(rename = "Failovers", default)]
    pub failovers: i32,
}
