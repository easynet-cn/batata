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
    pub id: Option<i64>,
    /// The `key` field.
    pub key: String,
    /// The `value` field.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `type` field.
    pub r#type: Option<i32>,
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
    pub id: Option<i64>,
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
    pub parent_cluster_id: Option<i64>,
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
    pub id: Option<i64>,
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
    pub id: Option<i64>,
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
    pub branch_status: Option<i32>,
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
    pub id: Option<i64>,
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
    pub id: Option<i64>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `secret` field.
    pub secret: String,
    /// The `mode` field.
    pub mode: i32,
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
    pub id: Option<i64>,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `branch_name` field.
    pub branch_name: String,
    /// The `release_id` field.
    pub release_id: i64,
    /// The `previous_release_id` field.
    pub previous_release_id: i64,
    /// The `operation` field.
    pub operation: i32,
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
    pub id: Option<i64>,
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
/// Item snapshot before and after an update.
///
/// Upstream `ConfigChangeContentBuilder.ItemPair`.
pub struct ItemPair {
    /// The item snapshot before the update.
    pub old_item: ItemDTO,
    /// The item snapshot after the update.
    pub new_item: ItemDTO,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Change-set payload persisted into `apollo_commit.change_sets`.
///
/// Upstream stores the serialized `ConfigChangeContentBuilder`
/// (`apollo-biz/.../utils/ConfigChangeContentBuilder.java`) verbatim in the
/// commit row: `createItems` and `deleteItems` hold full item snapshots while
/// `updateItems` holds `{oldItem, newItem}` pairs. This is deliberately
/// distinct from `ItemChangeSets`, which is the OpenAPI request contract where
/// `updateItems` is a plain list of item DTOs.
pub struct ConfigChangeContent {
    /// Items created by the change set.
    pub create_items: Vec<ItemDTO>,
    /// Item snapshots before and after each update.
    pub update_items: Vec<ItemPair>,
    /// Items deleted by the change set.
    pub delete_items: Vec<ItemDTO>,
}

impl ConfigChangeContent {
    /// Whether the change set carries any item mutation.
    ///
    /// Upstream `ConfigChangeContentBuilder.hasContent()`.
    pub fn has_content(&self) -> bool {
        !self.create_items.is_empty()
            || !self.update_items.is_empty()
            || !self.delete_items.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `InstanceConfigDTO` entity.
pub struct InstanceConfigDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i64>,
    /// The `instance_id` field.
    pub instance_id: i64,
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
    pub id: Option<i64>,
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
    pub id: Option<i64>,
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
    pub id: Option<i64>,
    /// The `consumer_id` field.
    pub consumer_id: i64,
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
    pub id: Option<i64>,
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
    pub id: Option<i64>,
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
/// A portal user.
///
/// Upstream `apollo-portal` `UserInfo` / `apollo_users`. The `password` field is
/// never serialized back to clients: upstream strips it from every response and
/// the OpenAPI user endpoint only ever returns the username.
pub struct UserDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i64>,
    /// The account name, which upstream uses as the primary key too.
    pub username: String,
    #[serde(default, skip_serializing)]
    /// The stored password hash. Never returned to clients.
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The contact email.
    pub email: Option<String>,
    #[serde(default = "default_user_enabled")]
    /// Whether the account can log in.
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `data_change_created_time` field.
    pub data_change_created_time: Option<String>,
}

/// Default for `UserDTO::enabled` — a freshly created account can log in.
fn default_user_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Links an open-platform consumer to a role.
///
/// Upstream `apollo_consumer_role`; this is what grants an OpenAPI consumer its
/// permissions.
pub struct ConsumerRoleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `id` field.
    pub id: Option<i64>,
    /// The consumer this link belongs to.
    pub consumer_id: i64,
    /// The granted role.
    pub role_id: i64,
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
    pub id: Option<i64>,
    /// The `user_id` field.
    pub user_id: String,
    /// The `role_id` field.
    pub role_id: i64,
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
    pub id: Option<i64>,
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

/// A single instance config view returned by the OpenAPI instance endpoints.
///
/// Upstream `apollo-portal` `OpenApiController` -> `OpenInstanceDTO`. It joins
/// the `InstanceConfig` (release binding) with the underlying `Instance`
/// (`ip`, `dataCenter`). `instance_app_id` is the app that *owns* the instance
/// config (the upstream `configAppId`); `app_id` is the namespace's app.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInstanceDTO {
    /// The namespace's app id (`appId`).
    pub app_id: String,
    /// The app that owns this instance config (`instanceAppId` / `configAppId`).
    pub instance_app_id: String,
    /// The cluster name.
    pub cluster_name: String,
    /// The namespace name.
    pub namespace_name: String,
    /// The data center the instance belongs to (`dataCenter`).
    pub data_center: Option<String>,
    /// The instance IP address (`ip`).
    pub ip: String,
    /// The release key this instance is currently bound to (`releaseKey`).
    pub release_key: Option<String>,
    /// The release id this instance is currently bound to (`releaseId`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_id: Option<i64>,
    /// The last time this instance config was modified (`lastModifiedTime`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_time: Option<String>,
}

/// Paged response for the OpenAPI instance endpoints.
///
/// Upstream wraps a `PageDTO<OpenInstanceDTO>` in `OpenInstancePageDTO` so the
/// same envelope is used by every instance listing endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInstancePageDTO {
    /// The paged content.
    pub content: Vec<OpenInstanceDTO>,
    /// 1-based page index.
    pub page: i32,
    /// Page size.
    pub size: i32,
    /// Total number of matching rows.
    pub total: i64,
}

