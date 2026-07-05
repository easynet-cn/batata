//! xDS server lifecycle implementation
//!
//! This module provides ServerLifecycle implementation for xDS service mesh support.

use std::borrow::Cow;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::context::AppContext;
use crate::initializer::traits::{
    GracefulShutdownable, InitResult, ServerHandle, ServerHealth, ServerKind, ServerLifecycle,
};

/// xDS server configuration
#[derive(Clone)]
pub struct XdsServerConfig {
    pub enabled: bool,
    pub port: u16,
    pub server_id: String,
    pub sync_interval_ms: u64,
    pub generate_listeners: bool,
    pub generate_routes: bool,
    pub default_listener_port: u16,
}

impl Default for XdsServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 15010,
            server_id: "batata-xds-server".to_string(),
            sync_interval_ms: 5000,
            generate_listeners: true,
            generate_routes: true,
            default_listener_port: 15001,
        }
    }
}

/// xDS server state tracker
pub struct XdsServerState {
    health: AtomicU8,
}

impl XdsServerState {
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

impl Default for XdsServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// xDS server lifecycle implementation
pub struct XdsServerLifecycle {
    config: XdsServerConfig,
    state: Arc<XdsServerState>,
}

impl XdsServerLifecycle {
    /// Create a new xDS server lifecycle
    pub fn new(config: XdsServerConfig) -> Self {
        Self {
            config,
            state: Arc::new(XdsServerState::new()),
        }
    }
    
    /// Check if xDS is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[async_trait]
impl ServerLifecycle for XdsServerLifecycle {
    fn kind(&self) -> ServerKind {
        ServerKind::Xds
    }

    fn name(&self) -> Cow<'static, str> {
        "xDS Server".into()
    }

    async fn start(&self, _ctx: &AppContext) -> InitResult<ServerHandle> {
        if !self.config.enabled {
            tracing::info!("xDS server is disabled, skipping start");
            let (handle, _) = ServerHandle::new(ServerKind::Xds, "xDS Server (disabled)");
            return Ok(handle);
        }

        self.state.set_health(ServerHealth::Starting);

        tracing::info!(
            "Starting xDS server on port {} (server_id: {})",
            self.config.port,
            self.config.server_id
        );

        let (handle, mut shutdown_rx) = ServerHandle::new(ServerKind::Xds, "xDS Server");

        self.state.set_health(ServerHealth::Running);

        let state = self.state.clone();
        tokio::spawn(async move {
            let _ = shutdown_rx.changed().await;
            state.set_health(ServerHealth::Stopping);
        });

        Ok(handle)
    }

    async fn stop(&self, handle: &ServerHandle) -> InitResult<()> {
        tracing::info!("Stopping xDS server (handle: {})", handle.name);

        handle.shutdown();
        self.state.set_health(ServerHealth::Stopped);

        Ok(())
    }

    fn health(&self) -> ServerHealth {
        self.state.health()
    }
}

#[async_trait]
impl GracefulShutdownable for XdsServerLifecycle {
    fn shutdown_order(&self) -> u8 {
        // xDS should shut down after gRPC (higher number = later)
        crate::lifecycle::shutdown_trait::priority::GRPC_SERVER + 5
    }

    async fn shutdown(&self) -> InitResult<()> {
        tracing::info!("Graceful shutdown for xDS server");
        self.state.set_health(ServerHealth::Stopping);
        self.state.set_health(ServerHealth::Stopped);
        Ok(())
    }

    async fn drain(&self, timeout: std::time::Duration) -> InitResult<()> {
        tracing::info!("Draining xDS server (timeout: {:?})", timeout);
        self.state.set_health(ServerHealth::Draining);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_xds_server_config_default() {
        let config = XdsServerConfig::default();
        
        assert!(!config.enabled);
        assert_eq!(config.port, 15010);
        assert_eq!(config.server_id, "batata-xds-server");
        assert_eq!(config.sync_interval_ms, 5000);
    }
    
    #[test]
    fn test_xds_server_state() {
        let state = XdsServerState::new();

        assert_eq!(state.health(), ServerHealth::Starting);

        state.set_health(ServerHealth::Running);
        assert_eq!(state.health(), ServerHealth::Running);
    }

    #[test]
    fn test_xds_server_lifecycle_disabled() {
        let config = XdsServerConfig {
            enabled: false,
            ..Default::default()
        };

        let lifecycle = XdsServerLifecycle::new(config);
        assert!(!lifecycle.is_enabled());
        assert_eq!(lifecycle.kind(), ServerKind::Xds);
        assert_eq!(lifecycle.name(), "xDS Server");
    }

    #[test]
    fn test_xds_server_lifecycle_enabled() {
        let config = XdsServerConfig {
            enabled: true,
            port: 15010,
            ..Default::default()
        };

        let lifecycle = XdsServerLifecycle::new(config);
        assert!(lifecycle.is_enabled());
    }
}
