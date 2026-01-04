// Main entry point for Batata Nacos-compatible server
// This file sets up and starts the HTTP and gRPC servers with their respective services

use std::sync::Arc;

use actix_web::{App, HttpServer, dev::Server, middleware::Logger, web};
use batata::{
    api::{
        consul::{
            AclService, agent::ConsulAgentService, catalog::ConsulCatalogService,
            health::ConsulHealthService, kv::ConsulKVService, route::consul_routes,
        },
        grpc::{bi_request_stream_server::BiRequestStreamServer, request_server::RequestServer},
    },
    auth, console,
    core::service::{
        cluster::ServerMemberManager,
        remote::{ConnectionManager, context_interceptor},
    },
    middleware::{auth::Authentication, rate_limit::RateLimiter},
    model::{self, common::AppState},
    service::{
        config_handler::{
            ClientConfigMetricHandler, ConfigBatchListenHandler, ConfigChangeClusterSyncHandler,
            ConfigChangeNotifyHandler, ConfigFuzzyWatchChangeNotifyHandler,
            ConfigFuzzyWatchHandler, ConfigFuzzyWatchSyncHandler, ConfigPublishHandler,
            ConfigQueryHandler, ConfigRemoveHandler,
        },
        handler::{
            ClientDetectionHandler, ConnectResetHandler, ConnectionSetupHandler,
            HealthCheckHandler, PushAckHandler, ServerCheckHanlder, ServerLoaderInfoHandler,
            ServerReloadHandler, SetupAckHandler,
        },
        naming::NamingService,
        naming_handler::{
            BatchInstanceRequestHandler, InstanceRequestHandler,
            NamingFuzzyWatchChangeNotifyHandler, NamingFuzzyWatchHandler,
            NamingFuzzyWatchSyncHandler, NotifySubscriberHandler, PersistentInstanceRequestHandler,
            ServiceListRequestHandler, ServiceQueryRequestHandler, SubscribeServiceRequestHandler,
        },
        rpc::{GrpcBiRequestStreamService, GrpcRequestService, HandlerRegistry},
    },
};

use tokio::signal;
use tokio::sync::broadcast;
use tonic::service::InterceptorLayer;
use tower::ServiceBuilder;
use tracing::{Subscriber, info, subscriber::set_global_default};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry, fmt::MakeWriter, layer::SubscriberExt};

