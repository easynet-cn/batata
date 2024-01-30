use serde::{Deserialize, Serialize};

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
