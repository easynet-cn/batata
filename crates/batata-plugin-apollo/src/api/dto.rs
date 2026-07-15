use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDTO {
    pub app_id: String,
    pub name: String,
    pub org_id: String,
    pub org_name: String,
    pub owner_name: String,
    pub owner_email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceDTO {
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_num: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub release_key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configurations: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_abandoned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloConfig {
    pub app_id: String,
    pub cluster: String,
    pub namespace_name: String,
    pub release_key: String,
    pub configurations: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDTO {
    pub namespace_name: String,
    pub notification_id: i64,
    pub messages: NotificationMessageDTO,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationMessageDTO {
    pub details: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRequestDTO {
    pub namespace_name: String,
    pub notification_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterDTO {
    pub name: String,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_cluster_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub change_sets: String,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrayReleaseRuleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub branch_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    pub release_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_status: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub app_id: String,
    pub cluster_name: String,
    pub data_center: String,
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigDTO {
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKeyDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub app_id: String,
    pub secret: String,
    pub mode: i8,
    pub is_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseHistoryDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub branch_name: String,
    pub release_id: i32,
    pub previous_release_id: i32,
    pub operation: i8,
    pub operation_context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppNamespaceDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub name: String,
    pub app_id: String,
    pub format: String,
    pub is_public: bool,
    pub comment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemChangeSets {
    pub create_items: Vec<ItemDTO>,
    pub update_items: Vec<ItemDTO>,
    pub delete_items: Vec<ItemDTO>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConfigDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub instance_id: i32,
    pub namespace_name: String,
    pub cluster_name: String,
    pub release_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configurations: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_last_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub audit_key: String,
    pub entity_name: String,
    pub entity_id: String,
    pub op_name: String,
    pub op_time: String,
    pub op_by: String,
    pub op_client_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub app_id: String,
    pub name: String,
    pub org_id: String,
    pub org_name: String,
    pub owner_name: String,
    pub owner_email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerTokenDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub consumer_id: i32,
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub permission_type: i32,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub role_name: String,
    pub role_type: i32,
    pub target_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRoleDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub user_id: String,
    pub role_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteDTO {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    pub user_id: String,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigExportDTO {
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub items: Vec<ItemDTO>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigImportDTO {
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub items: Vec<ItemDTO>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_change_created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchDTO {
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub key: String,
    pub value: String,
}

/// 灰度发布合并到主分支请求体
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceGrayReleaseDTO {
    /// 分支名称
    pub branch_name: String,
    /// 发布标题
    pub release_title: String,
    /// 发布注释
    #[serde(default)]
    pub release_comment: String,
    /// 发布人
    pub released_by: String,
    /// 是否紧急发布
    #[serde(default)]
    pub is_emergency_publish: bool,
    /// 变更集（要应用到主分支的配置变更）
    #[serde(default)]
    pub change_sets: ItemChangeSets,
}
