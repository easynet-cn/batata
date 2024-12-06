use config::Config;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            total_count: 0,
            page_number: 1,
            pages_available: 0,
            page_items: vec![],
        }
    }
}

impl<T> Page<T> {
    pub fn new(total_count: u64, page_number: u64, page_size: u64, page_items: Vec<T>) -> Self {
        Self {
            total_count: total_count,
            page_number: page_number,
            pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
            page_items: page_items,
        }
    }
}

#[derive(Error, Debug)]
pub enum BusinessError {
    #[error("user '{0}' not exist!")]
    UserNotExist(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorCode<'a> {
    pub code: i32,
    pub message: &'a str,
}

pub const SUCCESS: ErrorCode<'static> = ErrorCode {
    code: 0,
    message: "success",
};

pub const PARAMETER_MISSING: ErrorCode<'static> = ErrorCode {
    code: 10000,
    message: "parameter missing",
};

pub const ACCESS_DENIED: ErrorCode<'static> = ErrorCode {
    code: 10001,
    message: "access denied",
};

pub const DATA_ACCESS_ERROR: ErrorCode<'static> = ErrorCode {
    code: 10002,
    message: "data access error",
};

pub const TENANT_PARAM_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20001,
    message: "'tenant' parameter error",
};

pub const PARAMETER_VALIDATE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20002,
    message: "parameter validate error",
};

pub const MEDIA_TYPE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20003,
    message: "MediaType Error",
};

pub const RESOURCE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 20004,
    message: "resource not found",
};

pub const RESOURCE_CONFLICT: ErrorCode<'static> = ErrorCode {
    code: 20005,
    message: "resource conflict",
};

pub const CONFIG_LISTENER_IS_NULL: ErrorCode<'static> = ErrorCode {
    code: 20006,
    message: "config listener is null",
};

pub const CONFIG_LISTENER_ERROR: ErrorCode<'static> = ErrorCode {
    code: 20007,
    message: "config listener error",
};

pub const INVALID_DATA_ID: ErrorCode<'static> = ErrorCode {
    code: 20008,
    message: "invalid dataId",
};

pub const PARAMETER_MISMATCH: ErrorCode<'static> = ErrorCode {
    code: 20009,
    message: "parameter mismatch",
};

pub const SERVICE_NAME_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21000,
    message: "service name error",
};

pub const WEIGHT_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21001,
    message: "weight error",
};

pub const INSTANCE_METADATA_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21002,
    message: "instance metadata error",
};

pub const INSTANCE_NOT_FOUND: ErrorCode<'static> = ErrorCode {
    code: 21003,
    message: "instance not found",
};

pub const INSTANCE_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21004,
    message: "instance error",
};

pub const SERVICE_METADATA_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21005,
    message: "service metadata error",
};

pub const SELECTOR_ERROR: ErrorCode<'static> = ErrorCode {
    code: 21006,
    message: "selector error",
};

pub const SERVICE_ALREADY_EXIST: ErrorCode<'static> = ErrorCode {
    code: 21007,
    message: "service already exist",
};

pub const SERVICE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 21008,
    message: "service not exist",
};

pub const SERVICE_DELETE_FAILURE: ErrorCode<'static> = ErrorCode {
    code: 21009,
    message: "service delete failure",
};

pub const HEALTHY_PARAM_MISS: ErrorCode<'static> = ErrorCode {
    code: 21010,
    message: "healthy param miss",
};

pub const HEALTH_CHECK_STILL_RUNNING: ErrorCode<'static> = ErrorCode {
    code: 21011,
    message: "health check still runnin",
};

pub const ILLEGAL_NAMESPACE: ErrorCode<'static> = ErrorCode {
    code: 22000,
    message: "illegal namespace",
};

pub const NAMESPACE_NOT_EXIST: ErrorCode<'static> = ErrorCode {
    code: 22002,
    message: "namespace already exist",
};

pub const ILLEGAL_STATE: ErrorCode<'static> = ErrorCode {
    code: 23000,
    message: "illegal state",
};

pub const NODE_INFO_ERROR: ErrorCode<'static> = ErrorCode {
    code: 23001,
    message: "node info error",
};

pub const NODE_DOWN_FAILURE: ErrorCode<'static> = ErrorCode {
    code: 23002,
    message: "node down failure",
};

pub const SERVER_ERROR: ErrorCode<'static> = ErrorCode {
    code: 30000,
    message: "server error",
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Result<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> Result<T> {
    pub fn success(data: T) -> Result<T> {
        Result::<T> {
            code: SUCCESS.code,
            message: SUCCESS.message.to_string(),
            data,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub app_config: Config,
    pub database_connection: DatabaseConnection,
    pub context_path: String,
    pub token_secret_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResult {
    pub timestamp: String,
    pub status: i32,
    pub error: String,
    pub message: String,
    pub path: String,
}
