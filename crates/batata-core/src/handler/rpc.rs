// RPC service implementations for gRPC communication
// This file defines the core RPC services for handling client connections and message processing

use std::{collections::HashMap, pin::Pin, sync::Arc};

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, warn};

use crate::{
    GrpcAuthContext, GrpcAuthService, GrpcResource, PermissionAction,
    model::{Connection, GrpcClient},
    service::remote::ConnectionManager,
};

use crate::api::{
    grpc::{Payload, bi_request_stream_server::BiRequestStream},
    remote::model::ConnectionSetupRequest,
};

use batata_api::model::APPNAME;

/// Auth requirement level for handlers
/// Corresponds to Nacos's @Secured annotation with action and apiType parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRequirement {
    /// No authentication required (public endpoints, no @Secured annotation)
    None,
    /// Authentication required with READ action
    /// Corresponds to @Secured(action = ActionTypes.READ)
    Read,
    /// Authentication required with WRITE action
    /// Corresponds to @Secured(action = ActionTypes.WRITE)
    Write,
    /// Internal cluster operation - requires server identity verification
    /// Corresponds to @Secured(apiType = ApiType.INNER_API)
    Internal,
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

    /// Returns the sign type for this handler (config/naming/ai)
    /// Corresponds to signType parameter in @Secured
    fn sign_type(&self) -> &'static str {
        ""
    }

    /// Returns the resource type for this handler
    /// Used for permission checking
    fn resource_type(&self) -> crate::ResourceType {
        crate::ResourceType::Internal
    }
}

/// Trait for handling connection cleanup when a client disconnects.
/// Implemented by NamingService to deregister ephemeral instances.
#[tonic::async_trait]
pub trait ConnectionCleanupHandler: Send + Sync {
    /// Deregister all resources associated with a connection, returns affected service keys
    fn deregister_all_by_connection(&self, connection_id: &str) -> Vec<String>;
    /// Remove subscriber associated with a connection
    fn remove_subscriber(&self, connection_id: &str);
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

/// Check if authentication should be enabled for this request
/// Corresponds to Nacos's protocolAuthService.enableAuth(secured)
/// Returns true if auth plugin is configured and enabled for this sign type
pub fn enable_auth(auth_service: &GrpcAuthService, _sign_type: &str) -> bool {
    // For now, we follow Nacos logic:
    // - If auth is not enabled globally, return false
    // - If auth plugin is available, check if it enables auth for this sign type
    // - Default to true if auth is enabled and plugin is available
    auth_service.is_auth_enabled()
}

/// Check if authentication is valid, returns error Status if not
/// Corresponds to Nacos's protocolAuthService.validateIdentity()
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

/// Check permission for a resource with given action
/// Corresponds to Nacos's protocolAuthService.validateAuthority()
pub fn check_authority(
    auth_service: &GrpcAuthService,
    auth_context: &GrpcAuthContext,
    resource: &GrpcResource,
    action: PermissionAction,
    permissions: &[crate::GrpcPermissionInfo],
) -> Result<(), Status> {
    let result = auth_service.check_permission(auth_context, resource, action, permissions);

    if result.passed {
        Ok(())
    } else {
        Err(Status::permission_denied(
            result
                .message
                .unwrap_or_else(|| "permission denied".to_string()),
        ))
    }
}

/// Check if request comes from an authorized cluster node (internal operation)
pub fn check_server_identity(
    auth_service: &GrpcAuthService,
    payload: &Payload,
) -> Result<(), Status> {
    // If auth is not enabled, skip the check
    if !auth_service.is_auth_enabled() {
        return Ok(());
    }

    let headers = payload
        .metadata
        .as_ref()
        .map(|m| m.headers.clone())
        .unwrap_or_default();

    if auth_service.check_server_identity(&headers) {
        Ok(())
    } else {
        Err(Status::permission_denied(
            "request not from authorized cluster node",
        ))
    }
}

// Default handler for unregistered message types
// Provides enhanced error handling and logging for unknown message types
#[derive(Clone)]
pub struct DefaultHandler;

#[tonic::async_trait]
impl PayloadHandler for DefaultHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let metadata = payload.metadata.as_ref();
        let message_type = metadata.map(|m| m.r#type.clone()).unwrap_or_default();
        let client_ip = metadata.map(|m| m.client_ip.clone()).unwrap_or_default();

        // Log unknown message type with client information for debugging
        warn!(
            message_type = %message_type,
            client_ip = %client_ip,
            "Received unknown message type in DefaultHandler - no handler registered"
        );

        // Provide informative error message to client
        let error_message = if message_type.is_empty() {
            "Message type is empty or missing. Please ensure your client is sending valid gRPC requests."
        } else {
            &format!(
                "Unknown message type '{}'. This message type is not supported by the server. \
                Please check the client SDK version and ensure compatibility with the server API.",
                message_type
            )
        };

        Err(Status::invalid_argument(error_message))
    }

