//! Consul Naming Store to xDS Snapshot Sync Bridge
//!
//! Bridges the Consul plugin's native naming store with the batata-mesh xDS
//! server. Periodically scans registered Consul services and publishes them as
//! xDS Cluster (CDS) and ClusterLoadAssignment (EDS) resources so that Envoy
//! sidecars (bootstrapped via `/v1/connect/proxy/:service_id`) can discover
//! backend services through ADS.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;
use tracing::{debug, info, warn};

use batata_mesh::server::{XdsServer, XdsServerConfig};
use batata_mesh::snapshot::ResourceSnapshot;
use batata_mesh::xds::types::{
    Cluster, ClusterLoadAssignment, Endpoint, FilterChain, HealthStatus, LbPolicy, Listener,
    ListenerAddress, Locality, NetworkFilter,
};

use crate::model::AgentServiceRegistration;
use crate::naming_store::ConsulNamingStore;
use crate::namespace::DEFAULT_NAMESPACE;

/// Sync bridge that publishes Consul services to the xDS server.
pub struct ConsulSyncBridge {
    naming_store: Arc<ConsulNamingStore>,
    xds_server: Arc<XdsServer>,
    /// Last observed naming store revision; used to skip no-op updates.
    last_revision: std::sync::atomic::AtomicU64,
}

