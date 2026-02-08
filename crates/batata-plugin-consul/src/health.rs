// Consul Health API HTTP handlers
// Implements Consul-compatible health check endpoints
// Supports both in-memory storage and persistent storage via ConfigService

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{HttpRequest, HttpResponse, web};
use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use batata_api::naming::model::Instance as NacosInstance;
use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::model::{
    AgentService, CheckRegistration, CheckStatusUpdate, CheckUpdateParams, ConsulError,
    HealthCheck, HealthQueryParams, Node, ServiceHealth, ServiceQueryParams,
};

// Constants for ConfigService mapping
const CONSUL_CHECK_NAMESPACE: &str = "public";
const CONSUL_CHECK_GROUP: &str = "consul-checks";

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
// Persistent Health Check Service (Using ConfigService)
// ============================================================================

/// Stored check metadata for persistent storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CheckMetadata {
    check: HealthCheck,
    ttl_seconds: Option<u64>,
    last_updated: i64,
    deregister_after_secs: Option<u64>,
    service_id: Option<String>,
}

/// Consul Health Check service with database persistence
/// Uses Batata's ConfigService for storage
#[derive(Clone)]
pub struct ConsulHealthServicePersistent {
    db: Arc<DatabaseConnection>,
    naming_service: Arc<NamingService>,
    /// In-memory cache for performance
    cache: Arc<DashMap<String, StoredCheck>>,
    /// Service to checks mapping cache
    service_checks_cache: Arc<DashMap<String, Vec<String>>>,
    /// Node name for this agent
    node_name: String,
}

