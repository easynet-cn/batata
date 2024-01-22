mod console;

use actix_web::{web, App, HttpServer};
use config::Config;

use console::v1::health::{liveness, readiness};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let settings = Config::builder()
        .add_source(config::File::with_name("conf/application.yml"))
        .build()
        .unwrap();
    let address = settings
        .get_string("server.address")
        .unwrap_or("0.0.0.0".to_string());
    let server_port = settings.get_int("server.port").unwrap() as u16;

    HttpServer::new(|| {
        App::new().service(
            web::scope("nacos").service(
                web::scope("/v1/console/health")
                    .service(liveness)
                    .service(readiness),
            ),
        )
    })
    .bind((address, server_port))?
    .run()
    .await
}
