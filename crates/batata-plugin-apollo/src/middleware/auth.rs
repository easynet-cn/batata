//! Actix-web middleware enforcing Apollo's three authentication mechanisms.
//!
//! Upstream references:
//! - `adminservice/.../filter/AdminServiceAuthenticationFilter.java` — a static
//!   access token carried in the `Authorization` header (`Authorization: <token>`).
//!   The filter only activates once `apollo.adminservice.access.enabled=true` and
//!   a token is configured; batata enables it whenever `APOLLO_ADMIN_SERVICE_ACCESS_TOKENS`
//!   is non-empty.
//! - `portal/.../openapi/filter/ConsumerAuthenticationFilter.java` — openapi
//!   consumers send their opaque token verbatim in the `Authorization` header;
//!   the filter looks it up directly in `apollo_consumer_token` and rejects
//!   unknown tokens with 401. (The consumer is bound to a single app, so the
//!   requested `appId` must match the consumer's `appId`.)
//! - `portal/.../filter/UserTokenAuthenticationFilter.java` + `UserTokenService`
//!   — portal management endpoints require a user token issued by the portal
//!   login flow; batata issues and validates them through
//!   [`crate::service::UserTokenService`].

use std::future::{ready, Ready};
use std::rc::Rc;

use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{web, Error, HttpMessage};
use futures::future::LocalBoxFuture;

use crate::model::config::AuthConfig;
use crate::persistence::traits::ApolloPersistenceService;
// Consumer-token lookups go through the `dyn ApolloPersistenceService` vtable
// (the `ConsumerTokenPersistence` supertrait), so no direct import is needed.
use crate::service::UserTokenService;

/// Extracts the bearer credential from an `Authorization` header.
///
/// Accepts `Authorization: <scheme> <value>` and returns `value`; if the header
/// has no scheme (upstream adminservice sends the raw token), the whole header
/// value is returned.
fn bearer_from_header(value: &str) -> Option<String> {
    let v = value.trim();
    if let Some((_, rest)) = v.split_once(' ') {
        Some(rest.trim().to_string())
    } else {
        Some(v.to_string())
    }
}

/// Middleware enforcing the admin-service access token.
pub struct AdminAuthMiddleware;

impl AdminAuthMiddleware {
    /// Builds a new `AdminAuthMiddleware`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdminAuthMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for AdminAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AdminAuthMiddlewareInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AdminAuthMiddlewareInner { service: Rc::new(service) }))
    }
}

/// Inner service for [`AdminAuthMiddleware`].
pub struct AdminAuthMiddlewareInner<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AdminAuthMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        Box::pin(async move {
            let auth_config = req.app_data::<web::Data<AuthConfig>>().cloned();
            if let Some(cfg) = auth_config
                && !cfg.is_valid_admin_token("") {
                    // Access control enabled: a token is required.
                    let header = req
                        .headers()
                        .get("Authorization")
                        .and_then(|h| h.to_str().ok())
                        .and_then(bearer_from_header);
                    match header {
                        Some(token) if cfg.is_valid_admin_token(&token) => {}
                        _ => {
                            return Err(actix_web::error::ErrorUnauthorized(
                                "Missing or invalid admin-service access token",
                            ));
                        }
                    }
                }

            service.call(req).await
        })
    }
}

/// Middleware enforcing consumer tokens (openapi data) and user tokens
/// (portal management) on the openapi scope.
pub struct OpenApiAuthMiddleware;

impl OpenApiAuthMiddleware {
    /// Builds a new `OpenApiAuthMiddleware`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenApiAuthMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for OpenApiAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = OpenApiAuthMiddlewareInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(OpenApiAuthMiddlewareInner { service: Rc::new(service) }))
    }
}

