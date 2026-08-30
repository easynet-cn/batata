use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredInstance` entity.
pub struct StoredInstance {
    /// The `id` field.
    pub id: i32,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `data_center` field.
    pub data_center: String,
    /// The `ip` field.
    pub ip: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: i64,
    /// The `data_change_last_time` field.
    pub data_change_last_time: Option<i64>,
}