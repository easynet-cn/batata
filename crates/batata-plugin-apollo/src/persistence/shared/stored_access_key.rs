use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredAccessKey {
    pub id: i32,
    pub app_id: String,
    pub secret: String,
    pub mode: i8,
    pub is_enabled: bool,
    pub is_deleted: bool,
    pub deleted_at: i64,
    pub data_change_created_by: String,
    pub data_change_created_time: i64,
    pub data_change_last_modified_by: Option<String>,
    pub data_change_last_time: Option<i64>,
}