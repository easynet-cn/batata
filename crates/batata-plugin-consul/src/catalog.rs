// Consul Catalog API HTTP handlers
// Implements Consul-compatible service catalog endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde::{Deserialize, Serialize};

use batata_api::naming::model::Instance as NacosInstance;
use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::model::{AgentService, ConsulError, Weights};

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
    /// Create from a Nacos Instance
    pub fn from_instance(instance: &NacosInstance, node_name: &str, datacenter: &str) -> Self {
        let tags = instance
            .metadata
            .get("consul_tags")
            .and_then(|s| serde_json::from_str(s).ok());

        let enable_tag_override = instance
            .metadata
            .get("enable_tag_override")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        // Filter out Consul-specific metadata
        let service_meta: HashMap<String, String> = instance
            .metadata
            .iter()
            .filter(|(k, _)| !k.starts_with("consul_") && k.as_str() != "enable_tag_override")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            node: node_name.to_string(),
            address: instance.ip.clone(),
            datacenter: datacenter.to_string(),
            tagged_addresses: None,
            node_meta: None,
            service_kind: None,
            service_id: instance.instance_id.clone(),
            service_name: instance.service_name.clone(),
            service_tags: tags,
            service_address: instance.ip.clone(),
            service_weights: Weights {
                passing: instance.weight as i32,
                warning: 1,
            },
            service_meta: if service_meta.is_empty() {
                None
            } else {
                Some(service_meta)
            },
            service_port: instance.port as u16,
            service_enable_tag_override: enable_tag_override,
            create_index: 1,
            modify_index: 1,
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
}

// ============================================================================
// Consul Catalog Service
// ============================================================================

/// Consul Catalog service
/// Provides catalog operations using NamingService as backend
#[derive(Clone)]
pub struct ConsulCatalogService {
    naming_service: Arc<NamingService>,
    node_name: String,
    datacenter: String,
}

