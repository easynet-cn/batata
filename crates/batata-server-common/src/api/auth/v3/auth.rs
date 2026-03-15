use std::sync::LazyLock;
use std::time::{Duration, Instant};

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use batata_auth::service::ldap::LdapAuthService;

use crate::api::auth::{
    model::{AUTHORIZATION_HEADER, TOKEN_PREFIX, USER_NOT_FOUND_MESSAGE},
    service::auth::encode_jwt_token,
};
use crate::model::AppState;

/// Login attempt tracker for rate limiting
struct LoginAttempt {
    count: u32,
    first_attempt: Instant,
    locked_until: Option<Instant>,
}

static LOGIN_ATTEMPTS: LazyLock<DashMap<String, LoginAttempt>> = LazyLock::new(DashMap::new);

/// Maximum failed login attempts before lockout
const MAX_LOGIN_ATTEMPTS: u32 = 5;
/// Lockout duration in seconds
const LOCKOUT_DURATION_SECS: u64 = 300; // 5 minutes
/// Window for counting attempts in seconds
const ATTEMPT_WINDOW_SECS: u64 = 600; // 10 minutes

fn check_login_rate_limit(client_ip: &str) -> Result<(), String> {
    if let Some(mut attempt) = LOGIN_ATTEMPTS.get_mut(client_ip) {
        // Check if locked out
        if let Some(locked_until) = attempt.locked_until {
            if Instant::now() < locked_until {
                let remaining = locked_until.duration_since(Instant::now()).as_secs();
                return Err(format!(
                    "Account locked. Try again in {} seconds.",
                    remaining
                ));
            }
            // Lockout expired, reset
            attempt.count = 0;
            attempt.locked_until = None;
            attempt.first_attempt = Instant::now();
        }

        // Check if window expired, reset if so
        if attempt.first_attempt.elapsed().as_secs() > ATTEMPT_WINDOW_SECS {
            attempt.count = 0;
            attempt.first_attempt = Instant::now();
        }
    }
    Ok(())
}

fn record_failed_login(client_ip: &str) {
    let mut entry = LOGIN_ATTEMPTS
        .entry(client_ip.to_string())
        .or_insert(LoginAttempt {
            count: 0,
            first_attempt: Instant::now(),
            locked_until: None,
        });
    entry.count += 1;
    if entry.count >= MAX_LOGIN_ATTEMPTS {
        entry.locked_until = Some(Instant::now() + Duration::from_secs(LOCKOUT_DURATION_SECS));
        tracing::warn!(
            "IP {} locked out after {} failed login attempts",
            client_ip,
            entry.count
        );
    }
}

fn record_successful_login(client_ip: &str) {
    LOGIN_ATTEMPTS.remove(client_ip);
}

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
    req: HttpRequest,
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> impl Responder {
    internal_login(req, data, form, query).await
}

/// V1 API endpoint for backward compatibility with Nacos SDK
/// Route: POST /v1/auth/users/login
/// Nacos SDK sends username as query param and password as form body
#[post("/login")]
async fn login_v1(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> impl Responder {
    internal_login(req, data, form, query).await
}

/// Internal login handler shared by V1 and V3 APIs
async fn internal_login(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: Option<web::Form<LoginData>>,
    query: Option<web::Query<LoginData>>,
) -> HttpResponse {
    // Extract client IP for rate limiting
    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    // Check login rate limit before processing
    if let Err(msg) = check_login_rate_limit(&client_ip) {
        return HttpResponse::TooManyRequests().json(serde_json::json!({
            "code": 429,
            "message": msg,
        }));
    }

    let mut username = String::new();
    let mut password = String::new();

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
        record_failed_login(&client_ip);
        return HttpResponse::Forbidden().body(USER_NOT_FOUND_MESSAGE);
    }

    // Perform authentication and track result for rate limiting
    let response = if data.configuration.is_ldap_auth_enabled() {
        // Nacos-compatible admin bypass: admin users can authenticate
        // with local password even when LDAP is enabled.
        // Check if the user is a global admin in the local database first.
        let mut result = None;
        if let Ok(true) = data
            .persistence()
            .role_has_global_admin_by_username(&username)
            .await
        {
            // Try local auth for admin users
            let local_result = nacos_login(&data, &username, &password).await;
            if local_result.status().is_success() {
                result = Some(local_result);
            }
            // If local auth fails, fall through to LDAP
        }

        result.unwrap_or(ldap_login(&data, &username, &password).await)
    } else {
        // Standard Nacos authentication
        nacos_login(&data, &username, &password).await
    };

    // Track login result for rate limiting
    if response.status().is_success() {
        record_successful_login(&client_ip);
    } else {
        record_failed_login(&client_ip);
    }

    response
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

    // LDAP authentication successful
    // Nacos-compatible: store LDAP users with "LDAP_" prefix in database
    // to distinguish them from native users
    let ldap_username = format!("LDAP_{}", auth_result.username);

    // Check if user exists in local database (with LDAP prefix)
    let local_user = match data
        .persistence()
        .user_find_by_username(&ldap_username)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to query user '{}': {}", ldap_username, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to query user from database",
                "data": null
            }));
        }
    };

    // If user doesn't exist locally, create a placeholder user with LDAP_ prefix
    // This allows LDAP users to be assigned roles/permissions locally
    if local_user.is_none() {
        // Nacos uses a fixed encoded password for LDAP users
        let ldap_default_password = "LDAP_DEFAULT_ENCODED_PASSWORD";
        let hashed_password = match bcrypt::hash(ldap_default_password, 10u32) {
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
            .user_create(&ldap_username, &hashed_password, true)
            .await
        {
            tracing::error!("Failed to create LDAP user '{}': {}", ldap_username, e);
            // Continue anyway - user can still authenticate, just won't have local record
        } else {
            tracing::info!(
                username = %ldap_username,
                "Created local user mapping for LDAP user (Nacos-compatible LDAP_ prefix)"
            );
        }
    }

    // Generate JWT token using the original username (not prefixed)
    // This keeps the token consistent with what the user expects
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
