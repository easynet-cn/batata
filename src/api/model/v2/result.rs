use crate::api::model::v2::error_code::SUCCESS;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Result<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

pub fn success<T>(data: T) -> Result<T> {
    Result::<T> {
        code: SUCCESS.code,
        message: SUCCESS.message.to_string(),
        data,
    }
}
