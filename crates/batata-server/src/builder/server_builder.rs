//! Server builder
//!
//! This module provides a builder for creating server lifecycles.

use std::sync::Arc;

use crate::context::AppContext;
use crate::context::DeploymentMode;
use crate::lifecycle::{GrpcServerConfig, HttpServerConfig};
use crate::initializer::traits::{ServerKind, ServerLifecycle};

/// Server builder
///
/// Creates server lifecycles based on deployment mode and configuration.
pub struct ServerBuilder {
    http_servers: Vec<Arc<dyn ServerLifecycle>>,
    grpc_servers: Vec<Arc<dyn ServerLifecycle>>,
    xds_server: Option<Arc<dyn ServerLifecycle>>,
}

impl ServerBuilder {
    /// Create a new empty server builder
    pub fn new() -> Self {
        Self {
            http_servers: Vec::new(),
            grpc_servers: Vec::new(),
            xds_server: None,
        }
    }
    
    /// Add an HTTP server lifecycle
    pub fn with_http_server(mut self, server: Arc<dyn ServerLifecycle>) -> Self {
        self.http_servers.push(server);
        self
    }
    
    /// Add a gRPC server lifecycle
    pub fn with_grpc_server(mut self, server: Arc<dyn ServerLifecycle>) -> Self {
        self.grpc_servers.push(server);
        self
    }
    
    /// Set the xDS server lifecycle
    pub fn with_xds_server(mut self, server: Arc<dyn ServerLifecycle>) -> Self {
        self.xds_server = Some(server);
        self
    }
    
    /// Build servers based on deployment mode and context
    ///
    /// This automatically creates the appropriate servers for the
    /// given deployment mode.
    pub fn build_for_deployment(
        self,
        ctx: &AppContext,
    ) -> Vec<Arc<dyn ServerLifecycle>> {
        let mut servers = Vec::new();
        let deployment_mode = ctx.deployment_mode;
        
        match deployment_mode {
            DeploymentMode::Merged => {
                // Console + Main HTTP servers
                servers.extend(self.http_servers);
            }
            DeploymentMode::Server => {
                // Main HTTP server only
                if let Some(main) = self.http_servers.iter().find(|s| matches!(s.kind(), ServerKind::HttpMain)) {
                    servers.push(main.clone());
                }
            }
            DeploymentMode::Console => {
                // Console HTTP server only
                if let Some(console) = self.http_servers.iter().find(|s| matches!(s.kind(), ServerKind::HttpConsole)) {
                    servers.push(console.clone());
                }
            }
            DeploymentMode::ServerWithMcp => {
                // Main HTTP server + MCP
                servers.extend(self.http_servers);
            }
        }
        
        // Always add gRPC servers
        servers.extend(self.grpc_servers);
        
        // Add xDS server if present and enabled
        if let Some(ref xds) = self.xds_server {
            servers.push(xds.clone());
        }
        
        servers
    }
    
    /// Get all HTTP servers
    pub fn http_servers(&self) -> &[Arc<dyn ServerLifecycle>] {
        &self.http_servers
    }
    
    /// Get all gRPC servers
    pub fn grpc_servers(&self) -> &[Arc<dyn ServerLifecycle>] {
        &self.grpc_servers
    }
    
    /// Get the xDS server
    pub fn xds_server(&self) -> Option<&Arc<dyn ServerLifecycle>> {
        self.xds_server.as_ref()
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create HTTP server configurations from app context
pub fn create_http_configs(
    ctx: &AppContext,
) -> (
    HttpServerConfig,
    HttpServerConfig,
) {
    let main_config = HttpServerConfig {
        port: ctx.config.server_main_port(),
        address: ctx.config.server_address(),
        context_path: ctx.config.server_context_path(),
        workers: ctx.config.http_workers(),
        keep_alive_secs: ctx.config.http_keep_alive_secs(),
        max_payload_size: ctx.config.max_payload_size(),
        max_json_size: ctx.config.max_json_size(),
        compression_enabled: ctx.config.http_compression_enabled(),
        access_log_enabled: ctx.config.http_access_log_enabled(),
    };
    
    let console_config = HttpServerConfig {
        port: ctx.config.console_server_port(),
        address: ctx.config.server_address(),
        context_path: ctx.config.console_server_context_path(),
        workers: ctx.config.http_workers(),
        keep_alive_secs: ctx.config.console_keep_alive_secs(),
        max_payload_size: ctx.config.max_payload_size(),
        max_json_size: ctx.config.max_json_size(),
        compression_enabled: ctx.config.http_compression_enabled(),
        access_log_enabled: ctx.config.http_access_log_enabled(),
    };
    
    (main_config, console_config)
}

/// Helper to create gRPC server configurations from app context
pub fn create_grpc_configs(
    ctx: &AppContext,
) -> (
    GrpcServerConfig,
    GrpcServerConfig,
    GrpcServerConfig,
) {
    let sdk_config = GrpcServerConfig {
        port: ctx.config.sdk_server_port(),
        tls_enabled: ctx.config.grpc_tls_config().sdk_enabled,
        tcp_keepalive_secs: ctx.config.grpc_tcp_keepalive_secs(),
        tcp_nodelay: ctx.config.grpc_tcp_nodelay(),
        http2_keepalive_interval_secs: ctx.config.grpc_http2_keepalive_interval_secs(),
        http2_keepalive_timeout_secs: ctx.config.grpc_http2_keepalive_timeout_secs(),
        concurrency_limit: ctx.config.grpc_concurrency_limit(),
        max_concurrent_streams: ctx.config.grpc_max_concurrent_streams(),
        initial_connection_window_size: ctx.config.grpc_initial_connection_window_size(),
        initial_stream_window_size: ctx.config.grpc_initial_stream_window_size(),
        max_frame_size: ctx.config.grpc_max_frame_size(),
    };
    
    let cluster_config = GrpcServerConfig {
        port: ctx.config.cluster_server_port(),
        tls_enabled: ctx.config.grpc_tls_config().cluster_enabled,
        ..sdk_config.clone()
    };
    
    let raft_config = GrpcServerConfig {
        port: ctx.config.raft_port(),
        ..sdk_config.clone()
    };
    
    (sdk_config, cluster_config, raft_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{GrpcServerLifecycle, HttpServerLifecycle, XdsServerConfig, XdsServerLifecycle};
    
    #[test]
    fn test_server_builder_empty() {
        let builder = ServerBuilder::new();
        assert!(builder.http_servers().is_empty());
        assert!(builder.grpc_servers().is_empty());
        assert!(builder.xds_server().is_none());
    }
    
    #[test]
    fn test_server_builder_with_servers() {
        let http = Arc::new(HttpServerLifecycle::new_main(HttpServerConfig {
            port: 8848,
            address: "0.0.0.0".to_string(),
            context_path: "/nacos".to_string(),
            workers: 4,
            keep_alive_secs: 60,
            max_payload_size: 10 * 1024 * 1024,
            max_json_size: 1 * 1024 * 1024,
            compression_enabled: true,
            access_log_enabled: true,
        }));
        
        let grpc = Arc::new(GrpcServerLifecycle::new_sdk(GrpcServerConfig::default()));
        
        let xds = Arc::new(XdsServerLifecycle::new(XdsServerConfig::default()));
        
        let builder = ServerBuilder::new()
            .with_http_server(http.clone())
            .with_grpc_server(grpc.clone())
            .with_xds_server(xds.clone());
        
        assert_eq!(builder.http_servers().len(), 1);
        assert_eq!(builder.grpc_servers().len(), 1);
        assert!(builder.xds_server().is_some());
    }
}
