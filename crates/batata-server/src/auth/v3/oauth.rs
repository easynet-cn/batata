//! OAuth2/OIDC authentication endpoints
//!
//! Provides OAuth2 and OpenID Connect login endpoints for Batata.

use actix_web::{HttpRequest, HttpResponse, Responder, get, web};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        model::{AUTHORIZATION_HEADER, TOKEN_PREFIX},
        service::auth::encode_jwt_token,
    },
    model::common::AppState,
};

/// OAuth provider info for client
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthProviderInfo {
    name: String,
    login_url: String,
}

/// OAuth providers response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthProvidersResponse {
    enabled: bool,
    providers: Vec<OAuthProviderInfo>,
}

/// OAuth callback query parameters
#[derive(Debug, Deserialize)]
struct OAuthCallback {
    code: String,
    state: String,
}

/// Login result after OAuth authentication
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OAuthLoginResult {
    access_token: String,
    token_ttl: i64,
    global_admin: bool,
    username: String,
    provider: String,
}

/// Get available OAuth providers
#[get("oauth/providers")]
pub async fn get_providers(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let oauth_service = match data.oauth_service.as_ref() {
        Some(service) => service,
        None => {
            return HttpResponse::Ok().json(OAuthProvidersResponse {
                enabled: false,
                providers: vec![],
            });
        }
    };

    if !oauth_service.is_enabled() {
        return HttpResponse::Ok().json(OAuthProvidersResponse {
            enabled: false,
            providers: vec![],
        });
    }

    let base_url = get_base_url(&req);
    let providers: Vec<OAuthProviderInfo> = oauth_service
        .get_enabled_providers()
        .into_iter()
        .map(|name| OAuthProviderInfo {
            login_url: format!("{}/v3/auth/oauth/login/{}", base_url, name),
            name,
        })
        .collect();

    HttpResponse::Ok().json(OAuthProvidersResponse {
        enabled: true,
        providers,
    })
}

/// Initiate OAuth login flow - redirects to provider
#[get("oauth/login/{provider}")]
pub async fn oauth_login(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
) -> impl Responder {
    let provider_name = path.into_inner();

    let oauth_service = match data.oauth_service.as_ref() {
        Some(service) => service,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "code": 400,
                "message": "OAuth is not enabled",
                "data": null
            }));
        }
    };

    let base_url = get_base_url(&req);
    // Include provider in callback URL for easier routing
    let redirect_uri = format!("{}/v3/auth/oauth/callback/{}", base_url, provider_name);

    match oauth_service
        .get_authorization_url(&provider_name, &redirect_uri)
        .await
    {
        Ok((auth_url, _state)) => HttpResponse::Found()
            .append_header(("Location", auth_url))
            .finish(),
        Err(e) => {
            tracing::error!("Failed to generate OAuth authorization URL: {}", e);
            HttpResponse::BadRequest().json(serde_json::json!({
                "code": 400,
                "message": format!("Invalid OAuth provider: {}", provider_name),
                "data": null
            }))
        }
    }
}

/// OAuth callback endpoint - handles provider redirect
#[get("oauth/callback/{provider}")]
pub async fn oauth_callback(
    data: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<OAuthCallback>,
    req: HttpRequest,
) -> impl Responder {
    let provider_name = path.into_inner();

    let oauth_service = match data.oauth_service.as_ref() {
        Some(service) => service,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "code": 400,
                "message": "OAuth is not enabled",
                "data": null
            }));
        }
    };

    let base_url = get_base_url(&req);
    let redirect_uri = format!("{}/v3/auth/oauth/callback/{}", base_url, provider_name);

    // Exchange code for tokens
    let token_response = match oauth_service
        .exchange_code(&provider_name, &query.code, &redirect_uri, &query.state)
        .await
    {
        Ok(tokens) => tokens,
        Err(e) => {
            tracing::error!("OAuth token exchange failed: {}", e);
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "code": 401,
                "message": "OAuth authentication failed",
                "data": null
            }));
        }
    };

    // Get user info from provider
    let user_info = match oauth_service
        .get_user_info(&provider_name, &token_response.access_token)
        .await
    {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to get OAuth user info: {}", e);
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "code": 401,
                "message": "Failed to retrieve user information from OAuth provider",
                "data": null
            }));
        }
    };

    // Create or update local user
    let username = format!("oauth_{}_{}", provider_name, user_info.provider_user_id);

    // Check if user exists locally
    let local_user = match crate::auth::service::user::find_by_username(data.db(), &username).await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to query user '{}': {}", username, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to query user from database",
                "data": null
            }));
        }
    };

    // Create user if not exists
    if local_user.is_none() {
        let placeholder_password = format!("OAUTH_{}", uuid::Uuid::new_v4());
        let hashed_password = match bcrypt::hash(&placeholder_password, 10u32) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to hash placeholder password: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "code": 500,
                    "message": "Failed to create OAuth user",
                    "data": null
                }));
            }
        };

        if let Err(e) =
            crate::auth::service::user::create(data.db(), &username, &hashed_password).await
        {
            tracing::error!("Failed to create OAuth user '{}': {}", username, e);
            // Continue anyway - user can still authenticate
        } else {
            tracing::info!(
                username = %username,
                provider = %provider_name,
                "Created local user mapping for OAuth user"
            );
        }
    }

    // Generate JWT token
    let token_secret_key = data.configuration.token_secret_key();
    let token_expire_seconds = data.configuration.auth_token_expire_seconds();

    let access_token = match encode_jwt_token(&username, &token_secret_key, token_expire_seconds) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to generate JWT token: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to generate access token",
                "data": null
            }));
        }
    };

    let global_admin =
        crate::auth::service::role::has_global_admin_role_by_username(data.db(), &username)
            .await
            .ok()
            .unwrap_or_default();

    let login_result = OAuthLoginResult {
        access_token: access_token.clone(),
        token_ttl: token_expire_seconds,
        global_admin,
        username: username.clone(),
        provider: provider_name,
    };

    HttpResponse::Ok()
        .append_header((
            AUTHORIZATION_HEADER,
            format!("{}{}", TOKEN_PREFIX, access_token),
        ))
        .json(login_result)
}

/// Get base URL from request
fn get_base_url(req: &HttpRequest) -> String {
    let conn_info = req.connection_info();
    format!("{}://{}", conn_info.scheme(), conn_info.host())
}
