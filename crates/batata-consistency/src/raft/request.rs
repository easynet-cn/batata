//! Raft request and response types.
//!
//! These are the application-level commands that go through Raft consensus.

use serde::{Deserialize, Serialize};

/// Default value for `RaftRequest::UserCreate::source`, used when
/// deserializing log entries written before the `source` field existed.
pub(crate) fn default_user_source() -> String {
    "local".to_string()
}

/// Config delete history metadata embedded in ConfigRemove for atomic delete+history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigDeleteHistoryInfo {
    /// Raw config content captured for the delete history record.
    pub content: String,
    /// MD5 checksum of the captured content.
    pub md5: String,
    /// Owning application name.
    pub app_name: String,
    /// User that performed the delete.
    pub src_user: String,
    /// Source IP that performed the delete.
    pub src_ip: String,
    /// Extra info blob associated with the delete.
    pub ext_info: String,
    /// Encrypted data key for secure configs.
    pub encrypted_data_key: String,
}

/// Config history metadata embedded in ConfigPublish for atomic publish+history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigHistoryInfo {
    /// Operation type (e.g. `"I"`, `"U"`, `"D"`).
    pub op_type: String,
    /// Optional publish type (e.g. `"formal"`).
    pub publish_type: Option<String>,
    /// Optional extra info blob.
    pub ext_info: Option<String>,
}

/// Payload for `RaftRequest::ConfigPublish`. Boxed on the enum so the
/// `RaftRequest` discriminated union stays compact (largest variant was
/// ~400 bytes because of this 17-field struct).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigPublishPayload {
    /// Configuration data ID.
    pub data_id: String,
    /// Configuration group.
    pub group: String,
    /// Configuration tenant.
    pub tenant: String,
    /// Raw configuration content.
    pub content: String,
    /// MD5 checksum of the content.
    pub md5: String,
    /// Optional configuration type (e.g. `"properties"`).
    pub config_type: Option<String>,
    /// Optional owning application name.
    pub app_name: Option<String>,
    /// Optional tag.
    pub tag: Option<String>,
    /// Optional description.
    pub desc: Option<String>,
    /// Optional source user that published the config.
    pub src_user: Option<String>,
    /// Optional source IP that published the config.
    #[serde(default)]
    pub src_ip: Option<String>,
    /// Optional use field.
    #[serde(default)]
    pub r#use: Option<String>,
    /// Optional effect field.
    #[serde(default)]
    pub effect: Option<String>,
    /// Optional schema field.
    #[serde(default)]
    pub schema: Option<String>,
    /// Optional encrypted data key for secure configs.
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    /// Optional expected MD5 for a compare-and-swap publish.
    #[serde(default)]
    pub cas_md5: Option<String>,
    /// Optional config history metadata to insert atomically with the publish.
    #[serde(default)]
    pub history: Option<ConfigHistoryInfo>,
}

/// Payload for `RaftRequest::ConfigGrayPublish`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigGrayPublishPayload {
    /// Configuration data ID.
    pub data_id: String,
    /// Configuration group.
    pub group: String,
    /// Configuration tenant.
    pub tenant: String,
    /// Raw gray (beta) configuration content.
    pub content: String,
    /// Gray (beta) rule name.
    pub gray_name: String,
    /// Gray (beta) rule content.
    pub gray_rule: String,
    /// Optional owning application name.
    pub app_name: Option<String>,
    /// Optional encrypted data key for secure configs.
    pub encrypted_data_key: Option<String>,
    /// Optional source user that published the gray config.
    pub src_user: Option<String>,
    /// Optional source IP that published the gray config.
    pub src_ip: Option<String>,
    /// Optional expected MD5 for a compare-and-swap publish.
    pub cas_md5: Option<String>,
}

/// Payload for `RaftRequest::ConfigHistoryInsert`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigHistoryInsertPayload {
    /// History entry ID.
    pub id: i64,
    /// Configuration data ID.
    pub data_id: String,
    /// Configuration group.
    pub group: String,
    /// Configuration tenant.
    pub tenant: String,
    /// Raw configuration content at the time of the change.
    pub content: String,
    /// MD5 checksum of the content.
    pub md5: String,
    /// Optional owning application name.
    #[serde(default)]
    pub app_name: Option<String>,
    /// Optional source user that made the change.
    pub src_user: Option<String>,
    /// Optional source IP that made the change.
    pub src_ip: Option<String>,
    /// Operation type (e.g. `"I"`, `"U"`, `"D"`).
    pub op_type: String,
    /// Optional publish type (e.g. `"formal"`).
    #[serde(default)]
    pub publish_type: Option<String>,
    /// Optional gray (beta) rule name.
    #[serde(default)]
    pub gray_name: Option<String>,
    /// Optional extra info blob.
    #[serde(default)]
    pub ext_info: Option<String>,
    /// Optional encrypted data key for secure configs.
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    /// Epoch millis when the history entry was created.
    pub created_time: i64,
    /// Epoch millis when the history entry was last modified.
    pub last_modified_time: i64,
}

