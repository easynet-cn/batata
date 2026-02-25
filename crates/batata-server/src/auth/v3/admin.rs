use actix_web::{HttpResponse, Responder, post, web};
use serde::Deserialize;

use crate::{
    auth::{
        model::{AUTHORIZATION_HEADER, GLOBAL_ADMIN_ROLE, TOKEN_PREFIX},
        service::auth::encode_jwt_token,
    },
    model::common::AppState,
};

#[derive(Deserialize)]
struct InitAdminData {
    username: Option<String>,
    password: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResult {
    access_token: String,
    token_ttl: i64,
    global_admin: bool,
    username: String,
}

#[post("/user/admin")]
async fn init_admin(
    data: web::Data<AppState>,
    form: web::Form<InitAdminData>,
) -> impl Responder {
    let username = form.username.clone().unwrap_or_default();
    let password = form.password.clone().unwrap_or_default();

    if username.is_empty() || password.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "username and password cannot be empty"
        }));
    }

    // Check if a global admin already exists
    let has_admin = match data.persistence().role_has_global_admin().await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to check global admin existence: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to check admin status"
            }));
        }
    };

    if has_admin {
        return HttpResponse::Conflict().json(serde_json::json!({
            "code": 409,
            "message": "admin user already exists"
        }));
    }

    // Hash password
    let password_hash = match bcrypt::hash(&password, 10u32) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to hash password: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "code": 500,
                "message": "Failed to hash password"
            }));
        }
    };

    // Create user
    if let Err(e) = data
        .persistence()
        .user_create(&username, &password_hash, true)
        .await
    {
        tracing::error!("Failed to create admin user '{}': {}", username, e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": "Failed to create admin user"
        }));
    }

    // Create admin role
    if let Err(e) = data
        .persistence()
        .role_create(GLOBAL_ADMIN_ROLE, &username)
        .await
    {
        tracing::error!("Failed to create admin role for '{}': {}", username, e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": "Failed to create admin role"
        }));
    }

    tracing::info!(username = %username, "Admin user initialized successfully");

    // Generate JWT token
    let token_secret_key = data.configuration.token_secret_key();
    let token_expire_seconds = data.configuration.auth_token_expire_seconds();

    let access_token =
        match encode_jwt_token(&username, token_secret_key.as_str(), token_expire_seconds) {
            Ok(token) => token,
            Err(_) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "code": 500,
                    "message": "Failed to generate token"
                }));
            }
        };

    let login_result = LoginResult {
        access_token: access_token.clone(),
        token_ttl: token_expire_seconds,
        global_admin: true,
        username,
    };

    HttpResponse::Ok()
        .append_header((
            AUTHORIZATION_HEADER,
            format!("{}{}", TOKEN_PREFIX, access_token),
        ))
        .json(login_result)
}
