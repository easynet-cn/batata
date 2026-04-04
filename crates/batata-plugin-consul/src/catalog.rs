// Consul Catalog API HTTP handlers
// Implements Consul-compatible service catalog endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};

use crate::acl::{AclService, ResourceType};
use crate::config_entry::ConsulConfigEntryService;
use crate::index_provider::{ConsulIndexProvider, ConsulTable};

/// Handle blocking query wait if `index` query parameter is set.
async fn maybe_block(index_provider: &ConsulIndexProvider, index: Option<u64>, wait: Option<&str>) {
    if let Some(target_index) = index {
        let timeout = wait.and_then(ConsulIndexProvider::parse_wait_duration);
        index_provider
            .wait_for_change(ConsulTable::Catalog, target_index, timeout)
            .await;
    }
}
use crate::model::{AgentService, ConsulDatacenterConfig, ConsulError, Weights};

// ============================================================================
// Catalog Models
// ============================================================================

/// Catalog service entry (response for /v1/catalog/service/:service)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogService {
    #[serde(rename = "ID")]
    pub id: String,

    #[serde(rename = "Node")]
    pub node: String,

    #[serde(rename = "Address")]
    pub address: String,

    #[serde(rename = "Datacenter")]
    pub datacenter: String,

    #[serde(rename = "TaggedAddresses", skip_serializing_if = "Option::is_none")]
    pub tagged_addresses: Option<HashMap<String, String>>,

    #[serde(rename = "NodeMeta", skip_serializing_if = "Option::is_none")]
    pub node_meta: Option<HashMap<String, String>>,

    #[serde(rename = "ServiceKind", skip_serializing_if = "Option::is_none")]
    pub service_kind: Option<String>,

    #[serde(rename = "ServiceID")]
    pub service_id: String,

    #[serde(rename = "ServiceName")]
    pub service_name: String,

    #[serde(rename = "ServiceTags", skip_serializing_if = "Option::is_none")]
    pub service_tags: Option<Vec<String>>,

    #[serde(rename = "ServiceAddress")]
    pub service_address: String,

    #[serde(rename = "ServiceWeights")]
    pub service_weights: Weights,

    #[serde(rename = "ServiceMeta", skip_serializing_if = "Option::is_none")]
    pub service_meta: Option<HashMap<String, String>>,

    #[serde(rename = "ServicePort")]
    pub service_port: u16,

    #[serde(rename = "ServiceEnableTagOverride")]
    pub service_enable_tag_override: bool,

    #[serde(rename = "CreateIndex")]
    pub create_index: u64,

    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

impl CatalogService {
    /// Create from an AgentServiceRegistration
    pub fn from_registration(
        reg: &crate::model::AgentServiceRegistration,
        _node_name: &str,
        datacenter: &str,
        index: u64,
    ) -> Self {
        let ip = reg.effective_address();

        // Derive node name from IP if no metadata override available
        let instance_node = format!("node-{}", ip.replace('.', "-"));

        // Filter out internal metadata keys for service_meta
        let service_meta: Option<HashMap<String, String>> = reg.meta.as_ref().map(|m| {
            m.iter()
                .filter(|(k, _)| !k.starts_with("consul_") && k.as_str() != "enable_tag_override")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        });
        let service_meta = service_meta.filter(|m| !m.is_empty());

        let weight = reg.weight().max(1.0) as i32;

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            node: instance_node,
            address: ip.clone(),
            datacenter: datacenter.to_string(),
            tagged_addresses: None,
            node_meta: None,
            service_kind: reg.kind.clone(),
            service_id: reg.service_id(),
            service_name: reg.name.clone(),
            service_tags: reg.tags.clone(),
            service_address: ip,
            service_weights: reg.weights.clone().unwrap_or(Weights {
                passing: weight,
                warning: 1,
            }),
            service_meta,
            service_port: reg.effective_port(),
            service_enable_tag_override: reg.enable_tag_override.unwrap_or(false),
            create_index: index,
            modify_index: index,
        }
    }
}

/// Catalog node entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogNode {
    #[serde(rename = "ID")]
    pub id: String,

    #[serde(rename = "Node")]
    pub node: String,

    #[serde(rename = "Address")]
    pub address: String,

    #[serde(rename = "Datacenter")]
    pub datacenter: String,

    #[serde(rename = "TaggedAddresses", skip_serializing_if = "Option::is_none")]
    pub tagged_addresses: Option<HashMap<String, String>>,

    #[serde(rename = "Meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,

    #[serde(rename = "CreateIndex")]
    pub create_index: u64,

    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// Service kind
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceKind {
    Typical,
    ConnectProxy,
    ConnectGateway,
    ConnectSidecar,
    TerminatingGateway,
    IngressGateway,
    MeshGateway,
    ApiGateway,
    ConnectNative,
}

/// Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayConfig {
    #[serde(rename = "AssociatedServiceCount")]
    pub associated_service_count: i32,
}

