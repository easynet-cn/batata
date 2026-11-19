//! Consul Naming Store to xDS Snapshot Sync Bridge
//!
//! Bridges the Consul plugin's native naming store with the batata-mesh xDS
//! server. Periodically scans registered Consul services and publishes them as
//! xDS Cluster (CDS) and ClusterLoadAssignment (EDS) resources so that Envoy
//! sidecars (bootstrapped via `/v1/connect/proxy/:service_id`) can discover
//! backend services through ADS.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use tokio::sync::oneshot;
use tracing::{debug, info, warn};

use batata_mesh::server::{XdsServer, XdsServerConfig};
use batata_mesh::snapshot::ResourceSnapshot;
use batata_mesh::xds::rbac::{
    HttpPermissionMatch, IntentionsForService, RbacIntention, RbacPermission, encode_rbac_filters,
};
use batata_mesh::xds::types::{
    Cluster, ClusterLoadAssignment, Endpoint, FilterChain, HeaderMatchType, HeaderMatcher,
    HealthStatus, LbPolicy, Listener, ListenerAddress, Locality, NetworkFilter, PathMatch, Route,
    RouteAction, RouteConfiguration, RouteDestination, RouteMatch, SdsSecret, SdsSecretKind,
    TransportSocket, TransportSocketConfig, VirtualHost, WeightedCluster,
};

use crate::connect::{
    CompiledDiscoveryChain, ConsulConnectService, DiscoveryGraphNodeType, DiscoveryRouteMatch,
};
use crate::connect_ca::{
    ConnectMaterial, ConsulConnectCAService, IntentionAction as CaIntentionAction,
    IntentionPermission,
};
use crate::model::AgentServiceRegistration;
use crate::namespace::DEFAULT_NAMESPACE;
use crate::naming_store::ConsulNamingStore;

/// Base bind port for listeners published on the default snapshot.
///
/// Service registrations often omit `port` (0) and several services may share
/// one port, while Envoy rejects two listeners bound to the same address.
/// A stable synthetic port per service keeps the default LDS deterministic and
/// answerable. Per-node connect-proxy snapshots use their own bind ports.
const DEFAULT_LISTENER_BASE_PORT: u16 = 15001;

/// Standard SDS secret names published to every connect-proxy. The names are
/// fixed (not per-service) so the same `envoy.transport_sockets.tls` config can
/// be reused by all sidecars; Envoy resolves them through the SDS subscription
/// backed by this snapshot.
const SDS_CONNECT_LEAF: &str = "connect-leaf";
const SDS_CONNECT_ROOT: &str = "connect-root";

/// Name of the Envoy TLS transport socket extension. Used for both the
/// downstream (SDS) and upstream (inline) socket configs.
const TLS_TRANSPORT_SOCKET_NAME: &str = "envoy.transport_sockets.tls";

/// Convert batata's Connect intentions into the dependency-free RBAC input
/// consumed by `batata-mesh`. Only the fields batata actually stores are
/// mapped; unsupported upstream fields do not exist here.
fn intentions_to_rbac_input(
    destination: &str,
    intentions: Vec<crate::connect_ca::Intention>,
    trust_domain: &str,
    datacenter: &str,
) -> IntentionsForService {
    let converted = intentions
        .into_iter()
        .map(|i| {
            let action = match i.action {
                CaIntentionAction::Allow => batata_mesh::xds::rbac::IntentionAction::Allow,
                CaIntentionAction::Deny => batata_mesh::xds::rbac::IntentionAction::Deny,
            };
            let permissions = i
                .permissions
                .into_iter()
                .map(|p: IntentionPermission| {
                    let p_action = match p.action {
                        CaIntentionAction::Allow => batata_mesh::xds::rbac::IntentionAction::Allow,
                        CaIntentionAction::Deny => batata_mesh::xds::rbac::IntentionAction::Deny,
                    };
                    let http = p.http.map(|h| HttpPermissionMatch {
                        path_exact: h.path_exact,
                        path_prefix: h.path_prefix,
                        path_regex: h.path_regex,
                        methods: h.methods,
                    });
                    RbacPermission {
                        action: p_action,
                        http,
                    }
                })
                .collect();
            RbacIntention {
                source: i.source_name,
                precedence: i.precedence,
                action,
                permissions,
            }
        })
        .collect();
    IntentionsForService {
        destination: destination.to_string(),
        trust_domain: trust_domain.to_string(),
        datacenter: datacenter.to_string(),
        intentions: converted,
    }
}

/// Sync bridge that publishes Consul services to the xDS server.
pub struct ConsulSyncBridge {
    naming_store: Arc<ConsulNamingStore>,
    xds_server: Arc<XdsServer>,
    /// Discovery-chain compiler. When present, HTTP services are published as
    /// RDS `RouteConfiguration` resources and their listeners use HCM/RDS
    /// instead of a plain TCP proxy. Kept optional so tests and deployments
    /// without a Connect service keep the previous CDS/EDS-only behaviour.
    connect_service: Option<Arc<ConsulConnectService>>,
    /// Connect CA service holding intentions. When present, L4/L7 intentions
    /// are compiled into Envoy RBAC filters on each destination listener.
    connect_ca: Option<ConsulConnectCAService>,
    /// SPIFFE trust domain used when building RBAC principals. Batata's CA
    /// currently hard-codes `consul` (connect.rs), so this defaults to that.
    spiffe_trust_domain: String,
    /// Local datacenter, used in SPIFFE identities.
    datacenter: String,
    /// Last observed naming store revision; used to skip no-op updates.
    last_revision: std::sync::atomic::AtomicU64,
    /// Last observed Connect intention revision. Intention changes do not
    /// bump the naming-store revision, so RBAC filters need their own key.
    last_intention_revision: std::sync::atomic::AtomicU64,
    /// Last observed Connect CA revision. CA root/key/config rotations do not
    /// bump the naming-store or intention revisions, but they change the SDS
    /// secrets, so snapshots must be rebuilt when this advances.
    last_ca_revision: AtomicU64,
}

