//! Nacos to xDS Synchronization Bridge
//!
//! This module provides automatic synchronization between Nacos naming service
//! and xDS resource snapshots, enabling seamless service mesh integration.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};

use crate::conversion::{NacosInstance, NacosService};
use crate::server::XdsServer;
use crate::snapshot::ResourceSnapshot;
use crate::xds::types::{
    Cluster, ClusterLoadAssignment, FilterChain, Listener, ListenerAddress, NetworkFilter, Route,
    RouteAction, RouteConfiguration, RouteDestination, RouteMatch, VirtualHost,
};

/// Event debouncer that batches rapid events into a single processing cycle.
///
/// Follows Nacos EventProcessor pattern:
/// - Accumulates events for `debounce_after` duration
/// - Forces processing after `debounce_max` duration even if events keep arriving
pub struct Debouncer {
    /// Minimum quiet period before processing
    debounce_after: Duration,
    /// Maximum wait time before forcing processing
    debounce_max: Duration,
    /// Pending events flag
    has_pending: Arc<AtomicBool>,
    /// Time of first pending event
    first_event_time: Arc<parking_lot::RwLock<Option<Instant>>>,
    /// Time of last event
    last_event_time: Arc<parking_lot::RwLock<Option<Instant>>>,
}

impl Debouncer {
    pub fn new(debounce_after: Duration, debounce_max: Duration) -> Self {
        Self {
            debounce_after,
            debounce_max,
            has_pending: Arc::new(AtomicBool::new(false)),
            first_event_time: Arc::new(parking_lot::RwLock::new(None)),
            last_event_time: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Record an incoming event
    pub fn record_event(&self) {
        let now = Instant::now();
        if !self.has_pending.swap(true, Ordering::Relaxed) {
            *self.first_event_time.write() = Some(now);
        }
        *self.last_event_time.write() = Some(now);
    }

    /// Check if it's time to process events.
    /// Returns true if:
    /// 1. There are pending events AND
    /// 2. Either the quiet period has elapsed OR max wait exceeded
    pub fn should_process(&self) -> bool {
        if !self.has_pending.load(Ordering::Relaxed) {
            return false;
        }

        let now = Instant::now();

        // Check max wait time
        if let Some(first) = *self.first_event_time.read()
            && now.duration_since(first) >= self.debounce_max
        {
            return true;
        }

        // Check quiet period
        if let Some(last) = *self.last_event_time.read()
            && now.duration_since(last) >= self.debounce_after
        {
            return true;
        }

        false
    }

    /// Reset after processing
    pub fn reset(&self) {
        self.has_pending.store(false, Ordering::Relaxed);
        *self.first_event_time.write() = None;
        *self.last_event_time.write() = None;
    }
}

/// Configuration for the sync bridge
#[derive(Debug, Clone)]
pub struct SyncBridgeConfig {
    /// Sync interval in milliseconds
    pub sync_interval_ms: u64,
    /// Whether to generate default listeners
    pub generate_listeners: bool,
    /// Whether to generate default routes
    pub generate_routes: bool,
    /// Default listener port for generated listeners
    pub default_listener_port: u16,
    /// Whether to include unhealthy instances
    pub include_unhealthy: bool,
}

impl Default for SyncBridgeConfig {
    fn default() -> Self {
        Self {
            sync_interval_ms: 5000,
            generate_listeners: true,
            generate_routes: true,
            default_listener_port: 15001,
            include_unhealthy: false,
        }
    }
}

/// Service change event
#[derive(Debug, Clone)]
pub enum ServiceChangeEvent {
    /// Service was added or updated
    Updated(NacosServiceData),
    /// Service was removed
    Removed {
        namespace_id: String,
        group_name: String,
        service_name: String,
    },
}

/// Nacos service data for synchronization
#[derive(Debug, Clone)]
pub struct NacosServiceData {
    /// Namespace ID
    pub namespace_id: String,
    /// Group name
    pub group_name: String,
    /// Service name
    pub service_name: String,
    /// Protect threshold
    pub protect_threshold: f64,
    /// Service metadata
    pub metadata: HashMap<String, String>,
    /// Service instances
    pub instances: Vec<NacosInstanceData>,
}

/// Nacos instance data for synchronization
#[derive(Debug, Clone)]
pub struct NacosInstanceData {
    /// Instance ID
    pub instance_id: String,
    /// IP address
    pub ip: String,
    /// Port
    pub port: u16,
    /// Weight (0-100)
    pub weight: f64,
    /// Whether the instance is healthy
    pub healthy: bool,
    /// Whether the instance is enabled
    pub enabled: bool,
    /// Whether the instance is ephemeral
    pub ephemeral: bool,
    /// Cluster name
    pub cluster_name: String,
    /// Service name
    pub service_name: String,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl From<&NacosServiceData> for NacosService {
    fn from(data: &NacosServiceData) -> Self {
        NacosService {
            namespace_id: data.namespace_id.clone(),
            group_name: data.group_name.clone(),
            service_name: data.service_name.clone(),
            protect_threshold: data.protect_threshold,
            metadata: data.metadata.clone(),
            instances: data.instances.iter().map(|i| i.into()).collect(),
        }
    }
}

impl From<&NacosInstanceData> for NacosInstance {
    fn from(data: &NacosInstanceData) -> Self {
        NacosInstance {
            instance_id: data.instance_id.clone(),
            ip: data.ip.clone(),
            port: data.port,
            weight: data.weight,
            healthy: data.healthy,
            enabled: data.enabled,
            ephemeral: data.ephemeral,
            cluster_name: data.cluster_name.clone(),
            service_name: data.service_name.clone(),
            metadata: data.metadata.clone(),
        }
    }
}

/// Nacos to xDS Synchronization Bridge
///
/// This bridge automatically converts Nacos services to xDS resources
/// and updates the xDS server's snapshot cache.
pub struct NacosSyncBridge {
    /// Configuration
    config: SyncBridgeConfig,
    /// xDS server reference
    xds_server: Arc<XdsServer>,
    /// Current services cache
    services: Arc<RwLock<HashMap<String, NacosServiceData>>>,
    /// Event sender for service changes
    event_tx: mpsc::Sender<ServiceChangeEvent>,
    /// Event receiver for service changes
    event_rx: Arc<RwLock<Option<mpsc::Receiver<ServiceChangeEvent>>>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl NacosSyncBridge {
    /// Create a new sync bridge
    pub fn new(xds_server: Arc<XdsServer>, config: SyncBridgeConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);

        Self {
            config,
            xds_server,
            services: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
            shutdown_tx: None,
        }
    }

    /// Get the event sender for pushing service changes
    pub fn event_sender(&self) -> mpsc::Sender<ServiceChangeEvent> {
        self.event_tx.clone()
    }

    /// Start the synchronization background task
    pub async fn start(&mut self) -> Result<(), anyhow::Error> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let event_rx = self.event_rx.write().await.take();
        if event_rx.is_none() {
            return Err(anyhow::anyhow!("Sync bridge already started"));
        }
        let mut event_rx = event_rx.unwrap();

        let services = self.services.clone();
        let xds_server = self.xds_server.clone();
        let config = self.config.clone();

        info!(
            sync_interval_ms = config.sync_interval_ms,
            generate_listeners = config.generate_listeners,
            generate_routes = config.generate_routes,
            "Starting Nacos-xDS sync bridge"
        );

        tokio::spawn(async move {
            let mut sync_interval =
                tokio::time::interval(Duration::from_millis(config.sync_interval_ms));
            let debouncer = Debouncer::new(
                Duration::from_millis(100), // quiet period
                Duration::from_secs(5),     // max wait
            );

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Nacos-xDS sync bridge shutting down");
                        break;
                    }
                    Some(event) = event_rx.recv() => {
                        match event {
                            ServiceChangeEvent::Updated(service_data) => {
                                let key = service_key(&service_data.namespace_id, &service_data.group_name, &service_data.service_name);
                                debug!(service_key = %key, "Service updated");
                                services.write().await.insert(key, service_data);
                            }
                            ServiceChangeEvent::Removed { namespace_id, group_name, service_name } => {
                                let key = service_key(&namespace_id, &group_name, &service_name);
                                debug!(service_key = %key, "Service removed");
                                services.write().await.remove(&key);
                            }
                        }
                        // Record event for debouncing instead of immediate sync
                        debouncer.record_event();
                    }
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        if debouncer.should_process() {
                            // Process all batched events
                            if let Err(e) = sync_to_xds(&services, &xds_server, &config).await {
                                error!(error = %e, "Failed to sync to xDS (debounced)");
                            }
                            debouncer.reset();
                        }
                    }
                    _ = sync_interval.tick() => {
                        // Periodic full sync
                        if let Err(e) = sync_to_xds(&services, &xds_server, &config).await {
                            error!(error = %e, "Failed to sync to xDS (periodic)");
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the synchronization background task
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }

    /// Manually trigger a sync
    pub async fn sync_now(&self) -> Result<(), anyhow::Error> {
        sync_to_xds(&self.services, &self.xds_server, &self.config).await
    }

    /// Update a service
    pub async fn update_service(&self, service_data: NacosServiceData) {
        let key = service_key(
            &service_data.namespace_id,
            &service_data.group_name,
            &service_data.service_name,
        );
        self.services
            .write()
            .await
            .insert(key, service_data.clone());
        let _ = self
            .event_tx
            .send(ServiceChangeEvent::Updated(service_data))
            .await;
    }

    /// Remove a service
    pub async fn remove_service(&self, namespace_id: &str, group_name: &str, service_name: &str) {
        let key = service_key(namespace_id, group_name, service_name);
        self.services.write().await.remove(&key);
        let _ = self
            .event_tx
            .send(ServiceChangeEvent::Removed {
                namespace_id: namespace_id.to_string(),
                group_name: group_name.to_string(),
                service_name: service_name.to_string(),
            })
            .await;
    }

    /// Get current service count
    pub async fn service_count(&self) -> usize {
        self.services.read().await.len()
    }
}

/// Generate a service key for the cache
fn service_key(namespace_id: &str, group_name: &str, service_name: &str) -> String {
    format!("{}@@{}@@{}", namespace_id, group_name, service_name)
}

/// Sync services to xDS snapshot
async fn sync_to_xds(
    services: &Arc<RwLock<HashMap<String, NacosServiceData>>>,
    xds_server: &Arc<XdsServer>,
    config: &SyncBridgeConfig,
) -> Result<(), anyhow::Error> {
    let services_guard = services.read().await;

    if services_guard.is_empty() {
        return Ok(());
    }

    // Convert services to xDS resources
    let nacos_services: Vec<NacosService> = services_guard.values().map(|s| s.into()).collect();

    let clusters: Vec<Cluster> = nacos_services
        .iter()
        .map(crate::conversion::service_to_cluster)
        .collect();

    let endpoints: Vec<ClusterLoadAssignment> = nacos_services
        .iter()
        .map(crate::conversion::service_to_cluster_load_assignment)
        .collect();

    // Generate listeners if configured
    let listeners: Vec<Listener> = if config.generate_listeners {
        generate_listeners(&nacos_services, config)
    } else {
        Vec::new()
    };

    // Generate routes if configured
    let routes: Vec<RouteConfiguration> = if config.generate_routes {
        generate_routes(&nacos_services)
    } else {
        Vec::new()
    };

    // Create snapshot
    let snapshot = ResourceSnapshot::with_all_resources(clusters, endpoints, listeners, routes);

    debug!(
        clusters = snapshot.clusters.len(),
        endpoints = snapshot.endpoints.len(),
        listeners = snapshot.listeners.len(),
        routes = snapshot.routes.len(),
        version = %snapshot.version,
        "Syncing xDS snapshot"
    );

    // Update xDS server
    xds_server.update_snapshot(snapshot);

    Ok(())
}

/// Generate default listeners for services
fn generate_listeners(services: &[NacosService], config: &SyncBridgeConfig) -> Vec<Listener> {
    let mut listeners = Vec::new();

    // Create a single ingress listener that routes to all services
    let filter_chain = FilterChain::new("http-ingress")
        .with_filter(NetworkFilter::http_connection_manager("default-routes"));

    let listener = Listener::new(
        "ingress-listener",
        ListenerAddress::tcp("0.0.0.0", config.default_listener_port),
    )
    .with_filter_chain(filter_chain);

    listeners.push(listener);

    // Create per-service outbound listeners
    for service in services {
        let cluster_name = service.xds_cluster_name();
        let filter_chain = FilterChain::new(format!("{}-chain", cluster_name))
            .with_filter(NetworkFilter::tcp_proxy(&cluster_name));

        let listener = Listener::new(
            format!("{}-listener", cluster_name),
            ListenerAddress::tcp("0.0.0.0", 0), // Port 0 means dynamic
        )
        .with_filter_chain(filter_chain);

        listeners.push(listener);
    }

    listeners
}

/// Generate default routes for services
fn generate_routes(services: &[NacosService]) -> Vec<RouteConfiguration> {
    let mut routes = Vec::new();

    // Create a default route configuration
    let mut virtual_hosts: Vec<VirtualHost> = Vec::new();

    for service in services {
        let cluster_name = service.xds_cluster_name();
        let service_name = service.full_name();

        // Create virtual host for this service
        let vhost = VirtualHost::new(
            format!("{}-vhost", cluster_name),
            vec![
                service_name.clone(),
                format!("{}:*", service_name),
                format!("{}.svc", service_name),
                format!("{}.svc.cluster.local", service_name),
            ],
        )
        .with_route(Route::new(
            format!("{}-route", cluster_name),
            RouteMatch::prefix("/"),
            RouteAction::Route(RouteDestination::cluster(&cluster_name)),
        ));

        virtual_hosts.push(vhost);
    }

    // Add catch-all virtual host
    let catch_all_vhost = VirtualHost::new("catch-all".to_string(), vec!["*".to_string()])
        .with_route(Route::new(
            "catch-all-route".to_string(),
            RouteMatch::prefix("/"),
            RouteAction::DirectResponse(crate::xds::types::DirectResponseAction::new(
                404,
                Some("Service not found".to_string()),
            )),
        ));

    virtual_hosts.push(catch_all_vhost);

    let route_config = RouteConfiguration {
        name: "default-routes".to_string(),
        virtual_hosts,
        internal_only_headers: Vec::new(),
    };

    routes.push(route_config);

    routes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::XdsServerConfig;

    fn create_test_service_data() -> NacosServiceData {
        NacosServiceData {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            instances: vec![NacosInstanceData {
                instance_id: "instance-1".to_string(),
                ip: "192.168.1.1".to_string(),
                port: 8080,
                weight: 100.0,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: "test-service".to_string(),
                metadata: HashMap::new(),
            }],
        }
    }

    #[test]
    fn test_service_key() {
        let key = service_key("ns1", "group1", "svc1");
        assert_eq!(key, "ns1@@group1@@svc1");
    }

    #[test]
    fn test_nacos_service_data_conversion() {
        let data = create_test_service_data();
        let nacos_service: NacosService = (&data).into();

        assert_eq!(nacos_service.namespace_id, "public");
        assert_eq!(nacos_service.service_name, "test-service");
        assert_eq!(nacos_service.instances.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_bridge_creation() {
        let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
        let config = SyncBridgeConfig::default();
        let bridge = NacosSyncBridge::new(xds_server, config);

        assert_eq!(bridge.service_count().await, 0);
    }

    #[tokio::test]
    async fn test_sync_bridge_update_service() {
        let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
        let config = SyncBridgeConfig::default();
        let bridge = NacosSyncBridge::new(xds_server, config);

        let service_data = create_test_service_data();
        bridge.update_service(service_data).await;

        assert_eq!(bridge.service_count().await, 1);
    }

    #[tokio::test]
    async fn test_sync_bridge_remove_service() {
        let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
        let config = SyncBridgeConfig::default();
        let bridge = NacosSyncBridge::new(xds_server, config);

        let service_data = create_test_service_data();
        bridge.update_service(service_data).await;
        assert_eq!(bridge.service_count().await, 1);

        bridge
            .remove_service("public", "DEFAULT_GROUP", "test-service")
            .await;
        assert_eq!(bridge.service_count().await, 0);
    }

    #[tokio::test]
    async fn test_sync_now() {
        let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
        let config = SyncBridgeConfig::default();
        let bridge = NacosSyncBridge::new(xds_server.clone(), config);

        let service_data = create_test_service_data();
        bridge.update_service(service_data).await;

        // Manually trigger sync
        bridge.sync_now().await.unwrap();

        // Verify snapshot was updated
        let stats = xds_server.stats();
        assert!(stats.has_default_snapshot);
    }

    #[test]
    fn test_generate_routes() {
        let service = NacosService {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "my-service".to_string(),
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            instances: Vec::new(),
        };

        let routes = generate_routes(&[service]);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].name, "default-routes");
        // 1 service vhost + 1 catch-all
        assert_eq!(routes[0].virtual_hosts.len(), 2);
    }

    #[test]
    fn test_generate_listeners() {
        let service = NacosService {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "my-service".to_string(),
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            instances: Vec::new(),
        };

        let config = SyncBridgeConfig::default();
        let listeners = generate_listeners(&[service], &config);
        // 1 ingress + 1 per-service
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].name, "ingress-listener");
    }

    #[test]
    fn test_debouncer_no_pending() {
        let debouncer = Debouncer::new(Duration::from_millis(100), Duration::from_secs(5));
        assert!(!debouncer.should_process());
    }

    #[test]
    fn test_debouncer_record_and_check_before_quiet_period() {
        let debouncer = Debouncer::new(Duration::from_millis(500), Duration::from_secs(5));
        debouncer.record_event();

        // Immediately after recording, quiet period has not elapsed
        assert!(!debouncer.should_process());
    }

    #[test]
    fn test_debouncer_quiet_period_elapsed() {
        let debouncer = Debouncer::new(Duration::from_millis(10), Duration::from_secs(5));
        debouncer.record_event();

        // Wait for the quiet period to elapse
        std::thread::sleep(Duration::from_millis(20));

        assert!(debouncer.should_process());
    }

    #[test]
    fn test_debouncer_max_wait_exceeded() {
        let debouncer = Debouncer::new(
            Duration::from_secs(60),   // very long quiet period
            Duration::from_millis(10), // very short max wait
        );
        debouncer.record_event();

        // Wait for max wait to exceed
        std::thread::sleep(Duration::from_millis(20));

        // Even though quiet period hasn't elapsed, max wait forces processing
        assert!(debouncer.should_process());
    }

    #[test]
    fn test_debouncer_reset() {
        let debouncer = Debouncer::new(Duration::from_millis(10), Duration::from_secs(5));
        debouncer.record_event();

        std::thread::sleep(Duration::from_millis(20));
        assert!(debouncer.should_process());

        debouncer.reset();
        assert!(!debouncer.should_process());
    }

    #[test]
    fn test_debouncer_continuous_events_reset_quiet_period() {
        let debouncer = Debouncer::new(Duration::from_millis(50), Duration::from_secs(60));

        debouncer.record_event();
        std::thread::sleep(Duration::from_millis(20));
        // Record another event, resetting the last_event_time
        debouncer.record_event();
        std::thread::sleep(Duration::from_millis(20));

        // 20ms since last event, quiet period is 50ms, should not process
        assert!(!debouncer.should_process());

        // Wait for quiet period from last event
        std::thread::sleep(Duration::from_millis(40));
        assert!(debouncer.should_process());
    }
}
