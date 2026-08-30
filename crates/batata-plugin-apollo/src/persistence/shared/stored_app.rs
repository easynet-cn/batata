use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredApp` entity.
pub struct StoredApp {
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