/// A single namespace's diff produced by the `items/diff` endpoint.
///
/// Upstream `apollo-portal` `ItemController.diff(...)` returns
/// `Map<namespaceName, ItemDiffs>`; each value carries the create/update/delete
/// item lists that would result from synchronizing the source items into that
/// target namespace. `namespace_name` identifies the target.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDiffs {
    /// The target namespace name these diffs apply to.
    pub namespace_name: String,
    /// Items to create in the target namespace.
    pub create_items: Vec<ItemDTO>,
    /// Items to update in the target namespace.
    pub update_items: Vec<ItemDTO>,
    /// Items to delete in the target namespace.
    pub delete_items: Vec<ItemDTO>,
}

/// Request body for the `items/diff` and `items` (synchronize) endpoints.
///
/// Upstream `apollo-portal` `NamespaceSyncModel`: the items to sync plus the
/// list of target namespaces (`sync_to_namespaces`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceSyncModel {
    /// The namespace name of the source (the item set being synchronized from).
    pub namespace_name: String,
    /// The change sets (create/update/delete items) to synchronize.
    pub sync_items: ItemChangeSets,
    /// The target namespaces to synchronize the items into.
    #[serde(default)]
    pub sync_to_namespaces: Vec<String>,
}

/// Per-env cluster info returned by `env-cluster-info`.
///
/// Upstream `apollo-portal` `EnvClusterInfoDTO` carries `env` plus the list of
/// `ClusterInfoDTO` describing each cluster of that env.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvClusterInfo {
    /// The environment name (e.g. `DEV`, `PRO`).
    pub env: String,
    /// The clusters present under this env.
    pub clusters: Vec<ClusterInfoDTO>,
}

/// Env + cluster listing for an app.
///
/// Upstream `apollo-portal` `EnvClusterInfoDTO` is returned by
/// `openapi/v1/apps/{appId}/env-cluster-info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvClusterInfoDTO {
    /// One entry per environment the app exists in.
    pub env_cluster_info: Vec<EnvClusterInfo>,
}

/// A cluster summary inside `EnvClusterInfo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterInfoDTO {
    /// The cluster name.
    pub cluster_name: String,
    /// The cluster's parent (empty for the default cluster).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_cluster_name: Option<String>,
    /// The config app id (cluster app label).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_app_id: Option<String>,
}

/// Where an app namespace is used (which envs / clusters), returned by
/// `appnamespaces/{namespaceName}/usage`.
///
/// Upstream `apollo-portal` `AppNamespaceUsageDTO`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppNamespaceUsageDTO {
    /// The app namespace name.
    pub namespace_name: String,
    /// The environments where this app namespace is used.
    pub envs: Vec<String>,
    /// The clusters where this app namespace is used.
    pub clusters: Vec<String>,
    /// The apps that use this app namespace.
    pub used_by: Vec<String>,
}

/// An organization entry returned by `GET /openapi/v1/organizations`.
///
/// Upstream `apollo-portal` `OrganizationDTO` (`orgId` + `orgName`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationDTO {
    /// The organization id.
    pub org_id: String,
    /// The organization name.
    pub org_name: String,
}

/// System information returned by `GET /openapi/v1/system-info`.
///
/// Upstream `apollo-portal` `SystemInfoDTO` (`apolloVersion` +
/// `gitCommitId`). batata fills these from the build-time cargo environment
/// variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoDTO {
    /// The build version string.
    pub apollo_version: String,
    /// The git commit id of the build.
    pub git_commit_id: String,
}
