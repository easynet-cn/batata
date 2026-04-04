//! Consul KV domain models
//!
//! These models represent Consul's native KV store semantics.
//! NO Nacos concepts (no dataId, group, gray release, MD5 listener).

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};

// ============================================================================
// KV Pair
// ============================================================================

/// A Consul KV pair
///
/// Values are Base64-encoded in the API response.
/// Keys use flat path hierarchy (e.g., "config/app/database/url").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVPair {
    /// Key path
    #[serde(rename = "Key")]
    pub key: String,
    /// Raft index when this key was created
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    /// Raft index when this key was last modified
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
    /// Number of times the lock has been acquired
    #[serde(rename = "LockIndex")]
    pub lock_index: u64,
    /// User-defined flags (opaque to Consul)
    #[serde(rename = "Flags")]
    pub flags: u64,
    /// Base64-encoded value
    #[serde(rename = "Value")]
    pub value: Option<String>,
    /// Session ID holding the lock (if any)
    #[serde(rename = "Session")]
    pub session: Option<String>,
}

impl KVPair {
    /// Decode Base64 value to string
    pub fn decoded_value(&self) -> Option<String> {
        self.value.as_ref().and_then(|v| {
            BASE64
                .decode(v)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
        })
    }

    /// Decode Base64 value to raw bytes
    pub fn raw_value(&self) -> Option<Vec<u8>> {
        self.value.as_ref().and_then(|v| BASE64.decode(v).ok())
    }

    /// Create a KVPair with encoded value
    pub fn with_value(key: String, value: &[u8]) -> Self {
        Self {
            key,
            create_index: 0,
            modify_index: 0,
            lock_index: 0,
            flags: 0,
            value: Some(BASE64.encode(value)),
            session: None,
        }
    }

    /// Create a KVPair with string value
    pub fn with_string_value(key: String, value: &str) -> Self {
        Self::with_value(key, value.as_bytes())
    }
}

// ============================================================================
// Session
// ============================================================================

/// A Consul session
///
/// Sessions enable distributed locking and ephemeral key behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session unique identifier
    #[serde(rename = "ID")]
    pub id: String,
    /// Session name
    #[serde(rename = "Name")]
    pub name: String,
    /// Node this session is attached to
    #[serde(rename = "Node")]
    pub node: String,
    /// Health checks that keep this session alive
    #[serde(rename = "Checks", default)]
    pub checks: Vec<String>,
    /// Lock delay duration (e.g., "15s")
    #[serde(rename = "LockDelay")]
    pub lock_delay: String,
    /// Behavior when session is invalidated: "release" or "delete"
    #[serde(rename = "Behavior")]
    pub behavior: SessionBehavior,
    /// TTL for the session (e.g., "30s")
    #[serde(rename = "TTL")]
    pub ttl: String,
    /// Session creation index
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    /// Session modification index
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// What happens to locked keys when a session is invalidated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionBehavior {
    /// Release the lock (default)
    #[default]
    #[serde(rename = "release")]
    Release,
    /// Delete the locked key
    #[serde(rename = "delete")]
    Delete,
}

/// Session creation request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionRequest {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Node", default)]
    pub node: String,
    #[serde(rename = "Checks", default)]
    pub checks: Vec<String>,
    #[serde(rename = "LockDelay", default)]
    pub lock_delay: String,
    #[serde(rename = "Behavior", default)]
    pub behavior: SessionBehavior,
    #[serde(rename = "TTL", default)]
    pub ttl: String,
}

// ============================================================================
// Transaction
// ============================================================================

/// A KV transaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnOp {
    #[serde(rename = "KV")]
    pub kv: KVTxnOp,
}

/// A single KV transaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KVTxnOp {
    /// Operation verb: "set", "get", "delete", "cas", "delete-cas", "delete-tree"
    #[serde(rename = "Verb")]
    pub verb: TxnVerb,
    /// Key to operate on
    #[serde(rename = "Key")]
    pub key: String,
    /// Base64-encoded value (for set/cas)
    #[serde(rename = "Value", skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Flags (for set)
    #[serde(rename = "Flags", skip_serializing_if = "Option::is_none")]
    pub flags: Option<u64>,
    /// Index for CAS operations
    #[serde(rename = "Index", skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,
    /// Session for lock operations
    #[serde(rename = "Session", skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Transaction operation verbs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxnVerb {
    #[serde(rename = "set")]
    Set,
    #[serde(rename = "get")]
    Get,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "cas")]
    Cas,
    #[serde(rename = "delete-cas")]
    DeleteCas,
    #[serde(rename = "delete-tree")]
    DeleteTree,
    #[serde(rename = "lock")]
    Lock,
    #[serde(rename = "unlock")]
    Unlock,
    #[serde(rename = "check-index")]
    CheckIndex,
    #[serde(rename = "check-session")]
    CheckSession,
}

/// Transaction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnResult {
    #[serde(rename = "Results", skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<TxnResultItem>>,
    #[serde(rename = "Errors", skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<TxnError>>,
}

/// A single transaction result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnResultItem {
    #[serde(rename = "KV")]
    pub kv: KVPair,
}

/// A transaction error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxnError {
    #[serde(rename = "OpIndex")]
    pub op_index: u32,
    #[serde(rename = "What")]
    pub what: String,
}

// ============================================================================
// Query
// ============================================================================

/// KV query parameters
#[derive(Debug, Clone, Default)]
pub struct KVQueryParams {
    /// Datacenter
    pub dc: Option<String>,
    /// Return raw value (not JSON-wrapped)
    pub raw: bool,
    /// Return only keys (no values)
    pub keys: bool,
    /// Recursive prefix scan
    pub recurse: bool,
    /// Key separator for prefix listing
    pub separator: Option<String>,
    /// Namespace (enterprise)
    pub ns: Option<String>,
    /// CAS index for conditional operations
    pub cas: Option<u64>,
    /// Flags for write operations
    pub flags: Option<u64>,
    /// Session for lock/unlock operations
    pub acquire: Option<String>,
    /// Session to release
    pub release: Option<String>,
    /// Blocking query: wait index
    pub index: Option<u64>,
    /// Blocking query: max wait time
    pub wait: Option<String>,
}

/// Query response metadata
#[derive(Debug, Clone, Default)]
pub struct KVQueryMeta {
    /// Current Raft index
    pub last_index: u64,
    /// Whether the cluster has a known leader
    pub known_leader: bool,
}
