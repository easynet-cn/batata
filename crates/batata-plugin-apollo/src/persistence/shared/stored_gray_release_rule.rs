use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredGrayReleaseRule {
    pub id: i32,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub branch_name: String,
    pub rules: String,
    pub release_id: i64,
    pub branch_status: Option<i16>,
    pub is_deleted: bool,
    pub deleted_at: i64,
    pub data_change_created_by: String,
    pub data_change_created_time: i64,
    pub data_change_last_modified_by: Option<String>,
    pub data_change_last_time: Option<i64>,
}