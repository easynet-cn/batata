// Consul Health API HTTP handlers
// Implements Consul-compatible health check endpoints
// Uses InstanceCheckRegistry for unified health check management

use std::sync::Arc;
use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, web};

use batata_naming::healthcheck::registry::{
    CheckStatus, CheckType as RegistryCheckType, InstanceCheckConfig, InstanceCheckRegistry,
};

/// Convert Consul status string to CheckStatus
fn check_status_from_consul(s: &str) -> CheckStatus {
    match s.to_lowercase().as_str() {
        "passing" => CheckStatus::Passing,
        "warning" => CheckStatus::Warning,
        "critical" => CheckStatus::Critical,
        _ => CheckStatus::Critical,
    }
}

/// Convert Consul check type string to RegistryCheckType
fn check_type_from_consul(s: &str) -> RegistryCheckType {
    RegistryCheckType::from_str_value(s)
}

use crate::acl::{AclService, ResourceType};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::check_index::ConsulCheckIndex;
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{
    AgentService, AgentServiceRegistration, CONSUL_INTERNAL_CLUSTER, CONSUL_INTERNAL_GROUP,
    CONSUL_INTERNAL_NAMESPACE, CheckRegistration, CheckStatusUpdate, CheckUpdateParams,
    ConsulDatacenterConfig, ConsulError, HealthCheck, HealthQueryParams, Node, ServiceHealth,
    ServiceQueryParams,
};
use crate::naming_store::ConsulNamingStore;

/// Handle blocking query wait if `index` query parameter is set.
/// Returns once index advances past the target or timeout elapses.
async fn maybe_block(index_provider: &ConsulIndexProvider, index: Option<u64>, wait: Option<&str>) {
    if let Some(target_index) = index {
        let timeout = wait.and_then(ConsulIndexProvider::parse_wait_duration);
        index_provider
            .wait_for_change(ConsulTable::Catalog, target_index, timeout)
            .await;
    }
}

/// Consul Health Check service backed by InstanceCheckRegistry.
/// All check operations delegate to the unified registry, which immediately
/// syncs health status changes to the naming service.
#[derive(Clone)]
pub struct ConsulHealthService {
    registry: Arc<InstanceCheckRegistry>,
    check_index: Arc<ConsulCheckIndex>,
    /// Node name for this agent
    node_name: String,
}

