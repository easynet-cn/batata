//! OIDC (OpenID Connect) authentication endpoints.
//!
//! Provides Nacos-compatible OIDC login/logout endpoints at `/v1/auth/oidc/*`.
//! Implements the OIDC Authorization Code flow in single-IdP mode:
//!
//! - `GET  /login`    - Redirect to IdP authorization URL
//! - `GET  /callback` - Handle IdP callback, set cookies, redirect to frontend
//! - `GET  /logout`   - Clear cookies, optionally redirect to IdP logout
//! - `POST /logout`   - Same as GET logout (form-friendly)
//! - `GET  /config`   - Return OIDC configuration for frontend
//!
//! Cookie behavior mirrors Nacos:
//! - `accessToken` and `username` cookies are `HttpOnly=false` (frontend-readable)
//! - Cookies expire after 60 seconds (frontend syncs to localStorage then clears)

use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::Cookie;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::api::auth::{
    model::{NON_LOCAL_PASSWORD_SENTINEL, USER_SOURCE_OAUTH},
    service::auth::encode_jwt_token,
};
use crate::model::AppState;
use crate::model::response::Result as ApiResult;

/// Cookie expiration time in seconds.
///
/// Short-lived: the frontend reads the cookie, syncs the token to
/// `localStorage`, and then clears the cookie.  Keeping it at 60 s
/// matches the Nacos reference implementation.
const COOKIE_EXPIRATION_SECONDS: i64 = 60;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Query parameters received on the OIDC callback endpoint.
#[derive(Debug, Deserialize)]
struct OidcCallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    #[serde(rename = "error_description")]
    error_description: Option<String>,
}

/// Parameters accepted by the logout endpoint (query or form).
#[derive(Debug, Deserialize)]
struct OidcLogoutParams {
    /// Optional ID token hint for IdP RP-initiated logout.
    #[serde(rename = "idToken", default)]
    id_token: Option<String>,
    /// When `true`, attempt to redirect to the IdP `end_session_endpoint`.
    #[serde(rename = "redirect", default)]
    redirect: bool,
}

/// OIDC configuration response returned to the frontend console.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OidcConfigResponse {
    enabled: bool,
    auth_type: String,
    login_url: String,
    user_management_enabled: bool,
    role_management_enabled: bool,
    permission_management_enabled: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Select the primary provider for single-IdP OIDC mode.
///
/// Uses the first enabled provider. If an OIDC-type provider is configured
/// it should appear first in the provider list so it is picked automatically.
fn get_primary_provider(
    oauth_service: &dyn batata_common::OAuthProvider,
) -> Option<String> {
    oauth_service.get_enabled_providers().into_iter().next()
}

/// Extract the base URL (`scheme://host[:port]`) from the request.
fn get_base_url(req: &HttpRequest) -> String {
    let conn_info = req.connection_info();
    format!("{}://{}", conn_info.scheme(), conn_info.host())
}

/// Build the OIDC callback URL from the incoming request.
fn build_callback_url(req: &HttpRequest) -> String {
    format!("{}/v1/auth/oidc/callback", get_base_url(req))
}

/// URL-encode a string for use in a query parameter value.
fn url_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Build a frontend error-redirect URL: `/#/login?error=<encoded>`.
fn build_error_redirect_url(req: &HttpRequest, error_msg: &str) -> String {
    format!(
        "{}/#/login?error={}",
        get_base_url(req),
        url_encode(error_msg)
    )
}

/// Whether the request arrived over HTTPS.
fn is_https(req: &HttpRequest) -> bool {
    req.connection_info().scheme() == "https"
}

/// Build an `accessToken` cookie matching the Nacos convention.
fn build_access_token_cookie<'a>(token: &'a str, req: &HttpRequest) -> Cookie<'a> {
    Cookie::build("accessToken", token)
        .path("/")
        .http_only(false)
        .secure(is_https(req))
        .max_age(CookieDuration::seconds(COOKIE_EXPIRATION_SECONDS))
        .finish()
}

/// Build a `username` cookie (URL-encoded value) matching the Nacos convention.
fn build_username_cookie<'a>(encoded_username: &'a str, req: &HttpRequest) -> Cookie<'a> {
    Cookie::build("username", encoded_username)
        .path("/")
        .http_only(false)
        .secure(is_https(req))
        .max_age(CookieDuration::seconds(COOKIE_EXPIRATION_SECONDS))
        .finish()
}

