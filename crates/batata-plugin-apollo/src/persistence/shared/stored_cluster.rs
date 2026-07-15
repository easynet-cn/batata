use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCluster {
    pub id: i32,
    pub name: String,
    pub app_id: String,
    pub parent_cluster_id: i32,
    pub is_deleted: bool,
    pub deleted_at: i64,
    pub data_change_created_by: String,
    pub data_change_created_time: i64,
    pub data_change_last_modified_by: Option<String>,
    pub data_change_last_time: Option<i64>,
}
