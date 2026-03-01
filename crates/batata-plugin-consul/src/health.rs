// Consul Health API HTTP handlers
// Implements Consul-compatible health check endpoints
// Uses InstanceCheckRegistry for unified health check management

use std::sync::Arc;
use std::time::Duration;

use actix_web::{HttpRequest, HttpResponse, web};

use batata_api::naming::model::Instance as NacosInstance;
use batata_naming::healthcheck::registry::{
    CheckOrigin, CheckStatus, CheckType as RegistryCheckType, InstanceCheckConfig,
    InstanceCheckRegistry,
};
use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::model::{
    AgentService, CheckRegistration, CheckStatusUpdate, CheckUpdateParams, ConsulError,
    HealthCheck, HealthQueryParams, Node, ServiceHealth, ServiceQueryParams,
};

/// Consul Health Check service backed by InstanceCheckRegistry.
/// All check operations delegate to the unified registry, which immediately
/// syncs health status changes to NamingService.
#[derive(Clone)]
pub struct ConsulHealthService {
    #[allow(dead_code)]
    naming_service: Arc<NamingService>,
    registry: Arc<InstanceCheckRegistry>,
    /// Node name for this agent
    node_name: String,
}

impl ConsulHealthService {
    pub fn new(naming_service: Arc<NamingService>, registry: Arc<InstanceCheckRegistry>) -> Self {
        Self {
            naming_service,
            registry,
            node_name: "batata-node".to_string(),
        }
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
        let check_status = CheckStatus::from_consul_str(status);
        self.registry.ttl_update(check_id, check_status, output);
        Ok(())
    }

    /// Register a health check via the unified registry
    pub async fn register_check(&self, registration: CheckRegistration) -> Result<(), String> {
        let check_id = registration.effective_check_id();
        let check_type_str = registration.check_type();

        let check_type = RegistryCheckType::from_consul_str(check_type_str);

        // Parse TTL if present
        let ttl = registration
            .ttl
            .as_ref()
            .and_then(|s| parse_duration_to_std(s));

        // Parse interval
        let interval = registration
            .interval
            .as_ref()
            .and_then(|s| parse_duration_to_std(s))
            .unwrap_or(Duration::from_secs(10));

        // Parse timeout
        let timeout = registration
            .timeout
            .as_ref()
            .and_then(|s| parse_duration_to_std(s))
            .unwrap_or(Duration::from_secs(5));

        // Parse deregister critical after
        let deregister_critical_after = registration
            .deregister_critical_service_after
            .as_ref()
            .and_then(|s| parse_duration_to_std(s));

        // Parse initial status
        let initial_status = registration
            .status
            .as_ref()
            .map(|s| CheckStatus::from_consul_str(s))
            .unwrap_or(CheckStatus::Critical);

        let consul_service_id = registration.service_id.clone();

        let config = InstanceCheckConfig {
            check_id: check_id.clone(),
            name: registration.name.clone(),
            check_type,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: registration
                .service_name
                .clone()
                .or_else(|| registration.service_id.clone())
                .unwrap_or_default(),
            ip: "0.0.0.0".to_string(),
            port: 0,
            cluster_name: "DEFAULT".to_string(),
            http_url: registration.http.clone(),
            tcp_addr: registration.tcp.clone(),
            grpc_addr: registration.grpc.clone(),
            interval,
            timeout,
            ttl,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after,
            origin: CheckOrigin::ConsulAgent,
            initial_status,
            consul_service_id,
        };

        self.registry.register_check(config);
        Ok(())
    }

    /// Deregister a health check
    pub async fn deregister_check(&self, check_id: &str) -> Result<(), String> {
        if !self.registry.has_check(check_id) {
            return Err(format!("Check not found: {}", check_id));
        }
        self.registry.deregister_check(check_id);
        Ok(())
    }

    /// Get all checks for a service (by Consul service_id)
    pub async fn get_service_checks(&self, service_id: &str) -> Vec<HealthCheck> {
        let checks = self.registry.get_checks_for_consul_service(service_id);
        checks
            .into_iter()
            .map(|(config, status)| self.to_health_check(&config, &status))
            .collect()
    }

    /// Get a check by ID
    pub async fn get_check(&self, check_id: &str) -> Option<HealthCheck> {
        let (config, status) = self.registry.get_check(check_id)?;
        Some(self.to_health_check(&config, &status))
    }

    /// Get all checks
    pub async fn get_all_checks(&self) -> Vec<HealthCheck> {
        self.registry
            .get_all_checks()
            .into_iter()
            .map(|(config, status)| self.to_health_check(&config, &status))
            .collect()
    }