impl ConsulSyncBridge {
    /// Create a new sync bridge.
    pub fn new(naming_store: Arc<ConsulNamingStore>, xds_server: Arc<XdsServer>) -> Self {
        Self {
            naming_store,
            xds_server,
            last_revision: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Start the background sync loop.
    ///
    /// Returns a shutdown sender; dropping or sending on it stops the loop.
    pub fn start(self: &Arc<Self>) -> oneshot::Sender<()> {
        let (tx, rx) = oneshot::channel();
        let bridge = self.clone();
        tokio::spawn(async move {
            bridge.run(rx).await;
        });
        tx
    }

    /// Main sync loop — scans the naming store on a fixed interval and pushes
    /// a fresh snapshot whenever the revision changes.
    async fn run(&self, mut shutdown: oneshot::Receiver<()>) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.tick().await; // skip the immediate first tick

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.maybe_sync();
                }
                _ = &mut shutdown => {
                    info!("Consul xDS sync bridge shutting down");
                    return;
                }
            }
        }
    }

    /// Build and publish a snapshot only when the naming store revision changed.
    ///
    /// Also publishes per-proxy (per-node) snapshots for every connected Envoy
    /// instance so each sidecar receives only its own listeners (LDS) and the
    /// clusters/endpoints it needs.
    fn maybe_sync(&self) {
        let current = self.naming_store.revision();
        let last = self.last_revision.load(std::sync::atomic::Ordering::Relaxed);
        let changed = current != last;

        // Always refresh per-node snapshots for connected proxies, even when the
        // store revision is unchanged: a proxy may connect after the last sync.
        self.sync_proxy_snapshots();

        if !changed {
            return;
        }

        let snapshot = self.build_snapshot();
        self.xds_server.update_snapshot(snapshot);
        self.last_revision
            .store(current, std::sync::atomic::Ordering::Relaxed);
        debug!(revision = current, "Published xDS snapshot from Consul naming store");
    }

    /// Build and publish a per-node snapshot for every connected Envoy proxy.
    ///
    /// Each proxy's snapshot contains:
    /// - CDS: its own local service cluster plus each declared upstream cluster.
    /// - EDS: endpoints for those clusters.
    /// - LDS: one inbound listener (proxy port -> local service) and one
    ///   outbound listener per upstream (LocalBindPort -> upstream cluster).
    fn sync_proxy_snapshots(&self) {
        let node_ids = self.xds_server.connected_nodes();
        for node_id in node_ids {
            let Some(data) = self.naming_store.get_by_service_id_any_ns(&node_id) else {
                continue;
            };
            let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) else {
                continue;
            };
            // Only connect-proxy registrations carry upstream/listener config.
            if reg.kind.as_deref() != Some("connect-proxy") {
                continue;
            }
            let snapshot = self.build_proxy_snapshot(&reg);
            self.xds_server.update_node_snapshot(&node_id, snapshot);
        }
    }

    /// Build a per-proxy xDS snapshot scoped to a single connect-proxy's local
    /// service and upstreams.
    pub fn build_proxy_snapshot(&self, proxy_reg: &AgentServiceRegistration) -> ResourceSnapshot {
        let mut snapshot = ResourceSnapshot::new();

        let Some(proxy) = proxy_reg.proxy.as_ref() else {
            return snapshot;
        };

        let destination = proxy
            .get("DestinationServiceName")
            .and_then(|v| v.as_str())
            .unwrap_or(proxy_reg.name.as_str())
            .to_string();
        let local_port = proxy
            .get("LocalServicePort")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16);

        // ---- CDS + EDS for the local service ----
        if let Some(cluster) = self.build_service_cluster_and_endpoints(&destination, &mut snapshot)
        {
            // ---- Inbound listener: proxy port -> local service cluster ----
            if let Some(proxy_port) = proxy_reg.port {
                let inbound = consul_to_inbound_listener(proxy_port, &cluster);
                snapshot.add_listener(inbound);
            }
            let _ = local_port; // reserved for explicit local-address listeners later
        }

        // ---- Upstreams ----
        let upstreams = proxy
            .get("Upstreams")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for upstream in upstreams {
            let Some(dest_name) = upstream
                .get("DestinationName")
                .and_then(|v| v.as_str())
            else {
                continue;
            };
            let Some(bind_port) = upstream
                .get("LocalBindPort")
                .and_then(|v| v.as_u64())
                .map(|p| p as u16)
            else {
                continue;
            };

            // CDS + EDS for this upstream destination.
            if self
                .build_service_cluster_and_endpoints(dest_name, &mut snapshot)
                .is_some()
            {
                let outbound = consul_to_outbound_listener(bind_port, dest_name);
                snapshot.add_listener(outbound);
            }
        }

        snapshot
    }

    /// Build a cluster (CDS) and its endpoints (EDS) for a service name, adding
    /// them to the snapshot. Returns the cluster name if the service exists.
    fn build_service_cluster_and_endpoints(
        &self,
        service_name: &str,
        snapshot: &mut ResourceSnapshot,
    ) -> Option<String> {
        let entries = self.naming_store.scan_ns(DEFAULT_NAMESPACE);
        let mut regs: Vec<AgentServiceRegistration> = Vec::new();

        for (_key, data) in entries {
            if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) {
                if reg.name == service_name {
                    regs.push(reg);
                }
            }
        }

        if regs.is_empty() {
            return None;
        }

        let first = regs.first().expect("non-empty");
        snapshot.add_cluster(consul_to_xds_cluster(service_name, first));

        let mut cla = ClusterLoadAssignment::new(service_name);
        let locality = Locality::new("", "");
        let mut eps: Vec<Endpoint> = Vec::new();
        for reg in &regs {
            let ip = reg.effective_address();
            let port = reg.effective_port() as i32;
            let healthy = self.naming_store.is_healthy(&ip, port);
            if let Some(ep) = consul_to_endpoint(&ip, port, healthy) {
                eps.push(ep);
            }
        }
        if !eps.is_empty() {
            let weight = eps.iter().map(|e| e.weight).sum::<u32>().max(1);
            cla.add_locality(locality, eps, weight);
        }
        snapshot.add_endpoints(cla);

        Some(service_name.to_string())
    }

    /// Build an xDS resource snapshot from all services in the naming store.
    pub fn build_snapshot(&self) -> ResourceSnapshot {
        let entries = self.naming_store.scan_ns(DEFAULT_NAMESPACE);

        // Group service instances by service name.
        let mut groups: HashMap<String, Vec<AgentServiceRegistration>> = HashMap::new();
        for (_key, data) in entries {
            match serde_json::from_slice::<AgentServiceRegistration>(&data) {
                Ok(reg) => {
                    // Skip connect-proxy / gateway kinds — they are sidecars,
                    // not backend services that Envoy should load-balance to.
                    if let Some(ref kind) = reg.kind
                        && matches!(
                            kind.as_str(),
                            "connect-proxy" | "mesh-gateway" | "ingress-gateway" | "terminating-gateway"
                        )
                    {
                        continue;
                    }
                    groups.entry(reg.name.clone()).or_default().push(reg);
                }
                Err(e) => {
                    warn!("Failed to decode service registration for xDS: {}", e);
                }
            }
        }

        let mut clusters: Vec<Cluster> = Vec::new();
        let mut endpoints: Vec<ClusterLoadAssignment> = Vec::new();

        for (service_name, regs) in groups {
            // Use the first registration to derive cluster-level config.
            let first = regs.first().expect("group is non-empty");
            clusters.push(consul_to_xds_cluster(service_name.as_str(), first));

            let mut cla = ClusterLoadAssignment::new(&service_name);
            let locality = Locality::new("", "");
            let mut eps: Vec<Endpoint> = Vec::new();

            for reg in &regs {
                let ip = reg.effective_address();
                let port = reg.effective_port() as i32;
                let healthy = self.naming_store.is_healthy(&ip, port);
                if let Some(ep) = consul_to_endpoint(&ip, port, healthy) {
                    eps.push(ep);
                }
            }

            if !eps.is_empty() {
                let weight = eps.iter().map(|e| e.weight).sum::<u32>().max(1);
                cla.add_locality(locality, eps, weight);
            }
            endpoints.push(cla);
        }

        ResourceSnapshot::with_resources(clusters, endpoints)
    }
}

