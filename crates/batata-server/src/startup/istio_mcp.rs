//! Startup wiring for the Nacos-compatible Istio MCP server on port 18848.
//!
//! The MCP server reuses the same polling naming source as Path A xDS, but
//! keeps an independent service cache and event channel so a slow MCP sink
//! cannot block the xDS bridge. Binding happens here, before spawning the gRPC
//! task, so a port conflict fails startup instead of being logged in the
//! background.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, info};

use batata_api::naming::NamingServiceProvider;
use batata_common::build_service_key;
use batata_mesh::conversion::NacosService;
use batata_mesh::sync::{NacosServiceData, ServiceChangeEvent};
use batata_mesh::{
    BatataMcpService, McpServer, McpServerConfig, start_istio_mcp_grpc_server_on_listener,
};

use super::xds_nacos_source::spawn_nacos_source;

/// Owned handles for the Istio MCP server and its naming source.
pub struct IstioMcpServerHandle {
    service: Arc<BatataMcpService>,
    source_shutdown_tx: Option<oneshot::Sender<()>>,
    source_handle: Option<JoinHandle<()>>,
    bridge_handle: Option<JoinHandle<()>>,
    grpc_shutdown_tx: Option<oneshot::Sender<()>>,
    #[allow(dead_code)]
    grpc_server_handle: Option<JoinHandle<()>>,
}

impl IstioMcpServerHandle {
    /// Get the shared MCP resource source.
    pub fn service(&self) -> Arc<BatataMcpService> {
        self.service.clone()
    }

    /// Gracefully stop the naming source, bridge and gRPC server.
    pub async fn shutdown(mut self) {
        info!("Shutting down Istio MCP service");

        // Stop the producer first so it cannot refill the bridge queue.
        if let Some(tx) = self.source_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.source_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.bridge_handle.take() {
            let _ = handle.await;
        }

        if let Some(tx) = self.grpc_shutdown_tx.take() {
            let _ = tx.send(());
        }

        info!("Istio MCP service shutdown complete");
    }
}

/// Start the Istio MCP service when enabled.
///
/// Returns `Ok(None)` when disabled so the caller can keep one uniform startup
/// path. A bind failure is returned to the caller as an error.
pub async fn start_istio_mcp_service(
    enabled: bool,
    port: u16,
    naming_service: Arc<dyn NamingServiceProvider>,
) -> Result<Option<IstioMcpServerHandle>, Box<dyn std::error::Error + Send + Sync>> {
    if !enabled {
        info!("Istio MCP server disabled");
        return Ok(None);
    }

    info!(port = port, "Starting Istio MCP service");

    let mcp_server = Arc::new(McpServer::new(McpServerConfig::default()));
    let service = Arc::new(BatataMcpService::new(mcp_server.clone()));

    let poll_interval = Duration::from_millis(5000);
    let (event_tx, mut event_rx) = mpsc::channel(256);
    let (source_shutdown_tx, source_handle) =
        spawn_nacos_source(naming_service, event_tx, poll_interval);

    let bridge_server = mcp_server.clone();
    let bridge_service = service.clone();
    let bridge_handle = tokio::spawn(async move {
        let mut cache: HashMap<String, NacosServiceData> = HashMap::new();
        while let Some(event) = event_rx.recv().await {
            match event {
                ServiceChangeEvent::Updated(data) => {
                    let key =
                        build_service_key(&data.namespace_id, &data.group_name, &data.service_name);
                    cache.insert(key, data);
                }
                ServiceChangeEvent::Removed {
                    namespace_id,
                    group_name,
                    service_name,
                } => {
                    let key = build_service_key(&namespace_id, &group_name, &service_name);
                    cache.remove(&key);
                }
            }

            let services: Vec<NacosService> = cache.values().map(NacosService::from).collect();
            bridge_server.sync_services(&services).await;
            bridge_service.publish_snapshot();
        }
        info!("Istio MCP naming bridge stopped");
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.map_err(|e| {
        error!(error = %e, addr = %addr, "Failed to bind Istio MCP gRPC port");
        e
    })?;

    let (grpc_shutdown_tx, grpc_shutdown_rx) = oneshot::channel();
    let grpc_service = service.clone();
    let grpc_server_handle = tokio::spawn(async move {
        if let Err(e) = start_istio_mcp_grpc_server_on_listener(
            (*grpc_service).clone(),
            listener,
            grpc_shutdown_rx,
        )
        .await
        {
            error!(error = %e, "Istio MCP gRPC server error");
        }
    });

    info!(port = port, "Istio MCP gRPC server started");

    Ok(Some(IstioMcpServerHandle {
        service,
        source_shutdown_tx: Some(source_shutdown_tx),
        source_handle: Some(source_handle),
        bridge_handle: Some(bridge_handle),
        grpc_shutdown_tx: Some(grpc_shutdown_tx),
        grpc_server_handle: Some(grpc_server_handle),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use batata_api::naming::Instance;
    use batata_naming::NamingService;
    use tokio::time::{sleep, timeout};

    fn instance(ip: &str, port: u16) -> Instance {
        Instance {
            instance_id: format!("{ip}:{port}"),
            ip: ip.to_string(),
            port: i32::from(port),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn disabled_service_returns_none() {
        let naming = Arc::new(NamingService::new());
        let handle = start_istio_mcp_service(false, 0, naming)
            .await
            .expect("disabled startup");

        assert!(handle.is_none());
    }

    #[tokio::test]
    async fn naming_updates_reach_service_entries() {
        let naming = Arc::new(NamingService::new());
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            instance("127.0.0.1", 8080),
        );

        // Port 0 lets the OS choose a free port; the handle still owns the
        // bound listener and shuts it down with the rest of the service.
        let handle = start_istio_mcp_service(true, 0, naming)
            .await
            .expect("startup")
            .expect("enabled");

        let service = handle.service();
        timeout(Duration::from_secs(2), async {
            loop {
                if !service.server().get_service_entries().is_empty() {
                    return;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("service entry should be published");

        let entries = service.server().get_service_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].spec.hosts, vec!["webnacos"]);

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn naming_updates_publish_mcp_snapshots() {
        let naming = Arc::new(NamingService::new());
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            instance("127.0.0.1", 8080),
        );

        let handle = start_istio_mcp_service(true, 0, naming.clone())
            .await
            .expect("startup")
            .expect("enabled");
        let mut updates = handle.service().subscribe();

        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "api",
            instance("127.0.0.1", 9090),
        );

        timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.entries.len() == 2
                {
                    return;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("second naming update should publish a snapshot");

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn port_conflict_fails_startup_synchronously() {
        let naming = Arc::new(NamingService::new());
        let listener = TcpListener::bind("0.0.0.0:0")
            .await
            .expect("probe listener");
        let port = listener.local_addr().expect("addr").port();

        let error = match start_istio_mcp_service(true, port, naming).await {
            Ok(_) => panic!("bind conflict must fail startup"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("Address already in use")
                || error.to_string().contains("address in use")
        );

        drop(listener);
    }
}
