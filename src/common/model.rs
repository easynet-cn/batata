use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RestResult<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> RestResult<T> {
    pub fn success(data: T) -> RestResult<T> {
        RestResult::<T> {
            code: 200,
            message: "".to_string(),
            data,
        }
    }
}
