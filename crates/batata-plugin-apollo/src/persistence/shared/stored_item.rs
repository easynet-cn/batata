use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredItem {
    pub id: i32,
    pub namespace_id: i32,
    pub key: String,
    #[serde(rename = "type")]
    pub r#type: i8,
    pub value: String,
    pub comment: Option<String>,
    pub line_num: i32,
    pub is_deleted: bool,
    pub deleted_at: i64,
    pub data_change_created_by: String,
    pub data_change_created_time: i64,
    pub data_change_last_modified_by: Option<String>,
    pub data_change_last_time: Option<i64>,
}