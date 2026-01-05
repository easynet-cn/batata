// Consul Health API HTTP handlers
// Implements Consul-compatible health check endpoints

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;

use crate::api::naming::model::Instance as NacosInstance;
use crate::service::naming::NamingService;

use super::acl::{AclService, ResourceType};
use super::model::{
    AgentService, CheckRegistration, CheckStatusUpdate, CheckUpdateParams, ConsulError,
    HealthCheck, HealthQueryParams, Node, ServiceHealth, ServiceQueryParams,
};

/// Stored health check with metadata
#[derive(Debug, Clone)]
pub struct StoredCheck {
    pub check: HealthCheck,
    pub ttl_seconds: Option<u64>,
    pub last_updated: i64,
    pub deregister_after_secs: Option<u64>,
}

/// Consul Health Check service
/// Manages health checks and their states
#[derive(Clone)]
pub struct ConsulHealthService {
    #[allow(dead_code)] // Reserved for future health check integration with naming service
    naming_service: Arc<NamingService>,
    /// Health checks storage: check_id -> StoredCheck
    checks: Arc<DashMap<String, StoredCheck>>,
    /// Service to checks mapping: service_id -> Vec<check_id>
    service_checks: Arc<DashMap<String, Vec<String>>>,
    /// Node name for this agent
    node_name: String,
}

impl ConsulHealthService {
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self {
            naming_service,
            checks: Arc::new(DashMap::new()),
            service_checks: Arc::new(DashMap::new()),
            node_name: "batata-node".to_string(),
        }
    }

    /// Register a health check
    pub fn register_check(&self, registration: CheckRegistration) -> Result<(), String> {
        let check_id = registration.effective_check_id();
        let check_type = registration.check_type();

        // Parse TTL if present
        let ttl_seconds = registration.ttl.as_ref().and_then(|s| parse_duration(s));

        // Parse deregister timeout
        let deregister_after_secs = registration
            .deregister_critical_service_after
            .as_ref()
            .and_then(|s| parse_duration(s));

        let check = HealthCheck {
            node: self.node_name.clone(),
            check_id: check_id.clone(),
            name: registration.name.clone(),
            status: registration
                .status
                .clone()
                .unwrap_or_else(|| "critical".to_string()),
            notes: registration.notes.clone().unwrap_or_default(),
            output: String::new(),
            service_id: registration.service_id.clone().unwrap_or_default(),
            service_name: String::new(), // Will be filled when we look up the service
            service_tags: None,
            check_type: check_type.to_string(),
            create_index: Some(1),
            modify_index: Some(1),
        };

        let stored = StoredCheck {
            check,
            ttl_seconds,
            last_updated: current_timestamp(),
            deregister_after_secs,
        };

        self.checks.insert(check_id.clone(), stored);

        // Update service_checks mapping
        if let Some(service_id) = &registration.service_id {
            self.service_checks
                .entry(service_id.clone())
                .or_default()
                .push(check_id);
        }

        Ok(())
    }

    /// Deregister a health check
    pub fn deregister_check(&self, check_id: &str) -> Result<(), String> {
        if let Some((_, stored)) = self.checks.remove(check_id) {
            // Remove from service_checks mapping
            if !stored.check.service_id.is_empty()
                && let Some(mut checks) = self.service_checks.get_mut(&stored.check.service_id)
            {
                checks.retain(|id| id != check_id);
            }
            Ok(())
        } else {
            Err(format!("Check not found: {}", check_id))
        }
    }

    /// Update check status
    pub fn update_check_status(
        &self,
        check_id: &str,
        status: &str,
        output: Option<String>,
    ) -> Result<(), String> {
        if let Some(mut stored) = self.checks.get_mut(check_id) {
            stored.check.status = status.to_string();
            if let Some(out) = output {
                stored.check.output = out;
            }
            stored.last_updated = current_timestamp();
            Ok(())
        } else {
            Err(format!("Check not found: {}", check_id))
        }
    }

    /// Get all checks for a service
    pub fn get_service_checks(&self, service_id: &str) -> Vec<HealthCheck> {
        if let Some(check_ids) = self.service_checks.get(service_id) {
            check_ids
                .iter()
                .filter_map(|id| self.checks.get(id).map(|s| s.check.clone()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get a check by ID
    pub fn get_check(&self, check_id: &str) -> Option<HealthCheck> {
        self.checks.get(check_id).map(|s| s.check.clone())
    }

    /// Get all checks
    pub fn get_all_checks(&self) -> Vec<HealthCheck> {
        self.checks.iter().map(|s| s.check.clone()).collect()
    }

    /// Get checks by status
    pub fn get_checks_by_status(&self, status: &str) -> Vec<HealthCheck> {
        self.checks
            .iter()
            .filter(|s| s.check.status == status)
            .map(|s| s.check.clone())
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
        let mut checks = health_service.get_service_checks(&instance.instance_id);
        if checks.is_empty() {
            // Create a default check based on instance health
            checks.push(health_service.create_instance_check(&instance));
        }

        // Update check service info
        for check in &mut checks {
            check.service_name = service_name.clone();
            if let Some(tags) = &agent_service.tags {
                check.service_tags = Some(tags.clone());
            }
        }

        results.push(ServiceHealth {
            node: Node {
                id: uuid::Uuid::new_v4().to_string(),
                node: health_service.node_name.clone(),
                address: instance.ip.clone(),
                datacenter: query.dc.clone().unwrap_or_else(|| "dc1".to_string()),
                tagged_addresses: None,
                meta: None,
            },
            service: agent_service,
            checks,
        });
    }

    HttpResponse::Ok().json(results)
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
        let mut checks = health_service.get_service_checks(&instance.instance_id);
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
        health_service.get_all_checks()
    } else {
        health_service.get_checks_by_status(&state)
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

    match health_service.register_check(registration) {
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

    match health_service.deregister_check(&check_id) {
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

    match health_service.update_check_status(&check_id, "passing", output) {
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

    match health_service.update_check_status(&check_id, "warning", output) {
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

    match health_service.update_check_status(&check_id, "critical", output) {
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

    match health_service.update_check_status(&check_id, &status, update.output) {
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

    let checks = health_service.get_all_checks();

    // Return as a map keyed by check ID
    let checks_map: std::collections::HashMap<String, HealthCheck> = checks
        .into_iter()
        .map(|c| (c.check_id.clone(), c))
        .collect();

    HttpResponse::Ok().json(checks_map)
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

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_consul_health_service() {
        let naming_service = Arc::new(NamingService::new());
        let health_service = ConsulHealthService::new(naming_service);

        // Register a check
        let reg = CheckRegistration {
            name: "test-check".to_string(),
            check_id: Some("check-1".to_string()),
            service_id: Some("service-1".to_string()),
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

        health_service.register_check(reg).unwrap();

        // Get the check
        let check = health_service.get_check("check-1").unwrap();
        assert_eq!(check.name, "test-check");
        assert_eq!(check.status, "passing");

        // Update status
        health_service
            .update_check_status("check-1", "critical", Some("Failed".to_string()))
            .unwrap();
        let check = health_service.get_check("check-1").unwrap();
        assert_eq!(check.status, "critical");
        assert_eq!(check.output, "Failed");

        // Deregister
        health_service.deregister_check("check-1").unwrap();
        assert!(health_service.get_check("check-1").is_none());
    }
}
