use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredReleaseMessage {
    pub id: i32,
    pub message: String,
    pub data_change_created_time: i64,
}