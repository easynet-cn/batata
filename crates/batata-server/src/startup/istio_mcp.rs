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
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{error, info, warn};

use batata_api::naming::NamingServiceProvider;
use batata_common::build_service_key;
use batata_config::ConfigChangeNotifier;
use batata_mesh::conversion::NacosService;
use batata_mesh::sync::SyncBridgeConfig;
use batata_mesh::sync::{NacosServiceData, ServiceChangeEvent};
use batata_mesh::{
    BatataMcpService, IstioConfigSourceEntry, McpServer, McpServerConfig, build_istio_mcp_snapshot,
    is_istio_config_tag, start_istio_mcp_grpc_server_on_listener,
};

use super::istio_mcp_config::IstioMcpConfigProvider;
use super::xds_nacos_source::spawn_nacos_source;

/// Owned handles for the Istio MCP server and its naming/config sources.
pub struct IstioMcpServerHandle {
    service: Arc<BatataMcpService>,
    source_shutdown_tx: Option<oneshot::Sender<()>>,
    source_handle: Option<JoinHandle<()>>,
    config_shutdown_tx: Option<oneshot::Sender<()>>,
    config_handle: Option<JoinHandle<()>>,
    bridge_shutdown_tx: Option<oneshot::Sender<()>>,
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

    /// Gracefully stop the naming/config sources, bridge and gRPC server.
    pub async fn shutdown(mut self) {
        info!("Shutting down Istio MCP service");

        // Stop producers first so they cannot refill the bridge queue.
        if let Some(tx) = self.source_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.source_handle.take() {
            let _ = handle.await;
        }
        if let Some(tx) = self.config_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.config_handle.take() {
            let _ = handle.await;
        }
        if let Some(tx) = self.bridge_shutdown_tx.take() {
            let _ = tx.send(());
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
    service_entry_port: u16,
    domain_suffix: String,
    naming_service: Arc<dyn NamingServiceProvider>,
    config_provider: Arc<dyn IstioMcpConfigProvider>,
    config_notifier: Arc<ConfigChangeNotifier>,
) -> Result<Option<IstioMcpServerHandle>, Box<dyn std::error::Error + Send + Sync>> {
    if !enabled {
        info!("Istio MCP server disabled");
        return Ok(None);
    }

    info!(port = port, "Starting Istio MCP service");

    let mcp_server = Arc::new(McpServer::new(McpServerConfig {
        service_entry_port,
        domain_suffix,
        ..Default::default()
    }));
    let service = Arc::new(BatataMcpService::new(mcp_server.clone()));

    let poll_interval = Duration::from_millis(5000);
    let (event_tx, mut event_rx) = mpsc::channel(256);
    let (source_shutdown_tx, source_handle) =
        spawn_nacos_source(naming_service, event_tx, poll_interval);

    let (config_tx, mut config_rx) = mpsc::channel(64);
    let (config_shutdown_tx, mut config_shutdown_rx) = oneshot::channel();
    let config_source = config_provider.clone();
    let config_handle = tokio::spawn(async move {
        loop {
            let entries = config_source.load_virtual_service_configs().await;
            if config_tx.send(entries).await.is_err() {
                break;
            }
            tokio::select! {
                _ = &mut config_shutdown_rx => break,
                _ = sleep(Duration::from_secs(5)) => {}
            }
        }
        info!("Istio MCP config source stopped");
    });

    let mut publish_rx = config_notifier.subscribe_publishes();
    let bridge_provider = config_provider.clone();
    let bridge_notifier = config_notifier.clone();
    let bridge_server = mcp_server.clone();
    let bridge_service = service.clone();
    let xds_sync_config = SyncBridgeConfig::default();
    let (bridge_shutdown_tx, mut bridge_shutdown_rx) = oneshot::channel();
    let bridge_handle = tokio::spawn(async move {
        let mut cache: HashMap<String, NacosServiceData> = HashMap::new();
        let mut configs: Vec<IstioConfigSourceEntry> = Vec::new();
        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    match event {
                        ServiceChangeEvent::Updated(data) => {
                            let key = build_service_key(
                                &data.namespace_id,
                                &data.group_name,
                                &data.service_name,
                            );
                            cache.insert(key, data);
                        }
                        ServiceChangeEvent::Removed {
                            namespace_id,
                            group_name,
                            service_name,
                        } => {
                            let key =
                                build_service_key(&namespace_id, &group_name, &service_name);
                            cache.remove(&key);
                        }
                    }
                }
                Some(entries) = config_rx.recv() => {
                    configs = entries;
                }
                result = publish_rx.recv() => {
                    match result {
                        Ok(event) => {
                            // Upstream emits IstioConfigChangeEvent only for tagged
                            // publishes. Untagged publishes and tag removal must not
                            // rewrite the derived RDS snapshot.
                            if !is_istio_config_tag(&event.config_tags) {
                                continue;
                            }
                            configs = bridge_provider.load_virtual_service_configs().await;
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!(
                                skipped,
                                "Istio MCP config publish notifications lagged; reloading configs"
                            );
                            configs = bridge_provider.load_virtual_service_configs().await;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!("Istio MCP config publish channel closed; re-subscribing");
                            publish_rx = bridge_notifier.subscribe_publishes();
                            continue;
                        }
                    }
                }
                _ = &mut bridge_shutdown_rx => break,
                else => break,
            }

            let mut services: Vec<NacosService> = cache.values().map(NacosService::from).collect();
            services.sort_by_key(NacosService::xds_cluster_name);
            bridge_server.sync_services(&services).await;
            let xds_snapshot = build_istio_mcp_snapshot(&services, &configs, &xds_sync_config);
            bridge_service.publish_snapshot_with_xds(xds_snapshot);
        }
        info!("Istio MCP naming/config bridge stopped");
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
        config_shutdown_tx: Some(config_shutdown_tx),
        config_handle: Some(config_handle),
        bridge_shutdown_tx: Some(bridge_shutdown_tx),
        bridge_handle: Some(bridge_handle),
        grpc_shutdown_tx: Some(grpc_shutdown_tx),
        grpc_server_handle: Some(grpc_server_handle),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use batata_api::naming::Instance;
    use batata_naming::NamingService;
    use tokio::time::{sleep, timeout};

    use crate::startup::IstioMcpConfigProvider;

    struct EmptyConfigProvider;

    #[async_trait]
    impl IstioMcpConfigProvider for EmptyConfigProvider {
        async fn load_virtual_service_configs(&self) -> Vec<IstioConfigSourceEntry> {
            Vec::new()
        }
    }

    struct StaticConfigProvider(Vec<IstioConfigSourceEntry>);

    #[async_trait]
    impl IstioMcpConfigProvider for StaticConfigProvider {
        async fn load_virtual_service_configs(&self) -> Vec<IstioConfigSourceEntry> {
            self.0.clone()
        }
    }

    struct SharedConfigProvider(Arc<std::sync::Mutex<Vec<IstioConfigSourceEntry>>>);

    #[async_trait]
    impl IstioMcpConfigProvider for SharedConfigProvider {
        async fn load_virtual_service_configs(&self) -> Vec<IstioConfigSourceEntry> {
            self.0.lock().expect("config lock").clone()
        }
    }

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
        let handle = start_istio_mcp_service(
            false,
            0,
            0,
            "nacos".to_string(),
            naming,
            Arc::new(EmptyConfigProvider),
            Arc::new(ConfigChangeNotifier::new()),
        )
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
        let handle = start_istio_mcp_service(
            true,
            0,
            18_080,
            "nacos".to_string(),
            naming,
            Arc::new(EmptyConfigProvider),
            Arc::new(ConfigChangeNotifier::new()),
        )
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
        assert_eq!(entries[0].spec.ports[0].number, 18_080);

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn configured_domain_suffix_reaches_service_entries() {
        let naming = Arc::new(NamingService::new());
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            instance("127.0.0.1", 8080),
        );

        let handle = start_istio_mcp_service(
            true,
            0,
            18_080,
            ".example.test".to_string(),
            naming,
            Arc::new(EmptyConfigProvider),
            Arc::new(ConfigChangeNotifier::new()),
        )
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
        assert_eq!(entries[0].spec.hosts, vec!["web.example.test"]);

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

        let handle = start_istio_mcp_service(
            true,
            0,
            0,
            "nacos".to_string(),
            naming.clone(),
            Arc::new(EmptyConfigProvider),
            Arc::new(ConfigChangeNotifier::new()),
        )
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

        let snapshot = timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.entries.len() == 2
                {
                    return snapshot;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("second naming update should publish a snapshot");

        assert_eq!(snapshot.xds.clusters.len(), 2);
        assert_eq!(snapshot.xds.endpoints.len(), 2);
        assert!(!snapshot.xds.listeners.is_empty());
        assert_eq!(snapshot.xds.routes.len(), 1);
        assert!(snapshot.xds.clusters.contains_key("web"));
        assert!(snapshot.xds.clusters.contains_key("api"));

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn config_virtual_service_routes_reach_mcp_snapshots() {
        let naming = Arc::new(NamingService::new());
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            instance("127.0.0.1", 8080),
        );
        let config = IstioConfigSourceEntry {
            data_id: "web.yaml".to_string(),
            group: "DEFAULT".to_string(),
            namespace: "public".to_string(),
            config_tags: "virtual-service".to_string(),
            content: concat!(
                "apiVersion: networking.istio.io/v1alpha3\n",
                "kind: VirtualService\n",
                "metadata:\n  name: web\n",
                "spec:\n  hosts:\n  - web\n",
                "  http:\n  - name: config-route\n",
                "    match:\n      - uri:\n          prefix: /api\n",
                "    route:\n      - destination:\n          host: web\n",
            )
            .to_string(),
        };

        let handle = start_istio_mcp_service(
            true,
            0,
            0,
            "nacos".to_string(),
            naming.clone(),
            Arc::new(StaticConfigProvider(vec![config])),
            Arc::new(ConfigChangeNotifier::new()),
        )
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

        let snapshot = timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.xds.routes.contains_key("web_route_config")
                    && snapshot.xds.clusters.len() == 2
                {
                    return snapshot;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("config-derived route should reach MCP snapshot");

        assert_eq!(snapshot.xds.clusters.len(), 2);
        let default = &snapshot.xds.routes["default-routes"];
        assert!(
            default
                .virtual_hosts
                .iter()
                .any(|host| host.name == "web" && host.domains == vec!["web.nacos"])
        );
        let route = &snapshot.xds.routes["web_route_config"];
        assert_eq!(route.virtual_hosts.len(), 1);
        assert_eq!(route.virtual_hosts[0].routes[0].name, "config-route");

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn config_publish_events_push_rds_only_for_istio_tags() {
        let naming = Arc::new(NamingService::new());
        let configs = Arc::new(std::sync::Mutex::new(Vec::new()));
        let notifier = Arc::new(ConfigChangeNotifier::new());

        let handle = start_istio_mcp_service(
            true,
            0,
            0,
            "nacos".to_string(),
            naming.clone(),
            Arc::new(SharedConfigProvider(configs.clone())),
            notifier.clone(),
        )
        .await
        .expect("startup")
        .expect("enabled");
        let mut updates = handle.service().subscribe();

        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            instance("127.0.0.1", 8080),
        );
        timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.xds.clusters.contains_key("web")
                {
                    return;
                }
            }
        })
        .await
        .expect("naming source should publish the initial snapshot");