/// Build an expired (deletion) cookie for the given name.
fn build_expired_cookie(name: &'static str) -> Cookie<'static> {
    Cookie::build(name, "")
        .path("/")
        .max_age(CookieDuration::seconds(0))
        .finish()
}

// ---------------------------------------------------------------------------
// Endpoints
// ---------------------------------------------------------------------------

/// `GET /login` - Initiate OIDC login by redirecting to the IdP authorization URL.
#[get("login")]
pub async fn oidc_login(data: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let oauth_service: &dyn batata_common::OAuthProvider = match data.oauth_service.as_deref() {
        Some(svc) => svc,
        None => {
            return ApiResult::<String>::http_response(
                500,
                batata_common::error::API_FUNCTION_DISABLED.code,
                "OAuth service is not configured".to_string(),
                String::new(),
            );
        }
    };

    if !oauth_service.is_enabled() {
        return ApiResult::<String>::http_response(
            500,
            batata_common::error::API_FUNCTION_DISABLED.code,
            "OAuth is not enabled".to_string(),
            String::new(),
        );
    }

    let provider_name = match get_primary_provider(oauth_service) {
        Some(name) => name,
        None => {
            return ApiResult::<String>::http_response(
                500,
                batata_common::error::API_FUNCTION_DISABLED.code,
                "No OIDC provider configured".to_string(),
                String::new(),
            );
        }
    };

    let callback_url = build_callback_url(&req);

    match oauth_service
        .get_authorization_url(&provider_name, &callback_url)
        .await
    {
        Ok((auth_url, _state)) => {
            info!("Redirecting to IdP for OIDC authentication");
            HttpResponse::Found()
                .append_header(("Location", auth_url))
                .finish()
        }
        Err(e) => {
            error!("Failed to initiate OIDC login: {}", e);
            ApiResult::<String>::http_response(
                500,
                batata_common::error::SERVER_ERROR.code,
                format!("Failed to initiate login: {}", e),
                String::new(),
            )
        }
    }
}

/// `GET /callback` - Handle the IdP authorization-code response.
///
/// Exchanges the code for tokens, retrieves user info, creates/updates the
/// local user mapping, generates a JWT, and sets short-lived cookies for the
/// frontend to consume.
#[get("callback")]
pub async fn oidc_callback(
    data: web::Data<AppState>,
    query: web::Query<OidcCallbackParams>,
    req: HttpRequest,
) -> HttpResponse {
    // ---- IdP error response -------------------------------------------
    if let Some(ref error_code) = query.error {
        let error_msg = query
            .error_description
            .as_deref()
            .unwrap_or(error_code.as_str());
        warn!("OIDC authentication error: {} - {}", error_code, error_msg);
        return redirect_to_error(&req, error_msg);
    }

    // ---- Validate required parameters ---------------------------------
    let code = match query.code.as_deref().filter(|s| !s.is_empty()) {
        Some(c) => c.to_string(),
        None => return redirect_to_error(&req, "Missing authorization code"),
    };
    let state = match query.state.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => return redirect_to_error(&req, "Missing state parameter"),
    };

    // ---- Resolve OAuth service & provider -----------------------------
    let oauth_service: &dyn batata_common::OAuthProvider = match data.oauth_service.as_deref() {
        Some(svc) => svc,
        None => return redirect_to_error(&req, "OAuth service is not configured"),
    };

    if !oauth_service.is_enabled() {
        return redirect_to_error(&req, "OAuth is not enabled");
    }

    let provider_name = match get_primary_provider(oauth_service) {
        Some(name) => name,
        None => return redirect_to_error(&req, "No OIDC provider configured"),
    };

    let callback_url = build_callback_url(&req);

    // ---- Exchange code for tokens -------------------------------------
    let token_response = match oauth_service
        .exchange_code(&provider_name, &code, &callback_url, &state)
        .await
    {
        Ok(tokens) => tokens,
        Err(e) => {
            error!("OIDC token exchange failed: {}", e);
            return redirect_to_error(&req, &format!("Token exchange failed: {}", e));
        }
    };

    // ---- Retrieve user info -------------------------------------------
    let user_info = match oauth_service
        .get_user_info(&provider_name, &token_response.access_token)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            error!("Failed to get OIDC user info: {}", e);
            return redirect_to_error(&req, &format!("Failed to get user info: {}", e));
        }
    };

    // ---- Create or update local user ----------------------------------
    let username = format!("oauth_{}_{}", provider_name, user_info.provider_user_id);

    let persistence = data.persistence();
    let local_user = match persistence.user_find_by_username(&username).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to query user '{}': {}", username, e);
            return redirect_to_error(&req, &format!("Failed to query user: {}", e));
        }
    };

    if local_user.is_none() {
        match persistence
            .user_create_with_source(
                &username,
                NON_LOCAL_PASSWORD_SENTINEL,
                true,
                USER_SOURCE_OAUTH,
            )
            .await
        {
            Ok(()) => {
                info!(
                    username = %username,
                    provider = %provider_name,
                    "Created local user mapping for OIDC user"
                );
            }
            Err(e) => {
                // Non-fatal: the user can still authenticate with the JWT.
                error!("Failed to create OIDC user '{}': {}", username, e);
            }
        }
    }

    // ---- Generate JWT --------------------------------------------------
    let token_secret_key = data.configuration.token_secret_key();
    let token_expire_seconds = data.configuration.auth_token_expire_seconds();

    let access_token = match encode_jwt_token(&username, &token_secret_key, token_expire_seconds) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token: {}", e);
            return redirect_to_error(&req, &format!("Failed to generate token: {}", e));
        }
    };

    info!("OIDC authentication successful for user: {}", username);

    // ---- Set cookies and redirect to frontend -------------------------
    let encoded_username = url_encode(&username);
    let success_url = format!("{}/#/", get_base_url(&req));

    HttpResponse::Found()
        .cookie(build_access_token_cookie(&access_token, &req))
        .cookie(build_username_cookie(&encoded_username, &req))
        .append_header(("Location", success_url))
        .finish()
}

