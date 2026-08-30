use serde::{Deserialize, Serialize};

/// Consul-specific Raft request types.
///
/// These are applied by the Consul plugin handler and produce a dedicated
/// Raft log index space, independent of Batata core operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsulRaftRequest {
    // ==================== KV Operations ====================
    /// Put a key-value pair into the Consul KV store
    KVPut {
/// The `item` field.
        key: String,
        /// JSON-serialized StoredKV
        stored_kv_json: String,
        /// Optional session index key to write (kidx:session_id:key)
        session_index_key: Option<String>,
    },

    /// Delete a key from the Consul KV store
    KVDelete {
/// The `item` field.
        key: String,
        /// Optional session index key to clean up
        session_index_cleanup: Option<String>,
    },

    /// Delete all keys with a given prefix
    KVDeletePrefix { #[doc = "The `prefix` field."] prefix: String },

    /// Acquire a session lock on a KV key
    KVAcquireSession {
/// The `item` field.
        key: String,
/// The `item` field.
        session_id: String,
        /// JSON-serialized StoredKV with session set
        stored_kv_json: String,
    },

    /// Release a session lock on a single KV key
    KVReleaseSessionKey {
/// The `item` field.
        key: String,
/// The `item` field.
        session_id: String,
        /// JSON-serialized StoredKV with session cleared
        stored_kv_json: String,
    },

    /// Release all KV keys held by a session (on session destroy)
    KVReleaseSession {
/// The `item` field.
        session_id: String,
        /// Vec of (kv_key, updated_stored_kv_json) pairs
        updates: Vec<(String, String)>,
        /// Session index keys to delete
        index_keys_to_delete: Vec<String>,
    },

    /// Check-and-set: only update if modify_index matches
    KVCas {
/// The `item` field.
        key: String,
        /// JSON-serialized StoredKV
        stored_kv_json: String,
/// The `item` field.
        expected_modify_index: u64,
    },

    /// Execute a batch transaction of KV puts and deletes
    KVTransaction {
        /// Vec of (key, stored_kv_json) puts
        puts: Vec<(String, String)>,
        /// Keys to delete
        deletes: Vec<String>,
        /// Session index keys to put
        session_index_puts: Vec<String>,
        /// Session index keys to delete
        session_index_deletes: Vec<String>,
    },

    // ==================== Session Operations ====================
    /// Create a new Consul session
    SessionCreate {
/// The `item` field.
        session_id: String,
        /// JSON-serialized StoredSession
        stored_session_json: String,
    },

    /// Destroy a Consul session
    SessionDestroy { #[doc = "The `session_id` field."] session_id: String },

    /// Renew a Consul session
    SessionRenew {
/// The `item` field.
        session_id: String,
        /// JSON-serialized StoredSession with renewed TTL
        stored_session_json: String,
    },

    /// Clean up expired sessions
    SessionCleanupExpired { #[doc = "The `expired_session_ids` field."] expired_session_ids: Vec<String> },

    // ==================== ACL Operations ====================
    /// Create or update an ACL token
    ACLTokenSet {
/// The `item` field.
        accessor_id: String,
        /// JSON-serialized AclToken
        token_json: String,
    },

    /// Delete an ACL token
    ACLTokenDelete { #[doc = "The `accessor_id` field."] accessor_id: String },

    /// Create or update an ACL policy
    ACLPolicySet {
/// The `item` field.
        id: String,
        /// JSON-serialized AclPolicy
        policy_json: String,
    },

    /// Delete an ACL policy
    ACLPolicyDelete { #[doc = "The `id` field."] id: String },

    /// Create or update an ACL role
    ACLRoleSet {
/// The `item` field.
        id: String,
        /// JSON-serialized AclRole
        role_json: String,
    },

    /// Delete an ACL role
    ACLRoleDelete { #[doc = "The `id` field."] id: String },

    /// Create or update an ACL auth method
    ACLAuthMethodSet {
/// The `item` field.
        name: String,
        /// JSON-serialized AuthMethod
        method_json: String,
    },

    /// Delete an ACL auth method
    ACLAuthMethodDelete { #[doc = "The `name` field."] name: String },

    /// Bootstrap ACL (first token creation)
    ACLBootstrap {
        /// JSON-serialized AclToken
        token_json: String,
    },

    /// Create or update an ACL binding rule
    ACLBindingRuleSet {
/// The `item` field.
        id: String,
        /// JSON-serialized BindingRule
        rule_json: String,
    },

    /// Delete an ACL binding rule
    ACLBindingRuleDelete { #[doc = "The `id` field."] id: String },

    // ==================== Prepared Query Operations ====================
    /// Create a prepared query
    QueryCreate {
/// The `item` field.
        id: String,
        /// JSON-serialized PreparedQuery
        query_json: String,
    },

    /// Update a prepared query
    QueryUpdate {
/// The `item` field.
        id: String,
        /// JSON-serialized PreparedQuery
        query_json: String,
    },

    /// Delete a prepared query
    QueryDelete { #[doc = "The `id` field."] id: String },

    // ==================== Config Entry Operations ====================
    /// Apply (create or update) a config entry
    ConfigEntryApply {
        /// Composite key: "kind/name"
        key: String,
        /// JSON-serialized ConfigEntry
        entry_json: String,
    },

    /// Delete a config entry
    ConfigEntryDelete {
        /// Composite key: "kind/name"
        key: String,
    },

    // ==================== Connect CA Operations ====================
    /// Set a CA root certificate
    CARootSet {
/// The `item` field.
        id: String,
        /// JSON-serialized CARoot
        root_json: String,
    },

    /// Update CA configuration
    CAConfigUpdate {
        /// JSON-serialized CAConfig
        config_json: String,
    },

    /// Create or update an intention
    IntentionUpsert {
/// The `item` field.
        id: String,
        /// JSON-serialized Intention
        intention_json: String,
    },

    /// Delete an intention
    IntentionDelete { #[doc = "The `id` field."] id: String },

    /// Upsert intention by source/destination pair
    IntentionUpsertExact {
/// The `item` field.
        source: String,
/// The `item` field.
        destination: String,
        /// JSON-serialized Intention
        intention_json: String,
    },

    // ==================== Coordinate Operations ====================
    /// Update a node's network coordinate
    CoordinateBatchUpdate {
        /// Composite key: "node:segment"
        key: String,
        /// JSON-serialized CoordinateEntry
        entry_json: String,
    },

    // ==================== Peering Operations ====================
    /// Write a peering (create or update)
    PeeringWrite {
/// The `item` field.
        name: String,
        /// JSON-serialized Peering
        peering_json: String,
    },

    /// Delete a peering (soft delete with deleted_at)
    PeeringDelete { #[doc = "The `name` field."] name: String },

    // ==================== Operator Operations ====================
    /// Remove a Raft peer/server
    OperatorRemovePeer { #[doc = "The `server_key` field."] server_key: String },

    /// Update autopilot configuration
    OperatorAutopilotUpdate {
        /// JSON-serialized AutopilotConfiguration
        config_json: String,
    },

    // ==================== Namespace Operations ====================
    /// Create or update a namespace
    NamespaceUpsert {
/// The `item` field.
        name: String,
        /// JSON-serialized Namespace
        namespace_json: String,
    },

    /// Delete a namespace
    NamespaceDelete { #[doc = "The `name` field."] name: String },

    // ==================== Partition Operations ====================
    /// Create or update a partition
    PartitionUpsert {
/// The `item` field.
        name: String,
        /// JSON-serialized Partition
        partition_json: String,
    },

    /// Delete a partition (soft delete - sets DeletedAt)
    PartitionDelete { #[doc = "The `name` field."] name: String },

    // ==================== Catalog Operations ====================
    /// Register a service in the catalog
    CatalogRegister {
        /// NamingStore key: "namespace/service_name/service_id"
        key: String,
        /// JSON-serialized AgentServiceRegistration
        registration_json: String,
    },

    /// Deregister a service from the catalog
    CatalogDeregister {
        /// NamingStore key: "namespace/service_name/service_id"
        key: String,
    },

    // ==================== Health Check Operations ====================
    /// Persist a health check configuration (for restart recovery)
    HealthCheckRegister {
/// The `item` field.
        check_id: String,
        /// JSON-serialized InstanceCheckConfig
        config_json: String,
    },

    /// Remove a persisted health check configuration
    HealthCheckDeregister { #[doc = "The `check_id` field."] check_id: String },

    // ==================== Internal ====================
    /// No-operation command
    Noop,
}

impl ConsulRaftRequest {
/// The `op_type` method.
    pub fn op_type(&self) -> &'static str {
        match self {
            // KV
            Self::KVPut { .. } => "KVPut",
            Self::KVDelete { .. } => "KVDelete",
            Self::KVDeletePrefix { .. } => "KVDeletePrefix",
            Self::KVAcquireSession { .. } => "KVAcquireSession",
            Self::KVReleaseSessionKey { .. } => "KVReleaseSessionKey",
            Self::KVReleaseSession { .. } => "KVReleaseSession",
            Self::KVCas { .. } => "KVCas",
            Self::KVTransaction { .. } => "KVTransaction",
            // Session
            Self::SessionCreate { .. } => "SessionCreate",
            Self::SessionDestroy { .. } => "SessionDestroy",
            Self::SessionRenew { .. } => "SessionRenew",
            Self::SessionCleanupExpired { .. } => "SessionCleanupExpired",
            // ACL
            Self::ACLTokenSet { .. } => "ACLTokenSet",
            Self::ACLTokenDelete { .. } => "ACLTokenDelete",
            Self::ACLPolicySet { .. } => "ACLPolicySet",
            Self::ACLPolicyDelete { .. } => "ACLPolicyDelete",
            Self::ACLRoleSet { .. } => "ACLRoleSet",
            Self::ACLRoleDelete { .. } => "ACLRoleDelete",
            Self::ACLAuthMethodSet { .. } => "ACLAuthMethodSet",
            Self::ACLAuthMethodDelete { .. } => "ACLAuthMethodDelete",
            Self::ACLBootstrap { .. } => "ACLBootstrap",
            Self::ACLBindingRuleSet { .. } => "ACLBindingRuleSet",
            Self::ACLBindingRuleDelete { .. } => "ACLBindingRuleDelete",
            // Query
            Self::QueryCreate { .. } => "QueryCreate",
            Self::QueryUpdate { .. } => "QueryUpdate",
            Self::QueryDelete { .. } => "QueryDelete",
            // ConfigEntry
            Self::ConfigEntryApply { .. } => "ConfigEntryApply",
            Self::ConfigEntryDelete { .. } => "ConfigEntryDelete",
            // ConnectCA
            Self::CARootSet { .. } => "CARootSet",
            Self::CAConfigUpdate { .. } => "CAConfigUpdate",
            Self::IntentionUpsert { .. } => "IntentionUpsert",
            Self::IntentionDelete { .. } => "IntentionDelete",
            Self::IntentionUpsertExact { .. } => "IntentionUpsertExact",
            // Coordinate
            Self::CoordinateBatchUpdate { .. } => "CoordinateBatchUpdate",
            // Peering
            Self::PeeringWrite { .. } => "PeeringWrite",
            Self::PeeringDelete { .. } => "PeeringDelete",
            // Operator
            Self::OperatorRemovePeer { .. } => "OperatorRemovePeer",
            Self::OperatorAutopilotUpdate { .. } => "OperatorAutopilotUpdate",
            // Namespace
            Self::NamespaceUpsert { .. } => "NamespaceUpsert",
            Self::NamespaceDelete { .. } => "NamespaceDelete",
            // Partition
            Self::PartitionUpsert { .. } => "PartitionUpsert",
            Self::PartitionDelete { .. } => "PartitionDelete",
            // Catalog
            Self::CatalogRegister { .. } => "CatalogRegister",
            Self::CatalogDeregister { .. } => "CatalogDeregister",
            // HealthCheck
            Self::HealthCheckRegister { .. } => "HealthCheckRegister",
            Self::HealthCheckDeregister { .. } => "HealthCheckDeregister",
            // Internal
            Self::Noop => "Noop",
        }
    }
}

/// Response from a Consul Raft operation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsulRaftResponse {
/// The `success` field.
    pub success: bool,
/// The `data` field.
    pub data: Option<Vec<u8>>,
/// The `message` field.
    pub message: Option<String>,
}

impl ConsulRaftResponse {
/// The `success` associated function.
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
        }
    }

/// The `failure` associated function.
    pub fn failure(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_rule_raft_request_roundtrip() {
        let req = ConsulRaftRequest::ACLBindingRuleSet {
            id: "rule-123".into(),
            rule_json: r#"{"ID":"rule-123","AuthMethod":"kubernetes"}"#.into(),
        };
        let serialized = serde_json::to_vec(&req).unwrap();
        let deserialized: ConsulRaftRequest = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(deserialized.op_type(), "ACLBindingRuleSet");
    }

    #[test]
    fn test_binding_rule_delete_raft_request_roundtrip() {
        let req = ConsulRaftRequest::ACLBindingRuleDelete {
            id: "rule-456".into(),
        };
        let serialized = serde_json::to_vec(&req).unwrap();
        let deserialized: ConsulRaftRequest = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(deserialized.op_type(), "ACLBindingRuleDelete");
    }
}
