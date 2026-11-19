//! xDS Server Startup Module
//!
//! This module handles the initialization and startup of the xDS service
//! for service mesh integration (Envoy/Istio).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{error, info};

use batata_mesh::{
    server::{XdsServer, XdsServerConfig},
    start_xds_grpc_server,
    sync::{NacosSyncBridge, SyncBridgeConfig},
};

use batata_api::naming::NamingServiceProvider;

use crate::model::config::XdsConfig;

use super::xds_nacos_source::spawn_nacos_source;

/// xDS service state and handles
pub struct XdsServerHandle {
    /// The xDS server instance
    pub xds_server: Arc<XdsServer>,
    /// The Batata-xDS sync bridge (uses NacosSyncBridge type for Nacos SDK compatibility)
    sync_bridge: NacosSyncBridge,
    /// Shutdown signal sender for the gRPC server
    grpc_shutdown_tx: Option<oneshot::Sender<()>>,
    /// Handle for the gRPC server task
    #[allow(dead_code)]
    grpc_server_handle: Option<JoinHandle<()>>,
    /// Shutdown signal sender for the Nacos naming source
    source_shutdown_tx: Option<oneshot::Sender<()>>,
    /// Handle for the Nacos naming source task
    source_handle: Option<JoinHandle<()>>,
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

        // Stop the naming source first so it cannot refill the bridge queue.
        if let Some(tx) = self.source_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.source_handle.take() {
            let _ = handle.await;
        }

        // Signal the gRPC server to shutdown
        if let Some(tx) = self.grpc_shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Stop the sync bridge
        self.sync_bridge.stop().await;

        info!("xDS service shutdown complete");
    }

    /// Get the number of services currently synced
    pub async fn service_count(&self) -> usize {
        self.sync_bridge.service_count().await
    }
}

/// Starts the xDS service for service mesh integration
///
/// This initializes the xDS server, sync bridge, and starts the xDS gRPC server
/// on the configured port.
///
/// # Arguments
/// * `xds_config` - xDS server configuration
/// * `naming_service` - Naming provider that feeds services into the xDS bridge
///
/// # Returns
/// A `XdsServerHandle` containing the xDS server and sync bridge
pub async fn start_xds_service(
    xds_config: XdsConfig,
    naming_service: Arc<dyn NamingServiceProvider>,
) -> Result<XdsServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    if !xds_config.enabled {
        return Err("xDS service is not enabled".into());
    }

    let port = xds_config.port;

    info!(
        server_id = %xds_config.server_id,
        port = port,
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
        Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error + Send + Sync>
    })?;

    // Feed the bridge from real Nacos naming state.
    let (source_shutdown_tx, source_handle) = spawn_nacos_source(
        naming_service,
        sync_bridge.event_sender(),
        Duration::from_millis(xds_config.sync_interval_ms.max(1)),
    );

    // Start the xDS gRPC server
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let xds_server_clone = xds_server.clone();

    let grpc_handle = tokio::spawn(async move {
        if let Err(e) = start_xds_grpc_server(xds_server_clone, addr, shutdown_rx).await {
            error!(error = %e, "xDS gRPC server error");
        }
    });

    info!(port = port, "xDS gRPC server started");
    info!("xDS service started successfully");

    Ok(XdsServerHandle {
        xds_server,
        sync_bridge,
        grpc_shutdown_tx: Some(shutdown_tx),
        grpc_server_handle: Some(grpc_handle),
        source_shutdown_tx: Some(source_shutdown_tx),
        source_handle: Some(source_handle),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use batata_api::naming::Instance;
    use batata_naming::NamingService;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_start_xds_service_wires_naming_source() {
        let naming = Arc::new(NamingService::new());
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            Instance {
                instance_id: "web-1".to_string(),
                ip: "10.0.0.1".to_string(),
                port: 8080,
                weight: 100.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "web".to_string(),
                metadata: std::collections::HashMap::new(),
            },
        );

        let config = XdsConfig {
            enabled: true,
            port: 0,
            server_id: "test-server".to_string(),
            sync_interval_ms: 20,
            generate_listeners: true,
            generate_routes: true,
            default_listener_port: 15001,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        };
        let handle = start_xds_service(config, naming).await.unwrap();
        assert!(handle.source_handle.is_some());

        let server = handle.server();
        timeout(Duration::from_secs(3), async {
            loop {
                if let Some(snapshot) = server.snapshot_cache().get_snapshot("unused")
                    && snapshot.clusters.len() == 1
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("naming source did not reach the xDS snapshot");

        handle.shutdown().await;
    }

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
