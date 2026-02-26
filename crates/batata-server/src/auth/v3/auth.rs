use actix_web::{HttpResponse, Responder, post, web};
use serde::{Deserialize, Serialize};

use batata_auth::service::ldap::LdapAuthService;

use crate::{
    auth::{
        model::{AUTHORIZATION_HEADER, TOKEN_PREFIX, USER_NOT_FOUND_MESSAGE},
        service::auth::encode_jwt_token,
    },
    model::common::AppState,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResult {
    access_token: String,
    token_ttl: i64,
    global_admin: bool,
    username: String,
}

#[derive(Deserialize)]
struct LoginData {
    username: Option<String>,
    password: Option<String>,
}

#[post("user/login")]
async fn login(
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> impl Responder {
    internal_login(data, form, query).await
}

/// V1 API endpoint for backward compatibility with Nacos SDK
/// Route: POST /v1/auth/users/login
/// Nacos SDK sends username as query param and password as form body
#[post("/login")]
async fn login_v1(
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> impl Responder {
    internal_login(data, form, query).await
}

/// Internal login handler shared by V1 and V3 APIs
async fn internal_login(
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> HttpResponse {
    let mut username: String = "".to_string();
    let mut password: String = "".to_string();

    // Nacos SDK sends username as query param and password as form body,
    // so we merge both sources (query params take precedence for username,
    // form body takes precedence for password).
    if let Some(query_data) = &query {
        if let Some(v) = &query_data.username {
            username = v.to_string();
        }
        if let Some(v) = &query_data.password {
            password = v.to_string();
        }
    }
    if let Some(form_data) = &form {
        if let Some(v) = &form_data.username
            && !v.is_empty()
        {
            username = v.to_string();
        }
        if let Some(v) = &form_data.password
            && !v.is_empty()
        {
            password = v.to_string();
        }
    }

    if username.is_empty() || password.is_empty() {
        return HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE);
    }

    // Check if LDAP authentication is enabled
    if data.configuration.is_ldap_auth_enabled() {
        return ldap_login(&data, &username, &password).await;
    }

    // Standard Nacos authentication
    nacos_login(&data, &username, &password).await
}

/// Perform LDAP authentication
async fn ldap_login(data: &web::Data<AppState>, username: &str, password: &str) -> HttpResponse {
    let ldap_config = data.configuration.ldap_config();

    if !ldap_config.is_configured() {
        tracing::error!("LDAP authentication is enabled but not configured");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": "LDAP is not properly configured",
            "data": null
        }));
    }

    let ldap_service = LdapAuthService::new(ldap_config);
    let auth_result = ldap_service.authenticate(username, password).await;

    if !auth_result.success {
        tracing::warn!(
            username = %username,
            error = ?auth_result.error_message,
            "LDAP authentication failed"
        );
        return HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE);
    }

    // LDAP authentication successful, now check if user exists in local database
    let local_user = match data
        .persistence()
        .user_find_by_username(&auth_result.username)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to query user '{}': {}", auth_result.username, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to query user from database",
                "data": null
            }));
        }
    };

    // If user doesn't exist locally, create a placeholder user
    // This allows LDAP users to be assigned roles/permissions locally
    if local_user.is_none() {
        // Create user with LDAP prefix to indicate it's an LDAP user
        // The password is set to a placeholder since auth is via LDAP
        let placeholder_password = format!("LDAP_{}", uuid::Uuid::new_v4());
        let hashed_password = match bcrypt::hash(&placeholder_password, 10u32) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to hash placeholder password: {}", e);
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "code": 500,
                    "message": "Failed to create LDAP user mapping",
                    "data": null
                }));
            }
        };

        if let Err(e) = data
            .persistence()
            .user_create(&auth_result.username, &hashed_password, true)
            .await
        {
            tracing::error!(
                "Failed to create LDAP user '{}': {}",
                auth_result.username,
                e
            );
            // Continue anyway - user can still authenticate, just won't have local record
        } else {
            tracing::info!(
                username = %auth_result.username,
                "Created local user mapping for LDAP user"
            );
        }
    }

    // Generate JWT token
    generate_token_response(data, &auth_result.username).await
}

/// Perform standard Nacos authentication
async fn nacos_login(data: &web::Data<AppState>, username: &str, password: &str) -> HttpResponse {
    let user_option = match data.persistence().user_find_by_username(username).await {
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

    let user = match user_option {
        Some(u) => u,
        None => return HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE),
    };

    let bcrypt_result = bcrypt::verify(password, &user.password).unwrap_or(false);

    if bcrypt_result {
        return generate_token_response(data, &user.username).await;
    }

    HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE)
}

/// Generate JWT token and return login response
async fn generate_token_response(data: &web::Data<AppState>, username: &str) -> HttpResponse {
    let token_secret_key = data.configuration.token_secret_key();
    let token_expire_seconds = data.configuration.auth_token_expire_seconds();

    let access_token =
        match encode_jwt_token(username, token_secret_key.as_str(), token_expire_seconds) {
            Ok(token) => token,
            Err(_) => {
                return HttpResponse::InternalServerError().body("Failed to generate token");
            }
        };

    let global_admin = data
        .persistence()
        .role_has_global_admin_by_username(username)
        .await
        .ok()
        .unwrap_or_default();

    let login_result = LoginResult {
        access_token: access_token.clone(),
        token_ttl: token_expire_seconds,
        global_admin,
        username: username.to_string(),
    };

    HttpResponse::Ok()
        .append_header((
            AUTHORIZATION_HEADER,
            format!("{}{}", TOKEN_PREFIX, access_token),
        ))
        .json(login_result)
}
