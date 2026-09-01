use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredNamespaceLock` entity.
pub struct StoredNamespaceLock {
    /// The `id` field.
    pub id: i64,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `locked_by` field.
    pub locked_by: String,
    /// The `locked_at` field.
    pub locked_at: i64,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: i64,
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<i64>,
}