        // Delete notifications are deliberately ignored by the Istio RDS bridge.
        notifier.notify_change("public", "DEFAULT", "vs-web.yaml");
        let deleted = timeout(Duration::from_millis(500), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.xds.routes.contains_key("vs-web_route_config")
                {
                    return snapshot;
                }
            }
        })
        .await;
        assert!(deleted.is_err(), "delete notification must not push RDS");

        // Untagged publishes are ordinary config changes upstream and must not
        // rewrite the Istio RDS projection.
        notifier.notify_publish("public", "DEFAULT", "vs-web.yaml", "canary,gray");
        let untagged = timeout(Duration::from_millis(500), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.xds.routes.contains_key("vs-web_route_config")
                {
                    return snapshot;
                }
            }
        })
        .await;
        assert!(untagged.is_err(), "untagged publish must not push RDS");

        *configs.lock().expect("config lock") = vec![IstioConfigSourceEntry {
            data_id: "vs-web.yaml".to_string(),
            group: "DEFAULT".to_string(),
            namespace: "public".to_string(),
            config_tags: "virtual-service".to_string(),
            content: concat!(
                "apiVersion: networking.istio.io/v1alpha3\n",
                "kind: VirtualService\n",
                "metadata:\n  name: vs-web\n",
                "spec:\n  hosts:\n  - vs-web\n",
                "  http:\n  - name: vs-web-api\n",
                "    match:\n      - uri:\n          prefix: /api\n",
                "    route:\n      - destination:\n          host: vs-web\n",
            )
            .to_string(),
        }];
        notifier.notify_publish("public", "DEFAULT", "vs-web.yaml", "virtual-service");

        let snapshot = timeout(Duration::from_millis(1_000), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && snapshot.xds.routes.contains_key("vs-web_route_config")
                {
                    return snapshot;
                }
            }
        })
        .await
        .expect("publish event must push RDS before the 5s poll");

        let route = &snapshot.xds.routes["vs-web_route_config"];
        assert_eq!(route.virtual_hosts[0].routes[0].name, "vs-web-api");

        // Removing the Istio tag is also an untagged publish upstream; the
        // existing route stays until a later tagged publish replaces it.
        *configs.lock().expect("config lock") = Vec::new();
        notifier.notify_publish("public", "DEFAULT", "vs-web.yaml", "canary");
        let removed = timeout(Duration::from_millis(500), async {
            loop {
                if let Ok(snapshot) = updates.recv().await
                    && !snapshot.xds.routes.contains_key("vs-web_route_config")
                {
                    return snapshot;
                }
            }
        })
        .await;
        assert!(removed.is_err(), "untagged tag removal must not push RDS");

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn port_conflict_fails_startup_synchronously() {
        let naming = Arc::new(NamingService::new());
        let listener = TcpListener::bind("0.0.0.0:0")
            .await
            .expect("probe listener");
        let port = listener.local_addr().expect("addr").port();

        let error = match start_istio_mcp_service(
            true,
            port,
            0,
            "nacos".to_string(),
            naming,
            Arc::new(EmptyConfigProvider),
            Arc::new(ConfigChangeNotifier::new()),
        )
        .await
        {
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
