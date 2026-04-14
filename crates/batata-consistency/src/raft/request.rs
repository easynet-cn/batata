// Raft request and response types
// These are the application-level commands that go through Raft consensus

use serde::{Deserialize, Serialize};

/// Default value for `RaftRequest::UserCreate::source`, used when
/// deserializing log entries written before the `source` field existed.
pub(crate) fn default_user_source() -> String {
    "local".to_string()
}

/// Config delete history metadata embedded in ConfigRemove for atomic delete+history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigDeleteHistoryInfo {
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub src_user: String,
    pub src_ip: String,
    pub ext_info: String,
    pub encrypted_data_key: String,
}

/// Config history metadata embedded in ConfigPublish for atomic publish+history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigHistoryInfo {
    pub op_type: String,
    pub publish_type: Option<String>,
    pub ext_info: Option<String>,
}

/// Payload for `RaftRequest::ConfigPublish`. Boxed on the enum so the
/// `RaftRequest` discriminated union stays compact (largest variant was
/// ~400 bytes because of this 17-field struct).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigPublishPayload {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub config_type: Option<String>,
    pub app_name: Option<String>,
    pub tag: Option<String>,
    pub desc: Option<String>,
    pub src_user: Option<String>,
    #[serde(default)]
    pub src_ip: Option<String>,
    #[serde(default)]
    pub r#use: Option<String>,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    #[serde(default)]
    pub cas_md5: Option<String>,
    #[serde(default)]
    pub history: Option<ConfigHistoryInfo>,
}

/// Payload for `RaftRequest::ConfigGrayPublish`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigGrayPublishPayload {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub app_name: Option<String>,
    pub encrypted_data_key: Option<String>,
    pub src_user: Option<String>,
    pub src_ip: Option<String>,
    pub cas_md5: Option<String>,
}

/// Payload for `RaftRequest::ConfigHistoryInsert`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigHistoryInsertPayload {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    #[serde(default)]
    pub app_name: Option<String>,
    pub src_user: Option<String>,
    pub src_ip: Option<String>,
    pub op_type: String,
    #[serde(default)]
    pub publish_type: Option<String>,
    #[serde(default)]
    pub gray_name: Option<String>,
    #[serde(default)]
    pub ext_info: Option<String>,
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    pub created_time: i64,
    pub last_modified_time: i64,
}

/// Payload for `RaftRequest::PersistentInstanceRegister`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentInstanceRegisterPayload {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub instance_id: String,
    pub ip: String,
    pub port: u16,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub metadata: String,
    pub cluster_name: String,
}

/// Payload for `RaftRequest::PersistentInstanceUpdate`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentInstanceUpdatePayload {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub instance_id: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub weight: Option<f64>,
    pub healthy: Option<bool>,
    pub enabled: Option<bool>,
    pub metadata: Option<String>,
}

