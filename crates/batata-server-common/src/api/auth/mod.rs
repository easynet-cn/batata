// Authentication and authorization HTTP handlers
// This module handles user authentication, role-based access control, and permission management

use actix_web::HttpResponse;

use crate::model::app_state::AppState;
use crate::model::response::Result;

/// Whether the request arrived over the trusted console-to-server channel.
///
/// The separated console proxies management calls with the server identity
/// header. It has already authorized the real end user locally (that is where
/// the user's token lives), so the core must not re-run per-user checks on a
/// request that carries a valid server identity.
pub(crate) fn trusted_console_request(req: &actix_web::HttpRequest, data: &AppState) -> bool {
    let key = data.configuration.server_identity_key();

    if key.is_empty() {
        return false;
    }

    let expected = data.configuration.server_identity_value();

    req.headers()
        .get(&key)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == expected)
}

/// Resolve the auth data service backing the `/v3/auth/**` handlers.
///
/// These handlers are mounted in the console process as well as in the core, so
/// they must never reach for [`AppState::persistence`] — a separated console has
/// no database and panicking there would abort the worker instead of answering
/// the request. The auth data service is the deployment-aware seam: local
/// storage when merged, HTTP to the core when separated.
pub(crate) fn auth_data(
    data: &AppState,
) -> std::result::Result<&dyn batata_auth::service::AuthDataService, HttpResponse> {
    match data.try_auth_data() {
        Some(service) => Ok(service),
        None => Err(HttpResponse::InternalServerError().json(Result::<String> {
            code: 500,
            message: "auth data service not available".to_string(),
            data: String::new(),
        })),
    }
}

// API version 3 (current) endpoints
/// Provides the `v3` module.
pub mod v3 {
    mod admin;
    mod auth;
    pub mod oauth;
    pub mod oidc;
    mod permission;
    mod role;
    /// Provides the `route` module.
    pub mod route;
    mod user;
    mod visibility;
}

// Authentication data models and structures (re-exported from batata-auth)
/// Provides the `model` module.
pub mod model {
    pub use batata_auth::model::*;
}

// Re-export authentication service implementations from batata-auth crate
pub use batata_auth::service;
