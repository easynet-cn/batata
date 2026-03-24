use std::sync::LazyLock;
use std::time::{Duration, Instant};

use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::api::auth::model::{AUTHORIZATION_HEADER, TOKEN_PREFIX};
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
        return HttpResponse::Forbidden().body("user not found!");
    }

    // Delegate to auth plugin for login
    let auth_plugin = match data.auth_plugin.as_ref() {
        Some(p) => p,
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "auth plugin not configured",
            }));
        }
    };

    match auth_plugin.login(&username, &password).await {
        Ok(result) => {
            record_successful_login(&client_ip);

            let login_result = LoginResult {
                access_token: result.token.clone(),
                token_ttl: result.token_ttl,
                global_admin: result.is_global_admin,
                username: result.username,
            };

            HttpResponse::Ok()
                .append_header((
                    AUTHORIZATION_HEADER,
                    format!("{}{}", TOKEN_PREFIX, result.token),
                ))
                .json(login_result)
        }
        Err(msg) => {
            record_failed_login(&client_ip);
            HttpResponse::Forbidden().body(msg)
        }
    }
}