/// `GET /logout` - Clear session cookies and optionally redirect to IdP logout.
#[get("logout")]
pub async fn oidc_logout_get(
    data: web::Data<AppState>,
    query: web::Query<OidcLogoutParams>,
    req: HttpRequest,
) -> HttpResponse {
    oidc_logout_impl(data, query.into_inner(), &req).await
}

/// `POST /logout` - Same as GET logout but accepts form-encoded parameters.
#[post("logout")]
pub async fn oidc_logout_post(
    data: web::Data<AppState>,
    form: web::Form<OidcLogoutParams>,
    req: HttpRequest,
) -> HttpResponse {
    oidc_logout_impl(data, form.into_inner(), &req).await
}

/// Core logout logic shared by GET and POST handlers.
async fn oidc_logout_impl(
    data: web::Data<AppState>,
    params: OidcLogoutParams,
    req: &HttpRequest,
) -> HttpResponse {
    // Optional IdP RP-initiated logout redirect
    if params.redirect {
        let post_logout_uri = get_base_url(req);

        let logout_url: Option<String> = match data.oauth_service.as_deref() {
            Some(oauth_service) if oauth_service.is_enabled() => {
                match get_primary_provider(oauth_service) {
                    Some(provider_name) => {
                        match oauth_service
                            .build_logout_url(
                                &provider_name,
                                params.id_token.as_deref(),
                                &post_logout_uri,
                            )
                            .await
                        {
                            Ok(url) => url,
                            Err(e) => {
                                error!("Failed to build IdP logout URL: {}", e);
                                None
                            }
                        }
                    }
                    None => None,
                }
            }
            _ => None,
        };

        if let Some(url) = logout_url {
            info!("Redirecting to IdP for RP-initiated logout");
            return HttpResponse::Found()
                .cookie(build_expired_cookie("accessToken"))
                .cookie(build_expired_cookie("username"))
                .append_header(("Location", url))
                .finish();
        }

        // Fall through to local logout if no IdP logout URL is available
        warn!("IdP RP-initiated logout requested but no end_session_endpoint is available");
    }

    info!("User logged out via OIDC");

    HttpResponse::Ok()
        .cookie(build_expired_cookie("accessToken"))
        .cookie(build_expired_cookie("username"))
        .json(ApiResult::success("Logged out successfully"))
}

/// `GET /config` - Return OIDC configuration for the frontend console.
///
/// The console uses this to detect OIDC mode and hide user/role/permission
/// management controls (which are handled by the IdP when OIDC is active).
#[get("config")]
pub async fn oidc_config(data: web::Data<AppState>) -> HttpResponse {
    let enabled = match data.oauth_service.as_deref() {
        Some(svc) => svc.is_enabled() && get_primary_provider(svc).is_some(),
        None => false,
    };

    let config = OidcConfigResponse {
        enabled,
        auth_type: "oidc".to_string(),
        login_url: "/v1/auth/oidc/login".to_string(),
        // When OIDC is enabled, user/role/permission management is handled by IdP
        user_management_enabled: false,
        role_management_enabled: false,
        permission_management_enabled: false,
    };

    ApiResult::<OidcConfigResponse>::http_success(config)
}

// ---------------------------------------------------------------------------
// Internal redirect helper
// ---------------------------------------------------------------------------

/// Issue a 302 redirect to the frontend login page with an error parameter.
fn redirect_to_error(req: &HttpRequest, error_msg: &str) -> HttpResponse {
    HttpResponse::Found()
        .append_header(("Location", build_error_redirect_url(req, error_msg)))
        .finish()
}
