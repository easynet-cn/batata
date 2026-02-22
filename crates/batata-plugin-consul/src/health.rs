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
use crate::health_actor::{HealthActorHandle, NamingServiceTrait, start_deregistration_monitor};
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
    /// Check configuration parameters
    pub http: Option<String>,
    pub tcp: Option<String>,
    pub grpc: Option<String>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
}

/// Consul Health Check service (Actor-based implementation)
/// Manages health checks using the HealthStatusActor for lock-free operations
#[derive(Clone)]
pub struct ConsulHealthService {
    #[allow(dead_code)] // Reserved for future health check integration with naming service
    naming_service: Arc<NamingService>,
    /// Health status actor handle
    pub health_actor: HealthActorHandle,
    /// Node name for this agent
    node_name: String,
}

impl ConsulHealthService {
    pub fn new(naming_service: Arc<NamingService>, health_actor: HealthActorHandle) -> Self {
        Self {
            naming_service,
            health_actor,
            node_name: "batata-node".to_string(),
        }
    }

    /// Create a new ConsulHealthService with critical health check monitoring enabled
    /// This will automatically deregister services that have been in critical state
    /// for longer than the configured `deregister_critical_service_after` threshold.
    ///
    /// # Arguments
    /// * `naming_service` - The naming service for instance management
    /// * `health_actor` - The health actor for health check management
    /// * `check_reap_interval_secs` - Interval in seconds to check for critical services (default: 30)
    pub fn new_with_monitor(
        naming_service: Arc<NamingService>,
        health_actor: HealthActorHandle,
        check_reap_interval_secs: u64,
    ) -> Self {
        let naming_service_clone = naming_service.clone();
        let health_actor_clone = health_actor.clone();

        // Create a wrapper that implements NamingServiceTrait
        let naming_service_wrapper = Arc::new(NamingServiceWrapper::new(naming_service_clone));

        // Start the background monitor task
        tokio::spawn(async move {
            start_deregistration_monitor(
                health_actor_clone,
                Some(naming_service_wrapper),
                check_reap_interval_secs,
            ).await;
        });

        Self {
            naming_service,
            health_actor,
            node_name: "batata-node".to_string(),
        }
    }

    /// Get the health actor handle
    pub fn health_actor_handle(&self) -> HealthActorHandle {
        self.health_actor.clone()
    }

    /// Update check status asynchronously (via actor)
    pub async fn update_check_status_async(
        &self,
        check_id: &str,
        status: &str,
        output: Option<String>,
    ) -> Result<(), String> {
        self.health_actor
            .update_status(check_id.to_string(), status.to_string(), output)
            .await
    }

    /// Register a health check (via actor)
    pub async fn register_check(&self, registration: CheckRegistration) -> Result<(), String> {
        let check_config = crate::health_actor::HealthActorHandle::registration_to_config(&registration);
        self.health_actor.register_check(check_config).await
    }

    /// Deregister a health check (via actor)
    pub async fn deregister_check(&self, check_id: &str) -> Result<(), String> {
        self.health_actor.deregister_check(check_id.to_string()).await
    }

    /// Get all checks for a service (via actor)
    pub async fn get_service_checks(&self, service_id: &str) -> Vec<HealthCheck> {
        self.health_actor.get_service_checks(service_id.to_string()).await
    }

    /// Get a check by ID (via actor)
    pub async fn get_check(&self, check_id: &str) -> Option<HealthCheck> {
        let config = self.health_actor.get_config(check_id.to_string()).await?;
        let status = self.health_actor.get_status(check_id.to_string()).await?;

        Some(HealthCheck {
            node: self.node_name.clone(),
            check_id: config.check_id,
            name: config.name,
            status: status.status,
            notes: String::new(),
            output: status.output,
            service_id: config.service_id,
            service_name: config.service_name,
            service_tags: None,
            check_type: config.check_type,
            interval: config.interval,
            timeout: config.timeout,
            create_index: Some(1),
            modify_index: Some(1),
        })
    }

