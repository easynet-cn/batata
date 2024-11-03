use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

pub const DEFAULT_TOKEN_EXPIRE_SECONDS: i64 = 1800;

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
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NacosJwtPayload {
    pub sub: String,
    pub exp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBase {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
    pub tenant: String,
    pub app_name: String,
    pub _type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfoWrapper {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub md5: String,
    pub encrypted_data_key: String,
    pub tenant: String,
    pub app_name: String,
    pub _type: String,
    pub last_modified: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryInfo {
    pub id: i64,
    pub last_id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub app_name: String,
    pub md5: String,
    pub content: String,
    pub src_ip: String,
    pub src_user: String,
    pub op_type: String,
    pub created_time: NaiveDateTime,
    pub last_modified_time: NaiveDateTime,
    pub encrypted_data_key: String,
}