impl ConsulHealthServicePersistent {
    pub fn new(db: Arc<DatabaseConnection>, naming_service: Arc<NamingService>) -> Self {
        Self {
            db,
            naming_service,
            cache: Arc::new(DashMap::new()),
            service_checks_cache: Arc::new(DashMap::new()),
            node_name: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "batata-node".to_string()),
        }
    }

    /// Convert check_id to ConfigService dataId
    fn check_id_to_data_id(check_id: &str) -> String {
        format!("check:{}", check_id.replace('/', ":").replace(':', "_"))
    }

    /// Convert ConfigService dataId back to check_id
    fn data_id_to_check_id(data_id: &str) -> String {
        data_id
            .strip_prefix("check:")
            .unwrap_or(data_id)
            .replace('_', ":")
            .replace(':', "/")
    }

    /// Register a health check with persistence
    pub async fn register_check(&self, registration: CheckRegistration) -> Result<(), String> {
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
            service_name: String::new(),
            service_tags: None,
            check_type: check_type.to_string(),
            create_index: Some(1),
            modify_index: Some(1),
        };

        let metadata = CheckMetadata {
            check: check.clone(),
            ttl_seconds,
            last_updated: current_timestamp(),
            deregister_after_secs,
            service_id: registration.service_id.clone(),
        };

        let content =
            serde_json::to_string(&metadata).map_err(|e| format!("Serialization error: {}", e))?;

        let data_id = Self::check_id_to_data_id(&check_id);

        // Save to database
        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_CHECK_GROUP,
            CONSUL_CHECK_NAMESPACE,
            &content,
            "consul-check",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul Check: {}", check_id),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Update cache
        let stored = StoredCheck {
            check,
            ttl_seconds,
            last_updated: current_timestamp(),
            deregister_after_secs,
        };
        self.cache.insert(check_id.clone(), stored);

        // Update service_checks mapping
        if let Some(service_id) = &registration.service_id {
            self.service_checks_cache
                .entry(service_id.clone())
                .or_default()
                .push(check_id);
        }

        Ok(())
    }

    /// Deregister a health check
    pub async fn deregister_check(&self, check_id: &str) -> Result<(), String> {
        let data_id = Self::check_id_to_data_id(check_id);

        // Get check info for cleanup
        let stored = self.cache.remove(check_id);

        // Remove from service_checks mapping
        if let Some((_, stored)) = &stored
            && !stored.check.service_id.is_empty()
            && let Some(mut checks) = self.service_checks_cache.get_mut(&stored.check.service_id)
        {
            checks.retain(|id| id != check_id);
        }

        // Delete from database
        batata_config::service::config::delete(
            &self.db,
            &data_id,
            CONSUL_CHECK_GROUP,
            CONSUL_CHECK_NAMESPACE,
            "",
            "127.0.0.1",
            "system",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Update check status with persistence
    pub async fn update_check_status(
        &self,
        check_id: &str,
        status: &str,
        output: Option<String>,
    ) -> Result<(), String> {
        // Get existing check
        let stored = self.get_stored_check(check_id).await?;
        let mut check = stored.check;

        check.status = status.to_string();
        if let Some(out) = output {
            check.output = out;
        }

        let metadata = CheckMetadata {
            check: check.clone(),
            ttl_seconds: stored.ttl_seconds,
            last_updated: current_timestamp(),
            deregister_after_secs: stored.deregister_after_secs,
            service_id: if check.service_id.is_empty() {
                None
            } else {
                Some(check.service_id.clone())
            },
        };

        let content =
            serde_json::to_string(&metadata).map_err(|e| format!("Serialization error: {}", e))?;

        let data_id = Self::check_id_to_data_id(check_id);

        // Update in database
        batata_config::service::config::create_or_update(
            &self.db,
            &data_id,
            CONSUL_CHECK_GROUP,
            CONSUL_CHECK_NAMESPACE,
            &content,
            "consul-check",
            "system",
            "127.0.0.1",
            "",
            &format!("Consul Check: {}", check_id),
            "",
            "",
            "json",
            "",
            "",
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Update cache
        self.cache.insert(
            check_id.to_string(),
            StoredCheck {
                check: check.clone(),
                ttl_seconds: stored.ttl_seconds,
                last_updated: current_timestamp(),
                deregister_after_secs: stored.deregister_after_secs,
            },
        );

        // Update instance health in NamingService if this check is associated with a service
        if !check.service_id.is_empty() {
            self.sync_instance_health(&check.service_id, status);
        }

        Ok(())
    }

    /// Sync instance health status in NamingService based on check status
    fn sync_instance_health(&self, service_id: &str, status: &str) {
        // Instance is healthy only if status is "passing"
        let healthy = status == "passing";

        // Try to update instance health (this is best-effort)
        // Parse service_id to extract namespace, group, service_name, ip, port, cluster
        // Format: namespace@@group@@serviceName@@ip@@port@@cluster or just instance_id
        if let Some((namespace, group, service_name, ip, port, cluster)) =
            Self::parse_service_id(service_id)
        {
            let _ = self.naming_service.update_instance_health(
                &namespace,
                &group,
                &service_name,
                &ip,
                port,
                &cluster,
                healthy,
            );
        }
    }

    /// Parse service_id to extract instance identification
    fn parse_service_id(service_id: &str) -> Option<(String, String, String, String, i32, String)> {
        // Try to parse as structured format: namespace@@group@@service@@ip@@port@@cluster
        let parts: Vec<&str> = service_id.split("@@").collect();
        if parts.len() >= 5 {
            let port: i32 = parts[4].parse().ok()?;
            let cluster = if parts.len() > 5 {
                parts[5].to_string()
            } else {
                "DEFAULT".to_string()
            };
            return Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                parts[3].to_string(),
                port,
                cluster,
            ));
        }

        // Try simple format: service:ip:port
        let parts: Vec<&str> = service_id.split(':').collect();
        if parts.len() >= 3 {
            let port: i32 = parts[parts.len() - 1].parse().ok()?;
            let ip = parts[parts.len() - 2].to_string();
            let service = parts[0..parts.len() - 2].join(":");
            return Some((
                "public".to_string(),
                "DEFAULT_GROUP".to_string(),
                service,
                ip,
                port,
                "DEFAULT".to_string(),
            ));
        }

        None
    }

    /// Get a stored check from cache or database
    async fn get_stored_check(&self, check_id: &str) -> Result<StoredCheck, String> {
        // Check cache first
        if let Some(stored) = self.cache.get(check_id) {
            return Ok(stored.clone());
        }

        // Query from database
        let data_id = Self::check_id_to_data_id(check_id);
        match batata_config::service::config::find_one(
            &self.db,
            &data_id,
            CONSUL_CHECK_GROUP,
            CONSUL_CHECK_NAMESPACE,
        )
        .await
        {
            Ok(Some(config)) => {
                let metadata: CheckMetadata =
                    serde_json::from_str(&config.config_info.config_info_base.content)
                        .map_err(|e| format!("Failed to parse check metadata: {}", e))?;

                let stored = StoredCheck {
                    check: metadata.check,
                    ttl_seconds: metadata.ttl_seconds,
                    last_updated: metadata.last_updated,
                    deregister_after_secs: metadata.deregister_after_secs,
                };

                // Update cache
                self.cache.insert(check_id.to_string(), stored.clone());

                Ok(stored)
            }
            Ok(None) => Err(format!("Check not found: {}", check_id)),
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }

    /// Get a check by ID
    pub async fn get_check(&self, check_id: &str) -> Option<HealthCheck> {
        self.get_stored_check(check_id).await.ok().map(|s| s.check)
    }

    /// Get all checks for a service
    pub async fn get_service_checks(&self, service_id: &str) -> Vec<HealthCheck> {
        // Check cache first
        if let Some(check_ids) = self.service_checks_cache.get(service_id) {
            let mut checks = Vec::new();
            for id in check_ids.iter() {
                if let Some(check) = self.get_check(id).await {
                    checks.push(check);
                }
            }
            return checks;
        }

        // Query all checks from database and filter by service_id
        self.get_all_checks()
            .await
            .into_iter()
            .filter(|c| c.service_id == service_id)
            .collect()
    }

    /// Get all checks from database
    pub async fn get_all_checks(&self) -> Vec<HealthCheck> {
        // Search for all checks in ConfigService
        match batata_config::service::config::search_page(
            &self.db,
            1,
            1000,
            CONSUL_CHECK_NAMESPACE,
            "check:*",
            CONSUL_CHECK_GROUP,
            "",
            vec![],
            vec![],
            "",
        )
        .await
        {
            Ok(page) => {
                let mut checks = Vec::new();
                for info in page.page_items {
                    let check_id = Self::data_id_to_check_id(&info.data_id);
                    if let Some(check) = self.get_check(&check_id).await {
                        checks.push(check);
                    }
                }
                checks
            }
            Err(_) => Vec::new(),
        }
    }

    /// Get checks by status
    pub async fn get_checks_by_status(&self, status: &str) -> Vec<HealthCheck> {
        self.get_all_checks()
            .await
            .into_iter()
            .filter(|c| c.status == status)
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

        // Also include maintenance checks if they exist
        let maintenance_check_id = format!("_service_maintenance:{}", instance.instance_id);
        if let Some(maint_check) = health_service.get_check(&maintenance_check_id) {
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
// Persistent HTTP Handlers
// ============================================================================

/// GET /v1/health/service/:service (Persistent version)
pub async fn get_service_health_persistent(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());
    let passing_only = query.passing.unwrap_or(false);

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let instances =
        naming_service.get_instances(&namespace, "DEFAULT_GROUP", &service_name, "", passing_only);

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

        let mut checks = health_service
            .get_service_checks(&instance.instance_id)
            .await;
        if checks.is_empty() {
            checks.push(health_service.create_instance_check(&instance));
        }

        // Also include maintenance checks if they exist
        let maintenance_check_id = format!("_service_maintenance:{}", instance.instance_id);
        if let Some(maint_check) = health_service.get_check(&maintenance_check_id).await {
            checks.push(maint_check);
        }

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

/// GET /v1/health/checks/:service (Persistent version)
pub async fn get_service_checks_persistent(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

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

        for check in &mut checks {
            check.service_name = service_name.clone();
        }

        all_checks.extend(checks);
    }

    HttpResponse::Ok().json(all_checks)
}

/// GET /v1/health/state/:state (Persistent version)
pub async fn get_checks_by_state_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    let state = path.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

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

/// GET /v1/health/node/:node (Persistent version)
pub async fn get_node_checks_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    _query: web::Query<HealthQueryParams>,
) -> HttpResponse {
    let node = path.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Node, &node, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let checks: Vec<HealthCheck> = health_service
        .get_all_checks()
        .await
        .into_iter()
        .filter(|c| c.node == node)
        .collect();

    HttpResponse::Ok().json(checks)
}

/// PUT /v1/agent/check/register (Persistent version)
pub async fn register_check_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    body: web::Json<CheckRegistration>,
) -> HttpResponse {
    let registration = body.into_inner();

    let service_id = registration.service_id.as_deref().unwrap_or("");
    let authz = acl_service.authorize_request(&req, ResourceType::Service, service_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service.register_check(registration).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/deregister/:check_id (Persistent version)
pub async fn deregister_check_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let check_id = path.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service.deregister_check(&check_id).await {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/pass/:check_id (Persistent version)
pub async fn pass_check_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let output = query.note.clone();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service
        .update_check_status(&check_id, "passing", output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/warn/:check_id (Persistent version)
pub async fn warn_check_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let output = query.note.clone();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service
        .update_check_status(&check_id, "warning", output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/fail/:check_id (Persistent version)
pub async fn fail_check_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let output = query.note.clone();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    match health_service
        .update_check_status(&check_id, "critical", output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// PUT /v1/agent/check/update/:check_id (Persistent version)
pub async fn update_check_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<CheckStatusUpdate>,
) -> HttpResponse {
    let check_id = path.into_inner();
    let update = body.into_inner();

    let authz = acl_service.authorize_request(&req, ResourceType::Service, &check_id, true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let status = update.status.unwrap_or_else(|| "passing".to_string());

    match health_service
        .update_check_status(&check_id, &status, update.output)
        .await
    {
        Ok(()) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ConsulError::new(e)),
    }
}

/// GET /v1/agent/checks (Persistent version)
pub async fn list_agent_checks_persistent(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthServicePersistent>,
    acl_service: web::Data<AclService>,
    _query: web::Query<ServiceQueryParams>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let checks = health_service.get_all_checks().await;

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