    /// Get all checks (via actor)
    pub async fn get_all_checks(&self) -> Vec<HealthCheck> {
        let configs: Vec<(String, crate::health_actor::CheckConfig)> =
            self.health_actor.get_all_configs().await;
        let statuses: Vec<(String, crate::health_actor::HealthStatus)> =
            self.health_actor.get_all_status().await;

        configs
            .into_iter()
            .filter_map(|(check_id, config)| {
                statuses
                    .iter()
                    .find(|(id, _)| id == &check_id)
                    .map(|(_, status)| HealthCheck {
                        node: self.node_name.clone(),
                        check_id: config.check_id,
                        name: config.name,
                        status: status.status.clone(),
                        notes: String::new(),
                        output: status.output.clone(),
                        service_id: config.service_id,
                        service_name: config.service_name,
                        service_tags: None,
                        check_type: config.check_type,
                        interval: config.interval.clone(),
                        timeout: config.timeout.clone(),
                        create_index: Some(1),
                        modify_index: Some(1),
                    })
            })
            .collect()
    }

    /// Get checks by status (via actor)
    pub async fn get_checks_by_status(&self, status: &str) -> Vec<HealthCheck> {
        self.health_actor.get_checks_by_status(status.to_string()).await
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
            interval: registration.interval.clone(),
            timeout: registration.timeout.clone(),
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
            http: registration.http.clone(),
            tcp: registration.tcp.clone(),
            grpc: registration.grpc.clone(),
            interval: registration.interval.clone(),
            timeout: registration.timeout.clone(),
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
                http: stored.http.clone(),
                tcp: stored.tcp.clone(),
                grpc: stored.grpc.clone(),
                interval: stored.interval.clone(),
                timeout: stored.timeout.clone(),
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
                    http: None,
                    tcp: None,
                    grpc: None,
                    interval: None,
                    timeout: None,
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
            interval: None,
            timeout: None,
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
        let mut checks = health_service.get_service_checks(&instance.instance_id).await;
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
        let mut checks = health_service.get_service_checks(&instance.instance_id).await;
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

    match health_service.update_check_status_async(&check_id, "passing", output).await {
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

    match health_service.update_check_status_async(&check_id, "warning", output).await {
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

    match health_service.update_check_status_async(&check_id, "critical", output).await {
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

    match health_service.update_check_status_async(&check_id, &status, update.output).await {
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

    fn make_check_reg(
        name: &str,
        check_id: Option<&str>,
        service_id: Option<&str>,
    ) -> CheckRegistration {
        CheckRegistration {
            name: name.to_string(),
            check_id: check_id.map(|s| s.to_string()),
            service_id: service_id.map(|s| s.to_string()),
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

    #[test]
    fn test_default_status_is_critical() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let reg = make_check_reg("crit-check", Some("crit-1"), None);
        service.register_check(reg).unwrap();

        let check = service.get_check("crit-1").unwrap();
        assert_eq!(check.status, "critical");
    }

    #[test]
    fn test_deregister_nonexistent_check() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let result = service.deregister_check("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Check not found"));
    }

    #[test]
    fn test_update_nonexistent_check() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let result = service.update_check_status("nonexistent", "passing", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Check not found"));
    }

    #[test]
    fn test_get_nonexistent_check() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        assert!(service.get_check("nonexistent").is_none());
    }

    #[test]
    fn test_service_checks_mapping() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        // Register multiple checks for the same service
        let mut reg1 = make_check_reg("check-a", Some("svc-chk-a"), Some("svc-mapping"));
        reg1.status = Some("passing".to_string());
        service.register_check(reg1).unwrap();

        let mut reg2 = make_check_reg("check-b", Some("svc-chk-b"), Some("svc-mapping"));
        reg2.status = Some("warning".to_string());
        service.register_check(reg2).unwrap();

        let checks = service.get_service_checks("svc-mapping");
        assert_eq!(checks.len(), 2);

        let ids: Vec<&str> = checks.iter().map(|c| c.check_id.as_str()).collect();
        assert!(ids.contains(&"svc-chk-a"));
        assert!(ids.contains(&"svc-chk-b"));
    }

    #[test]
    fn test_service_checks_empty_for_unknown_service() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let checks = service.get_service_checks("unknown-service");
        assert!(checks.is_empty());
    }

    #[test]
    fn test_deregister_removes_from_service_mapping() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let mut reg = make_check_reg("dereg-map", Some("dereg-chk"), Some("dereg-svc"));
        reg.status = Some("passing".to_string());
        service.register_check(reg).unwrap();

        assert_eq!(service.get_service_checks("dereg-svc").len(), 1);

        service.deregister_check("dereg-chk").unwrap();
        assert!(service.get_service_checks("dereg-svc").is_empty());
    }