/// All operations that go through Raft consensus
/// Each variant represents a state machine command
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RaftRequest {
    // ==================== Config Operations ====================
    /// Publish or update a configuration (boxed to keep enum compact).
    ConfigPublish(Box<ConfigPublishPayload>),

    /// Remove a configuration
    ConfigRemove {
        data_id: String,
        group: String,
        tenant: String,
        /// Optional: insert delete history in the same Raft entry.
        /// Boxed so the ConfigRemove variant stays compact (history info is ~170B).
        #[serde(default)]
        history: Option<Box<ConfigDeleteHistoryInfo>>,
    },

    // ==================== Namespace Operations ====================
    /// Create a new namespace
    NamespaceCreate {
        namespace_id: String,
        namespace_name: String,
        namespace_desc: Option<String>,
    },

    /// Update an existing namespace
    NamespaceUpdate {
        namespace_id: String,
        namespace_name: String,
        namespace_desc: Option<String>,
    },

    /// Delete a namespace
    NamespaceDelete { namespace_id: String },

    // ==================== User Operations ====================
    /// Create a new user
    UserCreate {
        username: String,
        password_hash: String,
        enabled: bool,
        /// Identity provider for this account: "local", "oauth", or "ldap".
        /// Defaults to "local" when deserializing legacy log entries that
        /// predate this field.
        #[serde(default = "crate::raft::request::default_user_source")]
        source: String,
    },

    /// Update user information
    UserUpdate {
        username: String,
        password_hash: Option<String>,
        enabled: Option<bool>,
    },

    /// Delete a user
    UserDelete { username: String },

    // ==================== Role Operations ====================
    /// Create a new role
    RoleCreate { role: String, username: String },

    /// Delete a role assignment
    RoleDelete { role: String, username: String },

    // ==================== Permission Operations ====================
    /// Grant a permission to a role
    PermissionGrant {
        role: String,
        resource: String,
        action: String,
    },

    /// Revoke a permission from a role
    PermissionRevoke {
        role: String,
        resource: String,
        action: String,
    },

    // ==================== Config Gray (Beta) Operations ====================
    /// Publish or update a gray (beta) config (boxed).
    ConfigGrayPublish(Box<ConfigGrayPublishPayload>),

    /// Remove gray (beta) configs for a data_id/group/tenant (optionally by gray_name)
    ConfigGrayRemove {
        data_id: String,
        group: String,
        tenant: String,
        /// If non-empty, only delete the specific gray config with this name
        #[serde(default)]
        gray_name: String,
    },

    // ==================== Config History Operations ====================
    /// Record config change history (boxed).
    ConfigHistoryInsert(Box<ConfigHistoryInsertPayload>),

    // ==================== Config Tags Operations ====================
    /// Create or update config tags
    ConfigTagsUpdate {
        data_id: String,
        group: String,
        tenant: String,
        tag: String,
        tag_src_ip: Option<String>,
        tag_src_user: Option<String>,
    },

    /// Delete config tags
    ConfigTagsDelete {
        data_id: String,
        group: String,
        tenant: String,
        tag: String,
    },

    // ==================== Service/Instance Operations (for persistent instances) ====================
    /// Register a persistent service instance (boxed).
    PersistentInstanceRegister(Box<PersistentInstanceRegisterPayload>),

    /// Deregister a persistent service instance
    PersistentInstanceDeregister {
        namespace_id: String,
        group_name: String,
        service_name: String,
        instance_id: String,
    },

    /// Update a persistent service instance (boxed).
    PersistentInstanceUpdate(Box<PersistentInstanceUpdatePayload>),

    // ==================== Distributed Lock Operations (ADV-005) ====================
    /// Acquire a distributed lock
    LockAcquire {
        namespace: String,
        name: String,
        owner: String,
        ttl_ms: u64,
        fence_token: u64,
        owner_metadata: Option<String>,
    },

    /// Release a distributed lock
    LockRelease {
        namespace: String,
        name: String,
        owner: String,
        fence_token: Option<u64>,
    },

    /// Renew a distributed lock
    LockRenew {
        namespace: String,
        name: String,
        owner: String,
        ttl_ms: Option<u64>,
    },

    /// Force release a distributed lock (admin operation)
    LockForceRelease { namespace: String, name: String },

    /// Expire a lock (internal operation)
    LockExpire { namespace: String, name: String },

    // ==================== Plugin Extension Point ====================
    /// Generic plugin write operation routed through Raft consensus.
    /// Plugins register their own apply handlers; the core Raft does not
    /// interpret the payload — it only ensures consensus and log ordering.
    PluginWrite {
        /// Plugin identifier (e.g., "consul", "etcd")
        plugin_id: String,
        /// Operation type within the plugin (e.g., "kv_put", "session_create")
        op_type: String,
        /// Serialized plugin-specific request data
        payload: Vec<u8>,
    },

    // ==================== No-op for membership changes ====================
    /// No-operation command, used internally
    Noop,
}

