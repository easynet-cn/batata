use crate::api::v2::model;
use actix_web::{get, web};

#[get("/liveness")]
pub async fn liveness() -> web::Json<model::Result<String>> {
    web::Json(model::success("ok".to_string()))
}

#[get("/readiness")]
pub async fn readiness() -> web::Json<model::Result<String>> {
    web::Json(model::success("ok".to_string()))
}
