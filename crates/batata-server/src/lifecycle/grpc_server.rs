//! gRPC server lifecycle implementation
//!
//! This module provides concrete implementations of ServerLifecycle
//! for gRPC servers (SDK, cluster, and raft).

use std::borrow::Cow;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{
    GracefulShutdownable, InitResult, ServerHandle, ServerHealth, ServerKind, ServerLifecycle,
};

/// gRPC server kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcServerKind {
    Sdk,
    Cluster,
    Raft,
}

impl GrpcServerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            GrpcServerKind::Sdk => "GrpcSdk",
            GrpcServerKind::Cluster => "GrpcCluster",
            GrpcServerKind::Raft => "GrpcRaft",
        }
    }
}

impl From<GrpcServerKind> for ServerKind {
    fn from(kind: GrpcServerKind) -> Self {
        match kind {
            GrpcServerKind::Sdk => ServerKind::GrpcSdk,
            GrpcServerKind::Cluster => ServerKind::GrpcCluster,
            GrpcServerKind::Raft => ServerKind::GrpcRaft,
        }
    }
}

/// gRPC server configuration
#[derive(Clone)]
pub struct GrpcServerConfig {
    pub port: u16,
    pub tls_enabled: bool,
    pub tcp_keepalive_secs: u64,
    pub tcp_nodelay: bool,
    pub http2_keepalive_interval_secs: u64,
    pub http2_keepalive_timeout_secs: u64,
    pub concurrency_limit: usize,
    pub max_concurrent_streams: u32,
    pub initial_connection_window_size: u32,
    pub initial_stream_window_size: u32,
    pub max_frame_size: u32,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            port: 9848,
            tls_enabled: false,
            tcp_keepalive_secs: 60,
            tcp_nodelay: true,
            http2_keepalive_interval_secs: 30,
            http2_keepalive_timeout_secs: 10,
            concurrency_limit: 1024,
            max_concurrent_streams: 1024,
            initial_connection_window_size: 1024 * 1024 * 2,
            initial_stream_window_size: 1024 * 1024,
            max_frame_size: 1024 * 1024 * 4,
        }
    }
}

/// gRPC server state tracker
pub struct GrpcServerState {
    health: AtomicU8,
}

impl GrpcServerState {
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

impl Default for GrpcServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// gRPC server lifecycle implementation
pub struct GrpcServerLifecycle {
    kind: GrpcServerKind,
    config: GrpcServerConfig,
    state: Arc<GrpcServerState>,
}

impl GrpcServerLifecycle {
    /// Create a new SDK gRPC server lifecycle
    pub fn new_sdk(config: GrpcServerConfig) -> Self {
        Self {
            kind: GrpcServerKind::Sdk,
            config,
            state: Arc::new(GrpcServerState::new()),
        }
    }
    
    /// Create a new cluster gRPC server lifecycle
    pub fn new_cluster(config: GrpcServerConfig) -> Self {
        Self {
            kind: GrpcServerKind::Cluster,
            config,
            state: Arc::new(GrpcServerState::new()),
        }
    }
    
    /// Create a new Raft gRPC server lifecycle
    pub fn new_raft(config: GrpcServerConfig) -> Self {
        Self {
            kind: GrpcServerKind::Raft,
            config,
            state: Arc::new(GrpcServerState::new()),
        }
    }
    
    /// Get the server port
    pub fn port(&self) -> u16 {
        self.config.port
    }
}

#[async_trait]
impl ServerLifecycle for GrpcServerLifecycle {
    fn kind(&self) -> ServerKind {
        self.kind.into()
    }

    fn name(&self) -> Cow<'static, str> {
        match self.kind {
            GrpcServerKind::Sdk => "SDK gRPC Server".into(),
            GrpcServerKind::Cluster => "Cluster gRPC Server".into(),
            GrpcServerKind::Raft => "Raft gRPC Server".into(),
        }
    }

    async fn start(&self, _ctx: &AppContext) -> InitResult<ServerHandle> {
        self.state.set_health(ServerHealth::Starting);

        let addr = format!("0.0.0.0:{}", self.config.port);

        tracing::info!(
            "Starting {} on {} (TLS: {})",
            self.name(),
            addr,
            self.config.tls_enabled
        );

        let (handle, mut shutdown_rx) = ServerHandle::new(self.kind.into(), self.name());

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
impl GracefulShutdownable for GrpcServerLifecycle {
    fn shutdown_order(&self) -> u8 {
        crate::lifecycle::shutdown_trait::priority::GRPC_SERVER
    }

    async fn shutdown(&self) -> InitResult<()> {
        tracing::info!("Graceful shutdown for gRPC server: {}", self.name());
        self.state.set_health(ServerHealth::Stopping);
        self.state.set_health(ServerHealth::Stopped);
        Ok(())
    }

    async fn drain(&self, timeout: std::time::Duration) -> InitResult<()> {
        tracing::info!(
            "Draining gRPC server: {} (timeout: {:?})",
            self.name(),
            timeout
        );
        self.state.set_health(ServerHealth::Draining);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_grpc_server_kind_as_str() {
        assert_eq!(GrpcServerKind::Sdk.as_str(), "GrpcSdk");
        assert_eq!(GrpcServerKind::Cluster.as_str(), "GrpcCluster");
        assert_eq!(GrpcServerKind::Raft.as_str(), "GrpcRaft");
    }
    
    #[test]
    fn test_grpc_server_kind_conversion() {
        assert_eq!(ServerKind::from(GrpcServerKind::Sdk), ServerKind::GrpcSdk);
        assert_eq!(ServerKind::from(GrpcServerKind::Cluster), ServerKind::GrpcCluster);
        assert_eq!(ServerKind::from(GrpcServerKind::Raft), ServerKind::GrpcRaft);
    }
    
    #[test]
    fn test_grpc_server_config_default() {
        let config = GrpcServerConfig::default();
        
        assert_eq!(config.port, 9848);
        assert!(!config.tls_enabled);
        assert_eq!(config.tcp_keepalive_secs, 60);
        assert!(config.tcp_nodelay);
    }
    
    #[test]
    fn test_grpc_server_state() {
        let state = GrpcServerState::new();

        assert_eq!(state.health(), ServerHealth::Starting);

        state.set_health(ServerHealth::Running);
        assert_eq!(state.health(), ServerHealth::Running);
    }

    #[test]
    fn test_grpc_server_lifecycle_creation() {
        let config = GrpcServerConfig::default();
        
        let sdk = GrpcServerLifecycle::new_sdk(config.clone());
        assert_eq!(sdk.kind(), ServerKind::GrpcSdk);
        assert_eq!(sdk.name(), "SDK gRPC Server");
        assert_eq!(sdk.port(), 9848);
        
        let cluster = GrpcServerLifecycle::new_cluster(GrpcServerConfig {
            port: 9849,
            ..config.clone()
        });
        assert_eq!(cluster.kind(), ServerKind::GrpcCluster);
        assert_eq!(cluster.port(), 9849);
    }
}
