use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredInstance {
    pub id: i32,
    pub app_id: String,
    pub cluster_name: String,
    pub data_center: String,
    pub ip: String,
    pub data_change_created_time: i64,
    pub data_change_last_time: Option<i64>,
}