/// Payload for `RaftRequest::PersistentInstanceRegister`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentInstanceRegisterPayload {
    /// Namespace identifier the instance belongs to.
    pub namespace_id: String,
    /// Group name the instance belongs to.
    pub group_name: String,
    /// Service name the instance belongs to.
    pub service_name: String,
    /// Unique instance identifier.
    pub instance_id: String,
    /// Instance IP address.
    pub ip: String,
    /// Instance port.
    pub port: u16,
    /// Load-balancing weight.
    pub weight: f64,
    /// Whether the instance is currently healthy.
    pub healthy: bool,
    /// Whether the instance is currently enabled.
    pub enabled: bool,
    /// Pre-serialized JSON metadata blob for the instance.
    pub metadata: String,
    /// Cluster name the instance is registered under.
    pub cluster_name: String,
}

/// Payload for `RaftRequest::PersistentInstanceUpdate`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentInstanceUpdatePayload {
    /// Namespace identifier the instance belongs to.
    pub namespace_id: String,
    /// Group name the instance belongs to.
    pub group_name: String,
    /// Service name the instance belongs to.
    pub service_name: String,
    /// Unique instance identifier.
    pub instance_id: String,
    /// Optional new IP address.
    pub ip: Option<String>,
    /// Optional new port.
    pub port: Option<u16>,
    /// Optional new load-balancing weight.
    pub weight: Option<f64>,
    /// Optional new health flag.
    pub healthy: Option<bool>,
    /// Optional new enabled flag.
    pub enabled: Option<bool>,
    /// Optional new metadata blob.
    pub metadata: Option<String>,
}

