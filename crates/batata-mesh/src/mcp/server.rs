//! Istio MCP Server Implementation
//!
//! This module provides a Mesh Configuration Protocol (MCP) server that allows
//! Istio to discover services from Batata.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info};

use super::types::*;
use crate::conversion::NacosService;

/// MCP Server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Server ID
    pub server_id: String,
    /// Default namespace for ServiceEntry resources
    pub default_namespace: String,
    /// Export to namespaces (empty = all)
    pub export_to: Vec<String>,
    /// Whether to generate VirtualServices
    pub generate_virtual_services: bool,
    /// Whether to generate DestinationRules
    pub generate_destination_rules: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            server_id: "batata-mcp-server".to_string(),
            default_namespace: "default".to_string(),
            export_to: vec!["*".to_string()],
            generate_virtual_services: true,
            generate_destination_rules: true,
        }
    }
}

/// MCP Server for Istio integration
///
/// Provides Istio resources (ServiceEntry, VirtualService, DestinationRule)
/// based on Nacos service discovery data.
pub struct McpServer {
    /// Configuration
    config: McpServerConfig,
    /// ServiceEntry cache
    service_entries: DashMap<String, ServiceEntry>,
    /// VirtualService cache
    virtual_services: DashMap<String, VirtualService>,
    /// DestinationRule cache
    destination_rules: DashMap<String, DestinationRule>,
    /// Current version (atomic for sync access)
    version: AtomicU64,
    /// Change notification channel
    change_tx: mpsc::Sender<McpChangeEvent>,
    /// Change receiver (for consumers)
    change_rx: Arc<RwLock<Option<mpsc::Receiver<McpChangeEvent>>>>,
}

/// MCP change event
#[derive(Debug, Clone)]
pub enum McpChangeEvent {
    /// ServiceEntry was updated
    ServiceEntryUpdated { name: String },
    /// ServiceEntry was removed
    ServiceEntryRemoved { name: String },
    /// VirtualService was updated
    VirtualServiceUpdated { name: String },
    /// VirtualService was removed
    VirtualServiceRemoved { name: String },
    /// DestinationRule was updated
    DestinationRuleUpdated { name: String },
    /// DestinationRule was removed
    DestinationRuleRemoved { name: String },
    /// Full resync happened
    FullResync,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(config: McpServerConfig) -> Self {
        let (change_tx, change_rx) = mpsc::channel(1000);

        info!(
            server_id = %config.server_id,
            namespace = %config.default_namespace,
            "Creating MCP server"
        );

        Self {
            config,
            service_entries: DashMap::new(),
            virtual_services: DashMap::new(),
            destination_rules: DashMap::new(),
            version: AtomicU64::new(0),
            change_tx,
            change_rx: Arc::new(RwLock::new(Some(change_rx))),
        }
    }

    /// Get the change notification channel
    pub async fn take_change_receiver(&self) -> Option<mpsc::Receiver<McpChangeEvent>> {
        self.change_rx.write().await.take()
    }

    /// Get the current version
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Increment version and return new value
    fn next_version(&self) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Sync Nacos services to Istio resources
    pub async fn sync_services(&self, services: &[NacosService]) {
        let version = self.next_version();

        debug!(
            version = version,
            service_count = services.len(),
            "Syncing services to MCP"
        );

        // Track existing resources for cleanup
        let existing_se: std::collections::HashSet<String> = self
            .service_entries
            .iter()
            .map(|e| e.key().clone())
            .collect();

        let mut updated_se = std::collections::HashSet::new();

        // Convert each Nacos service to Istio resources
        for service in services {
            let name = self.service_entry_name(service);
            updated_se.insert(name.clone());

            // Create/Update ServiceEntry
            let se = self.nacos_to_service_entry(service);
            self.service_entries.insert(name.clone(), se);
            let _ = self
                .change_tx
                .send(McpChangeEvent::ServiceEntryUpdated { name: name.clone() })
                .await;

            // Create/Update VirtualService if configured
            if self.config.generate_virtual_services {
                let vs = self.nacos_to_virtual_service(service);
                self.virtual_services.insert(name.clone(), vs);
                let _ = self
                    .change_tx
                    .send(McpChangeEvent::VirtualServiceUpdated { name: name.clone() })
                    .await;
            }

            // Create/Update DestinationRule if configured
            if self.config.generate_destination_rules {
                let dr = self.nacos_to_destination_rule(service);
                self.destination_rules.insert(name.clone(), dr);
                let _ = self
                    .change_tx
                    .send(McpChangeEvent::DestinationRuleUpdated { name })
                    .await;
            }
        }

        // Remove stale ServiceEntries
        for name in existing_se.difference(&updated_se) {
            self.service_entries.remove(name);
            let _ = self
                .change_tx
                .send(McpChangeEvent::ServiceEntryRemoved { name: name.clone() })
                .await;

            if self.config.generate_virtual_services {
                self.virtual_services.remove(name);
                let _ = self
                    .change_tx
                    .send(McpChangeEvent::VirtualServiceRemoved { name: name.clone() })
                    .await;
            }

            if self.config.generate_destination_rules {
                self.destination_rules.remove(name);
                let _ = self
                    .change_tx
                    .send(McpChangeEvent::DestinationRuleRemoved { name: name.clone() })
                    .await;
            }
        }

        info!(
            version = version,
            service_entries = self.service_entries.len(),
            virtual_services = self.virtual_services.len(),
            destination_rules = self.destination_rules.len(),
            "MCP sync complete"
        );
    }