impl ConsulSyncBridge {
    /// Create a new sync bridge.
    pub fn new(naming_store: Arc<ConsulNamingStore>, xds_server: Arc<XdsServer>) -> Self {
        Self {
            naming_store,
            xds_server,
            connect_service: None,
            connect_ca: None,
            spiffe_trust_domain: "consul".to_string(),
            datacenter: "dc1".to_string(),
            last_revision: std::sync::atomic::AtomicU64::new(0),
            last_intention_revision: std::sync::atomic::AtomicU64::new(0),
            last_ca_revision: AtomicU64::new(0),
        }
    }

    /// Attach the discovery-chain compiler so config-entry L7 routing reaches
    /// the data plane. Without this, only CDS/EDS are published.
    pub fn with_connect_service(mut self, connect: Arc<ConsulConnectService>) -> Self {
        self.connect_service = Some(connect);
        self
    }

    /// Attach the Connect CA service so intentions are enforced in the data
    /// plane as Envoy RBAC filters. Without this, intentions stay HTTP-only.
    pub fn with_connect_ca(
        mut self,
        ca: ConsulConnectCAService,
        trust_domain: &str,
        datacenter: &str,
    ) -> Self {
        self.connect_ca = Some(ca);
        if !trust_domain.is_empty() {
            self.spiffe_trust_domain = trust_domain.to_string();
        }
        if !datacenter.is_empty() {
            self.datacenter = datacenter.to_string();
        }
        self
    }

