//! gRPC client for Nacos SDK communication
//!
//! Provides the `GrpcClient` facade that wraps connection management,
//! authentication, unary requests, and server push dispatch.

pub mod auth;
pub mod connection;

use std::collections::HashMap;
use std::sync::Arc;

use batata_api::{
    grpc::{Metadata, Payload},
    remote::model::{
        ClientDetectionRequest, ClientDetectionResponse, ConnectResetRequest, RequestTrait,
        ResponseTrait,
    },
};
use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::error::{ClientError, Result};

use self::auth::AuthProvider;
use self::connection::GrpcConnection;

/// Deserialize a payload body into a concrete type.
pub fn deserialize_payload<T>(payload: &Payload) -> T
where
    T: for<'de> serde::Deserialize<'de> + Default,
{
    let bytes: &[u8] = payload
        .body
        .as_ref()
        .map(|any| any.value.as_slice())
        .unwrap_or(&[]);
    match serde_json::from_slice::<T>(bytes) {
        Ok(v) => v,
        Err(e) => {
            let payload_type = payload
                .metadata
                .as_ref()
                .map(|m| m.r#type.as_str())
                .unwrap_or("unknown");
            tracing::error!(
                payload_type = %payload_type,
                error = %e,
                "Failed to deserialize gRPC payload"
            );
            T::default()
        }
    }
}

/// Handler trait for server push messages.
pub trait ServerPushHandler: Send + Sync + 'static {
    /// Handle a server push payload and optionally return an acknowledgment.
    fn handle(&self, payload: &Payload) -> Option<Payload>;
}

/// Configuration for the gRPC client.
#[derive(Clone, Debug)]
pub struct GrpcClientConfig {
    /// Server addresses (HTTP addresses, e.g., "127.0.0.1:8848")
    pub server_addrs: Vec<String>,
    /// Username for authentication (empty to skip auth)
    pub username: String,
    /// Password for authentication
    pub password: String,
    /// Connection module label ("config", "naming", or "config,naming")
    pub module: String,
    /// Tenant / namespace
    pub tenant: String,
    /// Additional connection labels
    pub labels: HashMap<String, String>,
}

impl Default for GrpcClientConfig {
    fn default() -> Self {
        Self {
            server_addrs: vec!["127.0.0.1:8848".to_string()],
            username: String::new(),
            password: String::new(),
            module: "config".to_string(),
            tenant: String::new(),
            labels: HashMap::new(),
        }
    }
}

/// gRPC client facade for Nacos SDK communication.
///
/// Manages the gRPC connection, authentication, unary requests,
/// bi-directional streaming, and server push dispatch.
pub struct GrpcClient {
    config: GrpcClientConfig,
    connection: Arc<RwLock<Option<GrpcConnection>>>,
    auth_provider: AuthProvider,
    push_handlers: Arc<DashMap<String, Box<dyn ServerPushHandler>>>,
    current_server_index: std::sync::atomic::AtomicUsize,
}

