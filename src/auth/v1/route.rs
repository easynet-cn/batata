use actix_web::{Scope, web};

pub fn routes() -> Scope {
    return web::scope("/v1/auth").service(super::auth::login);
}
