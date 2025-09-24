use std::sync::Arc;

use actix_web::{App, HttpServer, dev::Server, middleware::Logger, web};
use batata::{
    api::grpc::{bi_request_stream_server::BiRequestStreamServer, request_server::RequestServer},
    auth, console,
    core::service::{
        cluster::ServerMemberManager,
        remote::{ConnectionManager, context_interceptor},
    },
    middleware::auth::Authentication,
    model::{self, common::AppState},
    service::{
        handler::{ConnectionSetupHandler, HealthCheckHandler, ServerCheckHanlder},
        rpc::{GrpcBiRequestStreamService, GrpcRequestService, HandlerRegistry},
    },
};

use tonic::service::InterceptorLayer;
use tower::ServiceBuilder;
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
    let sdk_server_port = configuration.sdk_server_port();
    let cluster_server_port = configuration.cluster_server_port();

    let server_member_manager = Arc::new(ServerMemberManager::new(&configuration));

    let app_state = AppState {
        configuration,
        database_connection,
        server_member_manager: server_member_manager,
    };

    let server_app_state = app_state.clone();

    let layer = ServiceBuilder::new()
        .load_shed()
        .layer(InterceptorLayer::new(context_interceptor))
        .into_inner();

    let mut handler_registry = HandlerRegistry::new();

    let health_check_handler = Arc::new(HealthCheckHandler {});
    let server_check_hanlder = Arc::new(ServerCheckHanlder {});
    let connection_setup_handler = Arc::new(ConnectionSetupHandler {});

    handler_registry.register_handler(health_check_handler);
    handler_registry.register_handler(server_check_hanlder);
    handler_registry.register_handler(connection_setup_handler);

    let handler_registry_arc = Arc::new(handler_registry);

    let grpc_request_service = GrpcRequestService::from_arc(handler_registry_arc.clone());
    let connection_manager = Arc::new(ConnectionManager::new());
    let grpc_bi_request_stream_service =
        GrpcBiRequestStreamService::from_arc(handler_registry_arc, connection_manager);

    let grpc_sdk_addr = format!("0.0.0.0:{}", sdk_server_port).parse()?;

    let grpc_sdk_server = tonic::transport::Server::builder()
        .layer(layer.clone())
        .add_service(RequestServer::new(grpc_request_service.clone()))
        .add_service(BiRequestStreamServer::new(
            grpc_bi_request_stream_service.clone(),
        ))
        .serve(grpc_sdk_addr);

    tokio::spawn(grpc_sdk_server);

    let grpc_cluster_addr = format!("0.0.0.0:{}", cluster_server_port).parse()?;

    let grpc_cluster_server = tonic::transport::Server::builder()
        .layer(layer)
        .add_service(RequestServer::new(grpc_request_service))
        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
        .serve(grpc_cluster_addr);

    tokio::spawn(grpc_cluster_server);

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
                    .service(auth::v3::route::routes())
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
                    .service(auth::v1::route::routes())
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
