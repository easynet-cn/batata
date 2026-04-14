//! gRPC client for Batata SDK communication
//!
//! Provides the `GrpcClient` facade that wraps connection management,
//! authentication, unary requests, and server push dispatch.

pub mod auth;
pub mod connection;
pub mod health;
pub mod metrics;

use std::collections::HashMap;
use std::sync::Arc;

use batata_api::{
    grpc::{Metadata, Payload},
    remote::model::{
        ClientDetectionRequest, ClientDetectionResponse, ConnectResetRequest, RequestTrait,
        ResponseTrait, ServerCheckRequest, ServerCheckResponse,
    },
};
use dashmap::DashMap;
use tokio::sync::{Notify, RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::error::{ClientError, Result};

use self::auth::AuthProvider;
use self::connection::GrpcConnection;
use self::health::ConnectionHealthChecker;

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
    /// Server context path (e.g., "/nacos"). Default: "/nacos".
    pub context_path: String,
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
    /// TLS CA certificate path (PEM format)
    pub tls_ca_cert: Option<String>,
    /// TLS client certificate path (for mTLS)
    pub tls_client_cert: Option<String>,
    /// TLS client key path (for mTLS)
    pub tls_client_key: Option<String>,
    /// Whether to enable TLS
    pub tls_enabled: bool,
    /// gRPC request timeout in seconds. Default: 10.
    pub request_timeout_secs: u64,
    /// Push message channel capacity. Default: 256.
    pub push_channel_capacity: usize,
    /// Auth HTTP connect timeout in seconds. Default: 5.
    pub auth_connect_timeout_secs: u64,
    /// Auth HTTP request timeout in seconds. Default: 10.
    pub auth_request_timeout_secs: u64,
    /// Token refresh check interval in seconds. Default: 5 (matches Nacos).
    pub token_refresh_interval_secs: u64,
    /// Token refresh buffer: refresh this many seconds before expiry. Default: 300.
    pub token_refresh_buffer_secs: u64,
    /// Connection setup delay in milliseconds. Default: 100.
    pub connection_setup_delay_ms: u64,
    /// Health check max failures before triggering reconnect. Default: 3.
    pub health_check_max_failures: u32,
    /// Health check interval in seconds. Default: 5.
    pub health_check_interval_secs: u64,
    /// Base delay between failover connection attempts (ms). Default: 500.
    pub reconnect_backoff_base_ms: u64,
    /// Maximum delay between failover connection attempts (ms). Default: 5000.
    pub reconnect_backoff_max_ms: u64,
    /// Random jitter added to each failover delay (ms). Default: 250.
    pub reconnect_backoff_jitter_ms: u64,
}

impl GrpcClientConfig {
    pub fn with_tls(mut self, ca_cert: &str) -> Self {
        self.tls_enabled = true;
        self.tls_ca_cert = Some(ca_cert.to_string());
        self
    }

    pub fn with_mtls(mut self, ca_cert: &str, client_cert: &str, client_key: &str) -> Self {
        self.tls_enabled = true;
        self.tls_ca_cert = Some(ca_cert.to_string());
        self.tls_client_cert = Some(client_cert.to_string());
        self.tls_client_key = Some(client_key.to_string());
        self
    }
}

impl Default for GrpcClientConfig {
    fn default() -> Self {
        Self {
            server_addrs: vec!["127.0.0.1:8848".to_string()],
            context_path: "/nacos".to_string(),
            username: String::new(),
            password: String::new(),
            module: "config".to_string(),
            tenant: String::new(),
            labels: HashMap::new(),
            tls_ca_cert: None,
            tls_client_cert: None,
            tls_client_key: None,
            tls_enabled: false,
            request_timeout_secs: 10,
            push_channel_capacity: 256,
            auth_connect_timeout_secs: 5,
            auth_request_timeout_secs: 10,
            token_refresh_interval_secs: 5,
            token_refresh_buffer_secs: 300,
            connection_setup_delay_ms: 100,
            health_check_max_failures: 3,
            health_check_interval_secs: 5,
            reconnect_backoff_base_ms: 500,
            reconnect_backoff_max_ms: 5_000,
            reconnect_backoff_jitter_ms: 250,
        }
    }
}

/// Connection state machine (matches Nacos RpcClient states)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionState {
    Starting = 0,
    Running = 1,
    Unhealthy = 2,
    Reconnecting = 3,
    Shutdown = 4,
}

/// Listener for connection lifecycle events.
///
/// Services implement this to automatically redo state on reconnect.
#[async_trait::async_trait]
pub trait ConnectionEventListener: Send + Sync + 'static {
    /// Called when connection is (re-)established.
    async fn on_connected(&self);
    /// Called when connection is lost.
    async fn on_disconnected(&self);
}

