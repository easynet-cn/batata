use std::time::Duration;

use actix_web::{middleware::Logger, web, App, HttpServer};
use config::Config;
use middleware::auth::Authentication;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use tracing::{subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

pub mod api;
pub mod common;
pub mod console;
pub mod entity;
pub mod middleware;
pub mod service;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("nacos", "info", std::io::stdout);
    init_subscriber(subscriber);

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
        .max_lifetime(Duration::from_secs(max_lifetime));

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

pub fn get_subscriber(
    name: &str,
    env_filter: &str,
    sink: impl for<'a> MakeWriter<'a> + 'static + Send + Sync,
) -> impl Subscriber + Send + Sync {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name.into(), sink);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
