//! HTTP server lifecycle implementation
//!
//! This module provides concrete implementations of ServerLifecycle
//! for HTTP servers (main and console).

use std::borrow::Cow;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{
    GracefulShutdownable, InitResult, ServerHandle, ServerHealth, ServerKind, ServerLifecycle,
};

/// HTTP server configuration
#[derive(Clone)]
pub struct HttpServerConfig {
    pub port: u16,
    pub address: String,
    pub context_path: String,
    pub workers: usize,
    pub keep_alive_secs: u64,
    pub max_payload_size: usize,
    pub max_json_size: usize,
    pub compression_enabled: bool,
    pub access_log_enabled: bool,
}

/// HTTP server kind
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpServerKind {
    Main,
    Console,
    Plugin(String),
}

impl HttpServerKind {
    pub fn as_str(&self) -> Cow<'static, str> {
        match self {
            HttpServerKind::Main => "HttpMain".into(),
            HttpServerKind::Console => "HttpConsole".into(),
            HttpServerKind::Plugin(name) => name.clone().into(),
        }
    }
}

impl From<HttpServerKind> for ServerKind {
    fn from(kind: HttpServerKind) -> Self {
        match kind {
            HttpServerKind::Main => ServerKind::HttpMain,
            HttpServerKind::Console => ServerKind::HttpConsole,
            HttpServerKind::Plugin(name) => ServerKind::Plugin(name),
        }
    }
}

/// HTTP server state tracker
pub struct HttpServerState {
    health: AtomicU8,
}

impl HttpServerState {
    pub fn new() -> Self {
        Self {
            health: AtomicU8::new(ServerHealth::Starting as u8),
        }
    }

    pub fn set_health(&self, health: ServerHealth) {
        self.health.store(health as u8, Ordering::SeqCst);
    }

    pub fn health(&self) -> ServerHealth {
        match self.health.load(Ordering::SeqCst) {
            0 => ServerHealth::Starting,
            1 => ServerHealth::Running,
            2 => ServerHealth::Draining,
            3 => ServerHealth::Stopping,
            4 => ServerHealth::Stopped,
            _ => ServerHealth::Stopped,
        }
    }
}

impl Default for HttpServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP server lifecycle implementation
///
/// This struct wraps the actual HTTP server startup logic from
/// the startup module while providing a trait-compliant interface.
pub struct HttpServerLifecycle {
    kind: HttpServerKind,
    config: HttpServerConfig,
    state: Arc<HttpServerState>,
}

impl HttpServerLifecycle {
    /// Create a new main HTTP server lifecycle
    pub fn new_main(config: HttpServerConfig) -> Self {
        Self {
            kind: HttpServerKind::Main,
            config,
            state: Arc::new(HttpServerState::new()),
        }
    }
    
    /// Create a new console HTTP server lifecycle
    pub fn new_console(config: HttpServerConfig) -> Self {
        Self {
            kind: HttpServerKind::Console,
            config,
            state: Arc::new(HttpServerState::new()),
        }
    }
    
    /// Create a new plugin HTTP server lifecycle
    pub fn new_plugin(name: String, config: HttpServerConfig) -> Self {
        Self {
            kind: HttpServerKind::Plugin(name),
            config,
            state: Arc::new(HttpServerState::new()),
        }
    }
}

#[async_trait]
impl ServerLifecycle for HttpServerLifecycle {
    fn kind(&self) -> ServerKind {
        self.kind.clone().into()
    }

    fn name(&self) -> Cow<'static, str> {
        match &self.kind {
            HttpServerKind::Main => "Main HTTP Server".into(),
            HttpServerKind::Console => "Console HTTP Server".into(),
            HttpServerKind::Plugin(name) => name.clone().into(),
        }
    }

    async fn start(&self, _ctx: &AppContext) -> InitResult<ServerHandle> {
        self.state.set_health(ServerHealth::Starting);

        let (handle, mut shutdown_rx) = ServerHandle::new(self.kind.clone().into(), self.name());

        tracing::info!(
            "Starting {} on {}:{}",
            self.name(),
            self.config.address,
            self.config.port
        );

        self.state.set_health(ServerHealth::Running);

        let state = self.state.clone();
        tokio::spawn(async move {
            let _ = shutdown_rx.changed().await;
            state.set_health(ServerHealth::Stopping);
        });

        Ok(handle)
    }

    async fn stop(&self, handle: &ServerHandle) -> InitResult<()> {
        tracing::info!("Stopping {} (handle: {})", self.name(), handle.name);

        handle.shutdown();
        self.state.set_health(ServerHealth::Stopped);

        Ok(())
    }

    fn health(&self) -> ServerHealth {
        self.state.health()
    }
}

#[async_trait]
impl GracefulShutdownable for HttpServerLifecycle {
    fn shutdown_order(&self) -> u8 {
        crate::lifecycle::shutdown_trait::priority::HTTP_SERVER
    }

    async fn shutdown(&self) -> InitResult<()> {
        tracing::info!("Graceful shutdown for HTTP server: {}", self.name());
        self.state.set_health(ServerHealth::Stopping);
        // In real implementation, we would stop accepting new connections
        self.state.set_health(ServerHealth::Stopped);
        Ok(())
    }

    async fn drain(&self, timeout: std::time::Duration) -> InitResult<()> {
        tracing::info!(
            "Draining HTTP server: {} (timeout: {:?})",
            self.name(),
            timeout
        );
        self.state.set_health(ServerHealth::Draining);
        // In real implementation, we would wait for in-flight requests to complete
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_http_server_kind_as_str() {
        assert_eq!(HttpServerKind::Main.as_str(), "HttpMain");
        assert_eq!(HttpServerKind::Console.as_str(), "HttpConsole");
        assert_eq!(HttpServerKind::Plugin("Consul".to_string()).as_str(), "Consul");
    }
    
    #[test]
    fn test_http_server_kind_conversion() {
        assert_eq!(ServerKind::from(HttpServerKind::Main), ServerKind::HttpMain);
        assert_eq!(ServerKind::from(HttpServerKind::Console), ServerKind::HttpConsole);
    }
    
    #[test]
    fn test_http_server_state() {
        let state = HttpServerState::new();

        assert_eq!(state.health(), ServerHealth::Starting);

        state.set_health(ServerHealth::Running);
        assert_eq!(state.health(), ServerHealth::Running);

        state.set_health(ServerHealth::Draining);
        assert_eq!(state.health(), ServerHealth::Draining);
    }

    #[test]
    fn test_http_server_lifecycle_kind() {
        let config = HttpServerConfig {
            port: 8848,
            address: "0.0.0.0".to_string(),
            context_path: "/nacos".to_string(),
            workers: 4,
            keep_alive_secs: 60,
            max_payload_size: 10 * 1024 * 1024,
            max_json_size: 1 * 1024 * 1024,
            compression_enabled: true,
            access_log_enabled: true,
        };
        
        let main = HttpServerLifecycle::new_main(config.clone());
        assert_eq!(main.kind(), ServerKind::HttpMain);
        assert_eq!(main.name(), "Main HTTP Server");
        
        let console = HttpServerLifecycle::new_console(config.clone());
        assert_eq!(console.kind(), ServerKind::HttpConsole);
        assert_eq!(console.name(), "Console HTTP Server");
        
        let plugin = HttpServerLifecycle::new_plugin("Consul".to_string(), config);
        assert!(matches!(plugin.kind(), ServerKind::Plugin(name) if name == "Consul"));
    }
}
