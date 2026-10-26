use actix_web::{HttpResponse, Responder, post, web};
use serde::Deserialize;

use crate::api::auth::{
    auth_data,
    model::{AUTHORIZATION_HEADER, TOKEN_PREFIX},
    service::auth::encode_jwt_token,
};
use crate::model::AppState;
use crate::model::response::Result as ApiResult;

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
async fn init_admin(data: web::Data<AppState>, form: web::Form<InitAdminData>) -> impl Responder {
    let username = form.username.clone().unwrap_or_default();
    let password = form.password.clone().unwrap_or_default();

    if username.is_empty() || password.is_empty() {
        return ApiResult::<String>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            "username and password cannot be empty",
        );
    }

    if batata_server_api::validation::validate_username(&username).is_err() {
        return ApiResult::<String>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            format!(
                "invalid username format: must be alphanumeric, _, -, @, . (max {} chars)",
                batata_server_api::validation::MAX_USERNAME_LENGTH
            ),
        );
    }

    // Skip strict password validation for init_admin (bootstrap endpoint)
    // to allow Nacos-compatible default "nacos"/"nacos" credentials
    if password.len() > batata_server_api::validation::MAX_PASSWORD_LENGTH {
        return ApiResult::<String>::http_bad_request(
            &batata_common::error::PARAMETER_VALIDATE_ERROR,
            format!(
                "invalid password: must be at most {} characters",
                batata_server_api::validation::MAX_PASSWORD_LENGTH
            ),
        );
    }

    // Bootstrap is served from the deployment-aware auth data backend: a merged
    // deployment writes to its own store, while an independently deployed console
    // has no database and must ask the core instead.
    let auth = match auth_data(&data) {
        Ok(service) => service,
        Err(response) => return response,
    };

    // Check if a global admin already exists
    let has_admin = match auth.role_has_global_admin().await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to check global admin existence: {}", e);
            return ApiResult::<String>::http_internal_error(e);
        }
    };

    if has_admin {
        return ApiResult::<String>::http_response(
            409,
            batata_common::error::RESOURCE_CONFLICT.code,
            batata_common::error::RESOURCE_CONFLICT.message.to_string(),
            "admin user already exists",
        );
    }

    // Create the user and hand it the global admin role. The plaintext password
    // goes to the backend on purpose: the direct backend hashes it, the remote
    // backend forwards it so the core does.
    if let Err(e) = auth.create_admin_user(&username, &password).await {
        tracing::error!("Failed to create admin user '{}': {}", username, e);
        return ApiResult::<String>::http_internal_error(e);
    }

    // The new admin's cached role lookups are stale from here on.
    batata_auth::service::rbac_cache::invalidate_user(&username);

    tracing::info!(username = %username, "Admin user initialized successfully");

    // Generate JWT token
    let token_secret_key = data.configuration.token_secret_key();
    let token_expire_seconds = data.configuration.auth_token_expire_seconds();

    let access_token =
        match encode_jwt_token(&username, token_secret_key.as_str(), token_expire_seconds) {
            Ok(token) => token,
            Err(e) => {
                return ApiResult::<String>::http_internal_error(e);
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
