use std::time::Duration;

use actix_web::{web, App, HttpServer};
use config::Config;
use env_logger::Env;
use log;
use middleware::auth::Authentication;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use crate::middleware::logger::Logger;

pub mod api;
pub mod common;
pub mod console;
pub mod entity;
pub mod middleware;
pub mod service;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let app_config = Config::builder()
        .add_source(config::File::with_name("conf/application.yml"))
        .build()
        .unwrap();

    let max_connections = app_config
        .get_int("db.pool.config.maximumPoolSize")
        .unwrap_or(100) as u32;
    let min_connections = app_config
        .get_int("db.pool.config.minimumPoolSize")
        .unwrap_or(1) as u32;
    let connect_timeout = app_config
        .get_int("db.pool.config.connectionTimeout")
        .unwrap_or(30) as u64;
    let acquire_timeout = app_config
        .get_int("db.pool.config.initializationFailTimeout")
        .unwrap_or(8) as u64;
    let idle_timeout = app_config
        .get_int("db.pool.config.idleTimeout")
        .unwrap_or(10) as u64;
    let max_lifetime = app_config
        .get_int("db.pool.config.maxLifetime")
        .unwrap_or(30) as u64;

    let url = app_config.get_string("db.url").unwrap();

    let mut opt = ConnectOptions::new(url);

    opt.max_connections(max_connections)
        .min_connections(min_connections)
        .connect_timeout(Duration::from_secs(connect_timeout))
        .acquire_timeout(Duration::from_secs(acquire_timeout))
        .idle_timeout(Duration::from_secs(idle_timeout))
        .max_lifetime(Duration::from_secs(max_lifetime))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info);

    let database_connection: DatabaseConnection = Database::connect(opt).await.unwrap();
    let address = app_config
        .get_string("server.address")
        .unwrap_or("0.0.0.0".to_string());
    let server_port = app_config.get_int("server.port").unwrap_or(8848) as u16;
    let context_path = app_config
        .get_string("server.servlet.contextPath")
        .unwrap_or("/nacos".to_string());

    let token_secret_key = app_config
        .get_string("nacos.core.auth.plugin.nacos.token.secret.key")
        .unwrap();

    let app_state = api::model::AppState {
        app_config,
        database_connection,
        context_path: context_path.clone(),
        token_secret_key: token_secret_key.clone(),
    };

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(app_state.clone()))
            .service(
                web::scope(&context_path)
                    .service(crate::console::v1::router::routers())
                    .service(crate::console::v2::router::routers()),
            )
    })
    .bind((address, server_port))?
    .run()
    .await
}