/// Convert a Consul service registration into an xDS Cluster (CDS).
pub fn consul_to_xds_cluster(service_name: &str, reg: &AgentServiceRegistration) -> Cluster {
    let mut cluster = Cluster::new_eds(service_name);

    // Load balancing policy from service metadata, default round_robin.
    if let Some(meta) = reg.meta.as_ref()
        && let Some(lb) = meta.get("lb_policy")
    {
        cluster.lb_policy = match lb.to_lowercase().as_str() {
            "round_robin" | "roundrobin" => LbPolicy::RoundRobin,
            "least_request" | "leastrequest" | "least_conn" => LbPolicy::LeastRequest,
            "random" => LbPolicy::Random,
            "ring_hash" | "ringhash" | "consistent_hash" => LbPolicy::RingHash,
            "maglev" => LbPolicy::Maglev,
            _ => LbPolicy::RoundRobin,
        };
    }

    // Connect timeout from metadata.
    if let Some(meta) = reg.meta.as_ref()
        && let Some(timeout) = meta.get("connect_timeout_ms")
        && let Ok(ms) = timeout.parse::<u64>()
    {
        cluster.connect_timeout_ms = ms;
    }

    // Copy envoy-prefixed metadata.
    if let Some(meta) = reg.meta.as_ref() {
        for (k, v) in meta {
            if k.starts_with("envoy.") {
                cluster.metadata.insert(k.clone(), v.clone());
            }
        }
    }

    cluster
}

/// Convert a Consul service instance address into an xDS Endpoint.
///
/// Returns `None` if the address/port cannot be parsed into a socket address.
pub fn consul_to_endpoint(ip: &str, port: i32, healthy: bool) -> Option<Endpoint> {
    let addr: SocketAddr = format!("{}:{}", ip, port).parse().ok()?;
    let mut ep = Endpoint::new(addr);
    ep.health_status = if healthy {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    };
    Some(ep)
}

/// Build an inbound listener for a connect-proxy.
///
/// The listener binds to `0.0.0.0:{proxy_port}` and forwards traffic to the
/// local service cluster via the `envoy.filters.network.tcp_proxy` filter.
pub fn consul_to_inbound_listener(proxy_port: u16, local_cluster: &str) -> Listener {
    let name = format!("inbound_listener:{}", proxy_port);
    let filter_chain = FilterChain::new("inbound")
        .with_filter(NetworkFilter::tcp_proxy(local_cluster));
    Listener::new(name, ListenerAddress::tcp("0.0.0.0", proxy_port))
        .with_filter_chain(filter_chain)
}

/// Build an outbound listener for a connect-proxy upstream.
///
/// The listener binds to `127.0.0.1:{bind_port}` (so only the local application
/// can reach it) and forwards traffic to the upstream cluster.
pub fn consul_to_outbound_listener(bind_port: u16, upstream_cluster: &str) -> Listener {
    let name = format!("outbound_listener:{}", bind_port);
    let filter_chain = FilterChain::new(format!("outbound:{}", upstream_cluster))
        .with_filter(NetworkFilter::tcp_proxy(upstream_cluster));
    Listener::new(name, ListenerAddress::tcp("127.0.0.1", bind_port))
        .with_filter_chain(filter_chain)
}

