use actix_web::web;

pub fn routes() -> actix_web::Scope {
    web::scope("/")
}