/// Gateway-service association entry (response for /v1/catalog/gateway-services/:gateway)
#[derive(Debug, Clone, Serialize)]
pub struct GatewayServiceEntry {
    #[serde(rename = "Gateway")]
    pub gateway: GatewayServiceName,
    #[serde(rename = "Service")]
    pub service: GatewayServiceName,
    #[serde(rename = "GatewayKind")]
    pub gateway_kind: String,
    #[serde(rename = "Port", skip_serializing_if = "Option::is_none")]
    pub port: Option<i32>,
    #[serde(rename = "Protocol")]
    pub protocol: String,
    #[serde(rename = "Hosts", skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(rename = "FromWildcard")]
    pub from_wildcard: bool,
    #[serde(rename = "CreateIndex")]
    pub create_index: u64,
    #[serde(rename = "ModifyIndex")]
    pub modify_index: u64,
}

/// Service name within a gateway service entry
#[derive(Debug, Clone, Serialize)]
pub struct GatewayServiceName {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Namespace", skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(rename = "Partition", skip_serializing_if = "Option::is_none")]
    pub partition: Option<String>,
}

/// Service summary for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSummary {
    #[serde(rename = "Kind", skip_serializing_if = "Option::is_none")]
    pub kind: Option<ServiceKind>,

    #[serde(rename = "Name")]
    pub name: String,

    #[serde(rename = "Datacenter")]
    pub datacenter: String,

    #[serde(rename = "Tags")]
    pub tags: Vec<String>,

    #[serde(rename = "Nodes")]
    pub nodes: Vec<String>,

    #[serde(rename = "ExternalSources")]
    pub external_sources: Vec<String>,

    #[serde(rename = "InstanceCount")]
    pub instance_count: i32,

    #[serde(rename = "ChecksPassing")]
    pub checks_passing: i32,

    #[serde(rename = "ChecksWarning")]
    pub checks_warning: i32,

    #[serde(rename = "ChecksCritical")]
    pub checks_critical: i32,

    #[serde(rename = "GatewayConfig")]
    pub gateway_config: GatewayConfig,

    #[serde(rename = "TransparentProxy")]
    pub transparent_proxy: bool,
}

/// Service listing summary for UI (extends ServiceSummary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceListingSummary {
    #[serde(flatten)]
    pub service_summary: ServiceSummary,

    #[serde(rename = "ConnectedWithProxy")]
    pub connected_with_proxy: bool,

    #[serde(rename = "ConnectedWithGateway")]
    pub connected_with_gateway: bool,
}

/// Node detail with services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeServices {
    #[serde(rename = "Node")]
    pub node: CatalogNode,

    #[serde(rename = "Services")]
    pub services: HashMap<String, AgentService>,
}

/// Node detail with services as array (for /v1/catalog/node-services/:node)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeServiceList {
    #[serde(rename = "Node")]
    pub node: CatalogNode,

    #[serde(rename = "Services")]
    pub services: Vec<AgentService>,
}

/// Catalog registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(rename = "Node")]
    pub node: String,

    #[serde(rename = "Address")]
    pub address: String,

    #[serde(rename = "Datacenter", skip_serializing_if = "Option::is_none")]
    pub datacenter: Option<String>,

    #[serde(rename = "TaggedAddresses", skip_serializing_if = "Option::is_none")]
    pub tagged_addresses: Option<HashMap<String, String>>,

    #[serde(rename = "NodeMeta", skip_serializing_if = "Option::is_none")]
    pub node_meta: Option<HashMap<String, String>>,

    #[serde(rename = "Service", skip_serializing_if = "Option::is_none")]
    pub service: Option<CatalogServiceRegistration>,

    #[serde(rename = "Check", skip_serializing_if = "Option::is_none")]
    pub check: Option<CatalogCheck>,

    #[serde(rename = "Checks", skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<CatalogCheck>>,

    #[serde(rename = "SkipNodeUpdate", skip_serializing_if = "Option::is_none")]
    pub skip_node_update: Option<bool>,
}

/// Service registration in catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogServiceRegistration {
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(rename = "Service")]
    pub service: String,

    #[serde(rename = "Tags", skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(rename = "Address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,

    #[serde(rename = "Meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, String>>,

    #[serde(rename = "Port", skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    #[serde(rename = "Weights", skip_serializing_if = "Option::is_none")]
    pub weights: Option<Weights>,
}

/// Check in catalog registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogCheck {
    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,

    #[serde(rename = "Name")]
    pub name: String,

    #[serde(rename = "Status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
}

/// Catalog deregistration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogDeregistration {
    #[serde(rename = "Node")]
    pub node: String,

    #[serde(rename = "Datacenter", skip_serializing_if = "Option::is_none")]
    pub datacenter: Option<String>,

    #[serde(rename = "CheckID", skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,

    #[serde(rename = "ServiceID", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
}

/// Query parameters for catalog endpoints
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CatalogQueryParams {
    /// Filter by tag
    pub tag: Option<String>,

    /// Datacenter
    pub dc: Option<String>,

    /// Namespace (Enterprise)
    pub ns: Option<String>,

    /// Node metadata filter
    pub node_meta: Option<String>,

    /// Filter expression
    pub filter: Option<String>,

    /// Near node for sorting
    pub near: Option<String>,

    /// Blocking query: minimum index to wait for (X-Consul-Index)
    pub index: Option<u64>,

    /// Blocking query: maximum wait time (e.g. "5m", "30s")
    pub wait: Option<String>,
}

// ============================================================================
// Consul Catalog Service
// ============================================================================

