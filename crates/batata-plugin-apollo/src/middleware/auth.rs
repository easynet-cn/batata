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

use std::future::{Ready, ready};
use std::rc::Rc;

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::{Error, HttpMessage, web};
use futures::future::LocalBoxFuture;

use crate::api::dto::{NewUserTokenAudit, UserDTO};
use crate::auth::permission::Identity;
use crate::auth::token_rate_limiter::LIMITER;
use crate::auth::token_scope::{TokenScope, scope_allows_request};
use crate::model::config::AuthConfig;
use crate::persistence::traits::{
    ApolloPersistenceService, ServerConfigPersistence, UserPersistence,
};
use crate::service::{UserService, UserTokenAuditService, UserTokenService};

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
        ready(Ok(AdminAuthMiddlewareInner {
            service: Rc::new(service),
        }))
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
                && !cfg.is_valid_admin_token("")
            {
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
        ready(Ok(OpenApiAuthMiddlewareInner {
            service: Rc::new(service),
        }))
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
            if is_public_openapi_path(&path) {
                return service.call(req).await;
            }

            let auth_config = req.app_data::<web::Data<AuthConfig>>().cloned();
            let openapi_auth_enabled = auth_config
                .as_ref()
                .map(|cfg| cfg.openapi_auth_enabled)
                .unwrap_or(false);
            let persistence_ref: Option<std::sync::Arc<dyn ApolloPersistenceService>> = req
                .app_data::<web::Data<std::sync::Arc<dyn ApolloPersistenceService>>>()
                .cloned()
                .map(|p| (*p.into_inner()).clone());

            // Portal-management paths are guarded by a user token. Data paths
            // accept a user token first (upstream's UserTokenAuthenticationFilter
            // covers all of /openapi/**) and fall back to a consumer token.
            let requires_user_token = is_portal_management_path(&path, method.as_str());
            let explicit_token = extract_explicit_user_token(&req);
            let authorization_token = extract_authorization_credential(&req);

            // A presented token must always be validated, even when global
            // auth is disabled. Otherwise the UI would report a successful
            // login while the backend ignored its credential entirely.
            let candidates = explicit_token.into_iter().map(|token| (token, true)).chain(
                authorization_token
                    .clone()
                    .into_iter()
                    .map(|token| (token, false)),
            );

            for (token, explicit) in candidates {
                let arc = persistence_ref.clone().ok_or_else(|| {
                    actix_web::error::ErrorInternalServerError("persistence unavailable")
                })?;
                let valid = UserTokenService::new(arc.clone())
                    .validate(&token)
                    .await
                    .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
                if let Some(model) = valid {
                    if !LIMITER.try_acquire(&model.token_prefix, model.rate_limit) {
                        return Err(actix_web::error::ErrorTooManyRequests(
                            "Too Many Requests, the flow is limited",
                        ));
                    }
                    // Data paths carry env/cluster/namespace in the URL; enforce
                    // the same resource scope upstream applies in its
                    // PermissionValidator before the handler reads or writes.
                    let scope = TokenScope::parse(model.scopes.as_deref());
                    if !scope_allows_request(&scope, &path, method.as_str()) {
                        return Err(actix_web::error::ErrorForbidden(
                            "User token scope does not permit this resource",
                        ));
                    }
                    let user = UserPersistence::get_user(&*arc, &model.user_id)
                        .await
                        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
                        .ok_or_else(|| {
                            actix_web::error::ErrorUnauthorized("User token owner no longer exists")
                        })?;
                    if let Err(error) = UserTokenAuditService::new(arc.clone())
                        .record(NewUserTokenAudit {
                            token_id: Some(model.id),
                            user_id: model.user_id.clone(),
                            uri: path.clone(),
                            method: method.to_string(),
                        })
                        .await
                    {
                        tracing::warn!(%error, "failed to record Apollo user-token audit");
                    }
                    let last_used_ip = req
                        .connection_info()
                        .realip_remote_addr()
                        .map(str::to_string);
                    let last_used_user_agent = req
                        .headers()
                        .get("User-Agent")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    UserTokenService::new(arc.clone())
                        .record_usage(
                            &model,
                            last_used_ip.as_deref(),
                            last_used_user_agent.as_deref(),
                        )
                        .await;
                    req.extensions_mut().insert(model);
                    req.extensions_mut()
                        .insert(Identity::User(UserService::model_to_dto(&user)));
                    return service.call(req).await;
                }
                if explicit || requires_user_token {
                    return Err(actix_web::error::ErrorUnauthorized(
                        "Invalid or expired user token",
                    ));
                }
                // An Authorization value may still be a consumer token.
            }

            if requires_user_token {
                if openapi_auth_enabled {
                    return Err(actix_web::error::ErrorUnauthorized(
                        "Portal management requires a user token",
                    ));
                }
                let identity = compat_admin_identity(persistence_ref.as_ref()).await?;
                req.extensions_mut().insert(identity);
                return service.call(req).await;
            }

            let presented = match authorization_token {
                Some(token) => token,
                None => {
                    if !openapi_auth_enabled {
                        let identity = compat_admin_identity(persistence_ref.as_ref()).await?;
                        req.extensions_mut().insert(identity);
                        return service.call(req).await;
                    }
                    return Err(actix_web::error::ErrorUnauthorized(
                        "Openapi access requires an 'Authorization' header with a consumer token",
                    ));
                }
            };

            let app_id = match extract_app_id(&path) {
                Some(app_id) => app_id,
                None if !openapi_auth_enabled => {
                    let identity = compat_admin_identity(persistence_ref.as_ref()).await?;
                    req.extensions_mut().insert(identity);
                    return service.call(req).await;
                }
                None => {
                    return Err(actix_web::error::ErrorUnauthorized(
                        "Unable to resolve appId for consumer token",
                    ));
                }
            };

            let arc = persistence_ref.ok_or_else(|| {
                actix_web::error::ErrorInternalServerError("persistence unavailable")
            })?;
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
                        Some(consumer) if consumer.app_id == app_id => {
                            req.extensions_mut().insert(Identity::Consumer {
                                consumer_id: consumer.id,
                                app_id: Some(consumer.app_id.clone()),
                            });
                            req.extensions_mut().insert(consumer);
                        }
                        _ if !openapi_auth_enabled => {
                            // Backward compatibility: with auth disabled, an
                            // arbitrary Authorization value was historically
                            // ignored on data paths. SDK suites still send a
                            // placeholder token ("admin") that is not a real
                            // consumer row, so keep that path open. Explicit
                            // user-token headers remain fail-closed above.
                            let identity = compat_admin_identity(Some(&arc)).await?;
                            req.extensions_mut().insert(identity);
                        }
                        _ => {
                            return Err(actix_web::error::ErrorUnauthorized(
                                "Consumer token is not authorized for this appId",
                            ));
                        }
                    }
                }
                None if !openapi_auth_enabled => {
                    let identity = compat_admin_identity(Some(&arc)).await?;
                    req.extensions_mut().insert(identity);
                }
                None => {
                    return Err(actix_web::error::ErrorUnauthorized(
                        "Invalid consumer token",
                    ));
                }
            }

            service.call(req).await
        })
    }
}