    /// Get checks by status
    pub async fn get_checks_by_status(&self, status: &str) -> Vec<HealthCheck> {
        let check_status = CheckStatus::from_consul_str(status);
        self.registry
            .get_checks_by_status(&check_status)
            .into_iter()
            .map(|(config, s)| self.to_health_check(&config, &s))
            .collect()
    }

    /// Create a default health check for a service instance
    pub fn create_instance_check(&self, instance: &NacosInstance) -> HealthCheck {
        let status = if instance.healthy {
            "passing"
        } else {
            "critical"
        };

        HealthCheck {
            node: self.node_name.clone(),
            check_id: format!("service:{}", instance.instance_id),
            name: format!("Service '{}' check", instance.service_name),
            status: status.to_string(),
            notes: String::new(),
            output: if instance.healthy {
                "Instance is healthy".to_string()
            } else {
                "Instance is unhealthy".to_string()
            },
            service_id: instance.instance_id.clone(),
            service_name: instance.service_name.clone(),
            service_tags: instance
                .metadata
                .get("consul_tags")
                .and_then(|s| serde_json::from_str(s).ok()),
            check_type: "ttl".to_string(),
            interval: None,
            timeout: None,
            create_index: Some(1),
            modify_index: Some(1),
        }
    }

    /// Convert (InstanceCheckConfig, InstanceCheckStatus) to HealthCheck
    fn to_health_check(
        &self,
        config: &InstanceCheckConfig,
        status: &batata_naming::healthcheck::registry::InstanceCheckStatus,
    ) -> HealthCheck {
        let interval_str = if config.interval.as_secs() > 0 {
            Some(format!("{}s", config.interval.as_secs()))
        } else {
            None
        };

        let timeout_str = if config.timeout.as_secs() > 0 {
            Some(format!("{}s", config.timeout.as_secs()))
        } else {
            None
        };

        HealthCheck {
            node: self.node_name.clone(),
            check_id: config.check_id.clone(),
            name: config.name.clone(),
            status: status.status.as_str().to_string(),
            notes: String::new(),
            output: status.output.clone(),
            service_id: config.consul_service_id.clone().unwrap_or_default(),
            service_name: config.service_name.clone(),
            service_tags: None,
            check_type: config.check_type.as_str().to_string(),
            interval: interval_str,
            timeout: timeout_str,
            create_index: Some(1),
            modify_index: Some(1),
        }
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// GET /v1/health/service/:service
/// Returns the health information for a service
pub async fn get_service_health(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let passing_only = query.passing.unwrap_or(false);

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

    // Get instances from naming service
    let instances =
        naming_service.get_instances(&namespace, "DEFAULT_GROUP", &service_name, "", passing_only);

    // Filter by tag if specified
    let instances: Vec<_> = if let Some(ref tag) = query.tag {
        instances
            .into_iter()
            .filter(|inst| {
                inst.metadata
                    .get("consul_tags")
                    .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                    .map(|tags| tags.contains(tag))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        instances
    };

    let mut results: Vec<ServiceHealth> = Vec::new();

    for instance in instances {
        let agent_service = AgentService::from(&instance);

        // Get checks for this instance - first try stored checks, then create default
        let mut checks = health_service
            .get_service_checks(&instance.instance_id)
            .await;
        if checks.is_empty() {
            // Create a default check based on instance health
            checks.push(health_service.create_instance_check(&instance));
        }

        // Also include maintenance checks if they exist
        let maintenance_check_id = format!("_service_maintenance:{}", instance.instance_id);
        if let Some(maint_check) = health_service.get_check(&maintenance_check_id).await {
            checks.push(maint_check);
        }

        // Update check service info
        for check in &mut checks {
            check.service_name = service_name.clone();
            if let Some(tags) = &agent_service.tags {
                check.service_tags = Some(tags.clone());
            }
        }

        // If passing_only is set, skip instances with any critical check
        if passing_only && checks.iter().any(|c| c.status == "critical") {
            continue;
        }

        let ip = instance.ip.clone();
        let _port = instance.port as u16;

        // Build node tagged addresses
        let mut node_tagged_addresses = std::collections::HashMap::new();
        node_tagged_addresses.insert("lan".to_string(), ip.clone());
        node_tagged_addresses.insert("lan_ipv4".to_string(), ip.clone());
        node_tagged_addresses.insert("wan".to_string(), ip.clone());
        node_tagged_addresses.insert("wan_ipv4".to_string(), ip.clone());

        let mut node_meta = std::collections::HashMap::new();
        node_meta.insert("consul-network-segment".to_string(), "".to_string());
        node_meta.insert(
            "consul-version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        results.push(ServiceHealth {
            node: Node {
                id: uuid::Uuid::new_v4().to_string(),
                node: health_service.node_name.clone(),
                address: ip.clone(),
                datacenter: query.dc.clone().unwrap_or_else(|| "dc1".to_string()),
                tagged_addresses: Some(node_tagged_addresses),
                meta: Some(node_meta),
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

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(results)
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
pub async fn get_service_checks(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

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

    // Get instances to find all checks
    let instances =
        naming_service.get_instances(&namespace, "DEFAULT_GROUP", &service_name, "", false);

    let mut all_checks: Vec<HealthCheck> = Vec::new();

    for instance in instances {
        let mut checks = health_service
            .get_service_checks(&instance.instance_id)
            .await;
        if checks.is_empty() {
            checks.push(health_service.create_instance_check(&instance));
        }

        // Update service info
        for check in &mut checks {
            check.service_name = service_name.clone();
        }

        all_checks.extend(checks);
    }

    HttpResponse::Ok().json(all_checks)
}

/// GET /v1/health/state/:state
/// Returns the checks in the specified state
pub async fn get_checks_by_state(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
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

    HttpResponse::Ok().json(checks)
}

/// GET /v1/health/node/:node
/// Returns the checks on a specific node
pub async fn get_node_checks(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
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

    HttpResponse::Ok().json(checks)
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

    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", "1"))
        .json(checks_map)
}

/// GET /v1/health/connect/:service
/// Returns health for mesh-enabled service instances (stub - same as /health/service)
pub async fn get_connect_health(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    // Connect/mesh services are not supported, return same as regular service query
    get_service_health(
        req,
        naming_service,
        health_service,
        acl_service,
        path,
        query,
    )
    .await
}

/// GET /v1/health/ingress/:service
/// Returns health for ingress gateway instances (stub - returns empty)
pub async fn get_ingress_health(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let service_name = path.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Ingress gateways are not supported, return empty array
    HttpResponse::Ok().json(Vec::<ServiceHealth>::new())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a ConsulHealthService for tests
    fn create_test_service() -> ConsulHealthService {
        let naming_service = Arc::new(NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::new(naming_service.clone()));
        ConsulHealthService::new(naming_service, registry)
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
        assert!(result.unwrap_err().contains("Check not found"));
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
        assert_eq!(check.node, "batata-node");
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
    async fn test_create_instance_check_healthy() {
        let service = create_test_service();

        let mut instance = NacosInstance::new("10.0.0.1".to_string(), 8080);
        instance.instance_id = "web-001".to_string();
        instance.service_name = "web".to_string();
        instance.healthy = true;
        instance.enabled = true;

        let check = service.create_instance_check(&instance);
        assert_eq!(check.status, "passing");
        assert_eq!(check.check_id, "service:web-001");
        assert!(check.name.contains("web"));
        assert_eq!(check.output, "Instance is healthy");
        assert_eq!(check.service_id, "web-001");
        assert_eq!(check.service_name, "web");
    }

    #[tokio::test]
    async fn test_create_instance_check_unhealthy() {
        let service = create_test_service();

        let mut instance = NacosInstance::new("10.0.0.2".to_string(), 8080);
        instance.instance_id = "web-002".to_string();
        instance.service_name = "web".to_string();
        instance.healthy = false;
        instance.enabled = true;

        let check = service.create_instance_check(&instance);
        assert_eq!(check.status, "critical");
        assert_eq!(check.output, "Instance is unhealthy");
    }

    #[tokio::test]
    async fn test_create_instance_check_with_consul_tags() {
        let service = create_test_service();

        let mut instance = NacosInstance::new("10.0.0.3".to_string(), 8080);
        instance.instance_id = "tagged-001".to_string();
        instance.service_name = "tagged-svc".to_string();
        instance.healthy = true;
        instance
            .metadata
            .insert("consul_tags".to_string(), r#"["v1","primary"]"#.to_string());

        let check = service.create_instance_check(&instance);
        assert_eq!(
            check.service_tags,
            Some(vec!["v1".to_string(), "primary".to_string()])
        );
    }

    #[tokio::test]
    async fn test_create_instance_check_without_tags() {
        let service = create_test_service();

        let mut instance = NacosInstance::new("10.0.0.4".to_string(), 8080);
        instance.instance_id = "notag-001".to_string();
        instance.service_name = "notag-svc".to_string();
        instance.healthy = true;

        let check = service.create_instance_check(&instance);
        assert!(check.service_tags.is_none());
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
}
