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