/// Inner service for [`OpenApiAuthMiddleware`].
pub struct OpenApiAuthMiddlewareInner<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for OpenApiAuthMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let path = req.path().to_string();
        let method = req.method().clone();

        Box::pin(async move {
            // A handful of openapi paths are public (e.g. the login endpoint
            // used to bootstrap a user token). They bypass authentication.
            if is_public_openapi_path(&path) {
                return service.call(req).await;
            }

            // Openapi access control is opt-in, mirroring the admin side:
            // unless `APOLLO_OPENAPI_AUTH_ENABLED` is set the whole /openapi/v1
            // surface stays open. This keeps an unconfigured install (and the
            // in-process suite) usable instead of rejecting every request
            // before any consumer has been created.
            let auth_config = req.app_data::<web::Data<AuthConfig>>().cloned();
            let openapi_auth_enabled = auth_config
                .as_ref()
                .map(|cfg| cfg.openapi_auth_enabled)
                .unwrap_or(false);
            if !openapi_auth_enabled {
                return service.call(req).await;
            }

            // Paths that manage consumers / users / roles / permissions are
            // protected by a portal user token (a logged-in user), not by a
            // consumer token. Everything else under /openapi/v1 is protected by
            // the consumer token.
            let requires_user_token = is_portal_management_path(&path, method.as_str());

            let persistence = req
                .app_data::<web::Data<std::sync::Arc<dyn ApolloPersistenceService>>>()
                .cloned();

            // Resolve the persistence reference once; downstream calls go through
            // the combined `dyn ApolloPersistenceService` vtable.
            let persistence_ref: Option<std::sync::Arc<dyn ApolloPersistenceService>> =
                persistence.map(|p| (*p.into_inner()).clone());

            if requires_user_token {
                let token = extract_user_token(&req);
                match token {
                    Some(t) => {
                        let arc = persistence_ref.clone().ok_or_else(|| {
                            actix_web::error::ErrorInternalServerError("persistence unavailable")
                        })?;
                        let svc = UserTokenService::new(arc);
                        let valid = svc.validate(&t).await.map_err(|e| {
                            actix_web::error::ErrorInternalServerError(e.to_string())
                        })?;
                        match valid {
                            Some(model) => {
                                req.extensions_mut().insert(model);
                            }
                            None => {
                                return Err(actix_web::error::ErrorUnauthorized(
                                    "Invalid or expired user token",
                                ));
                            }
                        }
                    }
                    None => {
                        return Err(actix_web::error::ErrorUnauthorized(
                            "Portal management requires a user token",
                        ));
                    }
                }
            } else {
                // Consumer token: the raw Authorization header value is the token.
                let presented = req
                    .headers()
                    .get("Authorization")
                    .and_then(|h| h.to_str().ok())
                    .and_then(bearer_from_header)
                    .filter(|t| !t.is_empty());

                let presented = match presented {
                    Some(t) => t,
                    None => {
                        return Err(actix_web::error::ErrorUnauthorized(
                            "Openapi access requires an 'Authorization' header with a consumer token",
                        ));
                    }
                };

                // Resolve the appId from the path: /openapi/v1/apps/{appId}/...
                let app_id = match extract_app_id(&path) {
                    Some(a) => a,
                    None => {
                        return Err(actix_web::error::ErrorUnauthorized(
                            "Unable to resolve appId for consumer token",
                        ));
                    }
                };

                let arc = match persistence_ref {
                    Some(p) => p,
                    None => {
                        return Err(actix_web::error::ErrorInternalServerError(
                            "persistence unavailable",
                        ));
                    }
                };
                let svc_ref: &dyn ApolloPersistenceService = &*arc;

                let stored = svc_ref
                    .get_consumer_token_by_token(&presented)
                    .await
                    .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

                match stored {
                    Some(token_model) => {
                        let consumer = svc_ref
                            .get_consumer(token_model.consumer_id)
                            .await
                            .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
                        match consumer {
                            Some(c) if c.app_id == app_id => {
                                req.extensions_mut().insert(c);
                            }
                            _ => {
                                return Err(actix_web::error::ErrorUnauthorized(
                                    "Consumer token is not authorized for this appId",
                                ));
                            }
                        }
                    }
                    None => {
                        return Err(actix_web::error::ErrorUnauthorized(
                            "Invalid consumer token",
                        ));
                    }
                }
            }

            service.call(req).await
        })
    }
}

/// Openapi paths that are reachable without any authentication.
///
/// The login endpoint is the only public one: it is how a caller bootstraps a
/// user token before accessing the portal-management endpoints.
fn is_public_openapi_path(path: &str) -> bool {
    path == "/openapi/v1/user/login"
}

/// Whether a path is a portal-management endpoint (requires a user token).
fn is_portal_management_path(path: &str, method: &str) -> bool {    // Token self-management and consumer-token issuance are reached by a logged
    // in user, not by a consumer token.
    if path == "/openapi/v1/consumer-tokens" && method.eq_ignore_ascii_case("POST") {
        return true;
    }
    if path.starts_with("/openapi/v1/consumers/") && path.ends_with("/tokens") {
        return true;
    }
    if path.starts_with("/openapi/v1/users") {
        return true;
    }
    if path.starts_with("/openapi/v1/roles") {
        return true;
    }
    if path.starts_with("/openapi/v1/permissions") {
        return true;
    }
    if path.starts_with("/openapi/v1/organizations") {
        return true;
    }
    if path == "/openapi/v1/user" {
        return true;
    }
    false
}

/// Extracts the user token from the `Authorization` header (no scheme) or the
/// `user-token` header / `token` query parameter, mirroring upstream's
/// `UserTokenAuthenticationFilter`.
fn extract_user_token(req: &ServiceRequest) -> Option<String> {
    if let Some(h) = req.headers().get("Authorization").and_then(|h| h.to_str().ok()) {
        let trimmed = h.trim();
        if let Some((scheme, value)) = trimmed.split_once(' ') {
            if !scheme.eq_ignore_ascii_case("Consumer") {
                return Some(value.trim().to_string());
            }
        } else {
            return Some(trimmed.to_string());
        }
    }
    if let Some(h) = req.headers().get("user-token").and_then(|h| h.to_str().ok()) {
        return Some(h.trim().to_string());
    }
    if let Some(q) = req.query_string().split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        if k == "token" {
            Some(v.to_string())
        } else {
            None
        }
    }) {
        return Some(q);
    }
    None
}

/// Resolves the `{appId}` from an openapi path such as
/// `/openapi/v1/apps/{appId}/clusters/...`.
fn extract_app_id(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // ["openapi", "v1", "apps", "{appId}", ...]
    if segments.len() >= 4
        && segments[0] == "openapi"
        && segments[1] == "v1"
        && segments[2] == "apps"
    {
        Some(segments[3].to_string())
    } else {
        None
    }
}