    /// Compile the discovery chain for `service_name` into an RDS resource.
    ///
    /// Returns `None` for TCP services (no L7 route table) and for chains that
    /// cannot be represented by the checked-in xDS proto subset.
    pub fn build_discovery_route_config(&self, service_name: &str) -> Option<RouteConfiguration> {
        let connect = self.connect_service.as_ref()?;
        let chain = connect.get_discovery_chain(service_name).chain;
        discovery_chain_to_route_config(&chain)
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
        let last = self
            .last_revision
            .load(std::sync::atomic::Ordering::Relaxed);
        let current_intention = self
            .connect_ca
            .as_ref()
            .map(|ca| ca.intentions_revision())
            .unwrap_or(0);
        let last_intention = self
            .last_intention_revision
            .load(std::sync::atomic::Ordering::Relaxed);
        let current_ca = self
            .connect_ca
            .as_ref()
            .map(|ca| ca.ca_revision())
            .unwrap_or(0);
        let last_ca = self
            .last_ca_revision
            .load(std::sync::atomic::Ordering::Relaxed);
        let changed =
            current != last || current_intention != last_intention || current_ca != last_ca;

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
        self.last_intention_revision
            .store(current_intention, std::sync::atomic::Ordering::Relaxed);
        self.last_ca_revision
            .store(current_ca, std::sync::atomic::Ordering::Relaxed);
        debug!(
            revision = current,
            ca_revision = current_ca,
            "Published xDS snapshot from Consul naming store"
        );
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
    ///
    /// When the Connect CA has live material for the destination service the
    /// snapshot also carries the standard SDS secrets and mTLS transport
    /// sockets: the public (inbound) listener requires a client certificate,
    /// and each upstream cluster presents this proxy's leaf certificate while
    /// pinning the upstream's SPIFFE URI. The local service cluster and all
    /// app-facing (outbound) listeners stay plaintext so the application-side
    /// path is unchanged.
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

        // Live CA material for the service this proxy fronts. The same leaf
        // identity is reused as the client certificate on upstream clusters.
        let material = self
            .connect_ca
            .as_ref()
            .and_then(|ca| ca.get_connect_material_sync(&destination));

        if let Some(ref material) = material {
            add_connect_sds_secrets(&mut snapshot, material);
        }
        // Only enable mTLS on the inbound listener when real CA material is
        // available — an SDS socket referencing missing secrets would stall
        // the ADS handshake for proxies without a CA.
        let downstream_socket = material.as_ref().map(|_| downstream_sds_socket());

        // ---- CDS + EDS for the local service ----
        if let Some(cluster) = self.build_service_cluster_and_endpoints(&destination, &mut snapshot)
        {
            // ---- Inbound listener: proxy port -> local service cluster ----
            // The local cluster itself is deliberately left plaintext.
            if let Some(proxy_port) = proxy_reg.port {
                let inbound = build_inbound_listener(proxy_port, &cluster, downstream_socket);
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
            let Some(dest_name) = upstream.get("DestinationName").and_then(|v| v.as_str()) else {
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
                // Inline TLS is attached ONLY to upstream clusters. Applying
                // it to the local service cluster would break the plaintext
                // app-facing path.
                if let Some(ref material) = material {
                    let socket = upstream_inline_socket(
                        material,
                        dest_name,
                        &self.datacenter,
                        &self.spiffe_trust_domain,
                    );
                    if let Some(cluster) = snapshot.clusters.get_mut(dest_name) {
                        cluster.transport_socket = Some(socket);
                    }
                }

                // HTTP services get a discovery-chain-derived route table and
                // an HCM listener that subscribes to it over RDS; TCP services
                // keep the original tcp_proxy listener. Both outbound
                // listeners remain plaintext.
                let route_config = self.build_discovery_route_config(dest_name);
                add_outbound_listener(&mut snapshot, bind_port, dest_name, route_config);
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
                            "connect-proxy"
                                | "mesh-gateway"
                                | "ingress-gateway"
                                | "terminating-gateway"
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

        for (service_name, regs) in &groups {
            // Use the first registration to derive cluster-level config.
            let first = regs.first().expect("group is non-empty");
            clusters.push(consul_to_xds_cluster(service_name, first));

            let mut cla = ClusterLoadAssignment::new(service_name);
            let locality = Locality::new("", "");
            let mut eps: Vec<Endpoint> = Vec::new();

            for reg in regs {
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

        // Publish route tables for HTTP services on the default snapshot too,
        // so RDS is answerable before any proxy-specific snapshot exists.
        let routes = groups
            .keys()
            .filter_map(|service_name| self.build_discovery_route_config(service_name))
            .collect::<Vec<_>>();

        // Envoy bootstraps with `lds_config` over ADS. An empty default LDS
        // leaves that subscription with nothing to apply, so mirror each
        // backend service as a listener: HTTP services get an HCM filter
        // referencing their RDS route table, all others a TCP proxy to their
        // CDS cluster. Per-node snapshots for connect-proxy registrations
        // still take precedence (see `sync_proxy_snapshots`).
        let mut service_names = groups.keys().collect::<Vec<_>>();
        service_names.sort();
        let route_names: HashSet<&str> = routes.iter().map(|r| r.name.as_str()).collect();
        let listeners = service_names
            .into_iter()
            .enumerate()
            .map(|(index, service_name)| {
                let bind_port = DEFAULT_LISTENER_BASE_PORT + index as u16;
                let route_name = route_config_name(service_name);
                let is_http = route_names.contains(route_name.as_str());

                // Compile Connect intentions for this destination into L4/L7
                // RBAC filter configs. Network RBAC is prepended ahead of the
                // proxy filter; HTTP RBAC is injected into HCM before the
                // router when this service has an L7 route table.
                let mut network_rbac: Option<Vec<u8>> = None;
                let mut http_rbac: Option<Vec<u8>> = None;
                if let Some(ca) = &self.connect_ca {
                    let intentions = ca.match_intentions("destination", service_name);
                    if !intentions.is_empty() {
                        let input = intentions_to_rbac_input(
                            service_name,
                            intentions,
                            &self.spiffe_trust_domain,
                            &self.datacenter,
                        );
                        if let Some(encoded) = encode_rbac_filters(&input) {
                            if encoded.has_network_policies {
                                network_rbac = Some(encoded.network_config.clone());
                            }
                            if is_http && encoded.has_http_policies {
                                http_rbac = encoded.http_config.clone();
                            }
                        }
                    }
                }

                let filter = if is_http {
                    NetworkFilter::http_connection_manager_with_rbac(&route_name, http_rbac)
                } else {
                    NetworkFilter::tcp_proxy(service_name.as_str())
                };
                let mut chain = FilterChain::new(format!("default:{}", service_name));
                if let Some(config) = network_rbac {
                    chain = chain.with_filter(NetworkFilter::rbac(config));
                }
                let chain = chain.with_filter(filter);
                Listener::new(
                    format!("default_listener:{}", service_name),
                    ListenerAddress::tcp("127.0.0.1", bind_port),
                )
                .with_filter_chain(chain)
            })
            .collect::<Vec<_>>();

        ResourceSnapshot::with_all_resources(clusters, endpoints, listeners, routes)
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

/// Add the two standard Connect SDS secrets (leaf certificate + validation
/// context) built from a service's live CA material to a snapshot.
fn add_connect_sds_secrets(snapshot: &mut ResourceSnapshot, material: &ConnectMaterial) {
    snapshot.add_secret(SdsSecret {
        name: SDS_CONNECT_LEAF.to_string(),
        kind: SdsSecretKind::TlsCertificate {
            certificate_chain: material.leaf_cert_pem.clone(),
            private_key: material.private_key_pem.clone(),
        },
    });
    snapshot.add_secret(SdsSecret {
        name: SDS_CONNECT_ROOT.to_string(),
        kind: SdsSecretKind::ValidationContext {
            trusted_ca: material.root_pem.clone(),
        },
    });
}

/// Build the downstream SDS transport socket applied to a connect-proxy's
/// public (inbound) listener. Envoy resolves `connect-leaf`/`connect-root`
/// through SDS; requiring a client certificate is what makes the path mTLS.
fn downstream_sds_socket() -> TransportSocket {
    TransportSocket {
        name: TLS_TRANSPORT_SOCKET_NAME.to_string(),
        config: TransportSocketConfig::DownstreamSds {
            tls_certificate_secret_name: SDS_CONNECT_LEAF.to_string(),
            validation_context_secret_name: SDS_CONNECT_ROOT.to_string(),
            require_client_certificate: true,
            alpn_protocols: vec!["h2".to_string(), "http/1.1".to_string()],
        },
    }
}

/// Build an inline TLS transport socket for an upstream cluster. The proxy
/// presents its own leaf/key, validates the upstream against the CA root, and
/// pins the upstream service's SPIFFE URI as the SAN.
fn upstream_inline_socket(
    material: &ConnectMaterial,
    upstream: &str,
    datacenter: &str,
    trust_domain: &str,
) -> TransportSocket {
    TransportSocket {
        name: TLS_TRANSPORT_SOCKET_NAME.to_string(),
        config: TransportSocketConfig::UpstreamInline {
            certificate_chain: material.leaf_cert_pem.clone(),
            private_key: material.private_key_pem.clone(),
            trusted_ca: material.root_pem.clone(),
            san_uri: format!(
                "spiffe://{}/ns/default/dc/{}/svc/{}",
                trust_domain, datacenter, upstream
            ),
            sni: upstream.to_string(),
        },
    }
}

/// Build an inbound listener for a connect-proxy, optionally carrying a
/// downstream transport socket on its filter chain.
///
/// The listener binds to `0.0.0.0:{proxy_port}` and forwards traffic to the
/// local service cluster via the `envoy.filters.network.tcp_proxy` filter.
fn build_inbound_listener(
    proxy_port: u16,
    local_cluster: &str,
    transport_socket: Option<TransportSocket>,
) -> Listener {
    let mut filter_chain =
        FilterChain::new("inbound").with_filter(NetworkFilter::tcp_proxy(local_cluster));
    if let Some(socket) = transport_socket {
        filter_chain.transport_socket = Some(socket);
    }
    Listener::new(
        format!("inbound_listener:{}", proxy_port),
        ListenerAddress::tcp("0.0.0.0", proxy_port),
    )
    .with_filter_chain(filter_chain)
}

/// Build an inbound listener for a connect-proxy without mTLS. Kept as the
/// public entry point for callers/tests that do not have CA material.
pub fn consul_to_inbound_listener(proxy_port: u16, local_cluster: &str) -> Listener {
    build_inbound_listener(proxy_port, local_cluster, None)
}

/// Attach an outbound (app-facing) listener for an upstream, using an HCM/RDS
/// filter chain for HTTP services and a plain tcp_proxy listener otherwise.
/// App-facing listeners stay plaintext in both cases.
fn add_outbound_listener(
    snapshot: &mut ResourceSnapshot,
    bind_port: u16,
    dest_name: &str,
    route_config: Option<RouteConfiguration>,
) {
    let Some(route_config) = route_config else {
        snapshot.add_listener(consul_to_outbound_listener(bind_port, dest_name));
        return;
    };
    let route_name = route_config.name.clone();
    snapshot.add_route(route_config);
    let chain = FilterChain::new(format!("http-outbound:{}", dest_name))
        .with_filter(NetworkFilter::http_connection_manager(&route_name));
    snapshot.add_listener(
        Listener::new(
            format!("outbound_listener:{}", bind_port),
            ListenerAddress::tcp("127.0.0.1", bind_port),
        )
        .with_filter_chain(chain),
    );
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

/// Name of the RDS `RouteConfiguration` generated for a service.
///
/// The HCM filter in the corresponding listener uses this exact name in its
/// `rds.route_config_name`, otherwise Envoy would subscribe to a route table
/// that the ADS server never sends.
pub fn route_config_name(service: &str) -> String {
    format!("{}:routes", service)
}

/// Guard against cyclic / corrupted discovery chains.
const MAX_CHAIN_DEPTH: usize = 8;

/// Whether a discovery chain describes an L7 protocol that can use RDS.
fn is_http_protocol(protocol: &str) -> bool {
    matches!(
        protocol.to_ascii_lowercase().as_str(),
        "http" | "http2" | "grpc" | "http3"
    )
}

/// Compile a Consul discovery chain into an RDS `RouteConfiguration`.
///
/// Only HTTP-ish protocols are converted: a `tcp` chain (the default when no
/// `service-defaults` config entry exists) has no L7 route table, and emitting
/// one would silently change TCP proxy semantics. The graph walk handles
/// router (path/header matches), splitter (weighted clusters) and resolver
/// (single target) nodes. Nodes that need protos outside the checked-in subset
/// (regex matchers, redirect/direct-response) are skipped rather than guessed.
pub fn discovery_chain_to_route_config(
    chain: &CompiledDiscoveryChain,
) -> Option<RouteConfiguration> {
    if !is_http_protocol(&chain.protocol) {
        return None;
    }
    let routes = routes_from_node(chain, &chain.start_node, 0)?;
    if routes.is_empty() {
        return None;
    }
    Some(RouteConfiguration {
        name: route_config_name(&chain.service_name),
        virtual_hosts: vec![VirtualHost {
            name: chain.service_name.clone(),
            domains: vec!["*".to_string()],
            routes,
            ..Default::default()
        }],
        ..Default::default()
    })
}

fn routes_from_node(
    chain: &CompiledDiscoveryChain,
    node_name: &str,
    depth: usize,
) -> Option<Vec<Route>> {
    if depth > MAX_CHAIN_DEPTH {
        return None;
    }
    let node = chain.nodes.get(node_name)?;
    match node.node_type {
        DiscoveryGraphNodeType::Router => {
            let mut routes = Vec::new();
            for (i, entry) in node.routes.iter().enumerate() {
                let Some(route_match) = route_match_from_definition(entry.definition.as_ref())
                else {
                    continue;
                };
                let Some(dest) = resolve_destination(chain, &entry.next_node, depth + 1) else {
                    continue;
                };
                routes.push(Route::new(
                    format!("{}-route-{}", chain.service_name, i),
                    route_match,
                    RouteAction::Route(dest),
                ));
            }
            Some(routes)
        }
        DiscoveryGraphNodeType::Splitter | DiscoveryGraphNodeType::Resolver => {
            let dest = resolve_destination(chain, node_name, depth + 1)?;
            Some(vec![Route::new(
                format!("{}-default", chain.service_name),
                RouteMatch::prefix("/"),
                RouteAction::Route(dest),
            )])
        }
    }
}

/// Resolve a graph node to the cluster (or weighted clusters) it routes to.
fn resolve_destination(
    chain: &CompiledDiscoveryChain,
    node_name: &str,
    depth: usize,
) -> Option<RouteDestination> {
    if depth > MAX_CHAIN_DEPTH {
        return None;
    }
    let node = chain.nodes.get(node_name)?;
    match node.node_type {
        DiscoveryGraphNodeType::Resolver => {
            let resolver = node.resolver.as_ref()?;
            let target = chain.targets.get(&resolver.target)?;
            Some(RouteDestination::cluster(&target.service))
        }
        DiscoveryGraphNodeType::Splitter => {
            let total: f64 = node.splits.iter().map(|s| s.weight.max(0.0)).sum();
            if total <= 0.0 {
                return None;
            }
            let mut clusters = Vec::new();
            for split in &node.splits {
                if split.weight <= 0.0 {
                    continue;
                }
                let dest = resolve_destination(chain, &split.next_node, depth + 1)?;
                let name = match dest.cluster {
                    Some(name) => name,
                    None => dest.weighted_clusters.into_iter().next()?.name,
                };
                // Consul splitter weights are relative (usually summing to 100).
                // Normalize to a percentage so Envoy's `total_weight` is stable
                // even when a config entry uses 0.8/0.2 style fractions.
                let weight = ((split.weight / total) * 100.0).round().max(1.0) as u32;
                clusters.push(WeightedCluster::new(name, weight));
            }
            (!clusters.is_empty()).then(|| RouteDestination::weighted(clusters))
        }
        DiscoveryGraphNodeType::Router => {
            let first = node.routes.first()?;
            resolve_destination(chain, &first.next_node, depth + 1)
        }
    }
}

/// Translate a Consul router match definition into an Envoy route match.
fn route_match_from_definition(definition: Option<&DiscoveryRouteMatch>) -> Option<RouteMatch> {
    let Some(definition) = definition else {
        return Some(RouteMatch::prefix("/"));
    };
    let Some(http) = definition.http.as_ref() else {
        return Some(RouteMatch::prefix("/"));
    };

    let mut route_match = if let Some(path) = &http.path_exact {
        RouteMatch::path(path)
    } else if let Some(prefix) = &http.path_prefix {
        RouteMatch::prefix(prefix)
    } else if http.path_regex.is_some() {
        // RegexMatcher is not in the checked-in proto subset; a prefix "/"
        // would silently match everything and must not be substituted.
        return None;
    } else {
        RouteMatch::prefix("/")
    };

    for header in &http.header {
        let matcher = if let Some(exact) = &header.exact {
            Some(HeaderMatcher {
                name: header.name.clone(),
                match_type: HeaderMatchType::Exact(exact.clone()),
                invert_match: header.invert,
            })
        } else if let Some(prefix) = &header.prefix {
            Some(HeaderMatcher {
                name: header.name.clone(),
                match_type: HeaderMatchType::Prefix(prefix.clone()),
                invert_match: header.invert,
            })
        } else if let Some(suffix) = &header.suffix {
            Some(HeaderMatcher {
                name: header.name.clone(),
                match_type: HeaderMatchType::Suffix(suffix.clone()),
                invert_match: header.invert,
            })
        } else if header.present {
            Some(HeaderMatcher {
                name: header.name.clone(),
                match_type: HeaderMatchType::Present,
                invert_match: header.invert,
            })
        } else if header.regex.is_some() {
            return None;
        } else {
            None
        };
        if let Some(matcher) = matcher {
            route_match = route_match.with_header(matcher);
        }
    }

    // A single method restriction maps cleanly to a `:method` exact match.
    // Multiple methods would need an OR matcher, which the minimal proto does
    // not carry, so they are deliberately not approximated by an AND chain.
    if http.methods.len() == 1 {
        route_match = route_match.with_header(HeaderMatcher::exact(":method", &http.methods[0]));
    }

    // Keep the compiler honest about what it can encode: PathMatch::Regex is
    // skipped here rather than by letting the encoder drop the whole route.
    if matches!(
        route_match.path_match,
        PathMatch::Regex(_) | PathMatch::PathTemplate(_)
    ) {
        return None;
    }
    Some(route_match)
}

/// Helper to create an xDS server with default config.
pub fn create_xds_server() -> Arc<XdsServer> {
    Arc::new(XdsServer::new(XdsServerConfig::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentServiceRegistration;
    use batata_mesh::xds::types::{DiscoveryType, NetworkFilterType};
    use batata_plugin::PluginNamingStore;

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

    /// Regression: the default snapshot used to publish `Vec::new()` for LDS,
    /// so an Envoy bootstrapped with `lds_config` over ADS had no listener to
    /// apply. Every generated listener must also encode to real Envoy protobuf
    /// (not be silently skipped by `encode_resource`).
    #[test]
    fn test_default_snapshot_publishes_encodable_listeners() {
        use batata_mesh::xds::ResourceType;
        use batata_mesh::xds::encode::{NativeResource, encode_resource};

        let naming_store = Arc::new(ConsulNamingStore::new());
        let xds_server = create_xds_server();
        for (service, port) in [("web", 8080), ("api", 9090)] {
            let id = format!("{}-1", service);
            let reg = AgentServiceRegistration {
                name: service.to_string(),
                id: Some(id.clone()),
                address: Some("127.0.0.1".to_string()),
                port: Some(port),
                ..Default::default()
            };
            let key = ConsulNamingStore::build_key(DEFAULT_NAMESPACE, service, &id);
            naming_store
                .register(&key, serde_json::to_vec(&reg).unwrap().into())
                .unwrap();
        }

        let bridge = ConsulSyncBridge::new(naming_store, xds_server);
        let snapshot = bridge.build_snapshot();
        assert_eq!(snapshot.listeners.len(), 2);
        for listener in snapshot.listeners.values() {
            assert!(
                encode_resource(ResourceType::Listener, NativeResource::Listener(listener))
                    .is_some(),
                "default LDS listener {} must encode to real Envoy protobuf",
                listener.name
            );
            // No Connect service means no RDS table, so the filter must be a
            // tcp_proxy rather than an HCM pointing at a missing route.
            assert!(matches!(
                listener.filter_chains[0].filters[0].filter_type,
                NetworkFilterType::TcpProxy { .. }
            ));
        }

        // Stable, collision-free bind ports (sorted service names).
        let mut ports: Vec<u16> = snapshot
            .listeners
            .values()
            .map(|l| l.address.port)
            .collect();
        ports.sort();
        ports.dedup();
        assert_eq!(ports.len(), 2);
    }

    /// A DENY intention targeting a service must prepend a network RBAC
    /// filter to that service's listener, ahead of the proxy filter.
    #[tokio::test]
    async fn test_l4_deny_intention_prepends_rbac_filter() {
        use crate::connect_ca::IntentionRequest;

        let naming_store = Arc::new(ConsulNamingStore::new());
        let xds_server = create_xds_server();
        let reg = AgentServiceRegistration {
            name: "api".to_string(),
            id: Some("api-1".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(9090),
            ..Default::default()
        };
        let key = ConsulNamingStore::build_key(DEFAULT_NAMESPACE, "api", "api-1");
        naming_store
            .register(&key, serde_json::to_vec(&reg).unwrap().into())
            .unwrap();

        let ca = ConsulConnectCAService::new();
        ca.upsert_intention_exact(IntentionRequest {
            description: String::new(),
            source_ns: "default".to_string(),
            source_name: "web".to_string(),
            destination_ns: "default".to_string(),
            destination_name: "api".to_string(),
            action: CaIntentionAction::Deny,
            permissions: vec![],
            meta: HashMap::new(),
        })
        .await;

        let bridge =
            ConsulSyncBridge::new(naming_store, xds_server).with_connect_ca(ca, "consul", "dc1");
        let snapshot = bridge.build_snapshot();
        let listener = snapshot.listeners.get("default_listener:api").unwrap();
        let filters = &listener.filter_chains[0].filters;
        assert_eq!(filters.len(), 2, "RBAC + tcp_proxy expected");
        assert!(
            matches!(filters[0].filter_type, NetworkFilterType::Rbac { .. }),
            "first filter must be RBAC: {:?}",
            filters[0].filter_type
        );
        assert!(matches!(
            filters[1].filter_type,
            NetworkFilterType::TcpProxy { .. }
        ));
        // The encoded config must contain a real DENY-action RBAC policy.
        let NetworkFilterType::Rbac { config } = &filters[0].filter_type else {
            unreachable!()
        };
        let decoded = batata_mesh::xds::rbac::decode_network_filter_config(config.as_slice())
            .expect("RBAC filter config must decode as Envoy protobuf");
        assert_eq!(
            decoded.rules.unwrap().action,
            batata_mesh::xds::generated::rbac::rbac::Action::Deny as i32
        );
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

    /// Register "web" (local service), "api" (upstream) and a connect-proxy
    /// registration for web with one upstream. Shared by the mTLS snapshot
    /// tests so they exercise the exact same fixture.
    fn proxy_fixture() -> (Arc<ConsulNamingStore>, AgentServiceRegistration) {
        let naming_store = Arc::new(ConsulNamingStore::new());
        for (name, port) in [("web", 8080u16), ("api", 9090u16)] {
            let id = format!("{}-1", name);
            let reg = AgentServiceRegistration {
                name: name.to_string(),
                id: Some(id.clone()),
                address: Some("127.0.0.1".to_string()),
                port: Some(port),
                ..Default::default()
            };
            naming_store
                .register(
                    &ConsulNamingStore::build_key(DEFAULT_NAMESPACE, name, &id),
                    serde_json::to_vec(&reg).unwrap().into(),
                )
                .unwrap();
        }
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
        (naming_store, proxy_reg)
    }

    #[test]
    fn test_proxy_snapshot_has_connect_sds_secrets() {
        let (store, proxy) = proxy_fixture();
        let bridge = ConsulSyncBridge::new(store, create_xds_server()).with_connect_ca(
            ConsulConnectCAService::new(),
            "consul",
            "dc1",
        );
        let snapshot = bridge.build_proxy_snapshot(&proxy);

        assert!(snapshot.secrets.contains_key(SDS_CONNECT_LEAF));
        assert!(snapshot.secrets.contains_key(SDS_CONNECT_ROOT));
        match &snapshot.secrets[SDS_CONNECT_LEAF].kind {
            SdsSecretKind::TlsCertificate {
                certificate_chain,
                private_key,
            } => {
                assert!(!certificate_chain.is_empty());
                assert!(!private_key.is_empty());
            }
            _ => panic!("connect-leaf must be a TLS certificate secret"),
        }
        match &snapshot.secrets[SDS_CONNECT_ROOT].kind {
            SdsSecretKind::ValidationContext { trusted_ca } => {
                assert!(!trusted_ca.is_empty());
            }
            _ => panic!("connect-root must be a validation context secret"),
        }
    }

    #[test]
    fn test_proxy_inbound_listener_has_downstream_sds_socket() {
        let (store, proxy) = proxy_fixture();
        let bridge = ConsulSyncBridge::new(store, create_xds_server()).with_connect_ca(
            ConsulConnectCAService::new(),
            "consul",
            "dc1",
        );
        let snapshot = bridge.build_proxy_snapshot(&proxy);

        let listener = snapshot.listeners.get("inbound_listener:20000").unwrap();
        let socket = listener.filter_chains[0]
            .transport_socket
            .as_ref()
            .expect("inbound listener must carry a downstream TLS socket");
        match &socket.config {
            TransportSocketConfig::DownstreamSds {
                tls_certificate_secret_name,
                validation_context_secret_name,
                require_client_certificate,
                alpn_protocols,
            } => {
                assert_eq!(tls_certificate_secret_name, SDS_CONNECT_LEAF);
                assert_eq!(validation_context_secret_name, SDS_CONNECT_ROOT);
                assert!(*require_client_certificate);
                assert_eq!(alpn_protocols, &["h2".to_string(), "http/1.1".to_string()]);
            }
            _ => panic!("inbound socket must be a downstream SDS config"),
        }
    }

    #[test]
    fn test_proxy_upstream_cluster_has_inline_tls_socket() {
        let (store, proxy) = proxy_fixture();
        let bridge = ConsulSyncBridge::new(store, create_xds_server()).with_connect_ca(
            ConsulConnectCAService::new(),
            "consul",
            "dc1",
        );
        let snapshot = bridge.build_proxy_snapshot(&proxy);

        let cluster = snapshot.clusters.get("api").unwrap();
        let socket = cluster
            .transport_socket
            .as_ref()
            .expect("upstream cluster must carry an inline TLS socket");
        match &socket.config {
            TransportSocketConfig::UpstreamInline {
                certificate_chain,
                private_key,
                trusted_ca,
                san_uri,
                ..
            } => {
                assert!(!certificate_chain.is_empty());
                assert!(!private_key.is_empty());
                assert!(!trusted_ca.is_empty());
                assert_eq!(san_uri, "spiffe://consul/ns/default/dc/dc1/svc/api");
            }
            _ => panic!("upstream socket must be an inline TLS config"),
        }
    }

    #[test]
    fn test_proxy_local_cluster_and_outbound_listener_stay_plaintext() {
        let (store, proxy) = proxy_fixture();
        let bridge = ConsulSyncBridge::new(store, create_xds_server()).with_connect_ca(
            ConsulConnectCAService::new(),
            "consul",
            "dc1",
        );
        let snapshot = bridge.build_proxy_snapshot(&proxy);

        // The local service cluster feeds the plaintext app-facing path.
        let local = snapshot.clusters.get("web").unwrap();
        assert!(
            local.transport_socket.is_none(),
            "local service cluster must stay plaintext"
        );
        // App-facing (outbound) listeners must not carry a transport socket.
        let outbound = snapshot.listeners.get("outbound_listener:9090").unwrap();
        assert!(
            outbound.filter_chains[0].transport_socket.is_none(),
            "outbound app-facing listener must stay plaintext"
        );
    }

    #[test]
    fn test_ca_revision_triggers_snapshot_rebuild() {
        use std::sync::atomic::Ordering;

        let (store, _proxy) = proxy_fixture();
        let xds_server = create_xds_server();
        let ca = ConsulConnectCAService::new();
        let bridge = ConsulSyncBridge::new(store, xds_server.clone()).with_connect_ca(
            ca.clone(),
            "consul",
            "dc1",
        );

        // First sync establishes a baseline and records every revision.
        bridge.maybe_sync();
        let v0 = xds_server
            .snapshot_cache()
            .get_snapshot("")
            .unwrap()
            .version
            .clone();
        assert_eq!(
            bridge.last_ca_revision.load(Ordering::Relaxed),
            ca.ca_revision()
        );

        // Make only the CA revision stale: naming-store and intention
        // revisions stay current, so the CA key alone must force a rebuild.
        bridge
            .last_ca_revision
            .store(ca.ca_revision().wrapping_add(1), Ordering::Relaxed);
        bridge.maybe_sync();
        let v1 = xds_server
            .snapshot_cache()
            .get_snapshot("")
            .unwrap()
            .version
            .clone();
        assert_ne!(v0, v1, "a changed CA revision must rebuild the snapshot");
        assert_eq!(
            bridge.last_ca_revision.load(Ordering::Relaxed),
            ca.ca_revision()
        );
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

    use crate::config_entry::{ConfigEntry, ConsulConfigEntryService};
    use crate::connect::{
        DiscoveryGraphNode, DiscoveryHTTPHeaderMatch, DiscoveryHTTPRouteMatch, DiscoveryResolver,
        DiscoveryRoute, DiscoverySplit, DiscoveryTarget,
    };

    fn test_target(service: &str) -> DiscoveryTarget {
        DiscoveryTarget {
            id: format!("{}.default.dc1", service),
            service: service.to_string(),
            service_subset: String::new(),
            namespace: "default".to_string(),
            partition: "default".to_string(),
            datacenter: "dc1".to_string(),
            mesh_gateway: Default::default(),
            subset: Default::default(),
            connect_timeout: "5s".to_string(),
            sni: String::new(),
            name: service.to_string(),
        }
    }

    fn test_resolver_node(service: &str) -> DiscoveryGraphNode {
        DiscoveryGraphNode {
            node_type: DiscoveryGraphNodeType::Resolver,
            name: service.to_string(),
            routes: Vec::new(),
            splits: Vec::new(),
            resolver: Some(DiscoveryResolver {
                default: true,
                connect_timeout: "5s".to_string(),
                target: format!("{}.default.dc1", service),
                failover: None,
            }),
        }
    }

    fn test_chain(
        service: &str,
        protocol: &str,
        start_node: &str,
        nodes: HashMap<String, DiscoveryGraphNode>,
        targets: HashMap<String, DiscoveryTarget>,
    ) -> CompiledDiscoveryChain {
        CompiledDiscoveryChain {
            service_name: service.to_string(),
            namespace: "default".to_string(),
            datacenter: "dc1".to_string(),
            customization_hash: String::new(),
            protocol: protocol.to_string(),
            start_node: start_node.to_string(),
            nodes,
            targets,
            default: false,
        }
    }

    fn single_resolver_chain(service: &str, protocol: &str) -> CompiledDiscoveryChain {
        let start = format!("resolver:{}", service);
        let mut nodes = HashMap::new();
        nodes.insert(start.clone(), test_resolver_node(service));
        let mut targets = HashMap::new();
        targets.insert(format!("{}.default.dc1", service), test_target(service));
        test_chain(service, protocol, &start, nodes, targets)
    }

    #[test]
    fn test_tcp_discovery_chain_has_no_rds_resource() {
        // The default chain protocol is tcp: emitting an L7 route table would
        // silently change TCP proxy semantics.
        let chain = single_resolver_chain("payments", "tcp");
        assert!(discovery_chain_to_route_config(&chain).is_none());
    }

    #[test]
    fn test_http_resolver_chain_becomes_single_cluster_route() {
        let chain = single_resolver_chain("payments", "http");
        let config = discovery_chain_to_route_config(&chain).expect("http chain must produce RDS");
        assert_eq!(config.name, route_config_name("payments"));
        assert_eq!(config.virtual_hosts.len(), 1);
        assert_eq!(config.virtual_hosts[0].domains, vec!["*".to_string()]);
        let route = &config.virtual_hosts[0].routes[0];
        assert!(matches!(
            &route.match_config.path_match,
            PathMatch::Prefix(p) if p == "/"
        ));
        match &route.action {
            RouteAction::Route(dest) => {
                assert_eq!(dest.cluster.as_deref(), Some("payments"));
                assert!(dest.weighted_clusters.is_empty());
            }
            _ => panic!("expected a cluster route action"),
        }
    }

    #[test]
    fn test_splitter_chain_normalizes_relative_weights() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "splitter:payments".to_string(),
            DiscoveryGraphNode {
                node_type: DiscoveryGraphNodeType::Splitter,
                name: "payments".to_string(),
                routes: Vec::new(),
                splits: vec![
                    DiscoverySplit {
                        definition: None,
                        weight: 0.8,
                        next_node: "resolver:payments-v1".to_string(),
                    },
                    DiscoverySplit {
                        definition: None,
                        weight: 0.2,
                        next_node: "resolver:payments-v2".to_string(),
                    },
                ],
                resolver: None,
            },
        );
        nodes.insert(
            "resolver:payments-v1".to_string(),
            test_resolver_node("payments-v1"),
        );
        nodes.insert(
            "resolver:payments-v2".to_string(),
            test_resolver_node("payments-v2"),
        );
        let mut targets = HashMap::new();
        targets.insert(
            "payments-v1.default.dc1".to_string(),
            test_target("payments-v1"),
        );
        targets.insert(
            "payments-v2.default.dc1".to_string(),
            test_target("payments-v2"),
        );
        let chain = test_chain("payments", "http", "splitter:payments", nodes, targets);

        let config = discovery_chain_to_route_config(&chain).expect("splitter must produce RDS");
        let route = &config.virtual_hosts[0].routes[0];
        match &route.action {
            RouteAction::Route(dest) => {
                assert_eq!(dest.weighted_clusters.len(), 2);
                assert_eq!(dest.weighted_clusters[0].name, "payments-v1");
                assert_eq!(dest.weighted_clusters[0].weight, 80);
                assert_eq!(dest.weighted_clusters[1].weight, 20);
            }
            _ => panic!("expected a cluster route action"),
        }
    }

    #[test]
    fn test_router_chain_preserves_path_and_header_matches() {
        let mut nodes = HashMap::new();
        nodes.insert(
            "router:payments".to_string(),
            DiscoveryGraphNode {
                node_type: DiscoveryGraphNodeType::Router,
                name: "payments".to_string(),
                routes: vec![DiscoveryRoute {
                    definition: Some(DiscoveryRouteMatch {
                        http: Some(DiscoveryHTTPRouteMatch {
                            path_exact: None,
                            path_prefix: Some("/api".to_string()),
                            path_regex: None,
                            header: vec![DiscoveryHTTPHeaderMatch {
                                name: "x-env".to_string(),
                                exact: Some("prod".to_string()),
                                prefix: None,
                                suffix: None,
                                regex: None,
                                present: false,
                                invert: false,
                            }],
                            query_param: Vec::new(),
                            methods: vec!["GET".to_string()],
                        }),
                    }),
                    next_node: "resolver:payments".to_string(),
                }],
                splits: Vec::new(),
                resolver: None,
            },
        );
        nodes.insert(
            "resolver:payments".to_string(),
            test_resolver_node("payments"),
        );
        let mut targets = HashMap::new();
        targets.insert("payments.default.dc1".to_string(), test_target("payments"));
        let chain = test_chain("payments", "http", "router:payments", nodes, targets);

        let config = discovery_chain_to_route_config(&chain).expect("router must produce RDS");
        let route = &config.virtual_hosts[0].routes[0];
        assert!(matches!(
            &route.match_config.path_match,
            PathMatch::Prefix(p) if p == "/api"
        ));
        assert_eq!(route.match_config.headers.len(), 2);
        assert_eq!(route.match_config.headers[0].name, "x-env");
        assert_eq!(route.match_config.headers[1].name, ":method");
    }

    #[test]
    fn test_regex_router_match_is_skipped_not_approximated() {
        let regex_definition = DiscoveryRouteMatch {
            http: Some(DiscoveryHTTPRouteMatch {
                path_exact: None,
                path_prefix: None,
                path_regex: Some("^/v[0-9]+/.*".to_string()),
                header: Vec::new(),
                query_param: Vec::new(),
                methods: Vec::new(),
            }),
        };

        // Guard the matcher itself: an approximation such as prefix("/") would
        // silently match every request, so it must be rejected outright.
        assert!(route_match_from_definition(Some(&regex_definition)).is_none());

        let mut nodes = HashMap::new();
        nodes.insert(
            "router:payments".to_string(),
            DiscoveryGraphNode {
                node_type: DiscoveryGraphNodeType::Router,
                name: "payments".to_string(),
                routes: vec![DiscoveryRoute {
                    definition: Some(regex_definition),
                    next_node: "resolver:payments".to_string(),
                }],
                splits: Vec::new(),
                resolver: None,
            },
        );
        // A fully resolvable chain is essential: without the resolver node and
        // target the chain returns None for the wrong reason and the test would
        // still pass if the regex were approximated as prefix("/").
        nodes.insert(
            "resolver:payments".to_string(),
            test_resolver_node("payments"),
        );
        let mut targets = HashMap::new();
        targets.insert("payments.default.dc1".to_string(), test_target("payments"));
        let chain = test_chain("payments", "http", "router:payments", nodes, targets);
        assert!(discovery_chain_to_route_config(&chain).is_none());
    }

    #[test]
    fn test_bridge_publishes_rds_from_service_defaults() {
        let naming_store = Arc::new(ConsulNamingStore::new());
        let reg = AgentServiceRegistration {
            name: "payments".to_string(),
            id: Some("payments-1".to_string()),
            address: Some("127.0.0.1".to_string()),
            port: Some(8080),
            ..Default::default()
        };
        let key = ConsulNamingStore::build_key(DEFAULT_NAMESPACE, "payments", "payments-1");
        naming_store
            .register(&key, serde_json::to_vec(&reg).unwrap().into())
            .unwrap();

        // Without a connect service the snapshot stays CDS/EDS-only.
        let plain = ConsulSyncBridge::new(naming_store.clone(), create_xds_server());
        assert!(plain.build_snapshot().routes.is_empty());

        // A `service-defaults` with Protocol=http must reach the data plane.
        let entries = ConsulConfigEntryService::new();
        entries.entries_arc().insert(
            "service-defaults/payments".to_string(),
            ConfigEntry {
                kind: "service-defaults".to_string(),
                name: "payments".to_string(),
                namespace: None,
                partition: None,
                meta: None,
                extra: HashMap::from([("Protocol".to_string(), serde_json::json!("http"))]),
                create_index: 1,
                modify_index: 1,
            },
        );
        let connect =
            Arc::new(ConsulConnectService::new().with_config_entry_service(Arc::new(entries)));
        let bridge =
            ConsulSyncBridge::new(naming_store, create_xds_server()).with_connect_service(connect);
        let snapshot = bridge.build_snapshot();
        assert!(
            snapshot.routes.contains_key("payments:routes"),
            "HTTP service must publish an RDS resource"
        );
    }
}
