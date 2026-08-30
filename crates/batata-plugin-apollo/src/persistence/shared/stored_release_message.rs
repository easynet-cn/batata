use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents the `StoredReleaseMessage` entity.
pub struct StoredReleaseMessage {
    /// The `id` field.
    pub id: i32,
    /// The `message` field.
    pub message: String,
    /// The `data_change_created_time` field.
    pub data_change_created_time: i64,
}