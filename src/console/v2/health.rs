use crate::api::model::v2::result;
use actix_web::{get, web};

#[get("/liveness")]
pub async fn liveness() -> web::Json<result::Result<String>> {
    web::Json(result::success("ok".to_string()))
}

#[get("/readiness")]
pub async fn readiness() -> web::Json<result::Result<String>> {
    web::Json(result::success("ok".to_string()))
}