/// Consul Catalog service
/// Provides catalog operations using ConsulNamingStore as backend
#[derive(Clone)]
pub struct ConsulCatalogService {
    naming_store: Arc<crate::naming_store::ConsulNamingStore>,
    node_name: String,
    datacenter: String,
    index_provider: ConsulIndexProvider,
}

impl ConsulCatalogService {
    pub fn new(naming_store: Arc<crate::naming_store::ConsulNamingStore>) -> Self {
        Self::with_datacenter(naming_store, "dc1".to_string())
    }

    pub fn with_datacenter(
        naming_store: Arc<crate::naming_store::ConsulNamingStore>,
        datacenter: String,
    ) -> Self {
        Self {
            naming_store,
            node_name: "batata-node".to_string(),
            datacenter,
            index_provider: ConsulIndexProvider::default(),
        }
    }

    pub fn with_index_provider(mut self, index_provider: ConsulIndexProvider) -> Self {
        self.index_provider = index_provider;
        self
    }

    /// Get all unique service names with their tags
    pub fn get_services(&self, namespace: &str) -> HashMap<String, Vec<String>> {
        self.naming_store.service_names_with_tags(namespace)
    }

    /// Get all instances for a service
    pub fn get_service_instances(
        &self,
        namespace: &str,
        service_name: &str,
        tag_filter: Option<&str>,
    ) -> Vec<CatalogService> {
        let index = self.index_provider.current_index(ConsulTable::Catalog);
        self.naming_store
            .get_service_entries(namespace, service_name)
            .into_iter()
            .filter_map(|entry_bytes| {
                serde_json::from_slice::<crate::model::AgentServiceRegistration>(&entry_bytes).ok()
            })
            .filter(|reg| {
                if let Some(tag) = tag_filter {
                    reg.tags
                        .as_ref()
                        .map(|tags| tags.contains(&tag.to_string()))
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .map(|reg| {
                CatalogService::from_registration(&reg, &self.node_name, &self.datacenter, index)
            })
            .collect()
    }

    /// Get all nodes (for simplicity, we return one node per unique IP)
    pub fn get_nodes(&self, namespace: &str) -> Vec<CatalogNode> {
        let index = self.index_provider.current_index(ConsulTable::Catalog);
        let mut nodes: HashMap<String, CatalogNode> = HashMap::new();

        for service_name in self.naming_store.service_names(namespace) {
            for entry_bytes in self
                .naming_store
                .get_service_entries(namespace, &service_name)
            {
                let Ok(reg) =
                    serde_json::from_slice::<crate::model::AgentServiceRegistration>(&entry_bytes)
                else {
                    continue;
                };
                let ip = reg.effective_address();
                if ip.is_empty() {
                    continue;
                }
                if let std::collections::hash_map::Entry::Vacant(e) = nodes.entry(ip.clone()) {
                    e.insert(CatalogNode {
                        id: uuid::Uuid::new_v4().to_string(),
                        node: format!("node-{}", ip.replace('.', "-")),
                        address: ip,
                        datacenter: self.datacenter.clone(),
                        tagged_addresses: None,
                        meta: None,
                        create_index: index,
                        modify_index: index,
                    });
                }
            }
        }

        nodes.into_values().collect()
    }

    /// Get node details with services
    pub fn get_node(&self, namespace: &str, node_name: &str) -> Option<NodeServices> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());

        let index = self.index_provider.current_index(ConsulTable::Catalog);
        let mut node: Option<CatalogNode> = None;
        let mut services: HashMap<String, AgentService> = HashMap::new();

        for service_name in self.naming_store.service_names(namespace) {
            for entry_bytes in self
                .naming_store
                .get_service_entries(namespace, &service_name)
            {
                let Ok(reg) =
                    serde_json::from_slice::<crate::model::AgentServiceRegistration>(&entry_bytes)
                else {
                    continue;
                };
                let ip = reg.effective_address();
                let instance_node = format!("node-{}", ip.replace('.', "-"));

                // Match by node name formats: hostname, node-{ip}, raw IP, or "batata-node"
                if instance_node == node_name
                    || ip == node_name
                    || hostname == node_name
                    || node_name == "batata-node"
                {
                    if node.is_none() {
                        node = Some(CatalogNode {
                            id: uuid::Uuid::new_v4().to_string(),
                            node: node_name.to_string(),
                            address: ip,
                            datacenter: self.datacenter.clone(),
                            tagged_addresses: None,
                            meta: None,
                            create_index: index,
                            modify_index: index,
                        });
                    }

                    let agent_service = AgentService::from(&reg);
                    services.insert(agent_service.id.clone(), agent_service);
                }
            }
        }

        // If hostname matches but no services were found, still return the node
        if node.is_none() && (hostname == node_name || node_name == "batata-node") {
            node = Some(CatalogNode {
                id: uuid::Uuid::new_v4().to_string(),
                node: node_name.to_string(),
                address: "127.0.0.1".to_string(),
                datacenter: self.datacenter.clone(),
                tagged_addresses: None,
                meta: None,
                create_index: index,
                modify_index: index,
            });
        }

        node.map(|n| NodeServices { node: n, services })
    }

    /// Register a service via catalog
    pub fn register(&self, registration: &CatalogRegistration, namespace: &str) -> bool {
        if let Some(ref svc) = registration.service {
            let service_id = svc.id.clone().unwrap_or_else(|| svc.service.clone());
            let address = svc
                .address
                .clone()
                .unwrap_or_else(|| registration.address.clone());

            // Build AgentServiceRegistration from CatalogServiceRegistration
            let reg = crate::model::AgentServiceRegistration {
                name: svc.service.clone(),
                id: Some(service_id.clone()),
                tags: svc.tags.clone(),
                address: Some(address),
                port: svc.port,
                meta: svc.meta.clone(),
                weights: svc.weights.clone(),
                check: None,
                checks: None,
                kind: None,
                proxy: None,
                connect: None,
                enable_tag_override: None,
                tagged_addresses: None,
                namespace: None,
            };

            let data = match serde_json::to_vec(&reg) {
                Ok(v) => bytes::Bytes::from(v),
                Err(_) => return false,
            };

            let key = crate::naming_store::ConsulNamingStore::build_key(
                namespace,
                &svc.service,
                &service_id,
            );
            use batata_plugin::PluginNamingStore;
            self.naming_store.register(&key, data).is_ok()
        } else {
            // Just node registration, we don't track nodes separately
            true
        }
    }

    /// Deregister a service via catalog
    pub fn deregister(&self, deregistration: &CatalogDeregistration, namespace: &str) -> bool {
        if let Some(ref service_id) = deregistration.service_id {
            // Deregister by service_id
            self.naming_store
                .remove_by_service_id(namespace, service_id)
        } else {
            // Node deregistration - remove all services on this node
            let node = &deregistration.node;
            let mut deregistered = false;

            for service_name in self.naming_store.service_names(namespace) {
                for entry_bytes in self
                    .naming_store
                    .get_service_entries(namespace, &service_name)
                {
                    let Ok(reg) = serde_json::from_slice::<crate::model::AgentServiceRegistration>(
                        &entry_bytes,
                    ) else {
                        continue;
                    };
                    let ip = reg.effective_address();
                    let instance_node = format!("node-{}", ip.replace('.', "-"));
                    if &instance_node == node || &ip == node {
                        self.naming_store
                            .remove_by_service_id(namespace, &reg.service_id());
                        deregistered = true;
                    }
                }
            }

            deregistered
        }
    }

    /// Get service summary for UI
    /// Returns a list of service summaries with health check information
    pub fn get_service_summary(&self, namespace: &str) -> Vec<ServiceListingSummary> {
        let mut summaries: Vec<ServiceListingSummary> = Vec::new();

        for service_name in self.naming_store.service_names(namespace) {
            let regs: Vec<crate::model::AgentServiceRegistration> = self
                .naming_store
                .get_service_entries(namespace, &service_name)
                .into_iter()
                .filter_map(|b| serde_json::from_slice(&b).ok())
                .collect();

            if regs.is_empty() {
                continue;
            }

            let mut all_tags: Vec<String> = Vec::new();
            let mut all_nodes: Vec<String> = Vec::new();
            let mut external_sources: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut checks_passing = 0;
            let checks_warning = 0;
            let mut checks_critical = 0;

            for reg in &regs {
                // Collect tags
                if let Some(ref tags) = reg.tags {
                    for tag in tags {
                        if !all_tags.contains(tag) {
                            all_tags.push(tag.clone());
                        }
                    }
                }

                // Collect nodes (unique)
                let ip = reg.effective_address();
                let node_name = format!("node-{}", ip.replace('.', "-"));
                if !all_nodes.contains(&node_name) {
                    all_nodes.push(node_name);
                }

                // Collect external sources from meta
                if let Some(ext) = reg
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("external_source"))
                    .cloned()
                {
                    external_sources.insert(ext);
                }

                // Count health checks based on store health status
                if self
                    .naming_store
                    .is_healthy(&ip, reg.effective_port() as i32)
                {
                    checks_passing += 1;
                } else {
                    checks_critical += 1;
                }
            }

            // Detect service kind from registrations
            let kind = Self::detect_service_kind(&regs);
            let has_proxy = regs
                .iter()
                .any(|r| r.kind.as_deref() == Some("connect-proxy"));
            let has_connect = regs.iter().any(|r| {
                r.connect
                    .as_ref()
                    .and_then(|v| v.get("Native").and_then(|n| n.as_bool()))
                    .unwrap_or(false)
            });
            let transparent_proxy = regs.iter().any(|r| {
                r.proxy
                    .as_ref()
                    .and_then(|v| {
                        v.get("Mode")
                            .and_then(|m| m.as_str())
                            .map(|m| m == "transparent")
                    })
                    .unwrap_or(false)
            });

            summaries.push(ServiceListingSummary {
                service_summary: ServiceSummary {
                    kind,
                    name: service_name.clone(),
                    datacenter: self.datacenter.clone(),
                    tags: all_tags,
                    nodes: all_nodes,
                    external_sources: external_sources.into_iter().collect(),
                    instance_count: regs.len() as i32,
                    checks_passing,
                    checks_warning,
                    checks_critical,
                    gateway_config: GatewayConfig::default(),
                    transparent_proxy,
                },
                connected_with_proxy: has_proxy,
                connected_with_gateway: has_connect,
            });
        }

        // Sort by name for consistent output
        summaries.sort_by(|a, b| a.service_summary.name.cmp(&b.service_summary.name));

        summaries
    }