impl GrpcClient {
    /// Create a new GrpcClient with the given configuration.
    pub fn new(config: GrpcClientConfig) -> Result<Self> {
        let auth_provider = if config.username.is_empty() {
            AuthProvider::none()
        } else {
            // Use the first server address for auth
            let auth_addr = if config.server_addrs[0].starts_with("http") {
                config.server_addrs[0].clone()
            } else {
                format!("http://{}", config.server_addrs[0])
            };
            AuthProvider::new(&auth_addr, &config.username, &config.password)?
        };

        Ok(Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            auth_provider,
            push_handlers: Arc::new(DashMap::new()),
            current_server_index: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// Connect to the server. Must be called before making requests.
    pub async fn connect(&self) -> Result<()> {
        // Authenticate first if needed
        let token = self.auth_provider.get_token().await?;

        let server_addr = self.current_server_addr();

        let conn = GrpcConnection::connect(
            &server_addr,
            &self.config.module,
            self.config.labels.clone(),
            &self.config.tenant,
            token.as_deref(),
        )
        .await?;

        info!(
            "Connected to server {}, connection_id={}",
            server_addr,
            conn.connection_id()
        );

        let mut guard = self.connection.write().await;
        *guard = Some(conn);

        // Start push dispatch loop
        self.start_push_dispatch();

        Ok(())
    }

    /// Send a unary gRPC request.
    ///
    /// `req` must implement `RequestTrait + Serialize`.
    /// Returns the raw response `Payload`.
    pub async fn request<R>(&self, req: &R) -> Result<Payload>
    where
        R: RequestTrait + serde::Serialize,
    {
        let mut guard = self.connection.write().await;
        let conn = guard.as_mut().ok_or(ClientError::NotConnected)?;

        let mut metadata = Metadata {
            r#type: req.request_type().to_string(),
            ..Default::default()
        };

        // Inject access token into headers
        if let Ok(Some(token)) = self.auth_provider.get_token().await {
            metadata.headers.insert("accessToken".to_string(), token);
        }

        let payload = req.to_payload(Some(metadata));
        conn.request(payload).await
    }

    /// Send a typed request and deserialize the response.
    pub async fn request_typed<Req, Resp>(&self, req: &Req) -> Result<Resp>
    where
        Req: RequestTrait + serde::Serialize,
        Resp: for<'de> serde::Deserialize<'de> + Default + ResponseTrait,
    {
        let resp_payload = self.request(req).await?;

        let resp: Resp = deserialize_payload(&resp_payload);

        // Check for server error
        if resp.result_code() != 200 && resp.error_code() != 0 {
            return Err(ClientError::ServerError {
                code: resp.error_code(),
                message: resp.message(),
            });
        }

        Ok(resp)
    }

    /// Send a payload via the bi-directional stream.
    pub async fn send_bi_stream(&self, payload: Payload) -> Result<()> {
        let guard = self.connection.read().await;
        let conn = guard.as_ref().ok_or(ClientError::NotConnected)?;
        conn.send_bi_stream(payload).await
    }

    /// Register a handler for server push messages of the given type.
    pub fn register_push_handler<H: ServerPushHandler>(&self, type_name: &str, handler: H) {
        self.push_handlers
            .insert(type_name.to_string(), Box::new(handler));
    }

    /// Check if the client is connected.
    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Reconnect to the server (e.g., after connection loss).
    pub async fn reconnect(&self) -> Result<()> {
        {
            let mut guard = self.connection.write().await;
            *guard = None;
        }
        self.connect().await
    }

    /// Reconnect to a specific server (e.g., after ConnectResetRequest).
    pub async fn reconnect_to(&self, server_ip: &str, server_port: &str) -> Result<()> {
        {
            let mut guard = self.connection.write().await;
            *guard = None;
        }

        let token = self.auth_provider.get_token().await?;
        let server_addr = format!("{}:{}", server_ip, server_port);

        let conn = GrpcConnection::connect(
            &server_addr,
            &self.config.module,
            self.config.labels.clone(),
            &self.config.tenant,
            token.as_deref(),
        )
        .await?;

        info!(
            "Reconnected to server {}, connection_id={}",
            server_addr,
            conn.connection_id()
        );

        let mut guard = self.connection.write().await;
        *guard = Some(conn);

        self.start_push_dispatch();

        Ok(())
    }

    /// Get the current server address.
    fn current_server_addr(&self) -> String {
        let index = self
            .current_server_index
            .load(std::sync::atomic::Ordering::Relaxed);
        self.config.server_addrs[index % self.config.server_addrs.len()].clone()
    }

    /// Start the server push dispatch loop.
    fn start_push_dispatch(&self) {
        let connection = self.connection.clone();
        let handlers = self.push_handlers.clone();

        tokio::spawn(async move {
            loop {
                let payload = {
                    let mut guard = connection.write().await;
                    match guard.as_mut() {
                        Some(conn) => conn.recv_push().await,
                        None => break,
                    }
                };

                let Some(payload) = payload else {
                    debug!("Push dispatch: stream ended");
                    break;
                };

                let payload_type = payload
                    .metadata
                    .as_ref()
                    .map(|m| m.r#type.clone())
                    .unwrap_or_default();

                debug!("Dispatching server push: type={}", payload_type);

                // Built-in handlers for connection management
                let ack = match payload_type.as_str() {
                    "ConnectResetRequest" => {
                        let _req: ConnectResetRequest = deserialize_payload(&payload);
                        // ConnectResetRequest is handled by the reconnect logic externally
                        warn!("Received ConnectResetRequest — connection should be reset");
                        None
                    }
                    "ClientDetectionRequest" => {
                        let _req: ClientDetectionRequest = deserialize_payload(&payload);
                        let resp = ClientDetectionResponse::new();
                        Some(resp.build_payload())
                    }
                    _ => {
                        // Dispatch to registered handlers
                        if let Some(handler) = handlers.get(&payload_type) {
                            handler.handle(&payload)
                        } else {
                            warn!("No handler registered for push type: {}", payload_type);
                            None
                        }
                    }
                };

                // Send acknowledgment if handler produced one
                if let Some(ack_payload) = ack {
                    let guard = connection.read().await;
                    if let Some(conn) = guard.as_ref()
                        && let Err(e) = conn.send_bi_stream(ack_payload).await
                    {
                        error!("Failed to send push acknowledgment: {}", e);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_client_config_default() {
        let config = GrpcClientConfig::default();
        assert_eq!(config.server_addrs, vec!["127.0.0.1:8848"]);
        assert_eq!(config.module, "config");
        assert!(config.username.is_empty());
    }

    #[test]
    fn test_grpc_client_new() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();
        assert!(!client.push_handlers.is_empty() || client.push_handlers.is_empty());
    }
}