impl ConsulHealthService {
    pub fn new(registry: Arc<InstanceCheckRegistry>, check_index: Arc<ConsulCheckIndex>) -> Self {
        let node_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());
        Self {
            registry,
            check_index,
            node_name,
        }
    }

    pub fn with_node_name(mut self, node_name: String) -> Self {
        self.node_name = node_name;
        self
    }

    /// Get registry reference
    pub fn registry(&self) -> &Arc<InstanceCheckRegistry> {
        &self.registry
    }

    /// Update check status (pass/warn/fail). Immediately syncs to NamingService.
    pub async fn update_check_status_async(
        &self,
        check_id: &str,
        status: &str,
        output: Option<String>,
    ) -> Result<(), String> {
        let check_status = check_status_from_consul(status);
        self.registry.ttl_update(check_id, check_status, output);
        Ok(())
    }

    /// Register a health check via the unified registry
    pub async fn register_check(&self, registration: CheckRegistration) -> Result<(), String> {
        let check_id = registration.effective_check_id();
        let check_type_str = registration.check_type();

        let check_type = check_type_from_consul(check_type_str);

        // Parse TTL if present
        let ttl = registration
            .ttl
            .as_ref()
            .and_then(|s| crate::consul_meta::parse_go_duration(s));

        // Parse interval
        let interval = registration
            .interval
            .as_ref()
            .and_then(|s| crate::consul_meta::parse_go_duration(s))
            .unwrap_or(Duration::from_secs(10));

        // Parse timeout
        let timeout = registration
            .timeout
            .as_ref()
            .and_then(|s| crate::consul_meta::parse_go_duration(s))
            .unwrap_or(Duration::from_secs(5));

        // Parse deregister critical after
        let deregister_critical_after = registration
            .deregister_critical_service_after
            .as_ref()
            .and_then(|s| crate::consul_meta::parse_go_duration(s));

        // Parse initial status
        let initial_status = registration
            .status
            .as_ref()
            .map(|s| check_status_from_consul(s))
            .unwrap_or(CheckStatus::Critical);

        let config = InstanceCheckConfig {
            check_id: check_id.clone(),
            name: registration.name.clone(),
            check_type,
            namespace: CONSUL_INTERNAL_NAMESPACE.to_string(),
            group_name: CONSUL_INTERNAL_GROUP.to_string(),
            service_name: registration
                .service_name
                .clone()
                .or_else(|| registration.service_id.clone())
                .unwrap_or_default(),
            ip: registration
                .ip
                .clone()
                .unwrap_or_else(|| "0.0.0.0".to_string()),
            port: registration.port.unwrap_or(0),
            cluster_name: CONSUL_INTERNAL_CLUSTER.to_string(),
            http_url: registration.http.clone(),
            tcp_addr: registration.tcp.clone(),
            grpc_addr: registration.grpc.clone(),
            db_url: None, // Consul doesn't use database health checks
            interval,
            timeout,
            ttl,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after,
            initial_status,
            notes: registration.notes.clone().unwrap_or_default(),
        };

        // Register in check index for service_id → instance lookup
        if let Some(ref svc_id) = registration.service_id {
            if !svc_id.is_empty() {
                use batata_naming::healthcheck::registry::{
                    build_check_service_key, build_instance_key,
                };
                let service_name = registration
                    .service_name
                    .as_deref()
                    .or(registration.service_id.as_deref())
                    .unwrap_or_default();
                let ip = registration.ip.as_deref().unwrap_or("0.0.0.0");
                let port = registration.port.unwrap_or(0);
                let svc_key = build_check_service_key(
                    CONSUL_INTERNAL_NAMESPACE,
                    CONSUL_INTERNAL_GROUP,
                    service_name,
                );
                let inst_key = build_instance_key(
                    CONSUL_INTERNAL_NAMESPACE,
                    CONSUL_INTERNAL_GROUP,
                    service_name,
                    ip,
                    port,
                    CONSUL_INTERNAL_CLUSTER,
                );
                self.check_index.register(svc_id, &svc_key, &inst_key);
                // Register check_id → consul_service_id reverse mapping
                self.check_index.register_check(&check_id, svc_id);
            }
        }

        self.registry.register_check(config);
        Ok(())
    }

    /// Deregister a health check
    pub async fn deregister_check(&self, check_id: &str) -> Result<(), String> {
        if !self.registry.has_check(check_id) {
            return Err(format!("Check not found: {}", check_id));
        }
        self.check_index.remove_check(check_id);
        self.registry.deregister_check(check_id);
        Ok(())
    }

    /// Get all checks for a service (by Consul service_id)
    pub async fn get_service_checks(&self, service_id: &str) -> Vec<HealthCheck> {
        let checks = match self.check_index.lookup(service_id) {
            Some((_, instance_key)) => self.registry.get_instance_checks(&instance_key),
            None => Vec::new(),
        };
        checks
            .into_iter()
            .map(|(config, status)| self.to_health_check(&config, &status, service_id))
            .collect()
    }

    /// Get a check by ID
    pub async fn get_check(&self, check_id: &str) -> Option<HealthCheck> {
        let (config, status) = self.registry.get_check(check_id)?;
        let svc_id = self.resolve_service_id(&config);
        Some(self.to_health_check(&config, &status, &svc_id))
    }

    /// Get all checks
    pub async fn get_all_checks(&self) -> Vec<HealthCheck> {
        self.registry
            .get_all_checks()
            .into_iter()
            .map(|(config, status)| {
                let svc_id = self.resolve_service_id(&config);
                self.to_health_check(&config, &status, &svc_id)
            })
            .collect()
    }

    /// Get all checks (sync version for non-async contexts)
    pub fn get_all_checks_sync(&self) -> Vec<HealthCheck> {
        self.registry
            .get_all_checks()
            .into_iter()
            .map(|(config, status)| {
                let svc_id = self.resolve_service_id(&config);
                self.to_health_check(&config, &status, &svc_id)
            })
            .collect()
    }

    /// Get checks by status
    pub async fn get_checks_by_status(&self, status: &str) -> Vec<HealthCheck> {
        let check_status = check_status_from_consul(status);
        self.registry
            .get_checks_by_status(&check_status)
            .into_iter()
            .map(|(config, s)| {
                let svc_id = self.resolve_service_id(&config);
                self.to_health_check(&config, &s, &svc_id)
            })
            .collect()
    }

    /// Resolve the Consul service_id for a check from the check_index reverse mapping
    fn resolve_service_id(&self, config: &InstanceCheckConfig) -> String {
        self.check_index
            .lookup_service_id(&config.check_id)
            .unwrap_or_default()
    }

    /// Create a default health check for a service instance
    /// Create a default health check from a Consul-native service registration.
    ///
    /// Used as a fallback when no explicit checks are registered for a service instance.
    pub fn create_default_check(
        &self,
        reg: &AgentServiceRegistration,
        healthy: bool,
    ) -> HealthCheck {
        let service_id = reg.service_id();
        let status = if healthy { "passing" } else { "critical" };
        let output = if healthy {
            "Instance is healthy"
        } else {
            "Instance is unhealthy"
        };

        HealthCheck {
            node: self.node_name.clone(),
            check_id: format!("service:{}", service_id),
            name: format!("Service '{}' check", reg.name),
            status: status.to_string(),
            notes: String::new(),
            output: output.to_string(),
            service_id,
            service_name: reg.name.clone(),
            service_tags: reg.tags.clone().unwrap_or_default(),
            check_type: "ttl".to_string(),
            interval: None,
            timeout: None,
            exposed_port: 0,
            create_index: 1,
            modify_index: 1,
            definition: None,
        }
    }

    /// Convert (InstanceCheckConfig, InstanceCheckStatus) to HealthCheck
    fn to_health_check(
        &self,
        config: &InstanceCheckConfig,
        status: &batata_naming::healthcheck::registry::InstanceCheckStatus,
        service_id: &str,
    ) -> HealthCheck {
        use batata_naming::healthcheck::registry::CheckType;

        // Node-level checks (e.g., serfHealth) have no interval/timeout/definition
        let is_node_check = config.check_type == CheckType::None;

        let interval_str = if !is_node_check && config.interval.as_secs() > 0 {
            Some(format!("{}s", config.interval.as_secs()))
        } else {
            None
        };

        let timeout_str = if !is_node_check && config.timeout.as_secs() > 0 {
            Some(format!("{}s", config.timeout.as_secs()))
        } else {
            None
        };

        // Consul returns empty Definition {} for node checks, full definition for service checks
        let definition = if is_node_check {
            Some(crate::model::HealthCheckDefinition::default())
        } else {
            Some(crate::model::HealthCheckDefinition {
                http: config.http_url.clone(),
                tcp: config.tcp_addr.clone(),
                grpc: config.grpc_addr.clone(),
                udp: None,
                method: None,
                header: None,
                body: None,
                tls_server_name: None,
                tls_skip_verify: None,
                tcp_use_tls: None,
                grpc_use_tls: None,
                interval_duration: config.interval.as_nanos() as u64,
                timeout_duration: config.timeout.as_nanos() as u64,
                deregister_critical_service_after_duration: config
                    .deregister_critical_after
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0),
            })
        };

        HealthCheck {
            node: self.node_name.clone(),
            check_id: config.check_id.clone(),
            name: config.name.clone(),
            status: status.status.as_str().to_string(),
            notes: config.notes.clone(),
            output: status.output.clone(),
            service_id: service_id.to_string(),
            service_name: config.service_name.clone(),
            service_tags: vec![],
            check_type: config.check_type.as_str().to_string(),
            interval: interval_str,
            timeout: timeout_str,
            exposed_port: 0,
            create_index: 1,
            modify_index: 1,
            definition,
        }
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// GET /v1/health/service/:service
/// Returns the health information for a service
#[allow(clippy::too_many_arguments)]
pub async fn get_service_health(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let passing_only = query.passing.unwrap_or(false);
    let namespace = dc_config.resolve_ns(&query.ns);

    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_name,
        false, // read access only
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get service entries from ConsulNamingStore (Consul-native data)
    let entries = naming_store.get_service_entries(&namespace, &service_name);

    let mut results: Vec<ServiceHealth> = Vec::new();

    for entry_bytes in entries {
        let reg: AgentServiceRegistration = match serde_json::from_slice(&entry_bytes) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let service_id = reg.service_id();
        let healthy =
            naming_store.is_healthy(&reg.effective_address(), reg.effective_port() as i32);

        // Filter by tag(s) — supports multiple ?tag= params with AND semantics
        // (matches Consul: all specified tags must be present)
        let tag_filters = crate::consul_meta::parse_multi_param(&req, "tag");
        if !tag_filters.is_empty() {
            let service_tags = reg.tags.as_deref().unwrap_or_default();
            let all_match = tag_filters.iter().all(|t| service_tags.contains(t));
            if !all_match {
                continue;
            }
        }

        let agent_service = AgentService::from(&reg);

        // Get checks for this service instance from registry
        let mut checks = health_service.get_service_checks(&service_id).await;

        // Also include maintenance checks if they exist
        let maintenance_check_id = format!("_service_maintenance:{}", service_id);
        if let Some(maint_check) = health_service.get_check(&maintenance_check_id).await {
            checks.push(maint_check);
        }

        // Include node-level checks (e.g., serfHealth) — Consul always includes
        // node checks alongside service checks in health/service responses.
        // Node checks have empty ServiceID/ServiceName.
        let all_checks = health_service.get_all_checks().await;
        for check in &all_checks {
            if check.service_id.is_empty() {
                // This is a node-level check (like serfHealth)
                let already_included = checks.iter().any(|c| c.check_id == check.check_id);
                if !already_included {
                    checks.push(check.clone());
                }
            }
        }

        // If no checks at all (no service checks AND no node checks), create a default
        if checks.is_empty() {
            checks.push(health_service.create_default_check(&reg, healthy));
        }

        // Update service info on service-level checks (NOT on node-level checks)
        for check in &mut checks {
            if !check.service_id.is_empty() {
                check.service_name = service_name.clone();
                if let Some(tags) = &agent_service.tags {
                    check.service_tags = tags.clone();
                }
            }
        }

        // If passing_only is set, skip instances with any non-passing check
        // (matches Consul's HealthFilterIncludeOnlyPassing: excludes both warning AND critical)
        if passing_only && checks.iter().any(|c| c.status != "passing") {
            continue;
        }

        // Use real local IP for node address (not service address which may be empty)
        let node_ip = batata_common::local_ip();

        let mut node_tagged_addresses = std::collections::HashMap::new();
        node_tagged_addresses.insert("lan".to_string(), node_ip.clone());
        node_tagged_addresses.insert("lan_ipv4".to_string(), node_ip.clone());
        node_tagged_addresses.insert("wan".to_string(), node_ip.clone());
        node_tagged_addresses.insert("wan_ipv4".to_string(), node_ip.clone());

        let mut node_meta = std::collections::HashMap::new();
        node_meta.insert("consul-network-segment".to_string(), "".to_string());
        node_meta.insert("consul-version".to_string(), dc_config.full_version());

        results.push(ServiceHealth {
            node: Node {
                id: dc_config.node_id.clone(),
                node: dc_config.node_name.clone(),
                address: node_ip,
                datacenter: dc_config.resolve_dc(&query.dc),
                tagged_addresses: Some(node_tagged_addresses),
                meta: Some(node_meta),
                create_index: 1,
                modify_index: 1,
            },
            service: agent_service,
            checks,
        });
    }

    // Apply filter expression if specified
    let results = if let Some(ref filter) = query.filter {
        apply_service_health_filter(results, filter)
    } else {
        results
    };

    // Handle blocking query wait
    maybe_block(&index_provider, query.index, query.wait.as_deref()).await;

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(results)
}