/// All operations that go through Raft consensus.
///
/// Each variant represents a state machine command.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RaftRequest {
    // ==================== Config Operations ====================
    /// Publish or update a configuration (boxed to keep enum compact).
    ConfigPublish(Box<ConfigPublishPayload>),

    /// Remove a configuration
    ConfigRemove {
        /// Configuration data ID.
        data_id: String,
        /// Configuration group.
        group: String,
        /// Configuration tenant.
        tenant: String,
        /// Optional: insert delete history in the same Raft entry.
        /// Boxed so the ConfigRemove variant stays compact (history info is ~170B).
        #[serde(default)]
        history: Option<Box<ConfigDeleteHistoryInfo>>,
    },

    // ==================== Namespace Operations ====================
    /// Create a new namespace
    NamespaceCreate {
        /// Namespace identifier.
        namespace_id: String,
        /// Human-readable namespace name.
        namespace_name: String,
        /// Optional namespace description.
        namespace_desc: Option<String>,
    },

    /// Update an existing namespace
    NamespaceUpdate {
        /// Namespace identifier.
        namespace_id: String,
        /// Human-readable namespace name.
        namespace_name: String,
        /// Optional namespace description.
        namespace_desc: Option<String>,
    },

    /// Delete a namespace
    NamespaceDelete {
        /// Namespace identifier.
        namespace_id: String,
    },

    // ==================== User Operations ====================
    /// Create a new user
    UserCreate {
        /// Username.
        username: String,
        /// Hashed password.
        password_hash: String,
        /// Whether the account is enabled.
        enabled: bool,
        /// Identity provider for this account: "local", "oauth", or "ldap".
        /// Defaults to "local" when deserializing legacy log entries that
        /// predate this field.
        #[serde(default = "crate::raft::request::default_user_source")]
        source: String,
    },

    /// Update user information
    UserUpdate {
        /// Username.
        username: String,
        /// Optional new hashed password.
        password_hash: Option<String>,
        /// Optional new enabled flag.
        enabled: Option<bool>,
    },

    /// Delete a user
    UserDelete {
        /// Username.
        username: String,
    },

    // ==================== Role Operations ====================
    /// Create a new role
    RoleCreate {
        /// Granted role name.
        role: String,
        /// Username the role is granted to.
        username: String,
    },

    /// Delete a role assignment
    RoleDelete {
        /// Granted role name.
        role: String,
        /// Username the role is revoked from.
        username: String,
    },

    // ==================== Permission Operations ====================
    /// Grant a permission to a role
    PermissionGrant {
        /// Granted role name.
        role: String,
        /// Resource the permission applies to.
        resource: String,
        /// Action the permission allows.
        action: String,
    },

    /// Revoke a permission from a role
    PermissionRevoke {
        /// Granted role name.
        role: String,
        /// Resource the permission applies to.
        resource: String,
        /// Action the permission allows.
        action: String,
    },

    // ==================== Config Gray (Beta) Operations ====================
    /// Publish or update a gray (beta) config (boxed).
    ConfigGrayPublish(Box<ConfigGrayPublishPayload>),

    /// Remove gray (beta) configs for a data_id/group/tenant (optionally by gray_name)
    ConfigGrayRemove {
        /// Configuration data ID.
        data_id: String,
        /// Configuration group.
        group: String,
        /// Configuration tenant.
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
        /// Configuration data ID.
        data_id: String,
        /// Configuration group.
        group: String,
        /// Configuration tenant.
        tenant: String,
        /// Tag value to set.
        tag: String,
        /// Optional source IP that updated the tags.
        tag_src_ip: Option<String>,
        /// Optional source user that updated the tags.
        tag_src_user: Option<String>,
    },

    /// Delete config tags
    ConfigTagsDelete {
        /// Configuration data ID.
        data_id: String,
        /// Configuration group.
        group: String,
        /// Configuration tenant.
        tenant: String,
        /// Tag value to delete.
        tag: String,
    },

    // ==================== Service/Instance Operations (for persistent instances) ====================
    /// Register a persistent service instance (boxed).
    PersistentInstanceRegister(Box<PersistentInstanceRegisterPayload>),

    /// Deregister a persistent service instance
    PersistentInstanceDeregister {
        /// Namespace identifier the instance belongs to.
        namespace_id: String,
        /// Group name the instance belongs to.
        group_name: String,
        /// Service name the instance belongs to.
        service_name: String,
        /// Unique instance identifier.
        instance_id: String,
    },

    /// Update a persistent service instance (boxed).
    PersistentInstanceUpdate(Box<PersistentInstanceUpdatePayload>),

    // ==================== Distributed Lock Operations (ADV-005) ====================
    /// Acquire a distributed lock
    LockAcquire {
        /// Lock namespace.
        namespace: String,
        /// Lock name.
        name: String,
        /// Owner (client ID) acquiring the lock.
        owner: String,
        /// Time-to-live in milliseconds.
        ttl_ms: u64,
        /// Fence token for the acquiring owner.
        fence_token: u64,
        /// Optional owner metadata.
        owner_metadata: Option<String>,
    },

    /// Release a distributed lock
    LockRelease {
        /// Lock namespace.
        namespace: String,
        /// Lock name.
        name: String,
        /// Owner (client ID) releasing the lock.
        owner: String,
        /// Optional expected fence token for fencing.
        fence_token: Option<u64>,
    },

    /// Renew a distributed lock
    LockRenew {
        /// Lock namespace.
        namespace: String,
        /// Lock name.
        name: String,
        /// Owner (client ID) renewing the lock.
        owner: String,
        /// Optional new TTL in milliseconds.
        ttl_ms: Option<u64>,
    },

    /// Force release a distributed lock (admin operation)
    LockForceRelease {
        /// Lock namespace.
        namespace: String,
        /// Lock name.
        name: String,
    },

    /// Expire a lock (internal operation)
    LockExpire {
        /// Lock namespace.
        namespace: String,
        /// Lock name.
        name: String,
    },

    // ==================== Health Check Status Operations ====================
    /// Replicate an active health-check probe result so every cluster node
    /// agrees on the latest status. Only the leader writes; followers apply
    /// the entry to their local `InstanceCheckRegistry`. See
    /// `project_health_status_raft_sync_plan.md`.
    HealthCheckStatusUpdate {
        /// Composite check key identifying the probe.
        check_key: String,
        /// "passing" / "warning" / "critical" — keep as a string here so the
        /// raft crate doesn't have to depend on batata-naming's CheckStatus.
        status: String,
        /// Probe output text.
        output: String,
        /// Probe response time in milliseconds.
        response_time_ms: u64,
        /// Epoch millis of the probe.
        timestamp_ms: i64,
    },

    /// TTL-style status update (no consecutive-success/failure thresholding).
    /// Used by Consul session-bound checks (`/v1/agent/check/pass|fail|warn`).
    HealthCheckTtlUpdate {
        /// Composite check key identifying the probe.
        check_key: String,
        /// Probe status string ("passing" / "warning" / "critical").
        status: String,
        /// `None` keeps the existing output; `Some` replaces it.
        output: Option<String>,
        /// Epoch millis of the update.
        timestamp_ms: i64,
    },

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
            RaftRequest::HealthCheckStatusUpdate { .. } => "HealthCheckStatusUpdate",
            RaftRequest::HealthCheckTtlUpdate { .. } => "HealthCheckTtlUpdate",
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
