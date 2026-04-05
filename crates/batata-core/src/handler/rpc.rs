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
    grpc::{Metadata, Payload, bi_request_stream_server::BiRequestStream},
    remote::model::{ConnectionSetupRequest, ResponseCode},
};

use batata_api::model::APPNAME;
use batata_api::remote::RequestTrait;

/// Default timeout for bi-stream send operations (5 seconds).
const BISTREAM_SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Send a message on the bi-stream channel with timeout protection.
/// Returns true on success, false on timeout or channel error.
async fn bistream_send(
    tx: &mpsc::Sender<Result<Payload, Status>>,
    msg: Result<Payload, Status>,
) -> Result<(), String> {
    match tokio::time::timeout(BISTREAM_SEND_TIMEOUT, tx.send(msg)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("channel closed: {}", e)),
        Err(_) => Err("send timeout".to_string()),
    }
}

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
        let message_type = payload
            .metadata
            .as_ref()
            .map(|m| m.r#type.as_str())
            .unwrap_or("");

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

    /// Extract resource and permission action from the payload for authorization checking.
    ///
    /// Returns `Some((resource, action))` if this handler requires permission checking.
    /// Returns `None` if no permission checking is needed (default).
    fn resource_from_payload(
        &self,
        _payload: &crate::api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        None
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

/// Empty headers constant to avoid repeated allocations
static EMPTY_HEADERS: std::sync::LazyLock<HashMap<String, String>> =
    std::sync::LazyLock::new(HashMap::new);

/// Extract headers reference from payload without cloning
#[inline]
fn payload_headers(payload: &Payload) -> &HashMap<String, String> {
    payload
        .metadata
        .as_ref()
        .map(|m| &m.headers)
        .unwrap_or(&EMPTY_HEADERS)
}

/// Helper functions for extracting auth context from payload (sync, no role lookup)
pub fn extract_auth_context_from_payload(
    auth_service: &GrpcAuthService,
    payload: &Payload,
) -> GrpcAuthContext {
    auth_service.parse_identity(payload_headers(payload))
}

/// Resolve full auth context from payload (async, with role lookup from database)
pub async fn resolve_auth_context_from_payload(
    auth_service: &GrpcAuthService,
    payload: &Payload,
) -> GrpcAuthContext {
    auth_service
        .resolve_auth_context(payload_headers(payload))
        .await
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

    if auth_service.check_server_identity(payload_headers(payload)) {
        Ok(())
    } else {
        Err(Status::permission_denied(
            "request not from authorized cluster node",
        ))
    }
}

/// Build an auth error response as a gRPC Payload (not a Status error).
/// Nacos returns auth failures as successful gRPC responses with errorCode=403
/// in the response body. The SDK checks response.errorCode, not gRPC Status.
const NO_RIGHT: i32 = 403;

fn build_auth_error_payload(response_type: &str, error_code: i32, message: &str) -> Payload {
    let response = serde_json::json!({
        "resultCode": ResponseCode::Fail.code(),
        "errorCode": error_code,
        "success": false,
        "message": message,
    });
    Payload {
        metadata: Some(Metadata {
            r#type: response_type.to_string(),
            ..Default::default()
        }),
        body: Some(prost_types::Any {
            type_url: String::new(),
            value: serde_json::to_vec(&response).unwrap_or_default(),
        }),
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
        let message_type = metadata.map(|m| m.r#type.as_str()).unwrap_or_default();
        let client_ip = metadata.map(|m| m.client_ip.as_str()).unwrap_or_default();

        warn!(
            message_type = %message_type,
            client_ip = %client_ip,
            "Received unknown message type in DefaultHandler - no handler registered"
        );

        if message_type.is_empty() {
            Err(Status::invalid_argument(
                "Message type is empty or missing. Please ensure your client is sending valid gRPC requests.",
            ))
        } else {
            Err(Status::invalid_argument(format!(
                "Unknown message type '{}'. This message type is not supported by the server. \
                Please check the client SDK version and ensure compatibility with the server API.",
                message_type
            )))
        }
    }

    fn can_handle(&self) -> &'static str {
        "default"
    }
}

/// Maps gRPC message types to TPS control point names (following Nacos conventions).
pub fn grpc_tps_point(message_type: &str) -> Option<&'static str> {
    match message_type {
        "ConfigPublishRequest" => Some("ConfigPublish"),
        "ConfigQueryRequest" => Some("ConfigQuery"),
        "ConfigRemoveRequest" => Some("ConfigRemove"),
        "ConfigBatchListenRequest" => Some("ConfigListen"),
        "ConfigFuzzyWatchRequest" => Some("ConfigFuzzyWatch"),
        "InstanceRequest" => Some("RemoteNamingInstanceRegisterDeregister"),
        "BatchInstanceRequest" => Some("RemoteNamingInstanceBatchRegister"),
        "PersistentInstanceRequest" => Some("RemoteNamingInstanceRegisterDeregister"),
        "ServiceQueryRequest" => Some("RemoteNamingServiceQuery"),
        "ServiceListRequest" => Some("RemoteNamingServiceListQuery"),
        "SubscribeServiceRequest" => Some("RemoteNamingServiceSubscribeUnsubscribe"),
        "HealthCheckRequest" => Some("HealthCheck"),
        _ => None,
    }
}

/// Trait for TPS control checking in gRPC handler dispatch.
#[tonic::async_trait]
pub trait TpsChecker: Send + Sync {
    /// Check if a request should be rate-limited. Returns Ok(()) if allowed,
    /// or Err with a Status if the request should be rejected.
    async fn check_tps(&self, message_type: &str, client_ip: &str) -> Result<(), Status>;
}

// Registry for managing payload handlers by message type
// Supports dynamic handler registration with logging for debugging
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn PayloadHandler>>,
    default_handler: Arc<dyn PayloadHandler>,
    auth_service: Arc<GrpcAuthService>,
    tps_checker: Option<Arc<dyn TpsChecker>>,
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
            tps_checker: None,
        }
    }

    /// Create a new HandlerRegistry with auth service
    pub fn with_auth(auth_service: GrpcAuthService) -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: Arc::new(DefaultHandler {}),
            auth_service: Arc::new(auth_service),
            tps_checker: None,
        }
    }

    /// Set the TPS checker for rate limiting gRPC requests
    pub fn set_tps_checker(&mut self, checker: Arc<dyn TpsChecker>) {
        self.tps_checker = Some(checker);
    }

    /// Check TPS limits for a message type. Returns Ok(()) if allowed.
    pub async fn check_tps(&self, message_type: &str, client_ip: &str) -> Result<(), Status> {
        if let Some(ref checker) = self.tps_checker {
            checker.check_tps(message_type, client_ip).await
        } else {
            Ok(())
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

            // Log all unary gRPC requests at debug level
            tracing::debug!(
                "[GRPC-UNARY] type={}, conn={}",
                message_type,
                connection.meta_info.connection_id
            );

            // Log FuzzyWatch requests only when debug is enabled (avoids UTF-8 decode cost)
            if tracing::enabled!(tracing::Level::DEBUG)
                && (message_type.contains("FuzzyWatch") || message_type.contains("FuzzySubscribe"))
            {
                let body = payload
                    .body
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(&b.value))
                    .unwrap_or_default();
                tracing::debug!(
                    "[FUZZY-DIAG] Received unary request: type={}, connection={}, body={}",
                    message_type,
                    connection.meta_info.connection_id,
                    &body[..body.len().min(200)]
                );
            }

            let handler = self.handler_registry.get_handler(message_type);

            // Check TPS limits before processing
            let client_ip = &connection.meta_info.remote_ip;
            self.handler_registry
                .check_tps(message_type, client_ip)
                .await?;

            // Validate request parameters
            crate::handler::param_check::check_request_params(message_type, payload)?;

            // Check authentication based on handler's auth requirement
            // Following Nacos RemoteRequestAuthFilter logic:
            // 1. Check if INNER_API and innerApiAuthEnabled (skip auth if needed)
            // 2. Check if auth is enabled globally for non-INNER_API
            // 3. Check server identity
            // 4. Check if enableAuth(secured) returns true
            // 5. Validate identity
            // 6. Validate authority (permission)
            // Derive response type from request type (e.g., ConfigQueryRequest -> ConfigQueryResponse)
            let response_type = message_type.replace("Request", "Response");

            let auth_requirement = handler.auth_requirement();
            match auth_requirement {
                AuthRequirement::None => {}
                AuthRequirement::Internal => {
                    // INNER_API: check server identity first
                    if let Err(e) =
                        check_server_identity(self.handler_registry.auth_service(), payload)
                    {
                        return Ok(Response::new(build_auth_error_payload(
                            &response_type,
                            NO_RIGHT,
                            &e.message(),
                        )));
                    }
                }
                AuthRequirement::Read | AuthRequirement::Write => {
                    // Non-INNER_API: full auth flow
                    let auth_service = self.handler_registry.auth_service();

                    // Check server identity first (MATCHED case skips remaining auth)
                    if auth_service.check_server_identity(payload_headers(payload)) {
                        // Server identity matched - skip remaining auth checks
                    } else {
                        // Check if auth should be enabled for this sign type
                        let sign_type = handler.sign_type();
                        if !enable_auth(auth_service, sign_type) {
                            // Auth not enabled for this type - skip validation
                        } else {
                            // Validate identity and load roles from database
                            let auth_context =
                                resolve_auth_context_from_payload(auth_service, payload).await;
                            if let Err(e) = check_authentication(&auth_context) {
                                return Ok(Response::new(build_auth_error_payload(
                                    &response_type,
                                    NO_RIGHT,
                                    &e.message(),
                                )));
                            }

                            // Validate authority (permission)
                            if let Some((resource, action)) = handler.resource_from_payload(payload)
                            {
                                // Global admin bypasses permission checks
                                if !auth_context.has_admin_role() {
                                    // Load permissions from database for non-admin users
                                    let permissions = auth_service
                                        .load_permissions_for_roles(&auth_context.roles)
                                        .await;
                                    if let Err(e) = check_authority(
                                        auth_service,
                                        &auth_context,
                                        &resource,
                                        action,
                                        &permissions,
                                    ) {
                                        return Ok(Response::new(build_auth_error_payload(
                                            &response_type,
                                            NO_RIGHT,
                                            &e.message(),
                                        )));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let handler_start = std::time::Instant::now();
            let result = handler.handle(&connection, payload).await;
            let duration = handler_start.elapsed().as_secs_f64();
            let success = result.is_ok();

            metrics::counter!("grpc_handler_calls_total", "type" => message_type.to_string())
                .increment(1);
            metrics::histogram!("grpc_handler_duration_seconds", "type" => message_type.to_string()).record(duration);
            if !success {
                metrics::counter!("grpc_handler_errors_total", "type" => message_type.to_string())
                    .increment(1);
            }

            return match result {
                Ok(response_payload) => Ok(Response::new(response_payload)),
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
    config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService>,
    connection_cleanup: Option<Arc<dyn ConnectionCleanupHandler>>,
}

impl GrpcBiRequestStreamService {
    pub fn new(
        handler_registry: HandlerRegistry,
        connection_manager: ConnectionManager,
        config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService>,
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
        config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService>,
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
                        // Update last active time on every message
                        connection_manager.touch_connection(&connection_id);

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
                                    .cloned()
                                    .unwrap_or_else(|| "-".to_string());
                                con.meta_info.namespace_id = request.tenant;

                                let connection_id = con.meta_info.connection_id.clone();
                                info!(
                                    "ConnectionSetupRequest: connection_id={}, client_ip={}, version={}",
                                    connection_id, con.meta_info.client_ip, con.meta_info.version
                                );

                                // Move con instead of cloning - con is not used after this
                                let client = GrpcClient::new(con, tx.clone());

                                connection_manager.register(&connection_id, client).await;

                                // Always send SetupAckRequest with server abilities to client.
                                // Nacos 3.x SDK requires this to enable FuzzyWatch, LockService, etc.
                                {
                                    static SERVER_ABILITIES: std::sync::LazyLock<
                                        HashMap<String, bool>,
                                    > = std::sync::LazyLock::new(|| {
                                        let mut m = HashMap::with_capacity(5);
                                        m.insert("fuzzyWatch".to_string(), true);
                                        m.insert("lock".to_string(), true);
                                        m.insert("mcp".to_string(), true);
                                        m.insert("agent".to_string(), true);
                                        m.insert(
                                            "supportPersistentInstanceByGrpc".to_string(),
                                            true,
                                        );
                                        m
                                    });

                                    let mut ack =
                                        batata_api::remote::model::SetupAckRequest::default();
                                    ack.ability_table = Some(SERVER_ABILITIES.clone());
                                    let ack_payload = ack.build_server_push_payload();
                                    info!(
                                        "Sending SetupAckRequest to {}, type={}",
                                        connection_id,
                                        ack_payload
                                            .metadata
                                            .as_ref()
                                            .map(|m| m.r#type.as_str())
                                            .unwrap_or("?")
                                    );
                                    if let Err(e) = bistream_send(&tx, Ok(ack_payload)).await {
                                        tracing::warn!("Failed to send SetupAckRequest: {}", e);
                                    } else {
                                        info!(
                                            "SetupAckRequest sent successfully to {}",
                                            connection_id
                                        );
                                    }
                                }

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

                            // Check TPS limits before processing
                            let client_ip = &connection.meta_info.remote_ip;
                            if let Err(e) =
                                handler_registry.check_tps(message_type, client_ip).await
                            {
                                tracing::warn!(
                                    "TPS limit exceeded for {}: {}",
                                    message_type,
                                    e.message()
                                );
                                if let Err(send_err) = bistream_send(&tx, Err(e)).await {
                                    tracing::error!(
                                        "Failed to send TPS error response: {}",
                                        send_err
                                    );
                                    break;
                                }
                                continue;
                            }

                            // Validate request parameters
                            if let Err(e) = crate::handler::param_check::check_request_params(
                                message_type,
                                &payload,
                            ) {
                                tracing::warn!(
                                    "Param check failed for {}: {}",
                                    message_type,
                                    e.message()
                                );
                                if let Err(send_err) = bistream_send(&tx, Err(e)).await {
                                    tracing::error!(
                                        "Failed to send param check error: {}",
                                        send_err
                                    );
                                    break;
                                }
                                continue;
                            }

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
                                    if auth_service.check_server_identity(payload_headers(&payload))
                                    {
                                        // Server identity matched - skip remaining auth checks
                                        Ok(())
                                    } else {
                                        // Check if auth should be enabled for this sign type
                                        let sign_type = handler.sign_type();
                                        if !enable_auth(auth_service, sign_type) {
                                            // Auth not enabled for this type - skip validation
                                            Ok(())
                                        } else {
                                            // Validate identity and load roles from database
                                            let auth_context = resolve_auth_context_from_payload(
                                                auth_service,
                                                &payload,
                                            )
                                            .await;
                                            let auth_check = check_authentication(&auth_context);
                                            if auth_check.is_err() {
                                                auth_check
                                            } else if let Some((resource, action)) =
                                                handler.resource_from_payload(&payload)
                                            {
                                                // Global admin bypasses permission checks
                                                if auth_context.has_admin_role() {
                                                    Ok(())
                                                } else {
                                                    // Load permissions from database
                                                    let permissions = auth_service
                                                        .load_permissions_for_roles(
                                                            &auth_context.roles,
                                                        )
                                                        .await;
                                                    check_authority(
                                                        auth_service,
                                                        &auth_context,
                                                        &resource,
                                                        action,
                                                        &permissions,
                                                    )
                                                }
                                            } else {
                                                Ok(())
                                            }
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
                                // Return auth error as a response payload (not gRPC Status)
                                // to match Nacos behavior - SDK checks errorCode in response body
                                let resp_type = message_type.replace("Request", "Response");
                                let auth_payload =
                                    build_auth_error_payload(&resp_type, NO_RIGHT, &e.message());
                                if let Err(send_err) = bistream_send(&tx, Ok(auth_payload)).await {
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
                                    if let Err(e) = bistream_send(&tx, Ok(response_payload)).await {
                                        tracing::error!("Failed to send response: {}", e);

                                        break;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Handler error: {}", e);
                                    if let Err(send_err) = bistream_send(&tx, Err(e)).await {
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
                        // Client disconnection (broken pipe, stream reset) is normal -
                        // log at debug level to avoid noisy ERROR logs
                        tracing::debug!("Bi-stream closed: {}", e);

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
            connection_manager.unregister(&connection_id).await;
        });

        let output_stream = ReceiverStream::new(rx);

        Ok(Response::new(
            Box::pin(output_stream) as Self::requestBiStreamStream
        ))
    }
}
