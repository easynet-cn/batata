//! Visibility grant API endpoints (F-NAC-ADM-AUTH-001)
//!
//! Nacos-compatible visibility authorization endpoints.
//! Maps to Nacos `VisibilityGrantControllerV3`.
//!
//! * `POST   /v3/auth/visibility` — grant visibility permission (form params)
//! * `DELETE /v3/auth/visibility` — revoke visibility permission (query params)
//!
//! Visibility grants allow a resource owner or administrator to delegate
//! read (or read-write) visibility on a specific resource to another user.
//! Internally each grantee gets a dedicated role whose name is derived from
//! a SHA-256 prefix of the username, and a permission row whose resource
//! identifier encodes the namespace / resource-type / resource-name triple.

use actix_web::{HttpRequest, HttpResponse, Responder, delete, post, web};
use serde::Deserialize;

use batata_auth::service::visibility_grant::{
    DefaultVisibilityGrantService, VisibilityGrantService, VISIBILITY_RESOURCE,
};
use batata_core::service::GrpcAuthService;

use crate::model::app_state::AppState;
use crate::model::response::Result;
use crate::secured::Secured;
use crate::{ActionTypes, ApiType, SignType, secured};

/// Parameters for grant / revoke operations.
///
/// Field names use `rename_all = "camelCase"` so that `namespaceId` from
/// the query string or form body maps to `namespace_id`, matching the
/// Nacos API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisibilityGrantParam {
    /// Namespace ID, blank for the default namespace.
    namespace_id: Option<String>,
    /// Resource type (e.g. "config", "naming").
    resource_type: String,
    /// Resource name.
    resource_name: String,
    /// Grantee username.
    username: String,
    /// Grant action: `r`, `w`, or `rw`.
    action: String,
}

/// Build a [`DefaultVisibilityGrantService`] from the AppState persistence.
///
/// Returns an error response if the persistence layer is not available
/// (e.g. console-only remote mode where the server has no local database).
fn build_service(
    data: &web::Data<AppState>,
) -> std::result::Result<DefaultVisibilityGrantService, HttpResponse> {
    match data.persistence.as_ref() {
        Some(p) => Ok(DefaultVisibilityGrantService::new(p.clone())),
        None => Err(HttpResponse::InternalServerError().json(Result::<String> {
            code: 500,
            message: "persistence service not available".to_string(),
            data: String::new(),
        })),
    }
}

/// Check whether a service error is a client-side error (HTTP 400).
///
/// The [`DefaultVisibilityGrantService`] returns `anyhow::Error` with
/// human-readable messages for validation failures. We inspect the
/// message text to decide the HTTP status code.
fn is_client_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("not found")
        || msg.contains("blank")
        || msg.contains("unsupported")
        || msg.contains("invalid")
}

/// Map a service error to an HTTP response.
fn error_response(err: anyhow::Error) -> HttpResponse {
    if is_client_error(&err) {
        let msg = err.to_string();
        HttpResponse::BadRequest().json(Result::<String> {
            code: 400,
            message: msg.clone(),
            data: msg,
        })
    } else {
        tracing::error!("visibility grant service error: {err}");
        let msg = err.to_string();
        HttpResponse::InternalServerError().json(Result::<String> {
            code: 500,
            message: msg.clone(),
            data: msg,
        })
    }
}

/// Grant one visibility action to a user for a resource.
///
/// `POST /v3/auth/visibility`
///
/// Nacos sends the parameters as form-encoded body (`postFormOk`),
/// so we use `web::Form` here.
#[post("/visibility")]
async fn grant(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Form<VisibilityGrantParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, VISIBILITY_RESOURCE)
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let service = match build_service(&data) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let result = service
        .grant(
            params.namespace_id.as_deref().unwrap_or(""),
            &params.resource_type,
            &params.resource_name,
            &params.username,
            &params.action,
        )
        .await;

    match result {
        Ok(()) => {
            GrpcAuthService::clear_cache();
            let msg = "grant visibility permission ok!".to_string();
            HttpResponse::Ok().json(Result::<String>::new(0, msg.clone(), msg))
        }
        Err(err) => error_response(err),
    }
}

/// Revoke one visibility action from a user for a resource.
///
/// `DELETE /v3/auth/visibility`
///
/// Nacos sends the parameters as query string (`deleteJsonOk`),
/// so we use `web::Query` here.
#[delete("/visibility")]
async fn revoke(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<VisibilityGrantParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, VISIBILITY_RESOURCE)
            .action(ActionTypes::Write)
            .sign_type(SignType::Specified)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let service = match build_service(&data) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    let result = service
        .revoke(
            params.namespace_id.as_deref().unwrap_or(""),
            &params.resource_type,
            &params.resource_name,
            &params.username,
            &params.action,
        )
        .await;

    match result {
        Ok(()) => {
            GrpcAuthService::clear_cache();
            let msg = "revoke visibility permission ok!".to_string();
            HttpResponse::Ok().json(Result::<String>::new(0, msg.clone(), msg))
        }
        Err(err) => error_response(err),
    }
}