/// Openapi paths that are reachable without any authentication.
///
/// The token-issuing endpoint is the only public one: it bootstraps a PAT
/// without introducing a Portal session.
fn is_public_openapi_path(path: &str) -> bool {
    path == "/openapi/v1/user/login"
}

/// Whether a path is a portal-management endpoint (requires a user token).
fn is_portal_management_path(path: &str, method: &str) -> bool {
    if path == "/openapi/v1/consumer-tokens" && method.eq_ignore_ascii_case("POST") {
        return true;
    }
    if path.starts_with("/openapi/v1/consumers")
        || path.starts_with("/openapi/v1/consumer-tokens")
        || path.starts_with("/openapi/v1/users")
        || path.starts_with("/openapi/v1/user-tokens")
        || path.starts_with("/openapi/v1/roles")
        || path.starts_with("/openapi/v1/permissions")
        || path.starts_with("/openapi/v1/organizations")
        || path.starts_with("/openapi/v1/system/roles")
        || path.starts_with("/openapi/v1/audit")
        || path.starts_with("/openapi/v1/logs")
        || path.starts_with("/openapi/v1/trace")
        || path == "/openapi/v1/user"
        || path.contains("/roles")
        || path.contains("/permissions")
        || path.contains("/permission-init")
    {
        return true;
    }
    false
}

/// Compatibility identity used only when OpenAPI auth is disabled and no token
/// was presented. It preserves existing SDK behavior while still letting an
/// explicitly presented token be validated.
async fn compat_admin_identity(
    persistence: Option<&std::sync::Arc<dyn ApolloPersistenceService>>,
) -> Result<Identity, Error> {
    let username = match persistence {
        Some(persistence) => {
            ServerConfigPersistence::get_server_config(&**persistence, "superAdmin")
                .await
                .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?
                .map(|config| config.value)
                .unwrap_or_else(|| "apollo".to_string())
        }
        None => "apollo".to_string(),
    };
    Ok(Identity::User(UserDTO {
        id: None,
        username,
        password: String::new(),
        email: None,
        enabled: true,
        data_change_created_by: None,
        data_change_created_time: None,
    }))
}

/// Extracts a credential that is unambiguously a user token.
///
/// `X-Apollo-Token` is batata-ui's header; `user-token` and `token` are the
/// legacy aliases accepted by upstream's `UserTokenAuthenticationFilter`.
fn extract_explicit_user_token(req: &ServiceRequest) -> Option<String> {
    for header in ["X-Apollo-Token", "user-token"] {
        if let Some(value) = req
            .headers()
            .get(header)
            .and_then(|h| h.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    req.query_string().split('&').find_map(|kv| {
        let (key, value) = kv.split_once('=')?;
        (key == "token" && !value.is_empty()).then(|| value.to_string())
    })
}

/// Extracts the `Authorization` credential, stripping an optional scheme.
///
/// `Consumer` and `Basic` are excluded here because they can never carry a
/// user token. The same value is later tried as a consumer token.
fn extract_authorization_credential(req: &ServiceRequest) -> Option<String> {
    let trimmed = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(str::trim)?;
    if let Some((scheme, value)) = trimmed.split_once(' ') {
        if scheme.eq_ignore_ascii_case("Consumer") || scheme.eq_ignore_ascii_case("Basic") {
            return None;
        }
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    } else {
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }
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