    fn can_handle(&self) -> &'static str {
        "default"
    }
}

// Registry for managing payload handlers by message type
// Supports dynamic handler registration with logging for debugging
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

    /// Get handler for a specific message type
    /// Returns the default handler if no specific handler is registered
    pub fn get_handler(&self, message_type: &str) -> Arc<dyn PayloadHandler> {
        self.handlers
            .get(message_type)
            .unwrap_or(&self.default_handler)
            .clone()
    }

    /// Register a new handler for a specific message type
    /// Logs the registration for debugging purposes
    pub fn register_handler(&mut self, handler: Arc<dyn PayloadHandler>) {
        let message_type = handler.can_handle();
        info!(
            message_type = %message_type,
            "Registering handler for message type '{}'",
            message_type
        );
        self.handlers.insert(message_type.to_string(), handler);
    }

    /// Unregister a handler for a specific message type
    /// Useful for dynamic handler management
    pub fn unregister_handler(&mut self, message_type: &str) -> bool {
        if let Some(_handler) = self.handlers.remove(message_type) {
            info!(
                message_type = %message_type,
                "Unregistered handler for message type '{}'",
                message_type
            );
            true
        } else {
            warn!(
                message_type = %message_type,
                "Attempted to unregister non-existent handler for message type '{}'",
                message_type
            );
            false
        }
    }

    /// Get a list of all registered message types
    /// Useful for debugging and monitoring
    pub fn registered_message_types(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
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
            // Following Nacos RemoteRequestAuthFilter logic:
            // 1. Check if INNER_API and innerApiAuthEnabled (skip auth if needed)
            // 2. Check if auth is enabled globally for non-INNER_API
            // 3. Check server identity
            // 4. Check if enableAuth(secured) returns true
            // 5. Validate identity
            // 6. Validate authority (permission)
            let auth_requirement = handler.auth_requirement();
            match auth_requirement {
                AuthRequirement::None => {}
                AuthRequirement::Internal => {
                    // INNER_API: check server identity first
                    check_server_identity(self.handler_registry.auth_service(), payload)?;
                }
                AuthRequirement::Read | AuthRequirement::Write => {
                    // Non-INNER_API: full auth flow
                    let auth_service = self.handler_registry.auth_service();

                    // Check server identity first (MATCHED case skips remaining auth)
                    if auth_service.check_server_identity(
                        &payload
                            .metadata
                            .as_ref()
                            .map(|m| m.headers.clone())
                            .unwrap_or_default(),
                    ) {
                        // Server identity matched - skip remaining auth checks
                    } else {
                        // Check if auth should be enabled for this sign type
                        let sign_type = handler.sign_type();
                        if !enable_auth(auth_service, sign_type) {
                            // Auth not enabled for this type - skip validation
                        } else {
                            // Validate identity
                            let auth_context =
                                extract_auth_context_from_payload(auth_service, payload);
                            check_authentication(&auth_context)?;

                            // Validate authority (permission)
                            // TODO: Extract resource and permissions from handler context
                            // This requires handler to provide resource info and user permissions
                        }
                    }
                }
            }

            return match handler.handle(&connection, payload).await {
                Ok(reponse_payload) => Ok(Response::new(reponse_payload)),
                Err(err) => Err(err),
            };
        }

        warn!("Received gRPC request without metadata - invalid request format");
        Err(tonic::Status::invalid_argument(
            "Invalid request: missing metadata. Please ensure your client is sending properly formatted gRPC requests.",
        ))
    }
}

