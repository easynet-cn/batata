use crate::api::v2::model::Result;
use actix_web::{get, web};

#[get("/liveness")]
pub async fn liveness() -> web::Json<Result<String>> {
    web::Json(Result::<String>::success("ok".to_string()))
}

#[get("/readiness")]
pub async fn readiness() -> web::Json<Result<String>> {
    web::Json(Result::<String>::success("ok".to_string()))
}