/// Helper to create an xDS server with default config.
pub fn create_xds_server() -> Arc<XdsServer> {
    Arc::new(XdsServer::new(XdsServerConfig::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_plugin::PluginNamingStore;
    use crate::model::AgentServiceRegistration;
    use batata_mesh::xds::types::{DiscoveryType, NetworkFilterType};

    #[test]
    fn test_consul_to_xds_cluster_defaults() {
        let reg = AgentServiceRegistration {
            name: "web".to_string(),
            ..Default::default()
        };
        let cluster = consul_to_xds_cluster("web", &reg);
        assert_eq!(cluster.name, "web");
        assert_eq!(cluster.discovery_type, DiscoveryType::Eds);
        assert_eq!(cluster.lb_policy, LbPolicy::RoundRobin);
    }

    #[test]
    fn test_consul_to_xds_cluster_with_lb_policy() {
        let mut meta = HashMap::new();
        meta.insert("lb_policy".to_string(), "least_request".to_string());
        let reg = AgentServiceRegistration {
            name: "api".to_string(),
            meta: Some(meta),
            ..Default::default()
        };
        let cluster = consul_to_xds_cluster("api", &reg);
        assert_eq!(cluster.lb_policy, LbPolicy::LeastRequest);
    }

    #[test]
    fn test_consul_to_endpoint_healthy() {
        let ep = consul_to_endpoint("127.0.0.1", 8080, true).unwrap();
        assert_eq!(ep.address, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(ep.health_status, HealthStatus::Healthy);
    }

    #[test]
    fn test_consul_to_endpoint_unhealthy() {
        let ep = consul_to_endpoint("10.0.0.1", 9090, false).unwrap();
        assert_eq!(ep.health_status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_consul_to_endpoint_invalid_address() {
        assert!(consul_to_endpoint("not-an-ip", 8080, true).is_none());
    }

    #[test]
    fn test_build_snapshot_filters_proxies() {
        let naming_store = Arc::new(ConsulNamingStore::new());
        let xds_server = create_xds_server();

        // Register a normal service.
        let web_reg = AgentServiceRegistration {
            name: "web".to_string(),
            id: Some("web-1".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(8080),
            ..Default::default()
        };
        let key = ConsulNamingStore::build_key(DEFAULT_NAMESPACE, "web", "web-1");
        naming_store
            .register(&key, serde_json::to_vec(&web_reg).unwrap().into())
            .unwrap();

        // Register a connect-proxy that should be filtered out.
        let proxy_reg = AgentServiceRegistration {
            name: "web-sidecar-proxy".to_string(),
            id: Some("web-sidecar-proxy-1".to_string()),
            kind: Some("connect-proxy".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(20000),
            ..Default::default()
        };
        let proxy_key = ConsulNamingStore::build_key(
            DEFAULT_NAMESPACE,
            "web-sidecar-proxy",
            "web-sidecar-proxy-1",
        );
        naming_store
            .register(&proxy_key, serde_json::to_vec(&proxy_reg).unwrap().into())
            .unwrap();

        let bridge = ConsulSyncBridge::new(naming_store, xds_server);
        let snapshot = bridge.build_snapshot();

        // Only "web" should be present; the proxy must be excluded.
        assert_eq!(snapshot.clusters.len(), 1);
        assert!(snapshot.clusters.contains_key("web"));
        assert!(!snapshot.clusters.contains_key("web-sidecar-proxy"));

        let cla = snapshot.endpoints.get("web").unwrap();
        assert_eq!(cla.healthy_count(), 1);
    }

    #[test]
    fn test_consul_to_inbound_listener() {
        let listener = consul_to_inbound_listener(20000, "web");
        assert_eq!(listener.name, "inbound_listener:20000");
        assert_eq!(listener.address.address, "0.0.0.0");
        assert_eq!(listener.address.port, 20000);
        assert_eq!(listener.filter_chains.len(), 1);
        let filter = &listener.filter_chains[0].filters[0];
        match &filter.filter_type {
            NetworkFilterType::TcpProxy { cluster, .. } => {
                assert_eq!(cluster, "web");
            }
            _ => panic!("expected tcp_proxy filter"),
        }
    }

    #[test]
    fn test_consul_to_outbound_listener() {
        let listener = consul_to_outbound_listener(9090, "api");
        assert_eq!(listener.name, "outbound_listener:9090");
        assert_eq!(listener.address.address, "127.0.0.1");
        assert_eq!(listener.address.port, 9090);
        assert_eq!(listener.filter_chains.len(), 1);
        let filter = &listener.filter_chains[0].filters[0];
        match &filter.filter_type {
            NetworkFilterType::TcpProxy { cluster, .. } => {
                assert_eq!(cluster, "api");
            }
            _ => panic!("expected tcp_proxy filter"),
        }
    }

    #[test]
    fn test_build_proxy_snapshot_scoping() {
        let naming_store = Arc::new(ConsulNamingStore::new());
        let xds_server = create_xds_server();

        // Register the local service "web".
        let web_reg = AgentServiceRegistration {
            name: "web".to_string(),
            id: Some("web-1".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(8080),
            ..Default::default()
        };
        naming_store
            .register(
                &ConsulNamingStore::build_key(DEFAULT_NAMESPACE, "web", "web-1"),
                serde_json::to_vec(&web_reg).unwrap().into(),
            )
            .unwrap();

        // Register the upstream "api".
        let api_reg = AgentServiceRegistration {
            name: "api".to_string(),
            id: Some("api-1".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(9090),
            ..Default::default()
        };
        naming_store
            .register(
                &ConsulNamingStore::build_key(DEFAULT_NAMESPACE, "api", "api-1"),
                serde_json::to_vec(&api_reg).unwrap().into(),
            )
            .unwrap();

        // Register an unrelated service "db" that must NOT appear.
        let db_reg = AgentServiceRegistration {
            name: "db".to_string(),
            id: Some("db-1".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(5432),
            ..Default::default()
        };
        naming_store
            .register(
                &ConsulNamingStore::build_key(DEFAULT_NAMESPACE, "db", "db-1"),
                serde_json::to_vec(&db_reg).unwrap().into(),
            )
            .unwrap();

        // Connect-proxy for "web" with one upstream "api".
        let proxy_reg = AgentServiceRegistration {
            name: "web-sidecar-proxy".to_string(),
            id: Some("web-sidecar-proxy-1".to_string()),
            kind: Some("connect-proxy".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(20000),
            proxy: Some(serde_json::json!({
                "DestinationServiceName": "web",
                "LocalServicePort": 8080,
                "Upstreams": [
                    { "DestinationName": "api", "LocalBindPort": 9090 }
                ]
            })),
            ..Default::default()
        };

        let bridge = ConsulSyncBridge::new(naming_store, xds_server);
        let snapshot = bridge.build_proxy_snapshot(&proxy_reg);

        // CDS scoped to local + upstream only.
        assert_eq!(snapshot.clusters.len(), 2, "expected web + api clusters");
        assert!(snapshot.clusters.contains_key("web"));
        assert!(snapshot.clusters.contains_key("api"));
        assert!(!snapshot.clusters.contains_key("db"));

        // EDS for both.
        assert_eq!(snapshot.endpoints.len(), 2);
        assert!(snapshot.endpoints.contains_key("web"));
        assert!(snapshot.endpoints.contains_key("api"));

        // LDS: one inbound + one outbound.
        assert_eq!(snapshot.listeners.len(), 2);
        assert!(snapshot.listeners.contains_key("inbound_listener:20000"));
        assert!(snapshot.listeners.contains_key("outbound_listener:9090"));
    }

    #[test]
    fn test_build_proxy_snapshot_no_proxy_config() {
        let naming_store = Arc::new(ConsulNamingStore::new());
        let xds_server = create_xds_server();
        let proxy_reg = AgentServiceRegistration {
            name: "web-sidecar-proxy".to_string(),
            id: Some("web-sidecar-proxy-1".to_string()),
            kind: Some("connect-proxy".to_string()),
            port: Some(20000),
            proxy: None,
            ..Default::default()
        };
        let bridge = ConsulSyncBridge::new(naming_store, xds_server);
        let snapshot = bridge.build_proxy_snapshot(&proxy_reg);
        assert!(snapshot.clusters.is_empty());
        assert!(snapshot.listeners.is_empty());
    }
}