    /// Detect service kind from the first registration's kind field
    fn detect_service_kind(regs: &[crate::model::AgentServiceRegistration]) -> Option<ServiceKind> {
        for reg in regs {
            if let Some(ref kind_str) = reg.kind {
                return match kind_str.as_str() {
                    "connect-proxy" => Some(ServiceKind::ConnectProxy),
                    "mesh-gateway" => Some(ServiceKind::MeshGateway),
                    "ingress-gateway" => Some(ServiceKind::IngressGateway),
                    "terminating-gateway" => Some(ServiceKind::TerminatingGateway),
                    "api-gateway" => Some(ServiceKind::ApiGateway),
                    _ => None,
                };
            }
        }
        None
    }

    /// Get Connect-enabled service instances for a target service.
    /// Returns connect-proxy instances targeting this service, or native-connect instances.
    pub fn get_connect_service_instances(
        &self,
        namespace: &str,
        target_service: &str,
    ) -> Vec<CatalogService> {
        let index = self.index_provider.current_index(ConsulTable::Catalog);
        let mut results = Vec::new();

        for service_name in self.naming_store.service_names(namespace) {
            for entry_bytes in self
                .naming_store
                .get_service_entries(namespace, &service_name)
            {
                let Ok(reg) =
                    serde_json::from_slice::<crate::model::AgentServiceRegistration>(&entry_bytes)
                else {
                    continue;
                };
                if Self::is_connect_registration_for(&reg, target_service) {
                    results.push(CatalogService::from_registration(
                        &reg,
                        &self.node_name,
                        &self.datacenter,
                        index,
                    ));
                }
            }
        }

        results
    }

