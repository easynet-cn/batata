use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredRelease {
    pub id: i32,
    pub release_key: String,
    pub name: String,
    pub comment: Option<String>,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub configurations: String,
    pub release_id: Option<i64>,
    pub is_abandoned: bool,
    pub is_deleted: bool,
    pub deleted_at: i64,
    pub data_change_created_by: String,
    pub data_change_created_time: i64,
    pub data_change_last_modified_by: Option<String>,
    pub data_change_last_time: Option<i64>,
}