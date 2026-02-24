use actix_web::{Scope, web};

pub fn routes() -> Scope {
    web::scope("/v3/auth")
        .service(super::auth::login)
        .service(super::user::search_page)
        .service(super::user::search)
        .service(super::user::update)
        .service(super::user::create)
        .service(super::user::delete)
        .service(super::role::search_page)
        .service(super::role::create)
        .service(super::role::delete)
        .service(super::role::search)
        .service(super::permission::exist)
        .service(super::permission::search_page)
        .service(super::permission::create)
        .service(super::permission::delete)
        // OAuth2/OIDC endpoints
        .service(super::oauth::get_providers)
        .service(super::oauth::oauth_login)
        .service(super::oauth::oauth_callback)
}

/// V1 API routes for backward compatibility with Nacos SDK
/// Nacos SDK tries /v3/auth/user/login first, then falls back to /v1/auth/users/login
pub fn v1_routes() -> Scope {
    web::scope("/v1/auth/users")
        .service(super::auth::login_v1)
}
