//! Typed records stored in the Raft state machine's RocksDB column families.
//!
//! Each `Stored*` struct corresponds to the bincode-serialized value for a
//! specific CF. Split out of `mod.rs` purely for file-size / review hygiene —
//! no behavioral change.

/// Typed persistent instance record stored in `CF_INSTANCES`.
///
/// Serialized with `bincode` instead of `serde_json` to avoid the
/// per-apply 1.5µs decode tax measured in `raft_serialization_bench`.
/// `metadata` stays as a pre-serialized JSON blob so this struct can be
/// emitted without a second pass over the user's key/value pairs — the
/// hook consumers parse the metadata JSON themselves if they need it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredInstance {
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
    /// Epoch millis when the instance was registered.
    pub registered_time: i64,
    /// Epoch millis when the instance was last modified.
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed config record stored in `CF_CONFIG`.
///
/// Replaces the previous `serde_json::Value` dynamic dispatch which
/// dominated `apply_config_publish` CPU cost. Reader paths decode this
/// then re-emit as `serde_json::Value` for backward-compatible API.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfig {
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
    /// Optional configuration type (e.g. "properties").
    #[serde(default)]
    pub config_type: Option<String>,
    /// Optional owning application name.
    #[serde(default)]
    pub app_name: Option<String>,
    /// Optional comma-separated tags.
    #[serde(default)]
    pub config_tags: Option<String>,
    /// Optional description.
    #[serde(default)]
    pub desc: Option<String>,
    /// Optional use field.
    #[serde(default, rename = "use")]
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
    /// Optional source user that published the config.
    #[serde(default)]
    pub src_user: Option<String>,
    /// Optional source IP that published the config.
    #[serde(default)]
    pub src_ip: Option<String>,
    /// Epoch millis when the config was created.
    pub created_time: i64,
    /// Epoch millis when the config was last modified.
    pub modified_time: i64,
}

/// Typed config history record stored in `CF_CONFIG_HISTORY`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfigHistory {
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
    /// Owning application name.
    pub app_name: String,
    /// Optional source user that made the change.
    #[serde(default)]
    pub src_user: Option<String>,
    /// Optional source IP that made the change.
    #[serde(default)]
    pub src_ip: Option<String>,
    /// Operation type (e.g. "I", "U", "D").
    pub op_type: String,
    /// Publish type (e.g. "formal").
    pub publish_type: String,
    /// Gray (beta) rule name, if applicable.
    pub gray_name: String,
    /// Extra info blob.
    pub ext_info: String,
    /// Encrypted data key for secure configs.
    pub encrypted_data_key: String,
    /// Epoch millis when the history entry was created.
    pub created_time: i64,
    /// Epoch millis when the history entry was last modified.
    pub modified_time: i64,
}

/// Typed gray config record stored in `CF_CONFIG_GRAY`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfigGray {
    /// Configuration data ID.
    pub data_id: String,
    /// Configuration group.
    pub group: String,
    /// Configuration tenant.
    pub tenant: String,
    /// Raw gray configuration content.
    pub content: String,
    /// MD5 checksum of the content.
    pub md5: String,
    /// Owning application name.
    pub app_name: String,
    /// Gray (beta) rule name.
    pub gray_name: String,
    /// Gray (beta) rule content.
    pub gray_rule: String,
    /// Encrypted data key for secure configs.
    pub encrypted_data_key: String,
    /// Source user that published the gray config.
    pub src_user: String,
    /// Source IP that published the gray config.
    pub src_ip: String,
    /// Epoch millis when the gray config was created.
    pub created_time: i64,
    /// Epoch millis when the gray config was last modified.
    pub modified_time: i64,
}

/// Typed value for CF_NAMESPACE entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredNamespace {
    /// Namespace identifier.
    pub namespace_id: String,
    /// Human-readable namespace name.
    pub namespace_name: String,
    /// Optional namespace description.
    #[serde(default)]
    pub namespace_desc: Option<String>,
    /// Epoch millis when the namespace was created.
    #[serde(default)]
    pub created_time: i64,
    /// Epoch millis when the namespace was last modified.
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed value for CF_USERS entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredUser {
    /// Username.
    pub username: String,
    /// Hashed password.
    pub password_hash: String,
    /// Whether the account is enabled.
    pub enabled: bool,
    /// Epoch millis when the user was created.
    #[serde(default)]
    pub created_time: i64,
    /// Epoch millis when the user was last modified.
    #[serde(default)]
    pub modified_time: i64,
    /// Identity source: "local" (default), "oauth", or "ldap".
    /// Defaults to empty for backwards compatibility with rows persisted
    /// before this field existed; readers normalize empty to "local".
    #[serde(default)]
    pub source: String,
}

/// Typed value for CF_ROLES entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredRole {
    /// Granted role name.
    pub role: String,
    /// Username the role is granted to.
    pub username: String,
    /// Epoch millis when the role assignment was created.
    #[serde(default)]
    pub created_time: i64,
}

/// Typed value for CF_PERMISSIONS entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredPermission {
    /// Granted role name.
    pub role: String,
    /// Resource the permission applies to.
    pub resource: String,
    /// Action the permission allows (e.g. "rw").
    pub action: String,
    /// Epoch millis when the permission was created.
    #[serde(default)]
    pub created_time: i64,
}