    /// Generate ServiceEntry name from Nacos service
    fn service_entry_name(&self, service: &NacosService) -> String {
        if service.namespace_id.is_empty() || service.namespace_id == "public" {
            if service.group_name.is_empty() || service.group_name == "DEFAULT_GROUP" {
                service.service_name.clone()
            } else {
                format!("{}-{}", service.group_name, service.service_name)
            }
        } else {
            format!(
                "{}-{}-{}",
                service.namespace_id, service.group_name, service.service_name
            )
        }
    }

    /// Convert Nacos service to Istio ServiceEntry
    fn nacos_to_service_entry(&self, service: &NacosService) -> ServiceEntry {
        let name = self.service_entry_name(service);
        let host = self.service_host(service);

        // Determine port from instances
        let port = service
            .instances
            .first()
            .map(|i| i.port as u32)
            .unwrap_or(80);

        // Determine protocol from metadata
        let protocol = service
            .metadata
            .get("protocol")
            .map(|p| match p.to_lowercase().as_str() {
                "grpc" => Protocol::Grpc,
                "https" => Protocol::Https,
                "http2" => Protocol::Http2,
                "tcp" => Protocol::Tcp,
                _ => Protocol::Http,
            })
            .unwrap_or(Protocol::Http);

        // Build endpoints from instances
        let endpoints: Vec<WorkloadEntry> = service
            .instances
            .iter()
            .filter(|i| i.enabled && i.healthy)
            .map(|instance| {
                let mut ports = HashMap::new();
                ports.insert("http".to_string(), instance.port as u32);

                let locality =
                    if instance.cluster_name.is_empty() || instance.cluster_name == "DEFAULT" {
                        None
                    } else {
                        Some(instance.cluster_name.clone())
                    };

                let weight = Some((instance.weight as u32).clamp(1, 128));

                // Copy relevant labels
                let labels: HashMap<String, String> = instance
                    .metadata
                    .iter()
                    .filter(|(k, _)| k.starts_with("istio.") || k.starts_with("app."))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                WorkloadEntry {
                    address: Some(instance.ip.clone()),
                    ports,
                    labels,
                    locality,
                    weight,
                    ..Default::default()
                }
            })
            .collect();

        let resolution = if endpoints.is_empty() {
            Resolution::Dns
        } else {
            Resolution::Static
        };

        ServiceEntry {
            metadata: ResourceMetadata {
                name: name.clone(),
                namespace: self.config.default_namespace.clone(),
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("app".to_string(), service.service_name.clone());
                    labels.insert("source".to_string(), "batata".to_string());
                    if !service.namespace_id.is_empty() {
                        labels.insert("nacos-namespace".to_string(), service.namespace_id.clone());
                    }
                    if !service.group_name.is_empty() {
                        labels.insert("nacos-group".to_string(), service.group_name.clone());
                    }
                    labels
                },
                annotations: HashMap::new(),
                resource_version: format!("{}", self.version()),
            },
            spec: ServiceEntrySpec {
                hosts: vec![host],
                ports: vec![Port {
                    number: port,
                    name: "http".to_string(),
                    protocol,
                    target_port: None,
                }],
                resolution,
                location: Location::MeshInternal,
                endpoints,
                export_to: self.config.export_to.clone(),
                ..Default::default()
            },
        }
    }

    /// Convert Nacos service to Istio VirtualService
    fn nacos_to_virtual_service(&self, service: &NacosService) -> VirtualService {
        let name = self.service_entry_name(service);
        let host = self.service_host(service);

        // Get port from instances
        let port = service
            .instances
            .first()
            .map(|i| i.port as u32)
            .unwrap_or(80);

        // Default route to the service
        let default_route = HttpRoute {
            name: Some("default".to_string()),
            route: vec![HttpRouteDestination {
                destination: Destination {
                    host: host.clone(),
                    port: Some(PortSelector { number: Some(port) }),
                    ..Default::default()
                },
                weight: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        };

        // Check for timeout configuration
        let timeout = service.metadata.get("timeout").cloned();

        // Check for retry configuration
        let retries = service
            .metadata
            .get("retries")
            .and_then(|s| s.parse::<i32>().ok())
            .map(|attempts| HttpRetry {
                attempts,
                retry_on: Some("5xx,reset,connect-failure".to_string()),
                ..Default::default()
            });

        let http_routes = vec![HttpRoute {
            timeout,
            retries,
            ..default_route
        }];

        VirtualService {
            metadata: ResourceMetadata {
                name: name.clone(),
                namespace: self.config.default_namespace.clone(),
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("app".to_string(), service.service_name.clone());
                    labels.insert("source".to_string(), "batata".to_string());
                    labels
                },
                annotations: HashMap::new(),
                resource_version: format!("{}", self.version()),
            },
            spec: VirtualServiceSpec {
                hosts: vec![host],
                http: http_routes,
                export_to: self.config.export_to.clone(),
                ..Default::default()
            },
        }
    }

    /// Convert Nacos service to Istio DestinationRule
    fn nacos_to_destination_rule(&self, service: &NacosService) -> DestinationRule {
        let name = self.service_entry_name(service);
        let host = self.service_host(service);

        // Determine load balancer from metadata
        let lb_policy = service
            .metadata
            .get("lb_policy")
            .map(|p| match p.to_lowercase().as_str() {
                "round_robin" | "roundrobin" => SimpleLb::RoundRobin,
                "least_conn" | "least_request" => SimpleLb::LeastConn,
                "random" => SimpleLb::Random,
                _ => SimpleLb::RoundRobin,
            })
            .unwrap_or(SimpleLb::RoundRobin);

        // Connection pool settings from metadata
        let max_connections = service
            .metadata
            .get("max_connections")
            .and_then(|s| s.parse::<i32>().ok());

        let max_requests = service
            .metadata
            .get("max_requests")
            .and_then(|s| s.parse::<i32>().ok());

        let connection_pool = if max_connections.is_some() || max_requests.is_some() {
            Some(ConnectionPoolSettings {
                tcp: max_connections.map(|max| TcpSettings {
                    max_connections: Some(max),
                    ..Default::default()
                }),
                http: max_requests.map(|max| HttpSettings {
                    http2_max_requests: Some(max),
                    ..Default::default()
                }),
            })
        } else {
            None
        };

        // Outlier detection from metadata
        let outlier_detection = service
            .metadata
            .get("outlier_detection")
            .filter(|v| *v == "true")
            .map(|_| OutlierDetection {
                consecutive_5xx_errors: Some(5),
                interval: Some("10s".to_string()),
                base_ejection_time: Some("30s".to_string()),
                max_ejection_percent: Some(50),
                ..Default::default()
            });

        // Build subsets from Nacos clusters
        let cluster_names: std::collections::HashSet<String> = service
            .instances
            .iter()
            .map(|i| i.cluster_name.clone())
            .collect();

        let subsets: Vec<Subset> = cluster_names
            .into_iter()
            .filter(|c| !c.is_empty() && c != "DEFAULT")
            .map(|cluster_name| Subset {
                name: cluster_name.clone(),
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("cluster".to_string(), cluster_name);
                    labels
                },
                ..Default::default()
            })
            .collect();

        DestinationRule {
            metadata: ResourceMetadata {
                name: name.clone(),
                namespace: self.config.default_namespace.clone(),
                labels: {
                    let mut labels = HashMap::new();
                    labels.insert("app".to_string(), service.service_name.clone());
                    labels.insert("source".to_string(), "batata".to_string());
                    labels
                },
                annotations: HashMap::new(),
                resource_version: format!("{}", self.version()),
            },
            spec: DestinationRuleSpec {
                host,
                traffic_policy: Some(TrafficPolicy {
                    load_balancer: Some(LoadBalancerSettings {
                        simple: Some(lb_policy),
                        ..Default::default()
                    }),
                    connection_pool,
                    outlier_detection,
                    ..Default::default()
                }),
                subsets,
                export_to: self.config.export_to.clone(),
            },
        }
    }

    /// Generate service host from Nacos service
    fn service_host(&self, service: &NacosService) -> String {
        // Use DNS-style name: service.namespace.svc.cluster.local
        let service_part = if service.group_name.is_empty() || service.group_name == "DEFAULT_GROUP"
        {
            service.service_name.clone()
        } else {
            format!("{}.{}", service.service_name, service.group_name)
        };

        format!(
            "{}.{}.svc.cluster.local",
            service_part, self.config.default_namespace
        )
    }

    /// Get all ServiceEntries
    pub fn get_service_entries(&self) -> Vec<ServiceEntry> {
        self.service_entries
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get a ServiceEntry by name
    pub fn get_service_entry(&self, name: &str) -> Option<ServiceEntry> {
        self.service_entries.get(name).map(|e| e.value().clone())
    }

    /// Get all VirtualServices
    pub fn get_virtual_services(&self) -> Vec<VirtualService> {
        self.virtual_services
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get a VirtualService by name
    pub fn get_virtual_service(&self, name: &str) -> Option<VirtualService> {
        self.virtual_services.get(name).map(|e| e.value().clone())
    }

    /// Get all DestinationRules
    pub fn get_destination_rules(&self) -> Vec<DestinationRule> {
        self.destination_rules
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get a DestinationRule by name
    pub fn get_destination_rule(&self, name: &str) -> Option<DestinationRule> {
        self.destination_rules.get(name).map(|e| e.value().clone())
    }

    /// Get server statistics
    pub fn stats(&self) -> McpServerStats {
        McpServerStats {
            service_entries: self.service_entries.len(),
            virtual_services: self.virtual_services.len(),
            destination_rules: self.destination_rules.len(),
        }
    }
}

/// MCP server statistics
#[derive(Debug, Clone)]
pub struct McpServerStats {
    /// Number of ServiceEntries
    pub service_entries: usize,
    /// Number of VirtualServices
    pub virtual_services: usize,
    /// Number of DestinationRules
    pub destination_rules: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::NacosInstance;

    fn create_test_service() -> NacosService {
        NacosService {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            instances: vec![
                NacosInstance {
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
                },
                NacosInstance {
                    instance_id: "instance-2".to_string(),
                    ip: "192.168.1.2".to_string(),
                    port: 8080,
                    weight: 100.0,
                    healthy: true,
                    enabled: true,
                    ephemeral: true,
                    cluster_name: "DEFAULT".to_string(),
                    service_name: "test-service".to_string(),
                    metadata: HashMap::new(),
                },
            ],
        }
    }

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new(McpServerConfig::default());
        let stats = server.stats();
        assert_eq!(stats.service_entries, 0);
    }

    #[tokio::test]
    async fn test_sync_services() {
        let server = McpServer::new(McpServerConfig::default());
        let services = vec![create_test_service()];

        server.sync_services(&services).await;

        let stats = server.stats();
        assert_eq!(stats.service_entries, 1);
        assert_eq!(stats.virtual_services, 1);
        assert_eq!(stats.destination_rules, 1);
    }

    #[tokio::test]
    async fn test_get_service_entry() {
        let server = McpServer::new(McpServerConfig::default());
        let services = vec![create_test_service()];

        server.sync_services(&services).await;

        let se = server.get_service_entry("test-service").unwrap();
        assert_eq!(se.spec.endpoints.len(), 2);
        assert_eq!(se.spec.resolution, Resolution::Static);
    }

    #[test]
    fn test_service_entry_name() {
        let server = McpServer::new(McpServerConfig::default());

        let mut service = create_test_service();
        assert_eq!(server.service_entry_name(&service), "test-service");

        service.group_name = "custom-group".to_string();
        assert_eq!(
            server.service_entry_name(&service),
            "custom-group-test-service"
        );

        service.namespace_id = "ns1".to_string();
        assert_eq!(
            server.service_entry_name(&service),
            "ns1-custom-group-test-service"
        );
    }

    #[tokio::test]
    async fn test_service_removal() {
        let server = McpServer::new(McpServerConfig::default());

        // Add services
        let services = vec![create_test_service()];
        server.sync_services(&services).await;
        assert_eq!(server.stats().service_entries, 1);

        // Sync with empty list (should remove)
        server.sync_services(&[]).await;
        assert_eq!(server.stats().service_entries, 0);
    }

    #[test]
    fn test_nacos_to_service_entry() {
        let server = McpServer::new(McpServerConfig::default());
        let service = create_test_service();

        let se = server.nacos_to_service_entry(&service);

        assert_eq!(se.metadata.name, "test-service");
        assert_eq!(se.spec.ports[0].number, 8080);
        assert_eq!(se.spec.endpoints.len(), 2);
        assert!(se.spec.endpoints[0].address.is_some());
    }

    #[test]
    fn test_nacos_to_virtual_service() {
        let server = McpServer::new(McpServerConfig::default());
        let service = create_test_service();

        let vs = server.nacos_to_virtual_service(&service);

        assert_eq!(vs.metadata.name, "test-service");
        assert!(!vs.spec.hosts.is_empty());
        assert!(!vs.spec.http.is_empty());
    }

    #[test]
    fn test_nacos_to_destination_rule() {
        let server = McpServer::new(McpServerConfig::default());
        let service = create_test_service();

        let dr = server.nacos_to_destination_rule(&service);

        assert_eq!(dr.metadata.name, "test-service");
        assert!(dr.spec.traffic_policy.is_some());
    }
}