    #[test]
    fn test_get_all_checks() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let reg1 = make_check_reg("all-a", Some("all-chk-1"), None);
        let reg2 = make_check_reg("all-b", Some("all-chk-2"), None);
        service.register_check(reg1).unwrap();
        service.register_check(reg2).unwrap();

        let all = service.get_all_checks();
        assert!(all.len() >= 2);
        let ids: Vec<&str> = all.iter().map(|c| c.check_id.as_str()).collect();
        assert!(ids.contains(&"all-chk-1"));
        assert!(ids.contains(&"all-chk-2"));
    }

    #[test]
    fn test_get_checks_by_status() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let mut reg1 = make_check_reg("stat-a", Some("stat-pass"), None);
        reg1.status = Some("passing".to_string());
        service.register_check(reg1).unwrap();

        let mut reg2 = make_check_reg("stat-b", Some("stat-warn"), None);
        reg2.status = Some("warning".to_string());
        service.register_check(reg2).unwrap();

        let mut reg3 = make_check_reg("stat-c", Some("stat-crit"), None);
        reg3.status = Some("critical".to_string());
        service.register_check(reg3).unwrap();

        let passing = service.get_checks_by_status("passing");
        assert!(passing.iter().any(|c| c.check_id == "stat-pass"));
        assert!(passing.iter().all(|c| c.status == "passing"));

        let warning = service.get_checks_by_status("warning");
        assert!(warning.iter().any(|c| c.check_id == "stat-warn"));
        assert!(warning.iter().all(|c| c.status == "warning"));

        let critical = service.get_checks_by_status("critical");
        assert!(critical.iter().any(|c| c.check_id == "stat-crit"));
        assert!(critical.iter().all(|c| c.status == "critical"));
    }

    #[test]
    fn test_status_transitions() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let mut reg = make_check_reg("trans", Some("trans-chk"), None);
        reg.status = Some("passing".to_string());
        service.register_check(reg).unwrap();

        // passing -> warning -> critical -> passing
        service
            .update_check_status("trans-chk", "warning", Some("Slow response".to_string()))
            .unwrap();
        let check = service.get_check("trans-chk").unwrap();
        assert_eq!(check.status, "warning");
        assert_eq!(check.output, "Slow response");

        service
            .update_check_status("trans-chk", "critical", Some("Down".to_string()))
            .unwrap();
        let check = service.get_check("trans-chk").unwrap();
        assert_eq!(check.status, "critical");

        service
            .update_check_status("trans-chk", "passing", None)
            .unwrap();
        let check = service.get_check("trans-chk").unwrap();
        assert_eq!(check.status, "passing");
        // output should remain from previous update since we passed None
        assert_eq!(check.output, "Down");
    }

    #[test]
    fn test_check_node_name() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let reg = make_check_reg("node-chk", Some("node-chk-1"), None);
        service.register_check(reg).unwrap();

        let check = service.get_check("node-chk-1").unwrap();
        assert_eq!(check.node, "batata-node");
    }

    #[test]
    fn test_check_notes_and_output() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let mut reg = make_check_reg("noted", Some("noted-chk"), None);
        reg.notes = Some("Important check".to_string());
        reg.status = Some("passing".to_string());
        service.register_check(reg).unwrap();

        let check = service.get_check("noted-chk").unwrap();
        assert_eq!(check.notes, "Important check");
        assert!(check.output.is_empty()); // output starts empty
    }

    #[test]
    fn test_ttl_and_deregister_timeout_parsing() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let reg = CheckRegistration {
            name: "ttl-check".to_string(),
            check_id: Some("ttl-chk".to_string()),
            service_id: None,
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
        service.register_check(reg).unwrap();

        // Verify stored metadata (access the internal DashMap)
        let stored = service.checks.get("ttl-chk").unwrap();
        assert_eq!(stored.ttl_seconds, Some(60));
        assert_eq!(stored.deregister_after_secs, Some(300));
    }

    #[test]
    fn test_create_instance_check_healthy() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

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

    #[test]
    fn test_create_instance_check_unhealthy() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let mut instance = NacosInstance::new("10.0.0.2".to_string(), 8080);
        instance.instance_id = "web-002".to_string();
        instance.service_name = "web".to_string();
        instance.healthy = false;
        instance.enabled = true;

        let check = service.create_instance_check(&instance);
        assert_eq!(check.status, "critical");
        assert_eq!(check.output, "Instance is unhealthy");
    }

    #[test]
    fn test_create_instance_check_with_consul_tags() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

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

    #[test]
    fn test_create_instance_check_without_tags() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

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

    #[test]
    fn test_register_multiple_services_separate_checks() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let reg1 = make_check_reg("chk-svc1", Some("multi-svc1-chk"), Some("multi-svc1"));
        let reg2 = make_check_reg("chk-svc2", Some("multi-svc2-chk"), Some("multi-svc2"));
        service.register_check(reg1).unwrap();
        service.register_check(reg2).unwrap();

        assert_eq!(service.get_service_checks("multi-svc1").len(), 1);
        assert_eq!(service.get_service_checks("multi-svc2").len(), 1);

        // Deregistering one doesn't affect the other
        service.deregister_check("multi-svc1-chk").unwrap();
        assert!(service.get_service_checks("multi-svc1").is_empty());
        assert_eq!(service.get_service_checks("multi-svc2").len(), 1);
    }

    #[test]
    fn test_check_reregistration_overwrites() {
        let naming_service = Arc::new(NamingService::new());
        let service = ConsulHealthService::new(naming_service);

        let mut reg = make_check_reg("overwrite", Some("ow-chk"), None);
        reg.status = Some("passing".to_string());
        service.register_check(reg).unwrap();

        let check = service.get_check("ow-chk").unwrap();
        assert_eq!(check.status, "passing");

        // Re-register with different status
        let mut reg2 = make_check_reg("overwrite-v2", Some("ow-chk"), None);
        reg2.status = Some("critical".to_string());
        service.register_check(reg2).unwrap();

        let check = service.get_check("ow-chk").unwrap();
        assert_eq!(check.name, "overwrite-v2");
        assert_eq!(check.status, "critical");
    }
}

// ============================================================================
// NamingServiceTrait Implementation
// ============================================================================

/// Wrapper for NamingService to implement NamingServiceTrait
#[derive(Clone)]
pub struct NamingServiceWrapper {
    naming_service: Arc<NamingService>,
}

impl NamingServiceWrapper {
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self { naming_service }
    }
}

#[async_trait::async_trait]
impl NamingServiceTrait for NamingServiceWrapper {
    async fn deregister_instance(
        &self,
        namespace: &str,
        group: &str,
        service_name: &str,
        instance_id: &str,
        _ephemeral: bool,
    ) -> Result<(), String> {
        use batata_api::naming::model::Instance;

        // Build an instance with just the instance_id for deregistration
        let instance = Instance {
            instance_id: instance_id.to_string(),
            ..Default::default()
        };

        let result = self.naming_service.deregister_instance(
            namespace,
            group,
            service_name,
            &instance,
        );

        if result {
            Ok(())
        } else {
            Err(format!("Failed to deregister instance: {}", instance_id))
        }
    }
}

