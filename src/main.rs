use std::time::Duration;

use actix_web::{App, HttpServer, middleware::Logger, web};
use batata::{console, middleware::auth::Authentication, model::common::AppState};
use clap::Parser;
use config::Config;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

use tracing::{Subscriber, subscriber::set_global_default};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry, fmt::MakeWriter, layer::SubscriberExt};

#[derive(Parser)]
#[command()]
struct Cli {
    #[arg(short = 'm', long = "mode", default_value = "standalone")]
    mode: String,
    #[arg(short = 'f', long = "function_mode", default_value = "all")]
    function_mode: String,
    #[arg(short = 'd', long = "deployment", default_value = "merged")]
    deployment: String,
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let subscriber = get_subscriber("nacos", "info", std::io::stdout);
    init_subscriber(subscriber);

    let mut config_builder = Config::builder();

    config_builder = config_builder.add_source(config::File::with_name("conf/application.yml"));
    config_builder = config_builder.set_override("nacos.standalone", args.mode == "standalone")?;
    config_builder = config_builder.set_override("nacos.deployment.type", args.deployment)?;

    let app_config = config_builder.build()?;

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

    let url = app_config.get_string("db.url")?;

    let mut opt = ConnectOptions::new(url);

    opt.max_connections(max_connections)
        .min_connections(min_connections)
        .connect_timeout(Duration::from_secs(connect_timeout))
        .acquire_timeout(Duration::from_secs(acquire_timeout))
        .idle_timeout(Duration::from_secs(idle_timeout))
        .max_lifetime(Duration::from_secs(max_lifetime));

    let database_connection: DatabaseConnection = Database::connect(opt).await?;
    let address = app_config
        .get_string("server.address")
        .unwrap_or("0.0.0.0".to_string());
    let server_port = app_config.get_int("nacos.console.port").unwrap_or(8080) as u16;
    let context_path = app_config
        .get_string("nacos.console.contextPath")
        .unwrap_or("".to_string());

    let token_secret_key =
        app_config.get_string("nacos.core.auth.plugin.nacos.token.secret.key")?;

    let server_address = address.clone();
    let server_main_port = app_config.get_int("nacos.server.main.port").unwrap_or(8080) as u16;
    let server_context_path = app_config
        .get_string("nacos.server.contextPath")
        .unwrap_or("".to_string());

    let app_state = AppState {
        app_config,
        database_connection,
        context_path: context_path.clone(),
        token_secret_key: token_secret_key.clone(),
    };

    let server_app_state = app_state.clone();

    let console_server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(app_state.clone()))
            .service(
                web::scope(&context_path)
                    .service(console::v2::router::routers())
                    .service(console::v3::router::routers()),
            )
    })
    .bind((address, server_port))?
    .run();

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(server_app_state.clone()))
            .service(
                web::scope(&server_context_path)
                    .service(console::v2::router::routers())
                    .service(console::v3::router::routers()),
            )
    })
    .bind((server_address, server_main_port))?
    .run();

    tokio::try_join!(console_server, server)?;

    Ok(())
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
