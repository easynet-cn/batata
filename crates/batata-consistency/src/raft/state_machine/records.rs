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
    pub registered_time: i64,
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
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    #[serde(default)]
    pub config_type: Option<String>,
    #[serde(default)]
    pub app_name: Option<String>,
    #[serde(default)]
    pub config_tags: Option<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default, rename = "use")]
    pub r#use: Option<String>,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    #[serde(default)]
    pub src_user: Option<String>,
    #[serde(default)]
    pub src_ip: Option<String>,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Typed config history record stored in `CF_CONFIG_HISTORY`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfigHistory {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    #[serde(default)]
    pub src_user: Option<String>,
    #[serde(default)]
    pub src_ip: Option<String>,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub encrypted_data_key: String,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Typed gray config record stored in `CF_CONFIG_GRAY`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfigGray {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub encrypted_data_key: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Typed value for CF_NAMESPACE entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredNamespace {
    pub namespace_id: String,
    pub namespace_name: String,
    #[serde(default)]
    pub namespace_desc: Option<String>,
    #[serde(default)]
    pub created_time: i64,
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed value for CF_USERS entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredUser {
    pub username: String,
    pub password_hash: String,
    pub enabled: bool,
    #[serde(default)]
    pub created_time: i64,
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed value for CF_ROLES entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredRole {
    pub role: String,
    pub username: String,
    #[serde(default)]
    pub created_time: i64,
}

/// Typed value for CF_PERMISSIONS entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredPermission {
    pub role: String,
    pub resource: String,
    pub action: String,
    #[serde(default)]
    pub created_time: i64,
}
