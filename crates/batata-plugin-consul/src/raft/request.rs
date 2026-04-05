use serde::{Deserialize, Serialize};

/// Consul-specific Raft request types.
///
/// These are applied by the Consul plugin handler and produce a dedicated
/// Raft log index space, independent of Nacos operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsulRaftRequest {
    // ==================== KV Operations ====================
    /// Put a key-value pair into the Consul KV store
    KVPut {
        key: String,
        /// JSON-serialized StoredKV
        stored_kv_json: String,
        /// Optional session index key to write (kidx:session_id:key)
        session_index_key: Option<String>,
    },

    /// Delete a key from the Consul KV store
    KVDelete {
        key: String,
        /// Optional session index key to clean up
        session_index_cleanup: Option<String>,
    },

    /// Delete all keys with a given prefix
    KVDeletePrefix { prefix: String },

    /// Acquire a session lock on a KV key
    KVAcquireSession {
        key: String,
        session_id: String,
        /// JSON-serialized StoredKV with session set
        stored_kv_json: String,
    },

    /// Release a session lock on a single KV key
    KVReleaseSessionKey {
        key: String,
        session_id: String,
        /// JSON-serialized StoredKV with session cleared
        stored_kv_json: String,
    },

    /// Release all KV keys held by a session (on session destroy)
    KVReleaseSession {
        session_id: String,
        /// Vec of (kv_key, updated_stored_kv_json) pairs
        updates: Vec<(String, String)>,
        /// Session index keys to delete
        index_keys_to_delete: Vec<String>,
    },

    /// Check-and-set: only update if modify_index matches
    KVCas {
        key: String,
        /// JSON-serialized StoredKV
        stored_kv_json: String,
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
        session_id: String,
        /// JSON-serialized StoredSession
        stored_session_json: String,
    },

    /// Destroy a Consul session
    SessionDestroy { session_id: String },

    /// Renew a Consul session
    SessionRenew {
        session_id: String,
        /// JSON-serialized StoredSession with renewed TTL
        stored_session_json: String,
    },

    /// Clean up expired sessions
    SessionCleanupExpired { expired_session_ids: Vec<String> },

    // ==================== ACL Operations ====================
    /// Create or update an ACL token
    ACLTokenSet {
        accessor_id: String,
        /// JSON-serialized AclToken
        token_json: String,
    },

    /// Delete an ACL token
    ACLTokenDelete { accessor_id: String },

    /// Create or update an ACL policy
    ACLPolicySet {
        id: String,
        /// JSON-serialized AclPolicy
        policy_json: String,
    },

    /// Delete an ACL policy
    ACLPolicyDelete { id: String },

    /// Create or update an ACL role
    ACLRoleSet {
        id: String,
        /// JSON-serialized AclRole
        role_json: String,
    },

    /// Delete an ACL role
    ACLRoleDelete { id: String },

    /// Create or update an ACL auth method
    ACLAuthMethodSet {
        name: String,
        /// JSON-serialized AuthMethod
        method_json: String,
    },

    /// Delete an ACL auth method
    ACLAuthMethodDelete { name: String },

    /// Bootstrap ACL (first token creation)
    ACLBootstrap {
        /// JSON-serialized AclToken
        token_json: String,
    },

    // ==================== Prepared Query Operations ====================
    /// Create a prepared query
    QueryCreate {
        id: String,
        /// JSON-serialized PreparedQuery
        query_json: String,
    },

    /// Update a prepared query
    QueryUpdate {
        id: String,
        /// JSON-serialized PreparedQuery
        query_json: String,
    },

    /// Delete a prepared query
    QueryDelete { id: String },

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
        id: String,
        /// JSON-serialized Intention
        intention_json: String,
    },

    /// Delete an intention
    IntentionDelete { id: String },

    /// Upsert intention by source/destination pair
    IntentionUpsertExact {
        source: String,
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
        name: String,
        /// JSON-serialized Peering
        peering_json: String,
    },

    /// Delete a peering (soft delete with deleted_at)
    PeeringDelete { name: String },

    // ==================== Operator Operations ====================
    /// Remove a Raft peer/server
    OperatorRemovePeer { server_key: String },

    /// Update autopilot configuration
    OperatorAutopilotUpdate {
        /// JSON-serialized AutopilotConfiguration
        config_json: String,
    },

    // ==================== Namespace Operations ====================
    /// Create or update a namespace
    NamespaceUpsert {
        name: String,
        /// JSON-serialized Namespace
        namespace_json: String,
    },

    /// Delete a namespace
    NamespaceDelete { name: String },

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

    // ==================== Internal ====================
    /// No-operation command
    Noop,
}

impl ConsulRaftRequest {
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
            // Catalog
            Self::CatalogRegister { .. } => "CatalogRegister",
            Self::CatalogDeregister { .. } => "CatalogDeregister",
            // Internal
            Self::Noop => "Noop",
        }
    }
}

/// Response from a Consul Raft operation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsulRaftResponse {
    pub success: bool,
    pub data: Option<Vec<u8>>,
    pub message: Option<String>,
}

impl ConsulRaftResponse {
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
        }
    }

    pub fn failure(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(msg),
        }
    }
}
