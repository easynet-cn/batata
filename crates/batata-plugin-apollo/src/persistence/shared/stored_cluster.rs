use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents the `StoredCluster` entity.
pub struct StoredCluster {
    /// The `id` field.
    pub id: i64,
    /// The `name` field.
    pub name: String,
    /// The `app_id` field.
    pub app_id: String,
    /// The `parent_cluster_id` field.
    pub parent_cluster_id: i64,
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