/// Create a shutdown signal listener (reserved for graceful shutdown feature)
#[allow(dead_code)]
async fn shutdown_signal(mut shutdown_rx: broadcast::Receiver<()>) {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install signal handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown...");
        },
        _ = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown...");
        },
        _ = shutdown_rx.recv() => {
            info!("Received shutdown signal from broadcast channel");
        },
    }
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration and logging
    let configuration = model::common::Configuration::new();

    let subscriber = get_subscriber("nacos", "info", std::io::stdout);

    init_subscriber(subscriber)?;

    // Extract configuration parameters
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

    // Initialize server member management
    let server_member_manager = Arc::new(ServerMemberManager::new(&configuration));

    // Create application state wrapped in Arc for efficient sharing
    let app_state = Arc::new(AppState {
        configuration,
        database_connection,
        server_member_manager,
    });

    // Clone Arc references (cheap - just atomic increment)
    let app_state_arc = app_state.clone();

    // Setup gRPC interceptor layer
    let layer = ServiceBuilder::new()
        .load_shed()
        .layer(InterceptorLayer::new(context_interceptor))
        .into_inner();

    // Initialize gRPC handlers
    let mut handler_registry = HandlerRegistry::new();

    // Internal handlers
    let health_check_handler = Arc::new(HealthCheckHandler {});
    let server_check_hanlder = Arc::new(ServerCheckHanlder {});
    let connection_setup_handler = Arc::new(ConnectionSetupHandler {});
    let client_detection_handler = Arc::new(ClientDetectionHandler {});
    let server_loader_info_handler = Arc::new(ServerLoaderInfoHandler {});
    let server_reload_handler = Arc::new(ServerReloadHandler {});
    let connect_reset_handler = Arc::new(ConnectResetHandler {});
    let setup_ack_handler = Arc::new(SetupAckHandler {});
    let push_ack_handler = Arc::new(PushAckHandler {});

    handler_registry.register_handler(health_check_handler);
    handler_registry.register_handler(server_check_hanlder);
    handler_registry.register_handler(connection_setup_handler);
    handler_registry.register_handler(client_detection_handler);
    handler_registry.register_handler(server_loader_info_handler);
    handler_registry.register_handler(server_reload_handler);
    handler_registry.register_handler(connect_reset_handler);
    handler_registry.register_handler(setup_ack_handler);
    handler_registry.register_handler(push_ack_handler);

    // Config handlers
    let config_query_handler = Arc::new(ConfigQueryHandler {
        app_state: app_state_arc.clone(),
    });
    let config_publish_handler = Arc::new(ConfigPublishHandler {
        app_state: app_state_arc.clone(),
    });
    let config_remove_handler = Arc::new(ConfigRemoveHandler {
        app_state: app_state_arc.clone(),
    });
    let config_batch_listen_handler = Arc::new(ConfigBatchListenHandler {
        app_state: app_state_arc.clone(),
    });
    let config_change_notify_handler = Arc::new(ConfigChangeNotifyHandler {
        app_state: app_state_arc.clone(),
    });
    let config_change_cluster_sync_handler = Arc::new(ConfigChangeClusterSyncHandler {
        app_state: app_state_arc.clone(),
    });
    let config_fuzzy_watch_handler = Arc::new(ConfigFuzzyWatchHandler {
        app_state: app_state_arc.clone(),
    });
    let config_fuzzy_watch_change_notify_handler = Arc::new(ConfigFuzzyWatchChangeNotifyHandler {
        app_state: app_state_arc.clone(),
    });
    let config_fuzzy_watch_sync_handler = Arc::new(ConfigFuzzyWatchSyncHandler {
        app_state: app_state_arc.clone(),
    });
    let client_config_metric_handler = Arc::new(ClientConfigMetricHandler {
        app_state: app_state_arc.clone(),
    });

    handler_registry.register_handler(config_query_handler);
    handler_registry.register_handler(config_publish_handler);
    handler_registry.register_handler(config_remove_handler);
    handler_registry.register_handler(config_batch_listen_handler);
    handler_registry.register_handler(config_change_notify_handler);
    handler_registry.register_handler(config_change_cluster_sync_handler);
    handler_registry.register_handler(config_fuzzy_watch_handler);
    handler_registry.register_handler(config_fuzzy_watch_change_notify_handler);
    handler_registry.register_handler(config_fuzzy_watch_sync_handler);
    handler_registry.register_handler(client_config_metric_handler);

    // Naming handlers
    let naming_service = Arc::new(NamingService::new());

    let instance_request_handler = Arc::new(InstanceRequestHandler {
        naming_service: naming_service.clone(),
    });
    let batch_instance_request_handler = Arc::new(BatchInstanceRequestHandler {
        naming_service: naming_service.clone(),
    });
    let service_list_request_handler = Arc::new(ServiceListRequestHandler {
        naming_service: naming_service.clone(),
    });
    let service_query_request_handler = Arc::new(ServiceQueryRequestHandler {
        naming_service: naming_service.clone(),
    });
    let subscribe_service_request_handler = Arc::new(SubscribeServiceRequestHandler {
        naming_service: naming_service.clone(),
    });
    let persistent_instance_request_handler = Arc::new(PersistentInstanceRequestHandler {
        naming_service: naming_service.clone(),
    });
    let notify_subscriber_handler = Arc::new(NotifySubscriberHandler {
        naming_service: naming_service.clone(),
    });
    let naming_fuzzy_watch_handler = Arc::new(NamingFuzzyWatchHandler {
        naming_service: naming_service.clone(),
    });
    let naming_fuzzy_watch_change_notify_handler = Arc::new(NamingFuzzyWatchChangeNotifyHandler {
        naming_service: naming_service.clone(),
    });
    let naming_fuzzy_watch_sync_handler = Arc::new(NamingFuzzyWatchSyncHandler {
        naming_service: naming_service.clone(),
    });

    handler_registry.register_handler(instance_request_handler);
    handler_registry.register_handler(batch_instance_request_handler);
    handler_registry.register_handler(service_list_request_handler);
    handler_registry.register_handler(service_query_request_handler);
    handler_registry.register_handler(subscribe_service_request_handler);
    handler_registry.register_handler(persistent_instance_request_handler);
    handler_registry.register_handler(notify_subscriber_handler);
    handler_registry.register_handler(naming_fuzzy_watch_handler);
    handler_registry.register_handler(naming_fuzzy_watch_change_notify_handler);
    handler_registry.register_handler(naming_fuzzy_watch_sync_handler);

    let handler_registry_arc = Arc::new(handler_registry);

    // Create gRPC services
    let grpc_request_service = GrpcRequestService::from_arc(handler_registry_arc.clone());
    let connection_manager = Arc::new(ConnectionManager::new());
    let grpc_bi_request_stream_service =
        GrpcBiRequestStreamService::from_arc(handler_registry_arc, connection_manager);

    // Start SDK gRPC server
    let grpc_sdk_addr = format!("0.0.0.0:{}", sdk_server_port).parse()?;

    let grpc_sdk_server = tonic::transport::Server::builder()
        .layer(layer.clone())
        .add_service(RequestServer::new(grpc_request_service.clone()))
        .add_service(BiRequestStreamServer::new(
            grpc_bi_request_stream_service.clone(),
        ))
        .serve(grpc_sdk_addr);

    tokio::spawn(grpc_sdk_server);

    // Start cluster gRPC server
    let grpc_cluster_addr = format!("0.0.0.0:{}", cluster_server_port).parse()?;

    let grpc_cluster_server = tonic::transport::Server::builder()
        .layer(layer)
        .add_service(RequestServer::new(grpc_request_service))
        .add_service(BiRequestStreamServer::new(grpc_bi_request_stream_service))
        .serve(grpc_cluster_addr);

    tokio::spawn(grpc_cluster_server);

    // Create Consul service adapters
    let consul_agent_service = ConsulAgentService::new(naming_service.clone());
    let consul_health_service = ConsulHealthService::new(naming_service.clone());
    let consul_kv_service = ConsulKVService::new();
    let consul_catalog_service = ConsulCatalogService::new(naming_service.clone());
    let consul_acl_service = AclService::new();

    match depolyment_type.as_str() {
        model::common::NACOS_DEPLOYMENT_TYPE_CONSOLE => {
            console_server(
                app_state.clone(),
                console_context_path,
                console_server_address,
                console_server_port,
            )?
            .await?;
        }
        model::common::NACOS_DEPLOYMENT_TYPE_SERVER => {
            main_server(
                app_state.clone(),
                naming_service.clone(),
                consul_agent_service.clone(),
                consul_health_service.clone(),
                consul_kv_service.clone(),
                consul_catalog_service.clone(),
                consul_acl_service.clone(),
                server_context_path,
                server_address,
                server_main_port,
            )?
            .await?;
        }
        _ => {
            let console = console_server(
                app_state.clone(),
                console_context_path,
                console_server_address,
                console_server_port,
            )?;
            let main = main_server(
                app_state,
                naming_service,
                consul_agent_service,
                consul_health_service,
                consul_kv_service,
                consul_catalog_service,
                consul_acl_service,
                server_context_path,
                server_address,
                server_main_port,
            )?;
            tokio::try_join!(console, main)?;
        }
    }

    Ok(())
}

