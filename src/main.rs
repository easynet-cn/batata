use actix_web::{App, HttpServer, dev::Server, middleware::Logger, web};
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

    let depolyment_type = configuration.deployment_type();
    let database_connection = configuration.database_connection().await?;
    let server_address = configuration.server_address();
    let console_server_address = server_address.clone();
    let console_server_port = configuration.console_server_port();
    let console_context_path = configuration.console_server_context_path();
    let server_main_port = configuration.server_main_port();
    let server_context_path = configuration.server_context_path();

    let app_state = AppState {
        configuration,
        database_connection,
    };

    let server_app_state = app_state.clone();

    match depolyment_type.as_str() {
        model::common::NACOS_DEPLOYMENT_TYPE_CONSOLE => {
            console_server(
                app_state,
                console_context_path,
                console_server_address,
                console_server_port,
            )
            .await?;
        }
        model::common::NACOS_DEPLOYMENT_TYPE_SERVER => {
            main_server(
                app_state,
                server_context_path,
                server_address,
                server_main_port,
            )
            .await?;
        }
        _ => {
            tokio::try_join!(
                console_server(
                    app_state,
                    console_context_path,
                    console_server_address,
                    console_server_port,
                ),
                main_server(
                    server_app_state,
                    server_context_path,
                    server_address,
                    server_main_port,
                )
            )?;
        }
    }

    Ok(())
}

pub fn console_server(
    app_state: AppState,
    context_path: String,
    address: String,
    port: u16,
) -> Server {
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(app_state.clone()))
            .service(
                web::scope(&context_path)
                    .service(console::v2::route::routes())
                    .service(console::v3::route::routes()),
            )
    })
    .bind((address, port))
    .unwrap()
    .run()
}

pub fn main_server(
    app_state: AppState,
    context_path: String,
    address: String,
    port: u16,
) -> Server {
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Authentication)
            .app_data(web::Data::new(app_state.clone()))
            .service(
                web::scope(&context_path)
                    .service(console::v2::route::routes())
                    .service(console::v3::route::routes()),
            )
    })
    .bind((address, port))
    .unwrap()
    .run()
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
