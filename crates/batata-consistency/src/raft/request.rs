// Raft request and response types
// These are the application-level commands that go through Raft consensus

use serde::{Deserialize, Serialize};

/// All operations that go through Raft consensus
/// Each variant represents a state machine command
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RaftRequest {
    // ==================== Config Operations ====================
    /// Publish or update a configuration
    ConfigPublish {
        data_id: String,
        group: String,
        tenant: String,
        content: String,
        config_type: Option<String>,
        app_name: Option<String>,
        tag: Option<String>,
        desc: Option<String>,
        src_user: Option<String>,
    },

    /// Remove a configuration
    ConfigRemove {
        data_id: String,
        group: String,
        tenant: String,
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

    // ==================== Config History Operations ====================
    /// Record config change history
    ConfigHistoryInsert {
        id: i64,
        data_id: String,
        group: String,
        tenant: String,
        content: String,
        md5: String,
        src_user: Option<String>,
        src_ip: Option<String>,
        op_type: String,
        created_time: i64,
        last_modified_time: i64,
    },

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
    /// Register a persistent service instance
    PersistentInstanceRegister {
        namespace_id: String,
        group_name: String,
        service_name: String,
        instance_id: String,
        ip: String,
        port: u16,
        weight: f64,
        healthy: bool,
        enabled: bool,
        metadata: String, // JSON serialized metadata
        cluster_name: String,
    },

    /// Deregister a persistent service instance
    PersistentInstanceDeregister {
        namespace_id: String,
        group_name: String,
        service_name: String,
        instance_id: String,
    },

    /// Update a persistent service instance
    PersistentInstanceUpdate {
        namespace_id: String,
        group_name: String,
        service_name: String,
        instance_id: String,
        ip: Option<String>,
        port: Option<u16>,
        weight: Option<f64>,
        healthy: Option<bool>,
        enabled: Option<bool>,
        metadata: Option<String>,
    },

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

    #[test]
    fn test_raft_request_serialization() {
        let req = RaftRequest::ConfigPublish {
            data_id: "test-config".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "".to_string(),
            content: "key=value".to_string(),
            config_type: Some("properties".to_string()),
            app_name: None,
            tag: None,
            desc: None,
            src_user: None,
        };

        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: RaftRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(req.op_type(), deserialized.op_type());
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
