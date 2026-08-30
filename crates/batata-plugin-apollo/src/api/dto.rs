use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `AppDTO` entity.
pub struct AppDTO {
    /// The `app_id` field.
    pub app_id: String,
    /// The `name` field.
    pub name: String,
    /// The `org_id` field.
    pub org_id: String,
    /// The `org_name` field.
    pub org_name: String,
    /// The `owner_name` field.
    pub owner_name: String,
    /// The `owner_email` field.
    pub owner_email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `NamespaceDTO` entity.
pub struct NamespaceDTO {
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `format` field.
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `is_public` field.
    pub is_public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `comment` field.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ItemDTO` entity.
pub struct ItemDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `key` field.
    pub key: String,
    /// The `value` field.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `type` field.
    pub r#type: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `comment` field.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `line_num` field.
    pub line_num: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ReleaseDTO` entity.
pub struct ReleaseDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `release_key` field.
    pub release_key: String,
    /// The `name` field.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `comment` field.
    pub comment: Option<String>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `configurations` field.
    pub configurations: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `release_id` field.
    pub release_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `is_abandoned` field.
    pub is_abandoned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ApolloConfig` entity.
pub struct ApolloConfig {
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster` field.
    pub cluster: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `release_key` field.
    pub release_key: String,
    /// The `configurations` field.
    pub configurations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `NotificationDTO` entity.
pub struct NotificationDTO {
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `notification_id` field.
    pub notification_id: i64,
    /// The `messages` field.
    pub messages: NotificationMessageDTO,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `NotificationMessageDTO` entity.
pub struct NotificationMessageDTO {
    /// The `details` field.
    pub details: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `NotificationRequestDTO` entity.
pub struct NotificationRequestDTO {
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `notification_id` field.
    pub notification_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents the `ErrorResponse` entity.
pub struct ErrorResponse {
    /// The `status` field.
    pub status: i32,
    /// The `message` field.
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ClusterDTO` entity.
pub struct ClusterDTO {
    /// The `name` field.
    pub name: String,
    /// The `app_id` field.
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `parent_cluster_id` field.
    pub parent_cluster_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `comment` field.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `CommitDTO` entity.
pub struct CommitDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `change_sets` field.
    pub change_sets: String,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `comment` field.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `GrayReleaseRuleDTO` entity.
pub struct GrayReleaseRuleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `branch_name` field.
    pub branch_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `rules` field.
    pub rules: Option<String>,
    /// The `release_id` field.
    pub release_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `branch_status` field.
    pub branch_status: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `priority` field.
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `InstanceDTO` entity.
pub struct InstanceDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `data_center` field.
    pub data_center: String,
    /// The `ip` field.
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ServerConfigDTO` entity.
pub struct ServerConfigDTO {
    /// The `key` field.
    pub key: String,
    /// The `value` field.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `comment` field.
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `AccessKeyDTO` entity.
pub struct AccessKeyDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `secret` field.
    pub secret: String,
    /// The `mode` field.
    pub mode: i16,
    /// The `is_enabled` field.
    pub is_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ReleaseHistoryDTO` entity.
pub struct ReleaseHistoryDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `branch_name` field.
    pub branch_name: String,
    /// The `release_id` field.
    pub release_id: i32,
    /// The `previous_release_id` field.
    pub previous_release_id: i32,
    /// The `operation` field.
    pub operation: i16,
    /// The `operation_context` field.
    pub operation_context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `AppNamespaceDTO` entity.
pub struct AppNamespaceDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `name` field.
    pub name: String,
    /// The `app_id` field.
    pub app_id: String,
    /// The `format` field.
    pub format: String,
    /// The `is_public` field.
    pub is_public: bool,
    /// The `comment` field.
    pub comment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ItemChangeSets` entity.
pub struct ItemChangeSets {
    /// The `create_items` field.
    pub create_items: Vec<ItemDTO>,
    /// The `update_items` field.
    pub update_items: Vec<ItemDTO>,
    /// The `delete_items` field.
    pub delete_items: Vec<ItemDTO>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `InstanceConfigDTO` entity.
pub struct InstanceConfigDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `instance_id` field.
    pub instance_id: i32,
    #[serde(default)]
    /// The `config_app_id` field.
    pub config_app_id: Option<String>,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `release_key` field.
    pub release_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `configurations` field.
    pub configurations: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `AuditDTO` entity.
pub struct AuditDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `audit_key` field.
    pub audit_key: String,
    /// The `entity_name` field.
    pub entity_name: String,
    /// The `entity_id` field.
    pub entity_id: String,
    /// The `op_name` field.
    pub op_name: String,
    /// The `op_time` field.
    pub op_time: String,
    /// The `op_by` field.
    pub op_by: String,
    /// The `op_client_ip` field.
    pub op_client_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `detail` field.
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ConsumerDTO` entity.
pub struct ConsumerDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `name` field.
    pub name: String,
    /// The `org_id` field.
    pub org_id: String,
    /// The `org_name` field.
    pub org_name: String,
    /// The `owner_name` field.
    pub owner_name: String,
    /// The `owner_email` field.
    pub owner_email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ConsumerTokenDTO` entity.
pub struct ConsumerTokenDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `consumer_id` field.
    pub consumer_id: i32,
    /// The `token` field.
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `PermissionDTO` entity.
pub struct PermissionDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `permission_type` field.
    pub permission_type: i32,
    /// The `target_id` field.
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `RoleDTO` entity.
pub struct RoleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `role_name` field.
    pub role_name: String,
    /// The `role_type` field.
    pub role_type: i32,
    /// The `target_id` field.
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `UserRoleDTO` entity.
pub struct UserRoleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `user_id` field.
    pub user_id: String,
    /// The `role_id` field.
    pub role_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `FavoriteDTO` entity.
pub struct FavoriteDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i32>,
    /// The `user_id` field.
    pub user_id: String,
    /// The `app_id` field.
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ConfigExportDTO` entity.
pub struct ConfigExportDTO {
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `items` field.
    pub items: Vec<ItemDTO>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `ConfigImportDTO` entity.
pub struct ConfigImportDTO {
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `items` field.
    pub items: Vec<ItemDTO>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `SearchDTO` entity.
pub struct SearchDTO {
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `key` field.
    pub key: String,
    /// The `value` field.
    pub value: String,
}

/// Request body for merging a gray (canary) release into the main branch.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceGrayReleaseDTO {
    /// Branch name.
    pub branch_name: String,
    /// Release title.
    pub release_title: String,
    /// Release comment / notes.
    #[serde(default)]
    pub release_comment: String,
    /// Releaser, i.e. the user who performed the release.
    pub released_by: String,
    /// Whether this is an emergency release.
    #[serde(default)]
    pub is_emergency_publish: bool,
    /// Change set containing the configuration changes to apply to the main branch.
    #[serde(default)]
    pub change_sets: ItemChangeSets,
    /// Whether to delete the branch after merging (the upstream portal defaults to `true`).
    #[serde(default)]
    pub delete_branch: bool,
}
