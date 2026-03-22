use serde::{Deserialize, Serialize};

/// Consul-specific Raft request types.
///
/// These are applied by the Consul state machine and produce a dedicated
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

    // ==================== Internal ====================
    /// No-operation command
    Noop,
}

impl ConsulRaftRequest {
    pub fn op_type(&self) -> &'static str {
        match self {
            Self::KVPut { .. } => "KVPut",
            Self::KVDelete { .. } => "KVDelete",
            Self::KVDeletePrefix { .. } => "KVDeletePrefix",
            Self::KVAcquireSession { .. } => "KVAcquireSession",
            Self::KVReleaseSessionKey { .. } => "KVReleaseSessionKey",
            Self::KVReleaseSession { .. } => "KVReleaseSession",
            Self::KVCas { .. } => "KVCas",
            Self::KVTransaction { .. } => "KVTransaction",
            Self::SessionCreate { .. } => "SessionCreate",
            Self::SessionDestroy { .. } => "SessionDestroy",
            Self::SessionRenew { .. } => "SessionRenew",
            Self::SessionCleanupExpired { .. } => "SessionCleanupExpired",
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
