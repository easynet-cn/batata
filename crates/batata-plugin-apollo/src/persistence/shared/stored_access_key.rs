use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredAccessKey` entity.
pub struct StoredAccessKey {
    /// The `id` field.
    pub id: i32,
    /// The `app_id` field.
    pub app_id: String,
    /// The `secret` field.
    pub secret: String,
    /// The `mode` field.
    pub mode: i16,
    /// The `is_enabled` field.
    pub is_enabled: bool,
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