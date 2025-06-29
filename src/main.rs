use actix_web::{App, HttpServer, middleware::Logger, web};
use batata::{
    console,
    middleware::auth::Authentication,
    model::{self, common::AppState},
};

use tracing::{Subscriber, subscriber::set_global_default};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry, fmt::MakeWriter, layer::SubscriberExt};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let configuration = model::common::Configuration::new();

    let subscriber = get_subscriber("nacos", "info", std::io::stdout);

    init_subscriber(subscriber);

    let database_connection = configuration.database_connection().await?;

    let address = configuration
        .config
        .get_string("server.address")
        .unwrap_or("0.0.0.0".to_string());
    let server_port = configuration
        .config
        .get_int("nacos.console.port")
        .unwrap_or(8080) as u16;
    let context_path = configuration
        .config
        .get_string("nacos.console.contextPath")
        .unwrap_or("".to_string());

    let token_secret_key = configuration
        .config
        .get_string("nacos.core.auth.plugin.nacos.token.secret.key")?;

    let server_address = address.clone();
    let server_main_port = configuration
        .config
        .get_int("nacos.server.main.port")
        .unwrap_or(8080) as u16;
    let server_context_path = configuration
        .config
        .get_string("nacos.server.contextPath")
        .unwrap_or("".to_string());
    let app_state = AppState {
        configuration,
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