    /// Check if a registration is a Connect-enabled instance for the given target service.
    /// A service is Connect-enabled if:
    /// 1. It's a connect-proxy with DestinationServiceName matching the target
    /// 2. It has Connect.Native=true and the service name matches
    fn is_connect_registration_for(
        reg: &crate::model::AgentServiceRegistration,
        target_service: &str,
    ) -> bool {
        // Check if it's a connect-proxy targeting this service
        if reg.kind.as_deref() == Some("connect-proxy") {
            if let Some(proxy_val) = &reg.proxy {
                if let Some(dest) = proxy_val
                    .get("DestinationServiceName")
                    .or_else(|| proxy_val.get("destination_service_name"))
                    .and_then(|v| v.as_str())
                {
                    return dest == target_service;
                }
            }
        }

        // Check if it's a native Connect service with matching name
        if reg.name == target_service {
            if let Some(connect_val) = &reg.connect {
                if connect_val
                    .get("Native")
                    .or_else(|| connect_val.get("native"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }

        false
    }

    /// Get gateway service entries from config entries (ingress-gateway, terminating-gateway)
    pub fn get_gateway_services_from_config(
        &self,
        gateway_name: &str,
        config_entry_service: &ConsulConfigEntryService,
    ) -> Vec<GatewayServiceEntry> {
        let mut entries = Vec::new();

        // Check ingress-gateway config entries
        if let Some(entry) = config_entry_service.get_entry("ingress-gateway", gateway_name) {
            entries.extend(Self::extract_gateway_services(
                gateway_name,
                "ingress-gateway",
                &entry.extra,
                entry.create_index,
                entry.modify_index,
            ));
        }

        // Check terminating-gateway config entries
        if let Some(entry) = config_entry_service.get_entry("terminating-gateway", gateway_name) {
            entries.extend(Self::extract_gateway_services(
                gateway_name,
                "terminating-gateway",
                &entry.extra,
                entry.create_index,
                entry.modify_index,
            ));
        }

        // Check api-gateway config entries
        if let Some(entry) = config_entry_service.get_entry("api-gateway", gateway_name) {
            entries.extend(Self::extract_gateway_services(
                gateway_name,
                "api-gateway",
                &entry.extra,
                entry.create_index,
                entry.modify_index,
            ));
        }

        entries
    }

    /// Extract service associations from a gateway config entry
    fn extract_gateway_services(
        gateway_name: &str,
        gateway_kind: &str,
        extra: &HashMap<String, serde_json::Value>,
        create_index: u64,
        modify_index: u64,
    ) -> Vec<GatewayServiceEntry> {
        let mut entries = Vec::new();

        // For ingress-gateway, services are in "Listeners[].Services[]"
        if gateway_kind == "ingress-gateway"
            && let Some(listeners) = extra.get("Listeners").and_then(|v| v.as_array())
        {
            for listener in listeners {
                let protocol = listener
                    .get("Protocol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tcp")
                    .to_string();
                let port = listener
                    .get("Port")
                    .and_then(|v| v.as_i64())
                    .map(|p| p as i32);

                if let Some(services) = listener.get("Services").and_then(|v| v.as_array()) {
                    for svc in services {
                        let svc_name = svc
                            .get("Name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if svc_name.is_empty() {
                            continue;
                        }
                        let hosts: Option<Vec<String>> = svc
                            .get("Hosts")
                            .and_then(|v| serde_json::from_value(v.clone()).ok());
                        let from_wildcard = svc_name == "*";

                        entries.push(GatewayServiceEntry {
                            gateway: GatewayServiceName {
                                name: gateway_name.to_string(),
                                namespace: None,
                                partition: None,
                            },
                            service: GatewayServiceName {
                                name: svc_name,
                                namespace: None,
                                partition: None,
                            },
                            gateway_kind: gateway_kind.to_string(),
                            port,
                            protocol: protocol.clone(),
                            hosts,
                            from_wildcard,
                            create_index,
                            modify_index,
                        });
                    }
                }
            }
        }

        // For terminating-gateway, services are in "Services[]"
        if gateway_kind == "terminating-gateway"
            && let Some(services) = extra.get("Services").and_then(|v| v.as_array())
        {
            for svc in services {
                let svc_name = svc
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if svc_name.is_empty() {
                    continue;
                }
                entries.push(GatewayServiceEntry {
                    gateway: GatewayServiceName {
                        name: gateway_name.to_string(),
                        namespace: None,
                        partition: None,
                    },
                    service: GatewayServiceName {
                        name: svc_name,
                        namespace: None,
                        partition: None,
                    },
                    gateway_kind: gateway_kind.to_string(),
                    port: None,
                    protocol: "tcp".to_string(),
                    hosts: None,
                    from_wildcard: false,
                    create_index,
                    modify_index,
                });
            }
        }

        // For api-gateway, services are in "Listeners[]"
        if gateway_kind == "api-gateway"
            && let Some(listeners) = extra.get("Listeners").and_then(|v| v.as_array())
        {
            for listener in listeners {
                let protocol = listener
                    .get("Protocol")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tcp")
                    .to_string();
                let port = listener
                    .get("Port")
                    .and_then(|v| v.as_i64())
                    .map(|p| p as i32);
                let listener_name = listener
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");

                entries.push(GatewayServiceEntry {
                    gateway: GatewayServiceName {
                        name: gateway_name.to_string(),
                        namespace: None,
                        partition: None,
                    },
                    service: GatewayServiceName {
                        name: listener_name.to_string(),
                        namespace: None,
                        partition: None,
                    },
                    gateway_kind: gateway_kind.to_string(),
                    port,
                    protocol,
                    hosts: None,
                    from_wildcard: false,
                    create_index,
                    modify_index,
                });
            }
        }

        entries
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// GET /v1/catalog/services
/// Returns a list of all known services
pub async fn list_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let dc = dc_config.resolve_dc(&query.dc);
    let namespace = dc_config.resolve_ns(&query.ns);

    // Handle blocking query wait
    maybe_block(&index_provider, query.index, query.wait.as_deref()).await;

    let services = catalog.get_services(&namespace);
    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .insert_header(("X-Consul-Effective-Datacenter", dc))
        .json(services)
}

/// GET /v1/catalog/service/:service
/// Returns the nodes providing a specific service
pub async fn get_service(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);
    let tag_filter = query.tag.as_deref();
    let dc = dc_config.resolve_dc(&query.dc);

    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Handle blocking query wait
    maybe_block(&index_provider, query.index, query.wait.as_deref()).await;

    let mut services = catalog.get_service_instances(&namespace, &service_name, tag_filter);

    // Override datacenter with resolved DC from query params
    for svc in &mut services {
        svc.datacenter = dc.clone();
    }

    if services.is_empty() {
        HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .json(Vec::<CatalogService>::new())
    } else {
        HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .json(services)
    }
}

/// GET /v1/catalog/nodes
/// Returns a list of all known nodes
pub async fn list_nodes(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for node read
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let dc = dc_config.resolve_dc(&query.dc);
    let namespace = dc_config.resolve_ns(&query.ns);
    let mut nodes = catalog.get_nodes(&namespace);

    // Override datacenter with resolved DC from query params
    for node in &mut nodes {
        node.datacenter = dc.clone();
    }

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(nodes)
}

/// GET /v1/catalog/node/:node
/// Returns the node's services
pub async fn get_node(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let node_name = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);

    // Check ACL authorization for node read
    let authz = acl_service.authorize_request(&req, ResourceType::Node, &node_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match catalog.get_node(&namespace, &node_name) {
        Some(node_services) => HttpResponse::Ok()
            .insert_header((
                "X-Consul-Index",
                index_provider
                    .current_index(ConsulTable::Catalog)
                    .to_string(),
            ))
            .json(node_services),
        None => HttpResponse::NotFound()
            .json(ConsulError::new(format!("Node not found: {}", node_name))),
    }
}

/// PUT /v1/catalog/register
/// Register a node, service, or check
pub async fn register(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    body: web::Json<CatalogRegistration>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let namespace = dc_config.resolve_ns(&query.ns);
    let mut registration = body.into_inner();

    // Use resolved DC if registration doesn't specify one
    if registration.datacenter.is_none() {
        registration.datacenter = Some(dc_config.resolve_dc(&query.dc));
    }

    // Check ACL authorization for service write
    let service_name = registration
        .service
        .as_ref()
        .map(|s| s.service.as_str())
        .unwrap_or("");
    let authz = acl_service.authorize_request(&req, ResourceType::Service, service_name, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    if catalog.register(&registration, &namespace) {
        let idx = index_provider.increment(ConsulTable::Catalog);
        HttpResponse::Ok()
            .insert_header(("X-Consul-Index", idx.to_string()))
            .json(true)
    } else {
        HttpResponse::InternalServerError().json(ConsulError::new("Registration failed"))
    }
}

/// PUT /v1/catalog/deregister
/// Deregister a node, service, or check
pub async fn deregister(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    body: web::Json<CatalogDeregistration>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let namespace = dc_config.resolve_ns(&query.ns);
    let deregistration = body.into_inner();

    // Check ACL authorization for service/node write
    let resource = deregistration
        .service_id
        .as_deref()
        .or(Some(&deregistration.node))
        .unwrap_or("");
    let resource_type = if deregistration.service_id.is_some() {
        ResourceType::Service
    } else {
        ResourceType::Node
    };
    let authz = acl_service.authorize_request(&req, resource_type, resource, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    catalog.deregister(&deregistration, &namespace);
    let idx = index_provider.increment(ConsulTable::Catalog);
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", idx.to_string()))
        .json(true)
}

/// GET /v1/catalog/datacenters
/// Returns a list of all known datacenters, including those known from peering relationships
pub async fn list_datacenters(
    dc_config: web::Data<ConsulDatacenterConfig>,
    peering_service: web::Data<crate::peering::ConsulPeeringService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let mut datacenters = vec![dc_config.datacenter.clone()];

    // Add datacenters known from peering relationships
    for peering in peering_service.list_peerings() {
        if !peering.remote.datacenter.is_empty()
            && !datacenters.contains(&peering.remote.datacenter)
        {
            datacenters.push(peering.remote.datacenter.clone());
        }
    }

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(datacenters)
}

/// GET /v1/internal/ui/services
/// Returns service summary for UI
pub async fn ui_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let namespace = dc_config.resolve_ns(&query.ns);
    let summaries = catalog.get_service_summary(&namespace);

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(summaries)
}

/// GET /v1/catalog/connect/:service
/// Returns Connect-enabled service instances (connect-proxy targeting this service, or native connect)
pub async fn get_connect_service(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);
    let dc = dc_config.resolve_dc(&query.dc);

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let mut services = catalog.get_connect_service_instances(&namespace, &service_name);

    // Override datacenter with resolved DC from query params
    for svc in &mut services {
        svc.datacenter = dc.clone();
    }

    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(services)
}

/// GET /v1/catalog/node-services/:node
/// Returns the services for a specific node (array format)
pub async fn get_node_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let node_name = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);
    let dc = dc_config.resolve_dc(&query.dc);

    let authz = acl_service.authorize_request(&req, ResourceType::Node, &node_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match catalog.get_node(&namespace, &node_name) {
        Some(node_services) => {
            let mut node = node_services.node;
            // Override datacenter with resolved DC from query params
            node.datacenter = dc;
            let list = NodeServiceList {
                node,
                services: node_services.services.into_values().collect(),
            };
            HttpResponse::Ok()
                .insert_header((
                    "X-Consul-Index",
                    index_provider
                        .current_index(ConsulTable::Catalog)
                        .to_string(),
                ))
                .json(list)
        }
        None => HttpResponse::NotFound()
            .json(ConsulError::new(format!("Node not found: {}", node_name))),
    }
}

/// GET /v1/catalog/gateway-services/:gateway
/// Returns services associated with a gateway (ingress, terminating, mesh, api)
pub async fn get_gateway_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    catalog: web::Data<ConsulCatalogService>,
    config_entry_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let gateway_name = path.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &gateway_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let gateway_services =
        catalog.get_gateway_services_from_config(&gateway_name, &config_entry_service);
    HttpResponse::Ok()
        .insert_header((
            "X-Consul-Index",
            index_provider
                .current_index(ConsulTable::Catalog)
                .to_string(),
        ))
        .json(gateway_services)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentServiceRegistration;
    use crate::naming_store::ConsulNamingStore;
    use batata_plugin::PluginNamingStore;

    fn make_store() -> Arc<ConsulNamingStore> {
        Arc::new(ConsulNamingStore::new())
    }

    fn register_reg(store: &ConsulNamingStore, reg: &AgentServiceRegistration) {
        let service_id = reg.service_id();
        let key = ConsulNamingStore::build_key(
            crate::namespace::DEFAULT_NAMESPACE,
            &reg.name,
            &service_id,
        );
        let data = bytes::Bytes::from(serde_json::to_vec(reg).unwrap());
        store.register(&key, data).unwrap();
    }

    fn simple_reg(name: &str, id: &str, ip: &str, port: u16) -> AgentServiceRegistration {
        AgentServiceRegistration {
            name: name.to_string(),
            id: Some(id.to_string()),
            tags: None,
            address: Some(ip.to_string()),
            port: Some(port),
            meta: None,
            weights: None,
            check: None,
            checks: None,
            kind: None,
            proxy: None,
            connect: None,
            enable_tag_override: None,
            tagged_addresses: None,
            namespace: None,
        }
    }

    #[test]
    fn test_catalog_service_from_registration() {
        let reg = AgentServiceRegistration {
            name: "web".to_string(),
            id: Some("web-1".to_string()),
            tags: Some(vec!["http".to_string(), "api".to_string()]),
            address: Some("192.168.1.100".to_string()),
            port: Some(8080),
            meta: None,
            weights: None,
            check: None,
            checks: None,
            kind: None,
            proxy: None,
            connect: None,
            enable_tag_override: None,
            tagged_addresses: None,
            namespace: None,
        };

        let catalog_service = CatalogService::from_registration(&reg, "node1", "dc1", 1);

        assert_eq!(catalog_service.service_name, "web");
        assert_eq!(catalog_service.service_port, 8080);
        assert_eq!(catalog_service.service_address, "192.168.1.100");
        assert_eq!(catalog_service.service_id, "web-1");
        assert!(catalog_service.service_tags.is_some());
        assert_eq!(catalog_service.service_tags.unwrap().len(), 2);
    }

    #[test]
    fn test_catalog_service_operations() {
        let store = make_store();
        let catalog = ConsulCatalogService::new(store);

        // Register a service via catalog
        let registration = CatalogRegistration {
            id: None,
            node: "node1".to_string(),
            address: "192.168.1.100".to_string(),
            datacenter: Some("dc1".to_string()),
            tagged_addresses: None,
            node_meta: None,
            service: Some(CatalogServiceRegistration {
                id: Some("web-1".to_string()),
                service: "web".to_string(),
                tags: Some(vec!["http".to_string()]),
                address: Some("192.168.1.100".to_string()),
                meta: None,
                port: Some(8080),
                weights: None,
            }),
            check: None,
            checks: None,
            skip_node_update: None,
        };

        assert!(catalog.register(&registration, "public"));

        // Get services
        let services = catalog.get_services("public");
        assert!(services.contains_key("web"));

        // Get service instances
        let instances = catalog.get_service_instances("public", "web", None);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].service_id, "web-1");

