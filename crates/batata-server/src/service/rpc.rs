// RPC service implementations for gRPC communication
// This file defines the core RPC services for handling client connections and message processing

use std::{collections::HashMap, pin::Pin, sync::Arc};

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming};

use batata_core::{
    model::{Connection, GrpcClient},
    service::remote::ConnectionManager,
    GrpcAuthContext, GrpcAuthService, GrpcResource, PermissionAction,
};

use crate::api::{
    grpc::{Payload, bi_request_stream_server::BiRequestStream},
    model::APPNAME,
    remote::model::ConnectionSetupRequest,
};

/// Auth requirement level for handlers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRequirement {
    /// No authentication required (public endpoints)
    None,
    /// Authentication required but no specific permission
    Authenticated,
    /// Specific permission required (will be checked by handler)
    Permission,
}

// Trait for handling gRPC payload messages
#[tonic::async_trait]
pub trait PayloadHandler: Send + Sync {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let message_type = payload.metadata.clone().unwrap_or_default().r#type;

        Err(Status::unimplemented(format!(
            "Unknown message type '{}'",
            message_type
        )))
    }

    fn can_handle(&self) -> &'static str {
        ""
    }

    /// Returns the auth requirement for this handler
    /// Override this to enable authentication for specific handlers
    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::None
    }
}

/// Helper functions for extracting auth context from payload
pub fn extract_auth_context_from_payload(
    auth_service: &GrpcAuthService,
    payload: &Payload,
) -> GrpcAuthContext {
    let headers = payload
        .metadata
        .as_ref()
        .map(|m| m.headers.clone())
        .unwrap_or_default();

    auth_service.parse_identity(&headers)
}

/// Check if authentication is valid, returns error Status if not
pub fn check_authentication(auth_context: &GrpcAuthContext) -> Result<(), Status> {
    if !auth_context.auth_enabled {
        return Ok(());
    }

    if !auth_context.is_authenticated() {
        let error_msg = auth_context
            .auth_error
            .as_deref()
            .unwrap_or("user not authenticated");
        return Err(Status::unauthenticated(error_msg));
    }

    Ok(())
}

/// Check permission for a resource, returns error Status if denied
pub fn check_permission(
    auth_service: &GrpcAuthService,
    auth_context: &GrpcAuthContext,
    resource: &GrpcResource,
    action: PermissionAction,
    permissions: &[batata_core::GrpcPermissionInfo],
) -> Result<(), Status> {
    let result = auth_service.check_permission(auth_context, resource, action, permissions);

    if result.passed {
        Ok(())
    } else {
        Err(Status::permission_denied(
            result.message.unwrap_or_else(|| "permission denied".to_string()),
        ))
    }
}

// Default handler for unregistered message types
#[derive(Clone)]
pub struct DefaultHandler;

#[tonic::async_trait]
impl PayloadHandler for DefaultHandler {}

// Registry for managing payload handlers by message type
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn PayloadHandler>>,
    default_handler: Arc<dyn PayloadHandler>,
    auth_service: Arc<GrpcAuthService>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: Arc::new(DefaultHandler {}),
            auth_service: Arc::new(GrpcAuthService::default()),
        }
    }

    /// Create a new HandlerRegistry with auth service
    pub fn with_auth(auth_service: GrpcAuthService) -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: Arc::new(DefaultHandler {}),
            auth_service: Arc::new(auth_service),
        }
    }

    pub fn get_handler(&self, message_type: &str) -> Arc<dyn PayloadHandler> {
        self.handlers
            .get(message_type)
            .unwrap_or(&self.default_handler)
            .clone()
    }

    pub fn register_handler(&mut self, handler: Arc<dyn PayloadHandler>) {
        self.handlers
            .insert(handler.can_handle().to_string(), handler);
    }

    /// Get the auth service
    pub fn auth_service(&self) -> &GrpcAuthService {
        &self.auth_service
    }

    /// Check if auth is enabled
    pub fn is_auth_enabled(&self) -> bool {
        self.auth_service.is_auth_enabled()
    }
}

#[derive(Clone)]
pub struct GrpcRequestService {
    handler_registry: Arc<HandlerRegistry>,
}

impl GrpcRequestService {
    pub fn new(handler_registry: HandlerRegistry) -> Self {
        Self {
            handler_registry: Arc::new(handler_registry),
        }
    }

    pub fn from_arc(handler_registry: Arc<HandlerRegistry>) -> Self {
        Self { handler_registry }
    }
}

