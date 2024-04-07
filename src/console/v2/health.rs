use crate::api::v2::model::Result;
use actix_web::{get, web, Scope};

#[get("/liveness")]
pub async fn liveness() -> web::Json<Result<String>> {
    web::Json(Result::<String>::success("ok".to_string()))
}

#[get("/readiness")]
pub async fn readiness() -> web::Json<Result<String>> {
    web::Json(Result::<String>::success("ok".to_string()))
}

pub fn routes() -> Scope {
    return web::scope("/health").service(liveness).service(readiness);
}
