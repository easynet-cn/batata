//! Shared helpers for AI resource tracing at the HTTP layer.
//!
//! The operator and client IP are only known here, which is why tracing is
//! wired in the handlers rather than inside `batata-ai` — and why
//! `batata_common::ai_trace` lives in `batata-common` (this crate does not
//! depend on `batata-ai`).

use actix_web::{HttpMessage, HttpRequest};

/// Read the caller identity from the request.
pub fn get_username(req: &HttpRequest) -> String {
    req.extensions()
        .get::<batata_common::IdentityContext>()
        .map(|ctx| ctx.username.clone())
        .unwrap_or_default()
}

/// Read the client IP from the request.
pub fn client_ip(req: &HttpRequest) -> String {
    req.connection_info()
        .realip_remote_addr()
        .unwrap_or("-")
        .to_string()
}

/// Emit one AI resource trace line for a finished operation.
///
/// `outcome` is `Ok(())` on success and carries the error message otherwise;
/// upstream puts that message in the record's `ext` field.
pub fn trace_write(
    req: &HttpRequest,
    resource_type: &str,
    operation: &str,
    name: Option<&str>,
    version: Option<&str>,
    outcome: Result<(), String>,
) {
    let user = get_username(req);
    let ip = client_ip(req);
    let resource_id = name.unwrap_or("-");

    match outcome {
        Ok(()) => batata_common::ai_trace::log_success(
            resource_type,
            resource_id,
            version,
            operation,
            &user,
            &ip,
        ),
        Err(message) => batata_common::ai_trace::log_failure(
            resource_type,
            resource_id,
            version,
            operation,
            &user,
            &ip,
            &message,
        ),
    }
}

/// Render an operation result as `Ok(())` / `Err(message)` for [`trace_write`].
pub fn outcome_of<T, E: std::fmt::Display>(result: &Result<T, E>) -> Result<(), String> {
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
