use core::str;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: i32,
    pub page_number: i32,
    pub pages_available: i32,
    pub page_items: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosUser {
    pub username: String,
    pub password: String,
    pub token: String,
    pub global_admin: bool,
}
