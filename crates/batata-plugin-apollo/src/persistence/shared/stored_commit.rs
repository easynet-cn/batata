use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredCommit` entity.
pub struct StoredCommit {
    /// The `id` field.
    pub id: i64,
    /// The `change_sets` field.
    pub change_sets: String,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `comment` field.
    pub comment: Option<String>,
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