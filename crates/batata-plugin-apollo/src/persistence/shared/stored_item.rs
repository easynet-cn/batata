use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredItem` entity.
pub struct StoredItem {
    /// The `id` field.
    pub id: i32,
    /// The `namespace_id` field.
    pub namespace_id: i32,
    /// The `key` field.
    pub key: String,
    #[serde(rename = "type")]
    /// The `type` field.
    pub r#type: i16,
    /// The `value` field.
    pub value: String,
    /// The `comment` field.
    pub comment: Option<String>,
    /// The `line_num` field.
    pub line_num: i32,
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