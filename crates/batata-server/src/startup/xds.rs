//! xDS Server Startup Module
//!
//! This module handles the initialization and startup of the xDS service
//! for service mesh integration (Envoy/Istio).

use std::sync::Arc;

use tracing::{error, info};

use batata_mesh::{
    server::{XdsServer, XdsServerConfig},
    sync::{NacosSyncBridge, SyncBridgeConfig},
};

use crate::model::config::XdsConfig;
use crate::service::naming::NamingService;

/// xDS service state and handles
pub struct XdsServerHandle {
    /// The xDS server instance
    pub xds_server: Arc<XdsServer>,
    /// The Nacos-xDS sync bridge
    sync_bridge: NacosSyncBridge,
}

impl XdsServerHandle {
    /// Get the xDS server reference
    pub fn server(&self) -> Arc<XdsServer> {
        self.xds_server.clone()
    }

    /// Get sync bridge event sender for pushing service updates
    pub fn event_sender(&self) -> tokio::sync::mpsc::Sender<batata_mesh::sync::ServiceChangeEvent> {
        self.sync_bridge.event_sender()
    }

    /// Trigger a manual sync of services to xDS
    pub async fn sync_now(&self) -> Result<(), anyhow::Error> {
        self.sync_bridge.sync_now().await
    }

    /// Gracefully shutdown the xDS service
    pub async fn shutdown(mut self) {
        info!("Shutting down xDS service");
        self.sync_bridge.stop().await;
    }

    /// Get the number of services currently synced
    pub async fn service_count(&self) -> usize {
        self.sync_bridge.service_count().await
    }
}

/// Starts the xDS service for service mesh integration
///
/// This initializes the xDS server and sync bridge. The actual gRPC serving
/// is done through the batata_mesh::grpc module which can be added to existing
/// gRPC servers.
///
/// # Arguments
/// * `xds_config` - xDS server configuration
/// * `_naming_service` - Reference to the naming service (for future integration)
///
/// # Returns
/// A `XdsServerHandle` containing the xDS server and sync bridge
pub async fn start_xds_service(
    xds_config: XdsConfig,
    _naming_service: Arc<NamingService>,
) -> Result<XdsServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    if !xds_config.enabled {
        return Err("xDS service is not enabled".into());
    }

    info!(
        server_id = %xds_config.server_id,
        sync_interval_ms = xds_config.sync_interval_ms,
        generate_listeners = xds_config.generate_listeners,
        generate_routes = xds_config.generate_routes,
        "Starting xDS service"
    );

    // Create xDS server configuration
    let server_config = XdsServerConfig {
        server_id: xds_config.server_id.clone(),
        max_concurrent_streams: 1000,
        response_timeout_ms: 5000,
    };

    // Create xDS server
    let xds_server = Arc::new(XdsServer::new(server_config));

    // Create sync bridge configuration
    let sync_config = SyncBridgeConfig {
        sync_interval_ms: xds_config.sync_interval_ms,
        generate_listeners: xds_config.generate_listeners,
        generate_routes: xds_config.generate_routes,
        default_listener_port: xds_config.default_listener_port,
        include_unhealthy: false,
    };

    // Create and start sync bridge
    let mut sync_bridge = NacosSyncBridge::new(xds_server.clone(), sync_config);

    sync_bridge.start().await.map_err(|e| {
        error!(error = %e, "Failed to start xDS sync bridge");
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )) as Box<dyn std::error::Error + Send + Sync>
    })?;

    info!("xDS service started successfully");

    Ok(XdsServerHandle {
        xds_server,
        sync_bridge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xds_config_defaults() {
        let config = XdsConfig {
            enabled: true,
            port: 15010,
            server_id: "test-server".to_string(),
            sync_interval_ms: 5000,
            generate_listeners: true,
            generate_routes: true,
            default_listener_port: 15001,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        };

        assert!(config.enabled);
        assert_eq!(config.port, 15010);
        assert!(config.generate_listeners);
    }
}
