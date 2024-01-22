use actix_web::{get, HttpResponse, Responder};

#[get("/liveness")]
pub async fn liveness() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[get("/readiness")]
pub async fn readiness() -> impl Responder {
    HttpResponse::Ok().body("OK")
}
