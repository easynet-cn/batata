use actix_web::{Scope, web};

pub fn routes() -> Scope {
    web::scope("/v1/auth").service(super::auth::login)
}