pub fn console_server(
    app_state: Arc<AppState>,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::with_defaults())
            .wrap(Authentication)
            .app_data(web::Data::from(app_state.clone()))
            .service(
                web::scope(&context_path)
                    .service(auth::v3::route::routes())
                    .service(console::v3::route::routes()),
            )
    })
    .bind((address, port))?
    .run())
}

pub fn main_server(
    app_state: Arc<AppState>,
    naming_service: Arc<NamingService>,
    consul_agent_service: ConsulAgentService,
    consul_health_service: ConsulHealthService,
    consul_kv_service: ConsulKVService,
    consul_catalog_service: ConsulCatalogService,
    consul_acl_service: AclService,
    context_path: String,
    address: String,
    port: u16,
) -> Result<Server, std::io::Error> {
    Ok(HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(RateLimiter::with_defaults())
            .wrap(Authentication)
            .app_data(web::Data::from(app_state.clone()))
            .app_data(web::Data::new(naming_service.clone()))
            .app_data(web::Data::new(consul_agent_service.clone()))
            .app_data(web::Data::new(consul_health_service.clone()))
            .app_data(web::Data::new(consul_kv_service.clone()))
            .app_data(web::Data::new(consul_catalog_service.clone()))
            .app_data(web::Data::new(consul_acl_service.clone()))
            .service(
                web::scope(&context_path)
                    .service(auth::v1::route::routes())
                    .service(console::v3::route::routes()),
            )
            // Consul API routes (outside of Nacos context path)
            .service(consul_routes())
    })
    .bind((address, port))?
    .run())
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

pub fn init_subscriber(
    subscriber: impl Subscriber + Send + Sync,
) -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init().map_err(|e| format!("Failed to set logger: {}", e))?;
    set_global_default(subscriber).map_err(|e| format!("Failed to set subscriber: {}", e))?;
    Ok(())
}