/// Apply Consul filter expression to service health results.
/// Supports basic expressions like:
///   ServiceMeta.key == "value"
///   ServiceMeta["key"] == "value"
///   Service == "name"
fn apply_service_health_filter(results: Vec<ServiceHealth>, filter: &str) -> Vec<ServiceHealth> {
    // Parse simple filter: field == "value" or field != "value"
    let filter = filter.trim();

    // Try to parse: <selector> <op> "<value>"
    let (selector, op, expected) = if let Some(rest) = filter.strip_suffix('"') {
        if let Some(pos) = rest.rfind('"') {
            let expected = &rest[pos + 1..];
            let before = rest[..pos].trim();
            if let Some(selector) = before.strip_suffix("==") {
                (selector.trim(), "==", expected)
            } else if let Some(selector) = before.strip_suffix("!=") {
                (selector.trim(), "!=", expected)
            } else {
                return results;
            }
        } else {
            return results;
        }
    } else {
        return results;
    };

    results
        .into_iter()
        .filter(|entry| {
            let actual = resolve_service_health_field(entry, selector);
            match op {
                "==" => actual.as_deref() == Some(expected),
                "!=" => actual.as_deref() != Some(expected),
                _ => true,
            }
        })
        .collect()
}

/// Resolve a field selector against a ServiceHealth entry.
fn resolve_service_health_field(entry: &ServiceHealth, selector: &str) -> Option<String> {
    // ServiceMeta.key or ServiceMeta["key"]
    if let Some(key) = selector.strip_prefix("ServiceMeta.") {
        return entry.service.meta.as_ref()?.get(key).cloned();
    }
    if let Some(rest) = selector.strip_prefix("ServiceMeta[\"") {
        let key = rest.strip_suffix("\"]")?;
        return entry.service.meta.as_ref()?.get(key).cloned();
    }
    // Node.Meta.key
    if let Some(key) = selector.strip_prefix("Node.Meta.") {
        return entry.node.meta.as_ref()?.get(key).cloned();
    }
    // Service
    if selector == "Service" {
        return Some(entry.service.service.clone());
    }
    // ServiceID
    if selector == "ServiceID" {
        return Some(entry.service.id.clone());
    }
    // ServicePort
    if selector == "ServicePort" {
        return Some(entry.service.port.to_string());
    }
    // ServiceAddress
    if selector == "ServiceAddress" {
        return Some(entry.service.address.clone());
    }
    None
}