        // Deregister
        let deregistration = CatalogDeregistration {
            node: "node1".to_string(),
            datacenter: None,
            check_id: None,
            service_id: Some("web-1".to_string()),
        };

        assert!(catalog.deregister(&deregistration, "public"));

        // Verify deregistered
        let instances = catalog.get_service_instances("public", "web", None);
        assert!(instances.is_empty());
    }

    #[test]
    fn test_catalog_nodes() {
        let store = make_store();
        let catalog = ConsulCatalogService::new(store.clone());

        // Register services on different IPs
        register_reg(&store, &simple_reg("web", "web-1", "192.168.1.100", 8080));
        register_reg(&store, &simple_reg("web", "web-2", "192.168.1.101", 8080));

        // Get nodes
        let nodes = catalog.get_nodes("default");
        assert_eq!(nodes.len(), 2);

        // Verify node names (generated as "node-{ip with dots replaced by dashes}") and addresses
        let node_names: Vec<&str> = nodes.iter().map(|n| n.node.as_str()).collect();
        assert!(node_names.contains(&"node-192-168-1-100"));
        assert!(node_names.contains(&"node-192-168-1-101"));

        let node1 = nodes
            .iter()
            .find(|n| n.node == "node-192-168-1-100")
            .unwrap();
        assert_eq!(node1.address, "192.168.1.100");

        let node2 = nodes
            .iter()
            .find(|n| n.node == "node-192-168-1-101")
            .unwrap();
        assert_eq!(node2.address, "192.168.1.101");
    }

    #[test]
    fn test_service_summary() {
        let store = make_store();
        let catalog = ConsulCatalogService::new(store.clone());

        // Register a healthy web service (default: healthy in store)
        let web_reg = AgentServiceRegistration {
            name: "web".to_string(),
            id: Some("web-1".to_string()),
            tags: Some(vec!["http".to_string(), "api".to_string()]),
            address: Some("192.168.1.100".to_string()),
            port: Some(8080),
            meta: None,
            weights: None,
            check: None,
            checks: None,
            kind: None,
            proxy: None,
            connect: None,
            enable_tag_override: None,
            tagged_addresses: None,
            namespace: None,
        };
        register_reg(&store, &web_reg);

        // Register a db service and mark it unhealthy
        let db_reg = AgentServiceRegistration {
            name: "db".to_string(),
            id: Some("db-1".to_string()),
            tags: Some(vec!["db".to_string()]),
            address: Some("192.168.1.101".to_string()),
            port: Some(3306),
            meta: None,
            weights: None,
            check: None,
            checks: None,
            kind: None,
            proxy: None,
            connect: None,
            enable_tag_override: None,
            tagged_addresses: None,
            namespace: None,
        };
        register_reg(&store, &db_reg);
        // Mark db instance as unhealthy
        store.update_health("192.168.1.101", 3306, false);

        // Get service summary
        let summaries = catalog.get_service_summary("default");
        assert_eq!(summaries.len(), 2);

        // Find the web service summary
        let web_summary = summaries
            .iter()
            .find(|s| s.service_summary.name == "web")
            .unwrap();
        assert_eq!(web_summary.service_summary.datacenter, "dc1");
        assert_eq!(
            web_summary.service_summary.nodes,
            vec!["node-192-168-1-100"]
        );
        assert_eq!(web_summary.service_summary.instance_count, 1);
        assert_eq!(web_summary.service_summary.checks_passing, 1);
        assert_eq!(web_summary.service_summary.checks_critical, 0);
        assert_eq!(
            web_summary.service_summary.tags,
            vec!["http".to_string(), "api".to_string()]
        );
        assert!(!web_summary.connected_with_proxy);
        assert!(!web_summary.connected_with_gateway);

        // Find the db service summary
        let db_summary = summaries
            .iter()
            .find(|s| s.service_summary.name == "db")
            .unwrap();
        assert_eq!(db_summary.service_summary.instance_count, 1);
        assert_eq!(db_summary.service_summary.checks_passing, 0);
        assert_eq!(db_summary.service_summary.checks_critical, 1);
    }
}
