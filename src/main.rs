use actix_web::{web, App, HttpServer};
use config::Config;
use sea_orm::{Database, DatabaseConnection};

pub mod api;
pub mod common;
pub mod console;
pub mod core;

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
    let db_num = settings.get_int("db.num").unwrap();
    let mut conns: Vec<DatabaseConnection> = Vec::new();

    for i in 0..db_num {
        let db = Database::connect(
            &settings
                .get_string(format!("db.url.{}", i).as_str())
                .unwrap(),
        )
        .await
        .unwrap();

        conns.push(db);
    }

    let app_state = api::model::AppState { conns };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .service(
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
                    )
                    .service(
                        web::scope("/v1/console/namespaces")
                            .service(console::v1::namespace::get_namespaces),
                    ),
            )
    })
    .bind((address, server_port))?
    .run()
    .await
}
