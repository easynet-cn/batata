use actix_web::{get, web, HttpResponse, Responder, Scope};

#[get("/liveness")]
pub async fn liveness() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[get("/readiness")]
pub async fn readiness() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

pub fn routes() -> Scope {
    return web::scope("/health").service(liveness).service(readiness);
}