#[tonic::async_trait]
impl crate::api::grpc::request_server::Request for GrpcRequestService {
    async fn request(
        &self,
        request: tonic::Request<Payload>,
    ) -> std::result::Result<tonic::Response<Payload>, tonic::Status> {
        if let Some(metadata) = &request.get_ref().metadata {
            let connection = request
                .extensions()
                .get::<Connection>()
                .cloned()
                .unwrap_or_default();

            let message_type = &metadata.r#type;
            let payload = request.get_ref();

            let handler = self.handler_registry.get_handler(message_type);

            // Check authentication based on handler's auth requirement
            let auth_requirement = handler.auth_requirement();
            if auth_requirement != AuthRequirement::None {
                let auth_context =
                    extract_auth_context_from_payload(self.handler_registry.auth_service(), payload);
                check_authentication(&auth_context)?;
            }

            return match handler.handle(&connection, payload).await {
                Ok(reponse_payload) => Ok(Response::new(reponse_payload)),
                Err(err) => Err(err),
            };
        }

        Err(tonic::Status::not_found("unknow message type"))
    }
}

#[derive(Clone)]
pub struct GrpcBiRequestStreamService {
    handler_registry: Arc<HandlerRegistry>,
    connection_manager: Arc<ConnectionManager>,
    config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
}

impl GrpcBiRequestStreamService {
    pub fn new(
        handler_registry: HandlerRegistry,
        connection_manager: ConnectionManager,
        config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    ) -> Self {
        Self {
            handler_registry: Arc::new(handler_registry),
            connection_manager: Arc::new(connection_manager),
            config_subscriber_manager,
        }
    }
    pub fn from_arc(
        handler_registry: Arc<HandlerRegistry>,
        connection_manager: Arc<ConnectionManager>,
        config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    ) -> Self {
        Self {
            handler_registry,
            connection_manager,
            config_subscriber_manager,
        }
    }
}

#[tonic::async_trait]
impl BiRequestStream for GrpcBiRequestStreamService {
    type requestBiStreamStream =
        Pin<Box<dyn Stream<Item = Result<Payload, Status>> + Send + 'static>>;

    async fn request_bi_stream(
        &self,
        request: Request<Streaming<Payload>>,
    ) -> Result<Response<Self::requestBiStreamStream>, Status> {
        let connection = request
            .extensions()
            .get::<Connection>()
            .cloned()
            .unwrap_or_default();
        let connection_id = connection.meta_info.connection_id.clone();
        let mut inbound_stream = request.into_inner();

        let (tx, rx) = mpsc::channel(128);

        let handler_registry = self.handler_registry.clone();
        let connection_manager = self.connection_manager.clone();
        let config_subscriber_manager = self.config_subscriber_manager.clone();

        tokio::spawn(async move {
            while let Some(message) = inbound_stream.next().await {
                match message {
                    Ok(payload) => {
                        if let Some(metadata) = &payload.metadata {
                            let message_type = &metadata.r#type;

                            if message_type == "ConnectionSetupRequest" {
                                let request = ConnectionSetupRequest::from(&payload);

                                let mut con = connection.clone();

                                con.meta_info.client_ip = metadata.client_ip.clone();
                                con.meta_info.version = request.client_version;
                                con.meta_info.labels = request.labels.clone();
                                con.meta_info.app_name = con
                                    .meta_info
                                    .labels
                                    .get(APPNAME)
                                    .map_or("-".to_string(), |e| e.to_string());
                                con.meta_info.namespace_id = request.tenant;

                                let connection_id = con.meta_info.connection_id.clone();

                                // Move con instead of cloning - con is not used after this
                                let client = GrpcClient::new(con, tx.clone());

                                connection_manager.register(&connection_id, client);
                            }

                            let handler = handler_registry.get_handler(message_type);

                            // Check authentication based on handler's auth requirement
                            let auth_requirement = handler.auth_requirement();
                            if auth_requirement != AuthRequirement::None {
                                let auth_context = extract_auth_context_from_payload(
                                    handler_registry.auth_service(),
                                    &payload,
                                );
                                if let Err(e) = check_authentication(&auth_context) {
                                    tracing::warn!(
                                        "Authentication failed for {}: {}",
                                        message_type,
                                        e.message()
                                    );
                                    if let Err(send_err) = tx.send(Err(e)).await {
                                        tracing::error!(
                                            "Failed to send auth error response: {}",
                                            send_err
                                        );
                                        break;
                                    }
                                    continue;
                                }
                            }

                            match handler.handle(&connection, &payload).await {
                                Ok(response_payload) => {
                                    if let Err(e) = tx.send(Ok(response_payload)).await {
                                        tracing::error!("Failed to send response: {}", e);

                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Handler error: {}", e);
                                    if let Err(send_err) = tx.send(Err(e)).await {
                                        tracing::error!(
                                            "Failed to send error response: {}",
                                            send_err
                                        );

                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error receiving message: {}", e);

                        break;
                    }
                }
            }

            // 连接断开时清理资源
            config_subscriber_manager.unsubscribe_all(&connection_id);
            connection_manager.unregister(&connection_id);
        });

        let output_stream = ReceiverStream::new(rx);

        Ok(Response::new(
            Box::pin(output_stream) as Self::requestBiStreamStream
        ))
    }
}