/// GET /v1/health/checks/:service
/// Returns the checks for a service
#[allow(clippy::too_many_arguments)]
pub async fn get_service_checks(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    _dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = crate::namespace::DEFAULT_NAMESPACE; // health checks don't have namespace context yet

    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_name,
        false, // read access only
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get service entries from ConsulNamingStore
    let entries = naming_store.get_service_entries(namespace, &service_name);

    let mut all_checks: Vec<HealthCheck> = Vec::new();

    for entry_bytes in entries {
        let reg: AgentServiceRegistration = match serde_json::from_slice(&entry_bytes) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let service_id = reg.service_id();
        let healthy =
            naming_store.is_healthy(&reg.effective_address(), reg.effective_port() as i32);

        let mut checks = health_service.get_service_checks(&service_id).await;
        if checks.is_empty() {
            checks.push(health_service.create_default_check(&reg, healthy));
        }

        // Update service info
        for check in &mut checks {
            check.service_name = service_name.clone();
        }

        all_checks.extend(checks);
    }

    // Apply filter expression if provided
    if let Some(ref filter) = query.filter {
        all_checks = apply_health_check_filter(all_checks, filter);
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(all_checks)
}

/// GET /v1/health/state/:state
/// Returns the checks in the specified state
pub async fn get_checks_by_state(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let state = path.into_inner();

    // Check ACL authorization for service read (all services)
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        "", // all services
        false,
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Validate state
    let valid_states = ["passing", "warning", "critical", "any"];
    if !valid_states.contains(&state.as_str()) {
        return HttpResponse::BadRequest()
            .json(ConsulError::new(format!("Invalid state: {}", state)));
    }

    let checks = if state == "any" {
        health_service.get_all_checks().await
    } else {
        health_service.get_checks_by_status(&state).await
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(checks)
}

/// GET /v1/health/node/:node
/// Returns the checks on a specific node
pub async fn get_node_checks(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let node = path.into_inner();

    // Check ACL authorization for node read
    let authz = acl_service.authorize_request(&req, ResourceType::Node, &node, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Filter checks by node
    let checks: Vec<HealthCheck> = health_service
        .get_all_checks()
        .await
        .into_iter()
        .filter(|c| c.node == node)
        .collect();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(checks)
}

/// PUT /v1/agent/check/register
/// Register a new check
pub async fn register_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    body: web::Json<CheckRegistration>,
) -> HttpResponse {
    let registration = body.into_inner();

    // Check ACL authorization for service write (check registration requires service write)
    let service_id = registration.service_id.as_deref().unwrap_or("");
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        service_id,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service.register_check(registration).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/deregister/:check_id
/// Deregister a check
pub async fn deregister_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let check_id = path.into_inner();

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &check_id,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service.deregister_check(&check_id).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/pass/:check_id
/// Mark a check as passing
pub async fn pass_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let output = query.note.clone();

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service
        .update_check_status_async(&check_id, "passing", output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/warn/:check_id
/// Mark a check as warning
pub async fn warn_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let output = query.note.clone();

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service
        .update_check_status_async(&check_id, "warning", output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/fail/:check_id
/// Mark a check as failing
pub async fn fail_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let output = query.note.clone();

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service
        .update_check_status_async(&check_id, "critical", output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/update/:check_id
/// Update a check with custom status and output
pub async fn update_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<CheckStatusUpdate>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let update = body.into_inner();

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let status = update.status.unwrap_or_else(|| "passing".to_string());

    match health_service
        .update_check_status_async(&check_id, &status, update.output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// GET /v1/agent/checks
/// Returns all checks registered with the local agent
pub async fn list_agent_checks(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    _query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let checks = health_service.get_all_checks().await;

    // Return as a map keyed by check ID
    let checks_map: std::collections::HashMap<String, HealthCheck> = checks
        .into_iter()
        .map(|c| (c.check_id.clone(), c))
        .collect();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(checks_map)
}

/// GET /v1/health/connect/:service
/// Returns health for Connect-enabled service instances (connect-proxy or native connect)
#[allow(clippy::too_many_arguments)]
pub async fn get_connect_health(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let passing_only = query.passing.unwrap_or(false);
    let namespace = dc_config.resolve_ns(&query.ns);

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Scan all services to find Connect-enabled instances for this target service
    let all_service_names = naming_store.service_names(&namespace);

    let mut results: Vec<ServiceHealth> = Vec::new();

    for svc_name in all_service_names {
        let entries = naming_store.get_service_entries(&namespace, &svc_name);

        for entry_bytes in entries {
            let reg: AgentServiceRegistration = match serde_json::from_slice(&entry_bytes) {
                Ok(r) => r,
                Err(_) => continue,
            };

            if !is_connect_reg_for(&reg, &service_name) {
                continue;
            }

            let service_id = reg.service_id();
            let healthy =
                naming_store.is_healthy(&reg.effective_address(), reg.effective_port() as i32);

            let agent_service = AgentService::from(&reg);
            let mut checks = health_service.get_service_checks(&service_id).await;
            if checks.is_empty() {
                checks.push(health_service.create_default_check(&reg, healthy));
            }

            for check in &mut checks {
                check.service_name = svc_name.clone();
            }

            if passing_only && checks.iter().any(|c| c.status == "critical") {
                continue;
            }

            let ip = reg.effective_address();
            let mut node_tagged_addresses = std::collections::HashMap::new();
            node_tagged_addresses.insert("lan".to_string(), ip.clone());
            node_tagged_addresses.insert("wan".to_string(), ip.clone());

            results.push(ServiceHealth {
                node: Node {
                    id: uuid::Uuid::new_v4().to_string(),
                    node: health_service.node_name.clone(),
                    address: ip,
                    datacenter: dc_config.resolve_dc(&query.dc),
                    tagged_addresses: Some(node_tagged_addresses),
                    meta: None,
                    create_index: 1,
                    modify_index: 1,
                },
                service: agent_service,
                checks,
            });
        }
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(results)
}

/// GET /v1/health/ingress/:service
/// Returns health for ingress gateway instances that expose the given service
#[allow(clippy::too_many_arguments)]
pub async fn get_ingress_health(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let passing_only = query.passing.unwrap_or(false);
    let namespace = dc_config.resolve_ns(&query.ns);

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Scan all services to find ingress gateway instances
    let all_service_names = naming_store.service_names(&namespace);

    let mut results: Vec<ServiceHealth> = Vec::new();

    for svc_name in all_service_names {
        let entries = naming_store.get_service_entries(&namespace, &svc_name);

        for entry_bytes in entries {
            let reg: AgentServiceRegistration = match serde_json::from_slice(&entry_bytes) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Only include ingress-gateway instances
            let is_ingress = reg.kind.as_deref() == Some("ingress-gateway");
            if !is_ingress {
                continue;
            }

            let service_id = reg.service_id();
            let healthy =
                naming_store.is_healthy(&reg.effective_address(), reg.effective_port() as i32);

            let agent_service = AgentService::from(&reg);
            let mut checks = health_service.get_service_checks(&service_id).await;
            if checks.is_empty() {
                checks.push(health_service.create_default_check(&reg, healthy));
            }

            if passing_only && checks.iter().any(|c| c.status == "critical") {
                continue;
            }

            let ip = reg.effective_address();
            let mut node_tagged_addresses = std::collections::HashMap::new();
            node_tagged_addresses.insert("lan".to_string(), ip.clone());
            node_tagged_addresses.insert("wan".to_string(), ip.clone());

            results.push(ServiceHealth {
                node: Node {
                    id: uuid::Uuid::new_v4().to_string(),
                    node: health_service.node_name.clone(),
                    address: ip,
                    datacenter: dc_config.resolve_dc(&query.dc),
                    tagged_addresses: Some(node_tagged_addresses),
                    meta: None,
                    create_index: 1,
                    modify_index: 1,
                },
                service: agent_service,
                checks,
            });
        }
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(results)
}

/// Check if an AgentServiceRegistration is a Connect-enabled instance for the given target service.
fn is_connect_reg_for(reg: &AgentServiceRegistration, target_service: &str) -> bool {
    // Check if it's a connect-proxy targeting this service
    if reg.kind.as_deref() == Some("connect-proxy") {
        if let Some(ref proxy) = reg.proxy {
            let dest = proxy
                .get("DestinationServiceName")
                .or_else(|| proxy.get("destination_service_name"))
                .and_then(|v| v.as_str());
            if dest == Some(target_service) {
                return true;
            }
        }
    }

    // Check if it's a native Connect service with matching name
    if reg.name == target_service {
        if let Some(ref connect) = reg.connect {
            let is_native = connect
                .get("Native")
                .or_else(|| connect.get("native"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_native {
                return true;
            }
        }
    }

    false
}

// ============================================================================
// Helper functions
// ============================================================================

/// Parse a duration string like "30s", "5m", "1h" into seconds
fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, "ms")
    } else if let Some(stripped) = s.strip_suffix('s') {
        (stripped, "s")
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, "m")
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, "h")
    } else {
        (s, "s") // default to seconds
    };

    let num: u64 = num_str.parse().ok()?;

    match unit {
        "ms" => Some(num / 1000),
        "s" => Some(num),
        "m" => Some(num * 60),
        "h" => Some(num * 3600),
        _ => None,
    }
}

/// Parse a duration string into std::time::Duration
fn parse_duration_to_std(s: &str) -> Option<Duration> {
    parse_duration(s).map(Duration::from_secs)
}

/// Apply a Consul bexpr-style filter to health checks.
///
/// Supports basic expressions:
/// - `<Field> contains "<value>"`
/// - `<Field> == "<value>"`
/// - `<Field> != "<value>"`
///
/// Supported fields: Name, CheckID, Status, ServiceName, ServiceID.
fn apply_health_check_filter(checks: Vec<HealthCheck>, filter: &str) -> Vec<HealthCheck> {
    let filter = filter.trim();

    // Parse: <field> <operator> "<value>"
    if let Some(rest) = filter.strip_suffix('"')
        && let Some(pos) = rest.rfind('"')
    {
        let value = &rest[pos + 1..];
        let before = rest[..pos].trim();

        let get_field = |check: &HealthCheck, field: &str| -> Option<String> {
            match field {
                "Name" => Some(check.name.clone()),
                "CheckID" => Some(check.check_id.clone()),
                "Status" => Some(check.status.clone()),
                "ServiceName" => Some(check.service_name.clone()),
                "ServiceID" => Some(check.service_id.clone()),
                _ => None,
            }
        };

        if let Some(field) = before.strip_suffix("contains") {
            let field = field.trim();
            return checks
                .into_iter()
                .filter(|check| {
                    get_field(check, field)
                        .map(|actual| actual.contains(value))
                        .unwrap_or(true)
                })
                .collect();
        }

        if let Some(field) = before.strip_suffix("==") {
            let field = field.trim();
            return checks
                .into_iter()
                .filter(|check| {
                    get_field(check, field)
                        .map(|actual| actual == value)
                        .unwrap_or(true)
                })
                .collect();
        }

        if let Some(field) = before.strip_suffix("!=") {
            let field = field.trim();
            return checks
                .into_iter()
                .filter(|check| {
                    get_field(check, field)
                        .map(|actual| actual != value)
                        .unwrap_or(true)
                })
                .collect();
        }
    }

    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AgentServiceRegistration;
    use batata_naming::service::NamingService;

    /// Helper to create a ConsulHealthService for tests
    fn create_test_service() -> ConsulHealthService {
        let naming_service = Arc::new(NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(
            naming_service.clone(),
        ));
        let check_index = Arc::new(ConsulCheckIndex::new());
        ConsulHealthService::new(registry, check_index)
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s"), Some(30));
        assert_eq!(parse_duration("5m"), Some(300));
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("1000ms"), Some(1));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_check_registration() {
        let reg = CheckRegistration {
            name: "test-check".to_string(),
            check_id: None,
            service_id: Some("web-1".to_string()),
            service_name: None,
            notes: None,
            ttl: Some("30s".to_string()),
            http: None,
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            interval: None,
            timeout: None,
            deregister_critical_service_after: None,
            status: None,
            ..Default::default()
        };

        assert_eq!(reg.effective_check_id(), "service:web-1");
        assert_eq!(reg.check_type(), "ttl");
    }

    #[tokio::test]
    async fn test_consul_health_service() {
        let health_service = create_test_service();

        // Register a check
        let reg = CheckRegistration {
            name: "test-check".to_string(),
            check_id: Some("check-1".to_string()),
            service_id: Some("service-1".to_string()),
            service_name: None,
            notes: Some("Test notes".to_string()),
            ttl: Some("30s".to_string()),
            http: None,
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            interval: None,
            timeout: None,
            deregister_critical_service_after: None,
            status: Some("passing".to_string()),
            ..Default::default()
        };

        health_service.register_check(reg).await.unwrap();

        // Get the check
        let check = health_service.get_check("check-1").await.unwrap();
        assert_eq!(check.name, "test-check");
        assert_eq!(check.status, "passing");

        // Update status
        health_service
            .update_check_status_async("check-1", "critical", Some("Failed".to_string()))
            .await
            .unwrap();
        let check = health_service.get_check("check-1").await.unwrap();
        assert_eq!(check.status, "critical");
        assert_eq!(check.output, "Failed");

        // Deregister
        health_service.deregister_check("check-1").await.unwrap();
        assert!(health_service.get_check("check-1").await.is_none());
    }

    fn make_check_reg(
        name: &str,
        check_id: Option<&str>,
        service_id: Option<&str>,
    ) -> CheckRegistration {
        CheckRegistration {
            name: name.to_string(),
            check_id: check_id.map(|s| s.to_string()),
            service_id: service_id.map(|s| s.to_string()),
            service_name: None,
            notes: None,
            ttl: Some("30s".to_string()),
            http: None,
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            interval: None,
            timeout: None,
            deregister_critical_service_after: None,
            status: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_auto_generate_check_id_from_service_id() {
        let reg = make_check_reg("my-check", None, Some("web-svc"));
        assert_eq!(reg.effective_check_id(), "service:web-svc");
    }

    #[test]
    fn test_auto_generate_check_id_from_name() {
        let reg = CheckRegistration {
            name: "my-check".to_string(),
            check_id: None,
            service_id: None,
            service_name: None,
            notes: None,
            ttl: None,
            http: None,
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            interval: None,
            timeout: None,
            deregister_critical_service_after: None,
            status: None,
            ..Default::default()
        };
        assert_eq!(reg.effective_check_id(), "check:my-check");
    }

    #[test]
    fn test_check_type_detection() {
        // HTTP check
        let mut reg = make_check_reg("http-check", None, None);
        reg.ttl = None;
        reg.http = Some("http://localhost:8080/health".to_string());
        assert_eq!(reg.check_type(), "http");

        // TCP check
        let mut reg = make_check_reg("tcp-check", None, None);
        reg.ttl = None;
        reg.tcp = Some("localhost:8080".to_string());
        assert_eq!(reg.check_type(), "tcp");

        // gRPC check
        let mut reg = make_check_reg("grpc-check", None, None);
        reg.ttl = None;
        reg.grpc = Some("localhost:50051".to_string());
        assert_eq!(reg.check_type(), "grpc");

        // Default to ttl when no type specified
        let mut reg = make_check_reg("default-check", None, None);
        reg.ttl = None;
        assert_eq!(reg.check_type(), "ttl");
    }

    #[tokio::test]
    async fn test_default_status_is_critical() {
        let service = create_test_service();

        let reg = make_check_reg("crit-check", Some("crit-1"), None);
        service.register_check(reg).await.unwrap();

        let check = service.get_check("crit-1").await.unwrap();
        assert_eq!(check.status, "critical");
    }

    #[tokio::test]
    async fn test_deregister_nonexistent_check() {
        let service = create_test_service();

        let result = service.deregister_check("nonexistent").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Check not found"),
            "Expected 'Check not found' error, got: {}",
            err
        );
        assert!(
            err.contains("nonexistent"),
            "Error should mention the check ID, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_update_nonexistent_check() {
        let service = create_test_service();

        // Registry-based update silently ignores unknown checks (logs warning)
        let result = service
            .update_check_status_async("nonexistent", "passing", None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_nonexistent_check() {
        let service = create_test_service();

        assert!(service.get_check("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn test_service_checks_mapping() {
        let service = create_test_service();

        // Register multiple checks for the same service
        let mut reg1 = make_check_reg("check-a", Some("svc-chk-a"), Some("svc-mapping"));
        reg1.status = Some("passing".to_string());
        service.register_check(reg1).await.unwrap();

        let mut reg2 = make_check_reg("check-b", Some("svc-chk-b"), Some("svc-mapping"));
        reg2.status = Some("warning".to_string());
        service.register_check(reg2).await.unwrap();

        let checks = service.get_service_checks("svc-mapping").await;
        assert_eq!(checks.len(), 2);

        let ids: Vec<&str> = checks.iter().map(|c| c.check_id.as_str()).collect();
        assert!(ids.contains(&"svc-chk-a"));
        assert!(ids.contains(&"svc-chk-b"));

        // Verify statuses and names
        let chk_a = checks.iter().find(|c| c.check_id == "svc-chk-a").unwrap();
        assert_eq!(chk_a.name, "check-a");
        assert_eq!(chk_a.status, "passing");
        assert_eq!(chk_a.service_id, "svc-mapping");

        let chk_b = checks.iter().find(|c| c.check_id == "svc-chk-b").unwrap();
        assert_eq!(chk_b.name, "check-b");
        assert_eq!(chk_b.status, "warning");
        assert_eq!(chk_b.service_id, "svc-mapping");
    }

    #[tokio::test]
    async fn test_service_checks_empty_for_unknown_service() {
        let service = create_test_service();

        let checks = service.get_service_checks("unknown-service").await;
        assert!(checks.is_empty());
    }

    #[tokio::test]
    async fn test_deregister_removes_from_service_mapping() {
        let service = create_test_service();

        let mut reg = make_check_reg("dereg-map", Some("dereg-chk"), Some("dereg-svc"));
        reg.status = Some("passing".to_string());
        service.register_check(reg).await.unwrap();

        assert_eq!(service.get_service_checks("dereg-svc").await.len(), 1);

        service.deregister_check("dereg-chk").await.unwrap();
        assert!(service.get_service_checks("dereg-svc").await.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_checks() {
        let service = create_test_service();

        let reg1 = make_check_reg("all-a", Some("all-chk-1"), None);
        let reg2 = make_check_reg("all-b", Some("all-chk-2"), None);
        service.register_check(reg1).await.unwrap();
        service.register_check(reg2).await.unwrap();

        let all = service.get_all_checks().await;
        assert!(all.len() >= 2);
        let ids: Vec<&str> = all.iter().map(|c| c.check_id.as_str()).collect();
        assert!(ids.contains(&"all-chk-1"));
        assert!(ids.contains(&"all-chk-2"));

        // Verify check names and default status
        let chk1 = all.iter().find(|c| c.check_id == "all-chk-1").unwrap();
        assert_eq!(chk1.name, "all-a");
        assert_eq!(chk1.status, "critical"); // default status is critical

        let chk2 = all.iter().find(|c| c.check_id == "all-chk-2").unwrap();
        assert_eq!(chk2.name, "all-b");
        assert_eq!(chk2.status, "critical");
    }

    #[tokio::test]
    async fn test_get_checks_by_status() {
        let service = create_test_service();

        let mut reg1 = make_check_reg("stat-a", Some("stat-pass"), None);
        reg1.status = Some("passing".to_string());
        service.register_check(reg1).await.unwrap();

        let mut reg2 = make_check_reg("stat-b", Some("stat-warn"), None);
        reg2.status = Some("warning".to_string());
        service.register_check(reg2).await.unwrap();

        let mut reg3 = make_check_reg("stat-c", Some("stat-crit"), None);
        reg3.status = Some("critical".to_string());
        service.register_check(reg3).await.unwrap();

        let passing = service.get_checks_by_status("passing").await;
        assert!(passing.iter().any(|c| c.check_id == "stat-pass"));
        assert!(passing.iter().all(|c| c.status == "passing"));

        let warning = service.get_checks_by_status("warning").await;
        assert!(warning.iter().any(|c| c.check_id == "stat-warn"));
        assert!(warning.iter().all(|c| c.status == "warning"));

        let critical = service.get_checks_by_status("critical").await;
        assert!(critical.iter().any(|c| c.check_id == "stat-crit"));
        assert!(critical.iter().all(|c| c.status == "critical"));
    }

    #[tokio::test]
    async fn test_status_transitions() {
        let service = create_test_service();

        let mut reg = make_check_reg("trans", Some("trans-chk"), None);
        reg.status = Some("passing".to_string());
        service.register_check(reg).await.unwrap();

        // passing -> warning -> critical -> passing
        service
            .update_check_status_async("trans-chk", "warning", Some("Slow response".to_string()))
            .await
            .unwrap();
        let check = service.get_check("trans-chk").await.unwrap();
        assert_eq!(check.status, "warning");
        assert_eq!(check.output, "Slow response");

        service
            .update_check_status_async("trans-chk", "critical", Some("Down".to_string()))
            .await
            .unwrap();
        let check = service.get_check("trans-chk").await.unwrap();
        assert_eq!(check.status, "critical");

        service
            .update_check_status_async("trans-chk", "passing", None)
            .await
            .unwrap();
        let check = service.get_check("trans-chk").await.unwrap();
        assert_eq!(check.status, "passing");
    }

    #[tokio::test]
    async fn test_check_node_name() {
        let service = create_test_service();

        let reg = make_check_reg("node-chk", Some("node-chk-1"), None);
        service.register_check(reg).await.unwrap();

        let check = service.get_check("node-chk-1").await.unwrap();
        let expected_node = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());
        assert_eq!(check.node, expected_node);
    }

    #[tokio::test]
    async fn test_check_notes_and_output() {
        let service = create_test_service();

        let mut reg = make_check_reg("noted", Some("noted-chk"), None);
        reg.notes = Some("Important check".to_string());
        reg.status = Some("passing".to_string());
        service.register_check(reg).await.unwrap();

        let check = service.get_check("noted-chk").await.unwrap();
        assert!(check.output.is_empty()); // output starts empty
        assert_eq!(check.notes, "Important check");
        assert_eq!(check.name, "noted");
        assert_eq!(check.status, "passing");
    }

    #[tokio::test]
    async fn test_ttl_and_deregister_timeout_parsing() {
        let service = create_test_service();

        let reg = CheckRegistration {
            name: "ttl-check".to_string(),
            check_id: Some("ttl-chk".to_string()),
            service_id: None,
            service_name: None,
            notes: None,
            ttl: Some("1m".to_string()),
            http: None,
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            interval: None,
            timeout: None,
            deregister_critical_service_after: Some("5m".to_string()),
            status: None,
            ..Default::default()
        };
        service.register_check(reg).await.unwrap();

        // Verify the check was registered via the registry
        let (config, _status) = service.registry().get_check("ttl-chk").unwrap();
        assert_eq!(config.ttl, Some(Duration::from_secs(60)));
        assert_eq!(
            config.deregister_critical_after,
            Some(Duration::from_secs(300))
        );
    }

    #[tokio::test]
    async fn test_create_default_check_healthy() {
        let service = create_test_service();

        let reg = AgentServiceRegistration {
            id: Some("web-001".to_string()),
            name: "web".to_string(),
            address: Some("10.0.0.1".to_string()),
            port: Some(8080),
            ..Default::default()
        };

        let check = service.create_default_check(&reg, true);
        assert_eq!(check.status, "passing");
        assert_eq!(check.check_id, "service:web-001");
        assert!(check.name.contains("web"));
        assert_eq!(check.output, "Instance is healthy");
        assert_eq!(check.service_id, "web-001");
        assert_eq!(check.service_name, "web");
    }

    #[tokio::test]
    async fn test_create_default_check_unhealthy() {
        let service = create_test_service();

        let reg = AgentServiceRegistration {
            id: Some("web-002".to_string()),
            name: "web".to_string(),
            address: Some("10.0.0.2".to_string()),
            port: Some(8080),
            ..Default::default()
        };

        let check = service.create_default_check(&reg, false);
        assert_eq!(check.status, "critical");
        assert_eq!(check.output, "Instance is unhealthy");
    }

    #[tokio::test]
    async fn test_create_default_check_with_tags() {
        let service = create_test_service();

        let reg = AgentServiceRegistration {
            id: Some("tagged-001".to_string()),
            name: "tagged-svc".to_string(),
            address: Some("10.0.0.3".to_string()),
            port: Some(8080),
            tags: Some(vec!["v1".to_string(), "primary".to_string()]),
            ..Default::default()
        };

        let check = service.create_default_check(&reg, true);
        assert_eq!(
            check.service_tags,
            vec!["v1".to_string(), "primary".to_string()]
        );
    }

    #[tokio::test]
    async fn test_create_default_check_without_tags() {
        let service = create_test_service();

        let reg = AgentServiceRegistration {
            id: Some("notag-001".to_string()),
            name: "notag-svc".to_string(),
            address: Some("10.0.0.4".to_string()),
            port: Some(8080),
            ..Default::default()
        };

        let check = service.create_default_check(&reg, true);
        assert!(check.service_tags.is_empty());
    }

    #[test]
    fn test_parse_duration_edge_cases() {
        // Zero values
        assert_eq!(parse_duration("0s"), Some(0));
        assert_eq!(parse_duration("0m"), Some(0));
        assert_eq!(parse_duration("0h"), Some(0));

        // Large values
        assert_eq!(parse_duration("3600s"), Some(3600));
        assert_eq!(parse_duration("60m"), Some(3600));

        // Edge case: ms rounding
        assert_eq!(parse_duration("500ms"), Some(0));
        assert_eq!(parse_duration("1500ms"), Some(1));

        // No unit defaults to seconds
        assert_eq!(parse_duration("123"), Some(123));

        // Empty string
        assert_eq!(parse_duration(""), None);
    }

    #[tokio::test]
    async fn test_register_multiple_services_separate_checks() {
        let service = create_test_service();

        let reg1 = make_check_reg("chk-svc1", Some("multi-svc1-chk"), Some("multi-svc1"));
        let reg2 = make_check_reg("chk-svc2", Some("multi-svc2-chk"), Some("multi-svc2"));
        service.register_check(reg1).await.unwrap();
        service.register_check(reg2).await.unwrap();

        assert_eq!(service.get_service_checks("multi-svc1").await.len(), 1);
        assert_eq!(service.get_service_checks("multi-svc2").await.len(), 1);

        // Deregistering one doesn't affect the other
        service.deregister_check("multi-svc1-chk").await.unwrap();
        assert!(service.get_service_checks("multi-svc1").await.is_empty());
        assert_eq!(service.get_service_checks("multi-svc2").await.len(), 1);
    }

    #[tokio::test]
    async fn test_check_reregistration_overwrites() {
        let service = create_test_service();

        let mut reg = make_check_reg("overwrite", Some("ow-chk"), None);
        reg.status = Some("passing".to_string());
        service.register_check(reg).await.unwrap();

        let check = service.get_check("ow-chk").await.unwrap();
        assert_eq!(check.status, "passing");

        // Re-register with different status
        let mut reg2 = make_check_reg("overwrite-v2", Some("ow-chk"), None);
        reg2.status = Some("critical".to_string());
        service.register_check(reg2).await.unwrap();

        let check = service.get_check("ow-chk").await.unwrap();
        assert_eq!(check.name, "overwrite-v2");
        assert_eq!(check.status, "critical");
    }

    fn create_test_health_service_with_registry()
    -> (ConsulHealthService, Arc<InstanceCheckRegistry>) {
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(Arc::new(
            NamingService::new(),
        )));
        let check_index = Arc::new(ConsulCheckIndex::new());
        let service = ConsulHealthService::new(registry.clone(), check_index);
        (service, registry)
    }

    #[test]
    fn test_health_service_creation() {
        let (service, _) = create_test_health_service_with_registry();
        let expected_node = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "batata-node".to_string());
        assert_eq!(service.node_name, expected_node);
    }

    #[test]
    fn test_health_service_registry_access() {
        let (_service, registry) = create_test_health_service_with_registry();
        // Registry should be accessible and empty
        let checks = registry.get_all_checks();
        assert!(checks.is_empty(), "Registry should start empty");
    }

    #[test]
    fn test_check_status_from_consul_string() {
        assert_eq!(check_status_from_consul("passing"), CheckStatus::Passing);
        assert_eq!(check_status_from_consul("warning"), CheckStatus::Warning);
        assert_eq!(check_status_from_consul("critical"), CheckStatus::Critical);
        assert_eq!(check_status_from_consul("unknown"), CheckStatus::Critical); // default
    }

    #[test]
    fn test_check_type_from_consul_string() {
        assert_eq!(check_type_from_consul("tcp"), RegistryCheckType::Tcp);
        assert_eq!(check_type_from_consul("http"), RegistryCheckType::Http);
        assert_eq!(check_type_from_consul("ttl"), RegistryCheckType::Ttl);
        assert_eq!(check_type_from_consul("grpc"), RegistryCheckType::Grpc);
        assert_eq!(check_type_from_consul("unknown"), RegistryCheckType::None);
    }

    #[test]
    fn test_register_and_query_ttl_check() {
        let (_service, registry) = create_test_health_service_with_registry();

        let config = InstanceCheckConfig {
            check_id: "svc:web-1".to_string(),
            name: "Web health".to_string(),
            check_type: RegistryCheckType::Ttl,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "web".to_string(),
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: None,
            grpc_addr: None,
            db_url: None,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            ttl: Some(Duration::from_secs(30)),
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            initial_status: CheckStatus::Critical,
            notes: String::new(),
        };
        registry.register_check(config);

        // Check should be registered
        let check = registry.get_check("svc:web-1");
        assert!(check.is_some(), "Check should be registered");
        let (config, status) = check.unwrap();
        assert_eq!(config.check_type, RegistryCheckType::Ttl);
        assert_eq!(status.status, CheckStatus::Critical); // initial

        // Update via TTL
        registry.ttl_update(
            "svc:web-1",
            CheckStatus::Passing,
            Some("all good".to_string()),
        );
        let (_, status) = registry.get_check("svc:web-1").unwrap();
        assert_eq!(status.status, CheckStatus::Passing);
        assert_eq!(status.output, "all good");
    }

    #[test]
    fn test_pass_warn_fail_cycle() {
        let (_service, registry) = create_test_health_service_with_registry();

        let config = InstanceCheckConfig {
            check_id: "cycle-check".to_string(),
            name: "Cycle test".to_string(),
            check_type: RegistryCheckType::Ttl,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "cycle-svc".to_string(),
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: None,
            grpc_addr: None,
            db_url: None,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(5),
            ttl: Some(Duration::from_secs(30)),
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            initial_status: CheckStatus::Passing,
            notes: String::new(),
        };
        registry.register_check(config);

        // Pass
        registry.ttl_update("cycle-check", CheckStatus::Passing, Some("ok".to_string()));
        assert_eq!(
            registry.get_check("cycle-check").unwrap().1.status,
            CheckStatus::Passing
        );

        // Warn
        registry.ttl_update(
            "cycle-check",
            CheckStatus::Warning,
            Some("degraded".to_string()),
        );
        let (_, status) = registry.get_check("cycle-check").unwrap();
        assert_eq!(status.status, CheckStatus::Warning);
        assert!(
            status.status.is_healthy(),
            "Warning should still be healthy"
        );

        // Fail
        registry.ttl_update(
            "cycle-check",
            CheckStatus::Critical,
            Some("down".to_string()),
        );
        let (_, status) = registry.get_check("cycle-check").unwrap();
        assert_eq!(status.status, CheckStatus::Critical);
        assert!(
            !status.status.is_healthy(),
            "Critical should not be healthy"
        );
        assert!(
            status.critical_since.is_some(),
            "Critical should set critical_since"
        );

        // Pass again — should clear critical_since
        registry.ttl_update(
            "cycle-check",
            CheckStatus::Passing,
            Some("recovered".to_string()),
        );
        let (_, status) = registry.get_check("cycle-check").unwrap();
        assert_eq!(status.status, CheckStatus::Passing);
    }
}