/// gRPC client facade for Batata SDK communication.
///
/// Manages the gRPC connection, authentication, unary requests,
/// bi-directional streaming, and server push dispatch.
pub struct GrpcClient {
    config: GrpcClientConfig,
    connection: Arc<RwLock<Option<GrpcConnection>>>,
    auth_provider: AuthProvider,
    push_handlers: Arc<DashMap<String, Box<dyn ServerPushHandler>>>,
    current_server_index: Arc<std::sync::atomic::AtomicUsize>,
    /// Connection state (lock-free query)
    state: Arc<std::sync::atomic::AtomicU8>,
    /// Shutdown signal for background tasks
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    /// Connection event listeners (notified on connect/disconnect)
    event_listeners: Arc<std::sync::RwLock<Vec<Arc<dyn ConnectionEventListener>>>>,
    /// Connection and request metrics
    pub metrics: Arc<metrics::ClientMetrics>,
    /// Health checker with exponential backoff
    health_checker: Arc<ConnectionHealthChecker>,
    /// Signal to trigger reconnection from push dispatch or health check
    reconnect_notify: Arc<Notify>,
}

impl GrpcClient {
    /// Create a GrpcClient from the unified `ClientConfig`.
    ///
    /// This is the preferred constructor — it uses `ClientConfig` for all settings
    /// including proxy, context_path, auth_endpoint, and timeouts.
    pub fn from_config(client_config: &crate::ClientConfig) -> Result<Self> {
        let grpc_config = GrpcClientConfig {
            server_addrs: client_config.server_addrs.clone(),
            context_path: client_config.context_path.clone(),
            username: client_config.username.clone(),
            password: client_config.password.clone(),
            module: client_config.module.clone(),
            tenant: client_config.namespace.clone(),
            labels: client_config.labels.clone(),
            tls_enabled: client_config.tls_enabled,
            tls_ca_cert: client_config.tls_ca_path.clone(),
            ..Default::default()
        };

        let auth_provider = if client_config.has_jwt_auth() {
            let auth_url = client_config.full_auth_url(&client_config.server_addrs[0]);
            // Extract base URL (up to port) for auth provider
            let base_and_path = auth_url.rsplitn(2, "/v3/").last().unwrap_or(&auth_url);
            AuthProvider::with_context_path(
                base_and_path,
                "", // context_path already included in full_auth_url
                &client_config.username,
                &client_config.password,
            )?
        } else {
            AuthProvider::none()
        };

        let health_checker = Arc::new(ConnectionHealthChecker::new(
            grpc_config.health_check_max_failures,
            std::time::Duration::from_secs(grpc_config.health_check_interval_secs),
        ));

        Ok(Self {
            config: grpc_config,
            connection: Arc::new(RwLock::new(None)),
            auth_provider,
            push_handlers: Arc::new(DashMap::new()),
            current_server_index: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            state: Arc::new(std::sync::atomic::AtomicU8::new(
                ConnectionState::Starting as u8,
            )),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_listeners: Arc::new(std::sync::RwLock::new(Vec::new())),
            metrics: Arc::new(metrics::ClientMetrics::new()),
            health_checker,
            reconnect_notify: Arc::new(Notify::new()),
        })
    }

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
            AuthProvider::with_context_path(
                &auth_addr,
                &config.context_path,
                &config.username,
                &config.password,
            )?
        };

        let health_checker = Arc::new(ConnectionHealthChecker::new(
            config.health_check_max_failures,
            std::time::Duration::from_secs(config.health_check_interval_secs),
        ));

        Ok(Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            auth_provider,
            push_handlers: Arc::new(DashMap::new()),
            current_server_index: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            state: Arc::new(std::sync::atomic::AtomicU8::new(
                ConnectionState::Starting as u8,
            )),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_listeners: Arc::new(std::sync::RwLock::new(Vec::new())),
            metrics: Arc::new(metrics::ClientMetrics::new()),
            health_checker,
            reconnect_notify: Arc::new(Notify::new()),
        })
    }

    /// Connect to the server. Must be called before making requests.
    ///
    /// Tries every server in `server_addrs` in order, starting from the
    /// current index, with bounded exponential backoff between attempts.
    /// Once a server accepts the connection we stay on that index — sticky
    /// session, no rotation mid-session — until disconnection.
    pub async fn connect(&self) -> Result<()> {
        // Authenticate first if needed
        let token = self.auth_provider.get_token().await?;

        let start_index = self.current_server_index();
        let (conn, push_rx, connected_index, server_addr) =
            Self::connect_with_failover(&self.config, start_index, token.as_deref()).await?;

        info!(
            "Connected to server {}, connection_id={}",
            server_addr,
            conn.connection_id()
        );

        self.current_server_index
            .store(connected_index, std::sync::atomic::Ordering::Relaxed);

        let mut guard = self.connection.write().await;
        *guard = Some(conn);
        drop(guard);

        // Start push dispatch loop with the separate push receiver
        self.start_push_dispatch(push_rx);

        // Start background token refresh task
        self.auth_provider.start_token_refresh_task();

        self.set_state(ConnectionState::Running);
        self.health_checker.set_connected();

        // Start background health check and reconnection loop
        self.start_health_check_loop();

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
        // Use read lock only — GrpcConnection::request() now clones Channel
        // internally so no &mut self needed. This enables concurrent requests.
        let guard = self.connection.read().await;
        let conn = guard.as_ref().ok_or(ClientError::NotConnected)?;

        let mut metadata = Metadata {
            r#type: req.request_type().to_string(),
            ..Default::default()
        };

        // Inject access token into headers
        if let Ok(Some(token)) = self.auth_provider.get_token().await {
            metadata.headers.insert("accessToken".to_string(), token);
        }

        let payload = req.to_payload(Some(metadata));

        // Apply configurable request timeout
        match tokio::time::timeout(
            std::time::Duration::from_secs(self.config.request_timeout_secs),
            conn.request(payload),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(ClientError::Other(anyhow::anyhow!("gRPC request timeout"))),
        }
    }

    /// Send a typed request and deserialize the response.
    ///
    /// If the server returns an auth error (403/UNAUTHENTICATED), the token
    /// is refreshed and the request is retried once — matching Nacos Java SDK
    /// behavior (see `ConfigRpcTransportClient.requestProxy`).
    pub async fn request_typed<Req, Resp>(&self, req: &Req) -> Result<Resp>
    where
        Req: RequestTrait + serde::Serialize,
        Resp: for<'de> serde::Deserialize<'de> + Default + ResponseTrait,
    {
        let resp_payload = self.request(req).await?;

        let resp: Resp = deserialize_payload(&resp_payload);

        // Check for auth error — refresh token and retry once (Nacos reLogin pattern)
        if resp.error_code() == 403 || resp.result_code() == 403 {
            warn!("Auth error (403), refreshing token and retrying...");
            if let Err(e) = self.auth_provider.refresh().await {
                warn!("Token refresh failed: {}", e);
            } else {
                // Retry the request with new token
                let resp_payload = self.request(req).await?;
                let resp: Resp = deserialize_payload(&resp_payload);
                if resp.result_code() != 200 && resp.error_code() != 0 {
                    return Err(ClientError::ServerError {
                        code: resp.error_code(),
                        message: resp.message(),
                    });
                }
                return Ok(resp);
            }
        }

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
    /// Get a reference to the connection RwLock (for Transport trait impl).
    pub fn connection_ref(&self) -> &RwLock<Option<GrpcConnection>> {
        &self.connection
    }

    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Get current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        match self.state.load(std::sync::atomic::Ordering::Relaxed) {
            0 => ConnectionState::Starting,
            1 => ConnectionState::Running,
            2 => ConnectionState::Unhealthy,
            3 => ConnectionState::Reconnecting,
            _ => ConnectionState::Shutdown,
        }
    }

    fn set_state(&self, state: ConnectionState) {
        self.state
            .store(state as u8, std::sync::atomic::Ordering::Relaxed);
    }

    /// Register a connection event listener.
    pub fn add_connection_listener(&self, listener: Arc<dyn ConnectionEventListener>) {
        if let Ok(mut listeners) = self.event_listeners.write() {
            listeners.push(listener);
        }
    }

    /// Notify all listeners of connection event.
    async fn fire_connected(&self) {
        let listeners = self
            .event_listeners
            .read()
            .map(|l| l.clone())
            .unwrap_or_default();
        for listener in &listeners {
            listener.on_connected().await;
        }
    }

    async fn fire_disconnected(&self) {
        let listeners = self
            .event_listeners
            .read()
            .map(|l| l.clone())
            .unwrap_or_default();
        for listener in &listeners {
            listener.on_disconnected().await;
        }
    }

    /// Reconnect to the server (e.g., after connection loss).
    ///
    /// Fires disconnected/connected events and triggers service redo.
    pub async fn reconnect(&self) -> Result<()> {
        self.set_state(ConnectionState::Reconnecting);
        self.fire_disconnected().await;
        {
            let mut guard = self.connection.write().await;
            *guard = None;
        }
        let result = self.connect().await;
        if result.is_ok() {
            self.fire_connected().await;
        }
        result
    }

    /// Reconnect to a specific server (e.g., after ConnectResetRequest).
    pub async fn reconnect_to(&self, server_ip: &str, server_port: &str) -> Result<()> {
        {
            let mut guard = self.connection.write().await;
            *guard = None;
        }

        let token = self.auth_provider.get_token().await?;
        let server_addr = format!("{}:{}", server_ip, server_port);

        let (conn, push_rx) = GrpcConnection::connect(
            &server_addr,
            &self.config.module,
            self.config.labels.clone(),
            &self.config.tenant,
            token.as_deref(),
            self.config.push_channel_capacity,
        )
        .await?;

        info!(
            "Reconnected to server {}, connection_id={}",
            server_addr,
            conn.connection_id()
        );

        let mut guard = self.connection.write().await;
        *guard = Some(conn);
        drop(guard);

        self.start_push_dispatch(push_rx);

        Ok(())
    }

    /// Check if the server is healthy by sending a ServerCheckRequest.
    pub async fn check_server_status(&self) -> Result<bool> {
        let req = ServerCheckRequest::new();
        match self.request_typed::<_, ServerCheckResponse>(&req).await {
            Ok(resp) => Ok(resp.response.success || resp.response.result_code == 200),
            Err(e) => {
                warn!("Server health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get the address of the server we're currently connected to (or
    /// targeting, if a reconnect is in progress).
    ///
    /// Useful for tests and observability — lets callers learn which node in a
    /// multi-server deployment a client is talking to. Wraps the index modulo
    /// the configured server list length.
    pub fn current_server_addr(&self) -> String {
        let index = self
            .current_server_index
            .load(std::sync::atomic::Ordering::Relaxed);
        self.config.server_addrs[index % self.config.server_addrs.len()].clone()
    }

    /// Get the current server index (modulo server list length).
    pub fn current_server_index(&self) -> usize {
        let index = self
            .current_server_index
            .load(std::sync::atomic::Ordering::Relaxed);
        index % self.config.server_addrs.len()
    }

    /// Signal the reconnection loop to attempt a reconnect.
    pub fn signal_reconnect(&self) {
        self.reconnect_notify.notify_one();
    }

    /// Try to connect to each server in `config.server_addrs`, starting from
    /// `start_index` and wrapping around. Returns the established connection,
    /// push receiver, the absolute index of the server that accepted, and its
    /// address. Fails if every server in the list rejected within one full
    /// pass.
    ///
    /// Backoff: between failed attempts we sleep for
    /// `min(base * 2^attempt, max) + random(0..jitter)` ms.
    async fn connect_with_failover(
        config: &GrpcClientConfig,
        start_index: usize,
        token: Option<&str>,
    ) -> Result<(GrpcConnection, mpsc::Receiver<Payload>, usize, String)> {
        try_failover(config, start_index, |addr| {
            let module = config.module.clone();
            let labels = config.labels.clone();
            let tenant = config.tenant.clone();
            let push_capacity = config.push_channel_capacity;
            let token = token.map(|s| s.to_string());
            let addr = addr.to_string();
            async move {
                GrpcConnection::connect(
                    &addr,
                    &module,
                    labels,
                    &tenant,
                    token.as_deref(),
                    push_capacity,
                )
                .await
            }
        })
        .await
    }

    /// Start the background health check and auto-reconnection loop.
    ///
    /// Matches Nacos Java RpcClient behavior:
    /// - Periodic health checks at configurable intervals
    /// - Multiple retry attempts per check cycle (default: 3)
    /// - Exponential backoff on reconnection: min(retryTurns+1, 50) * 100ms
    /// - Reconnection triggered by health check failure or external signal
    fn start_health_check_loop(&self) {
        let connection = self.connection.clone();
        let state = self.state.clone();
        let shutdown = self.shutdown.clone();
        let health_checker = self.health_checker.clone();
        let reconnect_notify = self.reconnect_notify.clone();
        let event_listeners = self.event_listeners.clone();
        let metrics = self.metrics.clone();

        // Clone what we need for reconnection
        let config = self.config.clone();
        let push_handlers = self.push_handlers.clone();
        let current_server_index = self.current_server_index.clone();

        tokio::spawn(async move {
            let interval = health_checker.interval();

            loop {
                if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    debug!("Health check loop: shutdown signal received");
                    break;
                }

                // Wait for either the check interval or an external reconnect signal
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {},
                    _ = reconnect_notify.notified() => {
                        debug!("Health check loop: reconnect signal received");
                    },
                }

                if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                let current_state = state.load(std::sync::atomic::Ordering::Relaxed);
                if current_state == ConnectionState::Shutdown as u8 {
                    break;
                }

                // If we're in Running state, perform health check
                if current_state == ConnectionState::Running as u8 {
                    let is_healthy = {
                        let guard = connection.read().await;
                        if let Some(conn) = guard.as_ref() {
                            // Try health check with retries (matches Nacos healthCheck())
                            let mut healthy = false;
                            for retry in 0..health_checker.health_check_retry_times() {
                                let channel = conn.channel();
                                match GrpcConnection::health_check(&channel).await {
                                    Ok(true) => {
                                        healthy = true;
                                        break;
                                    }
                                    Ok(false) | Err(_) => {
                                        if retry < health_checker.health_check_retry_times() - 1 {
                                            // Random sleep 0-500ms between retries (matches Nacos)
                                            let jitter = rand::random::<u64>() % 500;
                                            tokio::time::sleep(std::time::Duration::from_millis(
                                                jitter,
                                            ))
                                            .await;
                                        }
                                    }
                                }
                            }
                            healthy
                        } else {
                            false
                        }
                    };

                    if is_healthy {
                        health_checker.record_success();
                        continue;
                    }

                    // Health check failed
                    if health_checker.record_failure() {
                        warn!(
                            "Health check failed {} times, triggering reconnection",
                            health_checker.failure_count()
                        );
                        state.store(
                            ConnectionState::Unhealthy as u8,
                            std::sync::atomic::Ordering::Relaxed,
                        );
                        // Fall through to reconnection logic below
                    } else {
                        continue;
                    }
                }

                // Reconnection logic (for Unhealthy or Reconnecting states)
                if current_state == ConnectionState::Unhealthy as u8
                    || current_state == ConnectionState::Reconnecting as u8
                {
                    state.store(
                        ConnectionState::Reconnecting as u8,
                        std::sync::atomic::Ordering::Relaxed,
                    );

                    // Fire disconnected event
                    {
                        let listeners = event_listeners
                            .read()
                            .map(|l| l.clone())
                            .unwrap_or_default();
                        for listener in &listeners {
                            listener.on_disconnected().await;
                        }
                    }

                    // Exponential backoff delay
                    let delay = health_checker.next_reconnect_delay();
                    debug!(
                        "Reconnecting after {:?} delay (retry turn {})",
                        delay,
                        health_checker.retry_turns()
                    );
                    tokio::time::sleep(delay).await;

                    // Clear old connection
                    {
                        let mut guard = connection.write().await;
                        *guard = None;
                    }

                    // Try to reconnect with failover across all configured servers.
                    // Advance the index from the last-known position so we cycle
                    // through nodes (Nacos round-robin failover semantics).
                    // Note: we don't have auth_provider here, so pass None for token
                    // The auth will be re-established on the next request.
                    let next_index =
                        (current_server_index.load(std::sync::atomic::Ordering::Relaxed) + 1)
                            % config.server_addrs.len().max(1);
                    let result = Self::connect_with_failover(&config, next_index, None).await;

                    match result {
                        Ok((conn, push_rx, connected_index, server_addr)) => {
                            info!(
                                "Reconnected to server {}, connection_id={}",
                                server_addr,
                                conn.connection_id()
                            );
                            current_server_index
                                .store(connected_index, std::sync::atomic::Ordering::Relaxed);

                            {
                                let mut guard = connection.write().await;
                                *guard = Some(conn);
                            }

                            // Start push dispatch for new connection
                            Self::start_push_dispatch_static(
                                push_rx,
                                connection.clone(),
                                push_handlers.clone(),
                                shutdown.clone(),
                                reconnect_notify.clone(),
                            );

                            state.store(
                                ConnectionState::Running as u8,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                            health_checker.set_connected();
                            metrics.record_reconnect(true);

                            // Fire connected event
                            {
                                let listeners = event_listeners
                                    .read()
                                    .map(|l| l.clone())
                                    .unwrap_or_default();
                                for listener in &listeners {
                                    listener.on_connected().await;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Reconnection failed: {}, will retry", e);
                            metrics.record_reconnect(false);
                        }
                    }
                }
            }
        });
    }

    /// Start the server push dispatch loop.
    ///
    /// The push receiver is passed separately so the dispatch loop does not
    /// need to hold the connection lock while waiting for messages.
    fn start_push_dispatch(&self, push_rx: mpsc::Receiver<Payload>) {
        Self::start_push_dispatch_static(
            push_rx,
            self.connection.clone(),
            self.push_handlers.clone(),
            self.shutdown.clone(),
            self.reconnect_notify.clone(),
        );
    }

    /// Static version of push dispatch that doesn't require &self.
    /// Used by both initial connect and reconnection in the health check loop.
    fn start_push_dispatch_static(
        mut push_rx: mpsc::Receiver<Payload>,
        connection: Arc<RwLock<Option<GrpcConnection>>>,
        handlers: Arc<DashMap<String, Box<dyn ServerPushHandler>>>,
        shutdown: Arc<std::sync::atomic::AtomicBool>,
        reconnect_notify: Arc<Notify>,
    ) {
        tokio::spawn(async move {
            loop {
                if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    debug!("Push dispatch: shutdown signal received");
                    break;
                }

                let payload = push_rx.recv().await;

                let Some(payload) = payload else {
                    // Stream ended — signal reconnection (matches Nacos onError/onCompleted)
                    warn!("Push dispatch: stream ended, signaling reconnect");
                    reconnect_notify.notify_one();
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
                        let req: ConnectResetRequest = deserialize_payload(&payload);
                        warn!(
                            "Received ConnectResetRequest, server_ip={}, server_port={}",
                            req.server_ip, req.server_port
                        );
                        // Clear connection and signal reconnection
                        // (matches Nacos: trigger server switch)
                        {
                            let mut guard = connection.write().await;
                            *guard = None;
                        }
                        reconnect_notify.notify_one();
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

/// Generic failover loop shared by initial connect and reconnect.
///
/// Iterates through `config.server_addrs` starting from `start_index`,
/// invoking `attempt(addr)` for each. Returns on the first success, mapping
/// it to `(value, absolute_index, address)`. Between failures we sleep
/// `min(base * 2^retry, max) + random(0..jitter)` ms. After a full pass
/// without success, returns the last error (wrapped in `ClientError::Other`
/// if no specific error was produced).
async fn try_failover<F, Fut, T>(
    config: &GrpcClientConfig,
    start_index: usize,
    mut attempt: F,
) -> Result<(T, mpsc::Receiver<Payload>, usize, String)>
where
    F: FnMut(&str) -> Fut,
    Fut: std::future::Future<Output = Result<(T, mpsc::Receiver<Payload>)>>,
{
    let n = config.server_addrs.len();
    if n == 0 {
        return Err(ClientError::Other(anyhow::anyhow!(
            "no server addresses configured"
        )));
    }

    let mut last_err: Option<ClientError> = None;
    for offset in 0..n {
        let idx = (start_index + offset) % n;
        let addr = config.server_addrs[idx].clone();
        match attempt(&addr).await {
            Ok((value, push_rx)) => return Ok((value, push_rx, idx, addr)),
            Err(e) => {
                warn!("Failover: connection to {} failed: {}", addr, e);
                last_err = Some(e);
                // Apply backoff before trying the next server (skip last attempt).
                if offset + 1 < n {
                    let delay = compute_backoff_ms(config, offset as u32);
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ClientError::Other(anyhow::anyhow!("all servers failed and no error captured"))
    }))
}

/// Compute the backoff delay in milliseconds for the given retry attempt.
fn compute_backoff_ms(config: &GrpcClientConfig, attempt: u32) -> u64 {
    let base = config.reconnect_backoff_base_ms;
    let max = config.reconnect_backoff_max_ms.max(base);
    // Saturating shift to avoid overflow on large attempt numbers.
    let exp = base.saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
    let capped = exp.min(max);
    let jitter = if config.reconnect_backoff_jitter_ms == 0 {
        0
    } else {
        rand::random::<u64>() % config.reconnect_backoff_jitter_ms
    };
    capped + jitter
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
        assert!(config.password.is_empty());
        assert!(config.tenant.is_empty());
        assert!(config.labels.is_empty());
    }

    #[test]
    fn test_grpc_client_config_custom() {
        let config = GrpcClientConfig {
            server_addrs: vec!["10.0.0.1:8848".to_string(), "10.0.0.2:8848".to_string()],
            username: "nacos".to_string(),
            password: "secret".to_string(),
            module: "naming".to_string(),
            tenant: "public".to_string(),
            labels: {
                let mut m = HashMap::new();
                m.insert("env".to_string(), "prod".to_string());
                m
            },
            ..Default::default()
        };
        assert_eq!(config.server_addrs.len(), 2);
        assert_eq!(config.module, "naming");
        assert_eq!(config.labels.get("env").unwrap(), "prod");
    }

    #[test]
    fn test_grpc_client_config_clone() {
        let config = GrpcClientConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.server_addrs, config.server_addrs);
        assert_eq!(cloned.module, config.module);
    }

    #[test]
    fn test_grpc_client_new() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();
        assert!(client.push_handlers.is_empty());
    }

    #[test]
    fn test_grpc_client_new_with_auth() {
        let config = GrpcClientConfig {
            username: "nacos".to_string(),
            password: "nacos".to_string(),
            ..Default::default()
        };
        let client = GrpcClient::new(config).unwrap();
        assert!(client.auth_provider.is_enabled());
    }

    #[test]
    fn test_grpc_client_new_without_auth() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();
        assert!(!client.auth_provider.is_enabled());
    }

    #[tokio::test]
    async fn test_grpc_client_not_connected() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();
        assert!(!client.is_connected().await);
    }

    #[test]
    fn test_register_push_handler() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();

        struct TestHandler;
        impl ServerPushHandler for TestHandler {
            fn handle(&self, _payload: &Payload) -> Option<Payload> {
                None
            }
        }

        client.register_push_handler("TestRequest", TestHandler);
        assert!(client.push_handlers.contains_key("TestRequest"));
        assert_eq!(client.push_handlers.len(), 1);
    }

    #[test]
    fn test_register_multiple_push_handlers() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();

        struct Handler1;
        impl ServerPushHandler for Handler1 {
            fn handle(&self, _: &Payload) -> Option<Payload> {
                None
            }
        }
        struct Handler2;
        impl ServerPushHandler for Handler2 {
            fn handle(&self, _: &Payload) -> Option<Payload> {
                None
            }
        }

        client.register_push_handler("Type1", Handler1);
        client.register_push_handler("Type2", Handler2);
        assert_eq!(client.push_handlers.len(), 2);
    }

    #[test]
    fn test_register_push_handler_overwrites() {
        let config = GrpcClientConfig::default();
        let client = GrpcClient::new(config).unwrap();

        struct Handler1;
        impl ServerPushHandler for Handler1 {
            fn handle(&self, _: &Payload) -> Option<Payload> {
                None
            }
        }
        struct Handler2;
        impl ServerPushHandler for Handler2 {
            fn handle(&self, _: &Payload) -> Option<Payload> {
                None
            }
        }

        client.register_push_handler("SameType", Handler1);
        client.register_push_handler("SameType", Handler2);
        assert_eq!(client.push_handlers.len(), 1);
    }

    #[test]
    fn test_current_server_addr() {
        let config = GrpcClientConfig {
            server_addrs: vec!["10.0.0.1:8848".to_string(), "10.0.0.2:8848".to_string()],
            ..Default::default()
        };
        let client = GrpcClient::new(config).unwrap();
        assert_eq!(client.current_server_addr(), "10.0.0.1:8848");
    }

    #[test]
    fn test_current_server_addr_wraps() {
        let config = GrpcClientConfig {
            server_addrs: vec!["10.0.0.1:8848".to_string(), "10.0.0.2:8848".to_string()],
            ..Default::default()
        };
        let client = GrpcClient::new(config).unwrap();
        // Set index beyond length
        client
            .current_server_index
            .store(3, std::sync::atomic::Ordering::Relaxed);
        // Should wrap: 3 % 2 = 1
        assert_eq!(client.current_server_addr(), "10.0.0.2:8848");
    }

    #[test]
    fn test_deserialize_payload_empty() {
        use batata_api::remote::model::ServerCheckResponse;

        let payload = Payload {
            metadata: None,
            body: None,
        };

        // Empty body should return default
        let resp: ServerCheckResponse = deserialize_payload(&payload);
        assert_eq!(resp.connection_id, "");
    }

    #[test]
    fn test_deserialize_payload_valid() {
        use batata_api::remote::model::{ResponseTrait, ServerCheckResponse};

        // Build a payload using the trait method
        let original = ServerCheckResponse {
            connection_id: "test-conn-id".to_string(),
            ..Default::default()
        };
        let payload = original.build_payload();

        let resp: ServerCheckResponse = deserialize_payload(&payload);
        assert_eq!(resp.connection_id, "test-conn-id");
    }

    // ---- Failover tests ----
    //
    // These tests exercise `try_failover` (the generic helper used by both
    // initial connect and reconnect) with synthetic attempt closures, so we
    // can assert routing behavior without standing up a real gRPC server.

    fn test_config(addrs: Vec<&str>) -> GrpcClientConfig {
        GrpcClientConfig {
            server_addrs: addrs.into_iter().map(String::from).collect(),
            // Make the test fast — no jitter, tiny base delay.
            reconnect_backoff_base_ms: 1,
            reconnect_backoff_max_ms: 1,
            reconnect_backoff_jitter_ms: 0,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_failover_skips_failing_server() {
        let config = test_config(vec!["bad:8848", "good:8848"]);
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let attempts_cl = attempts.clone();

        let result = try_failover(&config, 0, |addr| {
            let attempts = attempts_cl.clone();
            let addr_owned = addr.to_string();
            async move {
                attempts.lock().unwrap().push(addr_owned.clone());
                if addr_owned.starts_with("bad") {
                    Err(ClientError::NotConnected)
                } else {
                    let (_tx, rx) = mpsc::channel::<Payload>(1);
                    Ok(((), rx))
                }
            }
        })
        .await;

        let (_, _, idx, addr) = result.expect("failover should succeed on second server");
        assert_eq!(idx, 1, "should have settled on index 1");
        assert_eq!(addr, "good:8848");
        let attempts = attempts.lock().unwrap();
        assert_eq!(attempts.len(), 2, "should have tried both servers");
        assert_eq!(attempts[0], "bad:8848");
        assert_eq!(attempts[1], "good:8848");
    }

    #[tokio::test]
    async fn test_failover_all_fail_returns_error_bounded() {
        let config = test_config(vec!["a:8848", "b:8848", "c:8848"]);
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_cl = attempts.clone();

        let result: Result<((), _, _, _)> = try_failover(&config, 0, |_addr| {
            let attempts = attempts_cl.clone();
            async move {
                attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Err(ClientError::NotConnected)
            }
        })
        .await;

        assert!(result.is_err(), "all-failures must return an error");
        // Must attempt each address exactly once — no infinite loop.
        assert_eq!(
            attempts.load(std::sync::atomic::Ordering::Relaxed),
            3,
            "should try each server exactly once per pass"
        );
        match result.unwrap_err() {
            ClientError::NotConnected => {}
            other => panic!("expected last NotConnected error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_failover_index_wraps() {
        // Start at index 1, addrs[1] fails, addrs[0] should be tried next via wrap-around.
        let config = test_config(vec!["good:8848", "bad:8848"]);
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let attempts_cl = attempts.clone();

        let result = try_failover(&config, 1, |addr| {
            let attempts = attempts_cl.clone();
            let addr_owned = addr.to_string();
            async move {
                attempts.lock().unwrap().push(addr_owned.clone());
                if addr_owned.starts_with("bad") {
                    Err(ClientError::NotConnected)
                } else {
                    let (_tx, rx) = mpsc::channel::<Payload>(1);
                    Ok(((), rx))
                }
            }
        })
        .await;

        let (_, _, idx, addr) = result.expect("wrap-around should succeed");
        assert_eq!(idx, 0, "should wrap around to index 0");
        assert_eq!(addr, "good:8848");
        let attempts = attempts.lock().unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], "bad:8848", "first attempt is start_index=1");
        assert_eq!(attempts[1], "good:8848", "second attempt wraps to 0");
    }

    #[tokio::test]
    async fn test_failover_empty_server_list() {
        let config = test_config(vec![]);
        let result: Result<((), _, _, _)> =
            try_failover(&config, 0, |_| async { Err(ClientError::NotConnected) }).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_backoff_caps_at_max() {
        let config = GrpcClientConfig {
            reconnect_backoff_base_ms: 100,
            reconnect_backoff_max_ms: 1000,
            reconnect_backoff_jitter_ms: 0,
            ..Default::default()
        };
        assert_eq!(compute_backoff_ms(&config, 0), 100);
        assert_eq!(compute_backoff_ms(&config, 1), 200);
        assert_eq!(compute_backoff_ms(&config, 2), 400);
        assert_eq!(compute_backoff_ms(&config, 3), 800);
        // capped
        assert_eq!(compute_backoff_ms(&config, 4), 1000);
        assert_eq!(compute_backoff_ms(&config, 50), 1000);
    }

    #[test]
    fn test_compute_backoff_with_jitter() {
        let config = GrpcClientConfig {
            reconnect_backoff_base_ms: 100,
            reconnect_backoff_max_ms: 100,
            reconnect_backoff_jitter_ms: 50,
            ..Default::default()
        };
        for _ in 0..32 {
            let d = compute_backoff_ms(&config, 0);
            assert!(d >= 100, "delay must be at least base ({})", d);
            assert!(d < 150, "delay must be less than base + jitter ({})", d);
        }
    }

    #[tokio::test]
    async fn test_failover_succeeds_with_real_listener() {
        // Bind a TCP listener to an ephemeral port. We don't speak gRPC over
        // it — the failover helper just runs the closure we give it. This
        // proves the helper integrates with real-port resolution patterns.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let good = format!("127.0.0.1:{}", port);
        let config = test_config(vec!["127.0.0.1:1", &good]);

        let result = try_failover(&config, 0, |addr| {
            let addr_owned = addr.to_string();
            async move {
                if addr_owned == "127.0.0.1:1" {
                    Err(ClientError::NotConnected)
                } else {
                    let (_tx, rx) = mpsc::channel::<Payload>(1);
                    Ok(((), rx))
                }
            }
        })
        .await;

        let (_, _, idx, addr) = result.expect("must succeed on the listener address");
        assert_eq!(idx, 1);
        assert_eq!(addr, good);
        drop(listener);
    }
}