impl RaftRequest {
    /// Get the operation type as a string for logging
    pub fn op_type(&self) -> &'static str {
        match self {
            RaftRequest::ConfigPublish { .. } => "ConfigPublish",
            RaftRequest::ConfigRemove { .. } => "ConfigRemove",
            RaftRequest::NamespaceCreate { .. } => "NamespaceCreate",
            RaftRequest::NamespaceUpdate { .. } => "NamespaceUpdate",
            RaftRequest::NamespaceDelete { .. } => "NamespaceDelete",
            RaftRequest::UserCreate { .. } => "UserCreate",
            RaftRequest::UserUpdate { .. } => "UserUpdate",
            RaftRequest::UserDelete { .. } => "UserDelete",
            RaftRequest::RoleCreate { .. } => "RoleCreate",
            RaftRequest::RoleDelete { .. } => "RoleDelete",
            RaftRequest::PermissionGrant { .. } => "PermissionGrant",
            RaftRequest::PermissionRevoke { .. } => "PermissionRevoke",
            RaftRequest::ConfigGrayPublish { .. } => "ConfigGrayPublish",
            RaftRequest::ConfigGrayRemove { .. } => "ConfigGrayRemove",
            RaftRequest::ConfigHistoryInsert { .. } => "ConfigHistoryInsert",
            RaftRequest::ConfigTagsUpdate { .. } => "ConfigTagsUpdate",
            RaftRequest::ConfigTagsDelete { .. } => "ConfigTagsDelete",
            RaftRequest::PersistentInstanceRegister { .. } => "PersistentInstanceRegister",
            RaftRequest::PersistentInstanceDeregister { .. } => "PersistentInstanceDeregister",
            RaftRequest::PersistentInstanceUpdate { .. } => "PersistentInstanceUpdate",
            RaftRequest::LockAcquire { .. } => "LockAcquire",
            RaftRequest::LockRelease { .. } => "LockRelease",
            RaftRequest::LockRenew { .. } => "LockRenew",
            RaftRequest::LockForceRelease { .. } => "LockForceRelease",
            RaftRequest::LockExpire { .. } => "LockExpire",
            RaftRequest::PluginWrite {
                plugin_id, op_type, ..
            } => {
                // Return a static str for known plugins, fallback for unknown
                match (plugin_id.as_str(), op_type.as_str()) {
                    ("consul", op) => match op {
                        "kv_put" => "ConsulKVPut",
                        "kv_delete" => "ConsulKVDelete",
                        "session_create" => "ConsulSessionCreate",
                        _ => "PluginWrite",
                    },
                    _ => "PluginWrite",
                }
            }
            RaftRequest::Noop => "Noop",
        }
    }
}

/// Response from Raft consensus operations
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RaftResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Response data (serialized if needed)
    pub data: Option<Vec<u8>>,
    /// Error or status message
    pub message: Option<String>,
}

impl RaftResponse {
    /// Create a successful response
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            message: None,
        }
    }

    /// Create a successful response with data
    pub fn success_with_data(data: Vec<u8>) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    /// Create a failed response with message
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md5::Digest;

    #[test]
    fn test_raft_request_serialization() {
        let req = RaftRequest::ConfigPublish(Box::new(ConfigPublishPayload {
            data_id: "test-config".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "".to_string(),
            content: "key=value".to_string(),
            md5: const_hex::encode(md5::Md5::digest("key=value")),
            config_type: Some("properties".to_string()),
            app_name: None,
            tag: None,
            desc: None,
            src_user: None,
            src_ip: None,
            r#use: None,
            effect: None,
            schema: None,
            encrypted_data_key: None,
            cas_md5: None,
            history: None,
        }));

        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: RaftRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(req.op_type(), deserialized.op_type());
    }

    #[test]
    fn test_raft_request_enum_size_is_compact() {
        // After boxing the large variants (ConfigPublish, ConfigGrayPublish,
        // ConfigHistoryInsert, PersistentInstanceRegister, PersistentInstanceUpdate,
        // and ConfigRemove's history), the enum should be ~144 bytes
        // (discriminant + largest remaining variant, currently ConfigTagsUpdate
        // at ~144B). Before boxing, ConfigPublish alone made it ~480 bytes.
        // This test fails loudly if someone adds a new large variant without
        // boxing it, or un-boxes an already-boxed one.
        let size = std::mem::size_of::<RaftRequest>();
        assert!(
            size <= 160,
            "RaftRequest enum grew to {} bytes — did a large variant get un-boxed?",
            size
        );
    }

    #[test]
    fn test_raft_response() {
        let success = RaftResponse::success();
        assert!(success.success);

        let failure = RaftResponse::failure("test error");
        assert!(!failure.success);
        assert_eq!(failure.message, Some("test error".to_string()));
    }
}