#[derive(Clone)]
pub struct GrpcBiRequestStreamService {
    handler_registry: Arc<HandlerRegistry>,
    connection_manager: Arc<ConnectionManager>,
    config_subscriber_manager: Arc<crate::ConfigSubscriberManager>,
    connection_cleanup: Option<Arc<dyn ConnectionCleanupHandler>>,
}

impl GrpcBiRequestStreamService {
    pub fn new(
        handler_registry: HandlerRegistry,
        connection_manager: ConnectionManager,
        config_subscriber_manager: Arc<crate::ConfigSubscriberManager>,
        connection_cleanup: Option<Arc<dyn ConnectionCleanupHandler>>,
    ) -> Self {
        Self {
            handler_registry: Arc::new(handler_registry),
            connection_manager: Arc::new(connection_manager),
            config_subscriber_manager,
            connection_cleanup,
        }
    }
    pub fn from_arc(
        handler_registry: Arc<HandlerRegistry>,
        connection_manager: Arc<ConnectionManager>,
        config_subscriber_manager: Arc<crate::ConfigSubscriberManager>,
        connection_cleanup: Option<Arc<dyn ConnectionCleanupHandler>>,
    ) -> Self {
        Self {
            handler_registry,
            connection_manager,
            config_subscriber_manager,
            connection_cleanup,
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
        let connection_cleanup = self.connection_cleanup.clone();

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
                                // ConnectionSetupRequest is fully handled here - don't route through handler registry
                                continue;
                            }

                            // Response types (e.g., NotifySubscriberResponse, ConfigChangeNotifyResponse)
                            // are sent by clients as acknowledgments to server push notifications.
                            // They don't need handler routing - just silently accept them.
                            if message_type.ends_with("Response") {
                                tracing::debug!(
                                    "Received client response on bi-stream: {}",
                                    message_type
                                );
                                continue;
                            }

                            let handler = handler_registry.get_handler(message_type);

                            // Check authentication based on handler's auth requirement
                            // Following Nacos RemoteRequestAuthFilter logic
                            let auth_requirement = handler.auth_requirement();
                            let auth_result = match auth_requirement {
                                AuthRequirement::None => Ok(()),
                                AuthRequirement::Internal => {
                                    check_server_identity(handler_registry.auth_service(), &payload)
                                }
                                AuthRequirement::Read | AuthRequirement::Write => {
                                    let auth_service = handler_registry.auth_service();

                                    // Check server identity first
                                    if auth_service.check_server_identity(
                                        &payload
                                            .metadata
                                            .as_ref()
                                            .map(|m| m.headers.clone())
                                            .unwrap_or_default(),
                                    ) {
                                        // Server identity matched - skip remaining auth checks
                                        Ok(())
                                    } else {
                                        // Check if auth should be enabled for this sign type
                                        let sign_type = handler.sign_type();
                                        if !enable_auth(auth_service, sign_type) {
                                            // Auth not enabled for this type - skip validation
                                            Ok(())
                                        } else {
                                            // Validate identity
                                            let auth_context = extract_auth_context_from_payload(
                                                auth_service,
                                                &payload,
                                            );
                                            check_authentication(&auth_context)
                                        }
                                    }
                                }
                            };
                            if let Err(e) = auth_result {
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

            // Clean up resources when connection disconnects
            if let Some(ref cleanup) = connection_cleanup {
                let affected = cleanup.deregister_all_by_connection(&connection_id);
                if !affected.is_empty() {
                    info!(
                        "Deregistered ephemeral instances for disconnected connection {}: {} services affected",
                        connection_id,
                        affected.len()
                    );
                }
                cleanup.remove_subscriber(&connection_id);
            }
            config_subscriber_manager.unsubscribe_all(&connection_id);
            connection_manager.unregister(&connection_id);
        });

        let output_stream = ReceiverStream::new(rx);

        Ok(Response::new(
            Box::pin(output_stream) as Self::requestBiStreamStream
        ))
    }
}
