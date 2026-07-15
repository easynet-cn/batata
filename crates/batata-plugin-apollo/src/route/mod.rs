use actix_web::{web, HttpResponse, Responder};

pub mod admin;
pub mod config;
pub mod openapi;

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

pub fn configure_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.route("/health", web::get().to(health_check));
    admin::configure_admin_routes(cfg);
    config::configure_config_routes(cfg);
    openapi::configure_openapi_routes(cfg);
}
