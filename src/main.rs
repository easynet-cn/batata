use crate::middleware::logger::Logger;
use actix_web::{web, App, HttpServer};
use config::Config;
use env_logger::Env;
use log;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use std::time::Duration;

pub mod api;
pub mod common;
pub mod console;
pub mod core;
pub mod entity;
pub mod middleware;
pub mod service;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let settings = Config::builder()
        .add_source(config::File::with_name("conf/application.yml"))
        .build()
        .unwrap();

    let db_num = settings.get_int("db.num").unwrap();
    let mut conns: Vec<DatabaseConnection> = Vec::new();

    let max_connections = settings
        .get_int("db.pool.config.maximumPoolSize")
        .unwrap_or(100) as u32;
    let min_connections = settings
        .get_int("db.pool.config.minimumPoolSize")
        .unwrap_or(1) as u32;
    let connect_timeout = settings
        .get_int("db.pool.config.connectionTimeout")
        .unwrap_or(30) as u64;
    let acquire_timeout = settings
        .get_int("db.pool.config.initializationFailTimeout")
        .unwrap_or(1) as u64;
    let idle_timeout = settings.get_int("db.pool.config.idleTimeout").unwrap_or(10) as u64;
    let max_lifetime = settings.get_int("db.pool.config.maxLifetime").unwrap_or(30) as u64;

    for i in 0..db_num {
        let url = &settings
            .get_string(format!("db.url.{}", i).as_str())
            .unwrap();

        let mut opt = ConnectOptions::new(url);

        opt.max_connections(max_connections)
            .min_connections(min_connections)
            .connect_timeout(Duration::from_secs(connect_timeout))
            .acquire_timeout(Duration::from_secs(acquire_timeout))
            .idle_timeout(Duration::from_secs(idle_timeout))
            .max_lifetime(Duration::from_secs(max_lifetime))
            .sqlx_logging(true)
            .sqlx_logging_level(log::LevelFilter::Info);

        let db = Database::connect(opt).await.unwrap();

        conns.push(db);
    }

    let app_state = api::model::AppState { conns };

    let address = settings
        .get_string("server.address")
        .unwrap_or("0.0.0.0".to_string());
    let server_port = settings.get_int("server.port").unwrap_or(8848) as u16;
    let context_path = settings
        .get_string("server.servlet.contextPath")
        .unwrap_or("nacos".to_string())
        .clone();

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
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