impl ConsulCatalogService {
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self {
            naming_service,
            node_name: "batata-node".to_string(),
            datacenter: "dc1".to_string(),
        }
    }

    /// Get all unique service names with their tags
    pub fn get_services(&self, namespace: &str) -> HashMap<String, Vec<String>> {
        let (_, service_names) =
            self.naming_service
                .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

        let mut services: HashMap<String, Vec<String>> = HashMap::new();

        for service_name in service_names {
            let instances = self.naming_service.get_instances(
                namespace,
                "DEFAULT_GROUP",
                &service_name,
                "",
                false,
            );

            // Collect all unique tags for this service
            let mut all_tags: Vec<String> = Vec::new();
            for instance in instances {
                if let Some(tags_json) = instance.metadata.get("consul_tags")
                    && let Ok(tags) = serde_json::from_str::<Vec<String>>(tags_json)
                {
                    for tag in tags {
                        if !all_tags.contains(&tag) {
                            all_tags.push(tag);
                        }
                    }
                }
            }

            services.insert(service_name, all_tags);
        }

        services
    }

    /// Get all instances for a service
    pub fn get_service_instances(
        &self,
        namespace: &str,
        service_name: &str,
        tag_filter: Option<&str>,
    ) -> Vec<CatalogService> {
        let instances =
            self.naming_service
                .get_instances(namespace, "DEFAULT_GROUP", service_name, "", false);

        instances
            .iter()
            .filter(|inst| {
                if let Some(tag) = tag_filter {
                    inst.metadata
                        .get("consul_tags")
                        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                        .map(|tags| tags.contains(&tag.to_string()))
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .map(|inst| CatalogService::from_instance(inst, &self.node_name, &self.datacenter))
            .collect()
    }

    /// Get all nodes (for simplicity, we return one node per unique IP)
    pub fn get_nodes(&self, namespace: &str) -> Vec<CatalogNode> {
        let (_, service_names) =
            self.naming_service
                .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

        let mut nodes: HashMap<String, CatalogNode> = HashMap::new();

        for service_name in service_names {
            let instances = self.naming_service.get_instances(
                namespace,
                "DEFAULT_GROUP",
                &service_name,
                "",
                false,
            );

            for instance in instances {
                let node_key = instance.ip.clone();
                if let std::collections::hash_map::Entry::Vacant(e) = nodes.entry(node_key) {
                    e.insert(CatalogNode {
                        id: uuid::Uuid::new_v4().to_string(),
                        node: format!("node-{}", instance.ip.replace('.', "-")),
                        address: instance.ip.clone(),
                        datacenter: self.datacenter.clone(),
                        tagged_addresses: None,
                        meta: None,
                        create_index: 1,
                        modify_index: 1,
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

        let (_, service_names) =
            self.naming_service
                .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

        let mut node: Option<CatalogNode> = None;
        let mut services: HashMap<String, AgentService> = HashMap::new();

        for service_name in service_names {
            let instances = self.naming_service.get_instances(
                namespace,
                "DEFAULT_GROUP",
                &service_name,
                "",
                false,
            );

            for instance in instances {
                let instance_node = format!("node-{}", instance.ip.replace('.', "-"));

                // Match by node name formats: hostname, node-{ip}, raw IP, or "batata-node"
                if instance_node == node_name
                    || instance.ip == node_name
                    || hostname == node_name
                    || node_name == "batata-node"
                {
                    // Found a service on this node
                    if node.is_none() {
                        node = Some(CatalogNode {
                            id: uuid::Uuid::new_v4().to_string(),
                            node: node_name.to_string(),
                            address: instance.ip.clone(),
                            datacenter: self.datacenter.clone(),
                            tagged_addresses: None,
                            meta: None,
                            create_index: 1,
                            modify_index: 1,
                        });
                    }

                    let agent_service = AgentService::from(&instance);
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
                create_index: 1,
                modify_index: 1,
            });
        }

        node.map(|n| NodeServices { node: n, services })
    }

    /// Register a service via catalog
    pub fn register(&self, registration: &CatalogRegistration, namespace: &str) -> bool {
        if let Some(ref service) = registration.service {
            let service_id = service
                .id
                .clone()
                .unwrap_or_else(|| service.service.clone());

            let address = service
                .address
                .clone()
                .unwrap_or_else(|| registration.address.clone());

            let port = service.port.unwrap_or(0);

            let weight = service
                .weights
                .as_ref()
                .map(|w| w.passing as f64)
                .unwrap_or(1.0);

            let mut metadata: HashMap<String, String> = service.meta.clone().unwrap_or_default();

            // Store tags in metadata
            if let Some(ref tags) = service.tags {
                metadata.insert(
                    "consul_tags".to_string(),
                    serde_json::to_string(tags).unwrap_or_default(),
                );
            }

            let instance = NacosInstance {
                instance_id: service_id,
                ip: address,
                port: port as i32,
                weight,
                healthy: true,
                enabled: true,
                ephemeral: true,
                cluster_name: "DEFAULT".to_string(),
                service_name: service.service.clone(),
                metadata,
                instance_heart_beat_interval: 5000,
                instance_heart_beat_time_out: 15000,
                ip_delete_timeout: 30000,
                instance_id_generator: "simple".to_string(),
            };

            self.naming_service.register_instance(
                namespace,
                "DEFAULT_GROUP",
                &service.service,
                instance,
            )
        } else {
            // Just node registration, we don't track nodes separately
            true
        }
    }

    /// Deregister a service via catalog
    pub fn deregister(&self, deregistration: &CatalogDeregistration, namespace: &str) -> bool {
        if let Some(ref service_id) = deregistration.service_id {
            // Find and deregister the service
            let (_, service_names) =
                self.naming_service
                    .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

            for service_name in service_names {
                let instances = self.naming_service.get_instances(
                    namespace,
                    "DEFAULT_GROUP",
                    &service_name,
                    "",
                    false,
                );

                for instance in instances {
                    if &instance.instance_id == service_id {
                        return self.naming_service.deregister_instance(
                            namespace,
                            "DEFAULT_GROUP",
                            &service_name,
                            &instance,
                        );
                    }
                }
            }
            false
        } else {
            // Node deregistration - remove all services on this node
            let node = &deregistration.node;
            let (_, service_names) =
                self.naming_service
                    .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

            let mut deregistered = false;

            for service_name in service_names {
                let instances = self.naming_service.get_instances(
                    namespace,
                    "DEFAULT_GROUP",
                    &service_name,
                    "",
                    false,
                );

                for instance in instances {
                    let instance_node = format!("node-{}", instance.ip.replace('.', "-"));
                    if &instance_node == node || &instance.ip == node {
                        self.naming_service.deregister_instance(
                            namespace,
                            "DEFAULT_GROUP",
                            &service_name,
                            &instance,
                        );
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
        let (_, service_names) = self
            .naming_service
            .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

        let mut summaries: Vec<ServiceListingSummary> = Vec::new();

        for service_name in service_names {
            let instances = self.naming_service.get_instances(
                namespace,
                "DEFAULT_GROUP",
                &service_name,
                "",
                false,
            );

            if instances.is_empty() {
                continue;
            }

            // Collect all unique tags for this service
            let mut all_tags: Vec<String> = Vec::new();
            let mut all_nodes: Vec<String> = Vec::new();
            let mut external_sources: std::collections::HashSet<String> = std::collections::HashSet::new();

            let mut checks_passing = 0;
            let checks_warning = 0;
            let mut checks_critical = 0;

            for instance in &instances {
                // Collect tags
                if let Some(tags_json) = instance.metadata.get("consul_tags")
                    && let Ok(tags) = serde_json::from_str::<Vec<String>>(tags_json)
                {
                    for tag in tags {
                        if !all_tags.contains(&tag) {
                            all_tags.push(tag);
                        }
                    }
                }

                // Collect nodes (unique)
                let node_name = format!("node-{}", instance.ip.replace('.', "-"));
                if !all_nodes.contains(&node_name) {
                    all_nodes.push(node_name);
                }

                // Collect external sources from metadata
                if let Some(external_source) = instance.metadata.get("external_source") {
                    external_sources.insert(external_source.clone());
                }

                // Count health checks based on instance status
                if instance.healthy {
                    checks_passing += 1;
                } else {
                    checks_critical += 1;
                }
            }

            let summary = ServiceListingSummary {
                service_summary: ServiceSummary {
                    kind: None, // TODO: detect service kind from metadata
                    name: service_name.clone(),
                    datacenter: self.datacenter.clone(),
                    tags: all_tags,
                    nodes: all_nodes,
                    external_sources: external_sources.into_iter().collect(),
                    instance_count: instances.len() as i32,
                    checks_passing,
                    checks_warning,
                    checks_critical,
                    gateway_config: GatewayConfig::default(),
                    transparent_proxy: false, // TODO: detect from metadata
                },
                connected_with_proxy: false,  // Connect proxy not supported yet
                connected_with_gateway: false, // Gateway not supported yet
            };

            summaries.push(summary);
        }

        // Sort by name for consistent output
        summaries.sort_by(|a, b| a.service_summary.name.cmp(&b.service_summary.name));

        summaries
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
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let services = catalog.get_services(&namespace);
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(services)
}

/// GET /v1/catalog/service/:service
/// Returns the nodes providing a specific service
pub async fn get_service(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let tag_filter = query.tag.as_deref();

    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let services = catalog.get_service_instances(&namespace, &service_name, tag_filter);

    if services.is_empty() {
        HttpResponse::Ok()
            .insert_header(("X-Consul-Index", "1"))
            .json(Vec::<CatalogService>::new())
    } else {
        HttpResponse::Ok()
            .insert_header(("X-Consul-Index", "1"))
            .json(services)
    }
}

/// GET /v1/catalog/nodes
/// Returns a list of all known nodes
pub async fn list_nodes(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    // Check ACL authorization for node read
    let authz = acl_service.authorize_request(&req, ResourceType::Node, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let nodes = catalog.get_nodes(&namespace);
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(nodes)
}

/// GET /v1/catalog/node/:node
/// Returns the node's services
pub async fn get_node(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    let node_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    // Check ACL authorization for node read
    let authz = acl_service.authorize_request(&req, ResourceType::Node, &node_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match catalog.get_node(&namespace, &node_name) {
        Some(node_services) => HttpResponse::Ok()
            .insert_header(("X-Consul-Index", "1"))
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
    query: web::Query<CatalogQueryParams>,
    body: web::Json<CatalogRegistration>,
) -> HttpResponse {
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let registration = body.into_inner();

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
        HttpResponse::Ok().json(true)
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
    query: web::Query<CatalogQueryParams>,
    body: web::Json<CatalogDeregistration>,
) -> HttpResponse {
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
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
    HttpResponse::Ok().json(true)
}

/// GET /v1/catalog/datacenters
/// Returns a list of all known datacenters
pub async fn list_datacenters() -> HttpResponse {
    // For now, return a single datacenter
    HttpResponse::Ok().json(vec!["dc1"])
}

/// GET /v1/internal/ui/services
/// Returns service summary for UI
pub async fn ui_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let summaries = catalog.get_service_summary(&namespace);

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(summaries)
}

/// GET /v1/catalog/connect/:service
/// Returns the mesh-capable service instances (stub - returns same as /catalog/service)
pub async fn get_connect_service(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    // Connect/mesh services are not supported, return same as regular service query
    let service_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let tag_filter = query.tag.as_deref();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let services = catalog.get_service_instances(&namespace, &service_name, tag_filter);
    HttpResponse::Ok().json(services)
}

/// GET /v1/catalog/node-services/:node
/// Returns the services for a specific node (array format)
pub async fn get_node_services(
    req: HttpRequest,
    catalog: web::Data<ConsulCatalogService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CatalogQueryParams>,
) -> HttpResponse {
    let node_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    let authz = acl_service.authorize_request(&req, ResourceType::Node, &node_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match catalog.get_node(&namespace, &node_name) {
        Some(node_services) => {
            let list = NodeServiceList {
                node: node_services.node,
                services: node_services.services.into_values().collect(),
            };
            HttpResponse::Ok()
                .insert_header(("X-Consul-Index", "1"))
                .json(list)
        }
        None => HttpResponse::NotFound()
            .json(ConsulError::new(format!("Node not found: {}", node_name))),
    }
}

/// GET /v1/catalog/gateway-services/:gateway
/// Returns services for a gateway (stub - returns empty)
pub async fn get_gateway_services(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let gateway_name = path.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &gateway_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Gateway services are not supported, return empty array
    HttpResponse::Ok().json(Vec::<CatalogService>::new())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_instance(name: &str, ip: &str, port: i32) -> NacosInstance {
        NacosInstance {
            instance_id: format!("{}#{}#{}", ip, port, "DEFAULT"),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: name.to_string(),
            metadata: HashMap::new(),
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: "simple".to_string(),
        }
    }

    #[test]
    fn test_catalog_service_from_instance() {
        let mut instance = create_test_instance("web", "192.168.1.100", 8080);
        instance
            .metadata
            .insert("consul_tags".to_string(), r#"["http", "api"]"#.to_string());

        let catalog_service = CatalogService::from_instance(&instance, "node1", "dc1");

        assert_eq!(catalog_service.service_name, "web");
        assert_eq!(catalog_service.service_port, 8080);
        assert_eq!(catalog_service.service_address, "192.168.1.100");
        assert!(catalog_service.service_tags.is_some());
        assert_eq!(catalog_service.service_tags.unwrap().len(), 2);
    }

    #[test]
    fn test_catalog_service_operations() {
        let naming_service = Arc::new(NamingService::new());
        let catalog = ConsulCatalogService::new(naming_service.clone());

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
        let naming_service = Arc::new(NamingService::new());
        let catalog = ConsulCatalogService::new(naming_service.clone());

        // Register services on different IPs
        let reg1 = CatalogRegistration {
            id: None,
            node: "node1".to_string(),
            address: "192.168.1.100".to_string(),
            datacenter: None,
            tagged_addresses: None,
            node_meta: None,
            service: Some(CatalogServiceRegistration {
                id: Some("web-1".to_string()),
                service: "web".to_string(),
                tags: None,
                address: Some("192.168.1.100".to_string()),
                meta: None,
                port: Some(8080),
                weights: None,
            }),
            check: None,
            checks: None,
            skip_node_update: None,
        };

        let reg2 = CatalogRegistration {
            id: None,
            node: "node2".to_string(),
            address: "192.168.1.101".to_string(),
            datacenter: None,
            tagged_addresses: None,
            node_meta: None,
            service: Some(CatalogServiceRegistration {
                id: Some("web-2".to_string()),
                service: "web".to_string(),
                tags: None,
                address: Some("192.168.1.101".to_string()),
                meta: None,
                port: Some(8080),
                weights: None,
            }),
            check: None,
            checks: None,
            skip_node_update: None,
        };

        catalog.register(&reg1, "public");
        catalog.register(&reg2, "public");

        // Get nodes
        let nodes = catalog.get_nodes("public");
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_service_summary() {
        let naming_service = Arc::new(NamingService::new());
        let catalog = ConsulCatalogService::new(naming_service.clone());

        // Register a healthy service
        let mut metadata = HashMap::new();
        metadata.insert(
            "consul_tags".to_string(),
            r#"["http", "api"]"#.to_string(),
        );
        let instance1 = NacosInstance {
            instance_id: "web-1".to_string(),
            ip: "192.168.1.100".to_string(),
            port: 8080,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "web".to_string(),
            metadata: metadata.clone(),
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: "simple".to_string(),
        };

        // Register an unhealthy service
        metadata.insert("consul_tags".to_string(), r#"["db"]"#.to_string());
        let instance2 = NacosInstance {
            instance_id: "db-1".to_string(),
            ip: "192.168.1.101".to_string(),
            port: 3306,
            weight: 1.0,
            healthy: false, // unhealthy
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "db".to_string(),
            metadata: metadata,
            instance_heart_beat_interval: 5000,
            instance_heart_beat_time_out: 15000,
            ip_delete_timeout: 30000,
            instance_id_generator: "simple".to_string(),
        };

        naming_service.register_instance("public", "DEFAULT_GROUP", "web", instance1);
        naming_service.register_instance("public", "DEFAULT_GROUP", "db", instance2);

        // Get service summary
        let summaries = catalog.get_service_summary("public");
        assert_eq!(summaries.len(), 2);

        // Find the web service summary
        let web_summary = summaries
            .iter()
            .find(|s| s.service_summary.name == "web")
            .unwrap();
        assert_eq!(web_summary.service_summary.datacenter, "dc1");
        assert_eq!(web_summary.service_summary.nodes, vec!["node-192-168-1-100"]);
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
