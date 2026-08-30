use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredGrayReleaseRule` entity.
pub struct StoredGrayReleaseRule {
    /// The `id` field.
    pub id: i32,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `branch_name` field.
    pub branch_name: String,
    /// The `rules` field.
    pub rules: String,
    /// The `release_id` field.
    pub release_id: i64,
    /// The `branch_status` field.
    pub branch_status: Option<i16>,
    /// The `is_deleted` field.
    pub is_deleted: bool,
    /// The `deleted_at` field.
    pub deleted_at: i64,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: i64,
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<i64>,
}