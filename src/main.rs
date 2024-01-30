use actix_web::{web, App, HttpServer};
use config::Config;

pub mod api {
    pub mod model {
        pub mod v2 {
            pub mod error_code;
            pub mod result;
        }
    }
}

pub mod console {
    pub mod v1 {
        pub mod health;
    }
    pub mod v2 {
        pub mod health;
    }
}

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
    let context_path = settings
        .get_string("server.servlet.contextPath")
        .unwrap_or("nacos".to_string())
        .clone();

    HttpServer::new(move || {
        App::new().service(
            web::scope(&context_path)
                .service(
                    web::scope("/v1/console/health")
                        .service(console::v1::health::liveness)
                        .service(console::v1::health::readiness),
                )
                .service(
                    web::scope("/v2/console/health")
                        .service(console::v2::health::liveness)
                        .service(console::v2::health::readiness),
                ),
        )
    })
    .bind((address, server_port))?
    .run()
    .await
}
