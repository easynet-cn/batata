// Consul Agent API HTTP handlers
// Implements Consul-compatible service registration endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use sysinfo::System;

use batata_common::{ClusterManager, MemberState};
use batata_naming::healthcheck::registry::InstanceCheckRegistry;

use crate::acl::{AclService, ResourceType};
use crate::check_index::ConsulCheckIndex;
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::health::ConsulHealthService;
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::{
    AgentConfig, AgentHostInfo, AgentMaintenanceRequest, AgentMember, AgentMembersParams,
    AgentSelf, AgentService, AgentServiceChecksInfo, AgentServiceRegistration,
    AgentServiceWithChecks, AgentStats, AgentVersion, CONSUL_INTERNAL_CLUSTER,
    CONSUL_INTERNAL_GROUP, CONSUL_INTERNAL_NAMESPACE, CheckRegistration, ConsulDatacenterConfig,
    ConsulError, ConsulErrorBody, Coordinate, CounterMetric, GaugeMetric, HealthCheck, HostCPU,
    HostDisk, HostInfo, HostMemory, MaintenanceRequest, MetricsResponse, SampleMetric,
    ServiceQueryParams,
};
use crate::naming_store::ConsulNamingStore;
use crate::raft::{ConsulRaftRequest, ConsulRaftWriter};
use batata_plugin::PluginNamingStore;

/// Consul Agent service adapter
///
/// Uses ConsulNamingStore for native Consul service storage, completely
/// independent from the core Batata NamingService.
#[derive(Clone)]
pub struct ConsulAgentService {
    naming_store: Arc<ConsulNamingStore>,
    registry: Arc<InstanceCheckRegistry>,
    check_index: Arc<ConsulCheckIndex>,
    /// Optional Raft writer for cluster-mode replication
    raft_node: Option<Arc<ConsulRaftWriter>>,
}

impl ConsulAgentService {
    pub fn new(
        naming_store: Arc<ConsulNamingStore>,
        registry: Arc<InstanceCheckRegistry>,
        check_index: Arc<ConsulCheckIndex>,
    ) -> Self {
        Self {
            naming_store,
            registry,
            check_index,
            raft_node: None,
        }
    }

    /// Create an agent service with Raft-replicated storage (cluster mode).
    pub fn with_raft(
        naming_store: Arc<ConsulNamingStore>,
        registry: Arc<InstanceCheckRegistry>,
        check_index: Arc<ConsulCheckIndex>,
        raft_node: Arc<ConsulRaftWriter>,
    ) -> Self {
        Self {
            naming_store,
            registry,
            check_index,
            raft_node: Some(raft_node),
        }
    }

    /// Get the consul naming store
    pub fn naming_store(&self) -> &Arc<ConsulNamingStore> {
        &self.naming_store
    }

    /// Register the "consul" service and "serfHealth" check at startup.
    /// Matches Consul's leader_registrator_v1.go HandleAliveMember() behavior.
    pub async fn register_consul_service(
        &self,
        dc_config: &ConsulDatacenterConfig,
    ) -> Result<(), String> {
        use batata_common::local_ip;

        let ip = local_ip();
        let raft_port = dc_config.raft_port();

        // In cluster mode, each node registers its own consul service instance.
        // Use a node-unique service_id so instances don't overwrite each other
        // when replicated through Raft.
        let service_id = format!("consul-{}", dc_config.node_name);

        // Build service metadata matching Consul's exact fields
        let mut service_meta = HashMap::new();
        service_meta.insert("non_voter".to_string(), "false".to_string());
        service_meta.insert("read_replica".to_string(), "false".to_string());
        service_meta.insert("raft_version".to_string(), "3".to_string());
        service_meta.insert("serf_protocol_current".to_string(), "2".to_string());
        service_meta.insert("serf_protocol_min".to_string(), "1".to_string());
        service_meta.insert("serf_protocol_max".to_string(), "5".to_string());
        service_meta.insert("version".to_string(), dc_config.full_version());

        let reg = AgentServiceRegistration {
            id: Some(service_id.clone()),
            name: "consul".to_string(),
            tags: Some(vec![]),
            port: Some(raft_port),
            address: Some(String::new()),
            meta: Some(service_meta),
            weights: Some(crate::model::Weights {
                passing: 1,
                warning: 1,
            }),
            proxy: Some(serde_json::json!({"Mode": "", "MeshGateway": {}, "Expose": {}})),
            connect: Some(serde_json::json!({})),
            ..Default::default()
        };

        let store_key = ConsulNamingStore::build_key(
            crate::namespace::DEFAULT_NAMESPACE,
            "consul",
            &service_id,
        );
        let reg_json = serde_json::to_vec(&reg)
            .map_err(|e| format!("Failed to serialize consul registration: {}", e))?;

        let _ = self
            .naming_store
            .register(&store_key, bytes::Bytes::from(reg_json.clone()));

        if let Some(ref raft) = self.raft_node {
            let registration_json = String::from_utf8_lossy(&reg_json).to_string();
            match raft
                .write(ConsulRaftRequest::CatalogRegister {
                    key: store_key.clone(),
                    registration_json,
                })
                .await
            {
                Ok(r) if !r.success => {
                    tracing::error!("Raft CatalogRegister rejected: {:?}", r.message);
                }
                Err(e) => {
                    tracing::error!("Raft CatalogRegister failed: {}", e);
                }
                _ => {}
            }
        }

        let service_key = format!(
            "{}#{}#{}",
            CONSUL_INTERNAL_NAMESPACE, CONSUL_INTERNAL_GROUP, "consul"
        );
        let instance_key = format!(
            "{}#{}#{}#{}#{}#{}",
            CONSUL_INTERNAL_NAMESPACE,
            CONSUL_INTERNAL_GROUP,
            "consul",
            ip,
            raft_port,
            CONSUL_INTERNAL_CLUSTER
        );
        self.check_index
            .register(&service_id, &service_key, &instance_key);

        // Register serfHealth as NODE-level check (not associated with any service)
        {
            use batata_naming::healthcheck::registry::*;
            self.registry.register_check(InstanceCheckConfig {
                check_id: "serfHealth".to_string(),
                name: "Serf Health Status".to_string(),
                check_type: CheckType::None,
                namespace: CONSUL_INTERNAL_NAMESPACE.to_string(),
                group_name: CONSUL_INTERNAL_GROUP.to_string(),
                service_name: String::new(),
                ip: ip.clone(),
                port: 0,
                cluster_name: CONSUL_INTERNAL_CLUSTER.to_string(),
                http_url: None,
                tcp_addr: None,
                grpc_addr: None,
                db_url: None,
                interval: std::time::Duration::ZERO,
                timeout: std::time::Duration::ZERO,
                ttl: None,
                success_before_passing: 0,
                failures_before_critical: 0,
                deregister_critical_after: None,
                initial_status: CheckStatus::Passing,
                notes: String::new(),
                service_tags: vec![],
            });
            self.registry.update_check_result(
                "serfHealth",
                true,
                "Agent alive and reachable".to_string(),
                0,
            );
        }

        tracing::info!(
            "Consul self-service registered: node={}, addr={}:{}, dc={}",
            dc_config.node_name,
            ip,
            raft_port,
            dc_config.datacenter,
        );
        Ok(())
    }

    /// Deregister Consul service
    pub async fn deregister_consul_service(&self, dc_config: &ConsulDatacenterConfig) {
        let service_id = format!("consul-{}", dc_config.node_name);
        let store_key = ConsulNamingStore::build_key(
            crate::namespace::DEFAULT_NAMESPACE,
            "consul",
            &service_id,
        );
        self.naming_store
            .remove_by_service_id(crate::namespace::DEFAULT_NAMESPACE, &service_id);
        self.check_index.remove(&service_id);

        if let Some(ref raft) = self.raft_node {
            match raft
                .write(ConsulRaftRequest::CatalogDeregister { key: store_key })
                .await
            {
                Ok(r) if !r.success => {
                    tracing::error!("Raft CatalogDeregister rejected: {:?}", r.message);
                }
                Err(e) => {
                    tracing::error!("Raft CatalogDeregister failed: {}", e);
                }
                _ => {}
            }
        }

        tracing::info!("Consul service deregistered");
    }
}

/// PUT /v1/agent/service/register
/// Register a new service with the local agent
#[allow(clippy::too_many_arguments)]
pub async fn register_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    health_service: web::Data<ConsulHealthService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<ServiceQueryParams>,
    body: web::Json<AgentServiceRegistration>,
) -> HttpResponse {
    let registration = body.into_inner();

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &registration.name,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Validate service name
    if registration.name.is_empty() {
        return HttpResponse::BadRequest().consul_error(ConsulError::new("Missing service name"));
    }

    // Validate service registration (address, weights, metadata)
    let mut registration = registration;
    if let Err(e) = registration.validate() {
        return HttpResponse::BadRequest()
            .consul_error(ConsulError::new(format!("Invalid service: {}", e)));
    }

    // Auto-populate tagged addresses from service address (like Consul)
    registration.auto_populate_tagged_addresses();

    // Extract and validate checks using Consul's CheckTypes() logic
    let validated_checks = match registration.check_types() {
        Ok(checks) => checks,
        Err(e) => {
            return HttpResponse::BadRequest()
                .consul_error(ConsulError::new(format!("Validation failed: {}", e)));
        }
    };

    // Get service ID
    let service_id = registration.service_id();

    // Get the actual IP and port for health check registration
    let instance_ip = registration.effective_address();
    let instance_port = registration.effective_port() as i32;

    // Convert validated checks to CheckRegistration, populating IP and port
    let embedded_checks: Vec<CheckRegistration> = validated_checks
        .iter()
        .map(|vc| {
            let mut cr = vc.to_check_registration();
            cr.ip = Some(instance_ip.clone());
            cr.port = Some(instance_port);
            cr
        })
        .collect();

    tracing::info!(
        "Registering service: name={}, id={}, address={}, port={}, checks={}",
        registration.name,
        service_id,
        registration.address.as_ref().unwrap_or(&"N/A".to_string()),
        registration.port.unwrap_or(0),
        validated_checks.len()
    );

    // Bug #2 fix: If the same consul_service_id was previously registered with
    // different IP/port, clean up old health checks to avoid orphans.
    if let Some((_, old_instance_key)) = agent.check_index.lookup(&service_id) {
        let parts: Vec<&str> = old_instance_key.splitn(6, '#').collect();
        if parts.len() >= 6 {
            let old_ip = parts[3];
            let old_port: i32 = parts[4].parse().unwrap_or(0);
            if old_ip != instance_ip || old_port != instance_port {
                tracing::info!(
                    "Re-registration detected for consul_service_id={}: old={}:{}, new={}:{}. Cleaning up old checks.",
                    service_id,
                    old_ip,
                    old_port,
                    instance_ip,
                    instance_port
                );
                agent
                    .registry
                    .deregister_all_instance_checks(&old_instance_key);
            }
        }
        agent.check_index.remove(&service_id);
    }

    // Store in Consul naming store (native format — no conversion overhead)
    let namespace = dc_config.resolve_ns(&query.ns);
    let store_key = ConsulNamingStore::build_key(&namespace, &registration.name, &service_id);
    let reg_data = match serde_json::to_vec(&registration) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Failed to serialize service registration: {}", e);
            return HttpResponse::InternalServerError()
                .consul_error(ConsulError::new("Failed to register service"));
        }
    };

    if let Err(e) = agent
        .naming_store
        .register(&store_key, bytes::Bytes::from(reg_data.clone()))
        .map_err(|e| e.to_string())
    {
        tracing::error!("Failed to store service in ConsulNamingStore: {}", e);
        return HttpResponse::InternalServerError()
            .consul_error(ConsulError::new("Failed to register service"));
    }

    // Replicate through Raft in cluster mode
    if let Some(ref raft) = agent.raft_node {
        let registration_json = String::from_utf8_lossy(&reg_data).to_string();
        match raft
            .write(ConsulRaftRequest::CatalogRegister {
                key: store_key.clone(),
                registration_json,
            })
            .await
        {
            Ok(r) if !r.success => {
                tracing::error!("Raft CatalogRegister rejected: {:?}", r.message);
            }
            Err(e) => {
                tracing::error!("Raft CatalogRegister failed: {}", e);
            }
            _ => {}
        }
    }

    index_provider.increment(ConsulTable::Catalog);

    // Register the consul_service_id → instance mapping for O(1) lookup
    let instance_key = format!(
        "{}#{}#{}#{}#{}#{}",
        CONSUL_INTERNAL_NAMESPACE,
        CONSUL_INTERNAL_GROUP,
        registration.name,
        instance_ip,
        instance_port,
        CONSUL_INTERNAL_CLUSTER
    );
    let service_key = format!(
        "{}#{}#{}",
        CONSUL_INTERNAL_NAMESPACE, CONSUL_INTERNAL_GROUP, registration.name
    );
    agent
        .check_index
        .register(&service_id, &service_key, &instance_key);

    // Register validated checks with health service and persist via Raft
    for check_reg in embedded_checks {
        let check_id = check_reg
            .check_id
            .clone()
            .unwrap_or_else(|| "?".to_string());
        if let Err(e) = health_service.register_check(check_reg).await {
            tracing::warn!(
                "Failed to register embedded check '{}' for service '{}': {}",
                check_id,
                service_id,
                e
            );
        } else if let Some(ref raft) = agent.raft_node {
            // Persist check config for restart recovery
            if let Some(config) = agent.registry.get_check_config(&check_id) {
                if let Ok(config_json) = serde_json::to_string(&config) {
                    let _ = raft
                        .write(ConsulRaftRequest::HealthCheckRegister {
                            check_id: check_id.clone(),
                            config_json,
                        })
                        .await;
                }
            }
        }
    }

    // Handle replace-existing-checks parameter (like Consul)
    if query.replace_existing_checks.unwrap_or(false) {
        let new_check_ids: std::collections::HashSet<String> = validated_checks
            .iter()
            .map(|vc| vc.check_id.clone())
            .collect();
        let existing_checks = health_service.get_service_checks(&service_id).await;
        for check in existing_checks {
            if !new_check_ids.contains(&check.check_id) {
                let _ = health_service.deregister_check(&check.check_id).await;
            }
        }
    }

    // Handle Connect.SidecarService — extract and register as a separate proxy service
    // (matching Consul's agent_endpoint.go SidecarService handling)
    if let Some(ref connect) = registration.connect {
        if let Some(sidecar_def) = connect
            .get("SidecarService")
            .or(connect.get("sidecar_service"))
        {
            if let Ok(mut sidecar) =
                serde_json::from_value::<AgentServiceRegistration>(sidecar_def.clone())
            {
                // Auto-populate sidecar defaults
                if sidecar.name.is_empty() {
                    sidecar.name = format!("{}-sidecar-proxy", registration.name);
                }
                if sidecar.id.is_none() {
                    sidecar.id = Some(format!("{}-sidecar-proxy", service_id));
                }
                if sidecar.port.is_none() {
                    // Consul auto-assigns a port; use service port + 20000 as convention
                    sidecar.port = registration.port.map(|p| p.saturating_add(20000));
                }
                if sidecar.kind.is_none() {
                    sidecar.kind = Some("connect-proxy".to_string());
                }

                let sidecar_id = sidecar.service_id();
                let sidecar_key =
                    ConsulNamingStore::build_key(&namespace, &sidecar.name, &sidecar_id);
                if let Ok(sidecar_data) = serde_json::to_vec(&sidecar) {
                    let _ = agent
                        .naming_store
                        .register(&sidecar_key, bytes::Bytes::from(sidecar_data.clone()));

                    if let Some(ref raft) = agent.raft_node {
                        let sidecar_json = String::from_utf8_lossy(&sidecar_data).to_string();
                        let _ = raft
                            .write(ConsulRaftRequest::CatalogRegister {
                                key: sidecar_key,
                                registration_json: sidecar_json,
                            })
                            .await;
                    }

                    tracing::info!(
                        "Sidecar proxy registered: name={}, id={}",
                        sidecar.name,
                        sidecar_id
                    );
                }
            }
        }
    }

    tracing::info!(
        "Service registered: name={}, id={}",
        registration.name,
        service_id
    );

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

/// PUT /v1/agent/service/deregister/{service_id}
/// Deregister a service from the local agent
#[allow(clippy::too_many_arguments)]
pub async fn deregister_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    health_service: web::Data<ConsulHealthService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
) -> HttpResponse {
    let service_id = path.into_inner();

    // Anti-entropy protection: prevent deregistration of the built-in consul service
    // Aligned with Consul original (agent/local/state.go - skips consul service in updateSyncState)
    // Anti-entropy: prevent deregistering the built-in consul service for this node
    if service_id == format!("consul-{}", dc_config.node_name) || service_id == "consul" {
        return HttpResponse::Ok().finish();
    }

    let namespace = dc_config.resolve_ns(&query.ns);

    // Check ACL authorization for service write (deregister requires write)
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_id,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Find the store key before removal (needed for Raft replication)
    let store_key = agent
        .naming_store
        .find_key_by_service_id(&namespace, &service_id);

    // Remove from Consul naming store
    let deregistered = agent
        .naming_store
        .remove_by_service_id(&namespace, &service_id);

    if deregistered {
        // Clean up registry entries via O(1) lookup
        if let Some((_, instance_key)) = agent.check_index.lookup(&service_id) {
            // Collect check IDs before deregistering (for Raft persistence)
            let check_ids: Vec<String> = agent.registry.get_check_keys_for_instance(&instance_key);

            agent.registry.deregister_all_instance_checks(&instance_key);

            // Persist check deregistration via Raft
            if let Some(ref raft) = agent.raft_node {
                for cid in check_ids {
                    let _ = raft
                        .write(ConsulRaftRequest::HealthCheckDeregister { check_id: cid })
                        .await;
                }
            }
        }
        agent.check_index.remove(&service_id);

        // Replicate through Raft in cluster mode
        if let Some(ref raft) = agent.raft_node
            && let Some(key) = &store_key
        {
            match raft
                .write(ConsulRaftRequest::CatalogDeregister { key: key.clone() })
                .await
            {
                Ok(r) if !r.success => {
                    tracing::error!("Raft CatalogDeregister rejected: {:?}", r.message);
                }
                Err(e) => {
                    tracing::error!("Raft CatalogDeregister failed: {}", e);
                }
                _ => {}
            }
        }
    }

    if deregistered {
        index_provider.increment(ConsulTable::Catalog);
        // Clean up any associated health checks from old system
        let service_checks = health_service.get_service_checks(&service_id).await;
        for check in &service_checks {
            let _ = health_service.deregister_check(&check.check_id).await;
        }
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
        consul_ok(&meta).finish()
    } else {
        HttpResponse::NotFound().consul_error(ConsulError::new(format!(
            "Service not found: {}",
            service_id
        )))
    }
}

/// GET /v1/agent/services
/// Returns all services registered with the local agent
pub async fn list_services(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let namespace = dc_config.resolve_ns(&query.ns);

    // Check ACL authorization for service read (list requires read on all services)
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        "",    // empty prefix means all services
        false, // read access only
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Get all service entries from ConsulNamingStore
    let all_entries = agent.naming_store.scan_ns(&namespace);
    let mut services: std::collections::HashMap<String, AgentService> =
        std::collections::HashMap::new();

    for (_key, data) in all_entries {
        if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data) {
            let agent_service = AgentService::from(&reg);
            services.insert(agent_service.id.clone(), agent_service);
        }
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(services)
}

/// GET /v1/agent/service/{service_id}
/// Returns the full service definition for a single service instance
pub async fn get_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);

    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_id,
        false, // read access only
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Find the service by ID from ConsulNamingStore
    if let Some(data) = agent
        .naming_store
        .get_by_service_id(&namespace, &service_id)
        && let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data)
    {
        let agent_service = AgentService::from(&reg);
        let healthy = agent
            .naming_store
            .is_healthy(&reg.effective_address(), reg.effective_port() as i32);
        let checks = vec![create_service_health_check_from_reg(&reg, healthy)];
        let response = AgentServiceWithChecks {
            service: agent_service,
            checks: Some(checks),
        };
        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
        return consul_ok(&meta).json(response);
    }

    HttpResponse::NotFound().consul_error(ConsulError::new(format!(
        "Service not found: {}",
        service_id
    )))
}

// ============================================================================
// Health Check Helper Functions
// ============================================================================

/// Create a health check for a service instance from its registration and health status.
fn create_service_health_check_from_reg(
    reg: &AgentServiceRegistration,
    healthy: bool,
) -> crate::model::AgentCheck {
    use crate::model::AgentCheck;

    let service_id = reg.service_id();
    let (status, output, notes) = if healthy {
        (
            "passing",
            format!("Service '{}' is healthy", reg.name),
            "Service is running and accepting connections".to_string(),
        )
    } else {
        (
            "critical",
            format!("Service '{}' is unhealthy", reg.name),
            "Service is not responding or health check failed".to_string(),
        )
    };

    AgentCheck {
        check_id: format!("service:{}:{}", reg.name, service_id),
        name: format!("{} Health Check", reg.name),
        status: status.to_string(),
        notes,
        output,
        service_id,
        service_name: reg.name.clone(),
        check_type: "service".to_string(),
    }
}

/// Aggregate the worst status from a list of health checks
fn aggregate_status(checks: &[HealthCheck]) -> String {
    let mut worst = "passing";
    for check in checks {
        match check.status.as_str() {
            "critical" => return "critical".to_string(),
            "warning" => worst = "warning",
            _ => {}
        }
    }
    worst.to_string()
}

/// GET /v1/agent/health/service/id/{service_id}
/// Returns the aggregated health status of a service by its ID
#[allow(clippy::too_many_arguments)]
pub async fn agent_health_service_by_id(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);

    let authz = acl_service.authorize_request(&req, ResourceType::Agent, &service_id, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Search for the service instance by ID from ConsulNamingStore
    if let Some(data) = agent
        .naming_store
        .get_by_service_id(&namespace, &service_id)
        && let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data)
    {
        let healthy = agent
            .naming_store
            .is_healthy(&reg.effective_address(), reg.effective_port() as i32);
        let agent_service = AgentService::from(&reg);
        let checks = health_service.get_service_checks(&service_id).await;
        let status = if checks.is_empty() {
            if healthy {
                "passing".to_string()
            } else {
                "critical".to_string()
            }
        } else {
            aggregate_status(&checks)
        };

        let info = AgentServiceChecksInfo {
            aggregated_status: status.clone(),
            service: agent_service,
            checks,
        };

        let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
        return match status.as_str() {
            "passing" => consul_ok(&meta).json(info),
            "warning" => {
                let mut b = HttpResponse::TooManyRequests();
                meta.apply_headers(&mut b);
                b.json(info)
            }
            _ => {
                let mut b = HttpResponse::ServiceUnavailable();
                meta.apply_headers(&mut b);
                b.json(info)
            }
        };
    }

    HttpResponse::NotFound().consul_error(ConsulError::new(format!(
        "Service ID '{}' not found",
        service_id
    )))
}

/// GET /v1/agent/health/service/name/{service_name}
/// Returns the aggregated health status of all instances of a service by name
#[allow(clippy::too_many_arguments)]
pub async fn agent_health_service_by_name(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_name = path.into_inner();
    let namespace = dc_config.resolve_ns(&query.ns);

    let authz = acl_service.authorize_request(&req, ResourceType::Agent, &service_name, false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let entries = agent
        .naming_store
        .get_service_entries(&namespace, &service_name);

    if entries.is_empty() {
        return HttpResponse::NotFound().consul_error(ConsulError::new(format!(
            "Service '{}' not found",
            service_name
        )));
    }

    let mut results = Vec::new();
    let mut worst_status = "passing";

    for entry_bytes in &entries {
        let reg: AgentServiceRegistration = match serde_json::from_slice(entry_bytes) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let svc_id = reg.service_id();
        let healthy = agent
            .naming_store
            .is_healthy(&reg.effective_address(), reg.effective_port() as i32);
        let agent_service = AgentService::from(&reg);
        let checks = health_service.get_service_checks(&svc_id).await;
        let status = if checks.is_empty() {
            if healthy {
                "passing".to_string()
            } else {
                "critical".to_string()
            }
        } else {
            aggregate_status(&checks)
        };

        if status == "critical" {
            worst_status = "critical";
        } else if status == "warning" && worst_status != "critical" {
            worst_status = "warning";
        }

        results.push(AgentServiceChecksInfo {
            aggregated_status: status,
            service: agent_service,
            checks,
        });
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    match worst_status {
        "passing" => consul_ok(&meta).json(results),
        "warning" => {
            let mut b = HttpResponse::TooManyRequests();
            meta.apply_headers(&mut b);
            b.json(results)
        }
        _ => {
            let mut b = HttpResponse::ServiceUnavailable();
            meta.apply_headers(&mut b);
            b.json(results)
        }
    }
}

/// PUT /v1/agent/service/maintenance/{service_id}
/// Places a service into maintenance mode
pub async fn set_service_maintenance(
    req: HttpRequest,
    _agent: web::Data<ConsulAgentService>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<MaintenanceRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let enable = query.enable;
    let reason = query
        .reason
        .clone()
        .unwrap_or_else(|| "Maintenance".to_string());

    // Check ACL authorization for service write (maintenance requires write)
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_id,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let check_id = format!("_service_maintenance:{}", service_id);

    if enable {
        // Create a critical maintenance health check
        let registration = CheckRegistration {
            name: "Service Maintenance Mode".to_string(),
            check_id: Some(check_id),
            service_id: Some(service_id),
            status: Some("critical".to_string()),
            notes: Some(reason),
            ..Default::default()
        };
        let _ = health_service.register_check(registration).await;
    } else {
        // Remove the maintenance check
        let _ = health_service.deregister_check(&check_id).await;
    }

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

// ============================================================================
// Agent Core API Handlers
// ============================================================================

// ============================================================================
// Cluster Integration Handlers (Using ClusterManager)
// ============================================================================

/// Extend-info keys that may carry a datacenter label on `ExtendedMemberInfo`.
/// Checked in order. Matches the metadata emitted by the cluster datacenter
/// plumbing (see `batata-core` member metadata population).
const DC_METADATA_KEYS: [&str; 2] = ["dc", "datacenter"];

/// Compute the number of distinct datacenters the cluster currently knows
/// about.
///
/// The count always includes the local datacenter (`self_dc`). Any member
/// whose `extend_info` carries a non-empty `dc`/`datacenter` entry contributes
/// its DC name to the set. When no peer metadata is present the result
/// degenerates to 1, matching Consul OSS single-DC behavior.
fn count_known_datacenters(cluster_manager: &dyn ClusterManager, self_dc: &str) -> usize {
    let mut dcs: std::collections::HashSet<String> = std::collections::HashSet::new();
    dcs.insert(self_dc.to_string());
    for member in cluster_manager.all_members_extended() {
        for key in DC_METADATA_KEYS {
            if let Some(value) = member.extend_info.get(key) {
                if let Some(name) = value.as_str() {
                    if !name.is_empty() {
                        dcs.insert(name.to_string());
                        break;
                    }
                }
            }
        }
    }
    dcs.len()
}

/// Convert MemberState to Consul member status
fn member_state_to_consul_status(state: &MemberState) -> i32 {
    match state {
        MemberState::Up => 1,         // alive
        MemberState::Down => 4,       // failed
        MemberState::Suspicious => 2, // leaving
    }
}

/// GET /v1/agent/members
/// Returns the actual cluster members from ClusterManager.
pub async fn get_agent_members(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    _query: web::Query<AgentMembersParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Get all members from ClusterManager
    let members: Vec<AgentMember> = member_manager
        .all_members_extended()
        .iter()
        .map(|m| {
            let mut tags = HashMap::new();
            tags.insert("role".to_string(), "consul".to_string());
            tags.insert("dc".to_string(), dc_config.datacenter.clone());
            tags.insert("port".to_string(), dc_config.consul_port.to_string());
            tags.insert("vsn".to_string(), "2".to_string());
            tags.insert("vsn_min".to_string(), "1".to_string());
            tags.insert("vsn_max".to_string(), "3".to_string());
            tags.insert("build".to_string(), dc_config.batata_version.clone());

            // Add node state as tag
            let state_str = match m.state {
                MemberState::Up => "alive",
                MemberState::Down => "failed",
                MemberState::Suspicious => "suspicious",
            };
            tags.insert("state".to_string(), state_str.to_string());

            // Parse address to get IP and port
            let (addr, port) = if let Some(pos) = m.address.rfind(':') {
                let ip = &m.address[..pos];
                let port: u16 = m.address[pos + 1..].parse().unwrap_or(8301);
                (ip.to_string(), port)
            } else {
                (m.address.clone(), 8301)
            };

            AgentMember {
                name: m.address.clone(),
                addr,
                port,
                tags,
                status: member_state_to_consul_status(&m.state),
                ..Default::default()
            }
        })
        .collect();

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(members)
}

/// GET /v1/agent/self
/// Returns real cluster information from ClusterManager.
pub async fn get_agent_self(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let self_member = member_manager.get_self_member();
    let health_summary = member_manager.health_summary();

    // Use stable node_id and node_name from dc_config (persisted in data_dir/consul-node-id)
    let config = AgentConfig {
        datacenter: dc_config.datacenter.clone(),
        node_name: dc_config.node_name.clone(),
        node_id: dc_config.node_id.clone(),
        server: true,
        revision: dc_config.batata_version.clone(),
        version: dc_config.full_version(),
        primary_datacenter: dc_config.primary_datacenter.clone(),
    };

    // Tags matching get_agent_self (Consul's delegate.AgentLocalMember().Tags)
    let mut tags = HashMap::new();
    tags.insert("role".to_string(), "consul".to_string());
    tags.insert("dc".to_string(), dc_config.datacenter.clone());
    tags.insert("port".to_string(), dc_config.raft_port().to_string());
    tags.insert(
        "build".to_string(),
        format!("{}:{}", dc_config.full_version(), dc_config.batata_version),
    );
    tags.insert("id".to_string(), dc_config.node_id.clone());
    tags.insert("raft_vsn".to_string(), "3".to_string());
    tags.insert("vsn".to_string(), "2".to_string());
    tags.insert("vsn_min".to_string(), "2".to_string());
    tags.insert("vsn_max".to_string(), "3".to_string());
    tags.insert("segment".to_string(), "".to_string());

    // Parse self_member address
    let (addr, port) = if let Some(pos) = self_member.address.rfind(':') {
        let ip = &self_member.address[..pos];
        let port: u16 = self_member.address[pos + 1..].parse().unwrap_or(8301);
        (ip.to_string(), port)
    } else {
        (self_member.address.clone(), 8301)
    };

    let member = AgentMember {
        name: dc_config.node_name.clone(),
        addr: addr.clone(),
        port,
        tags,
        status: member_state_to_consul_status(&self_member.state),
        ..Default::default()
    };

    let service_count = agent.naming_store.service_count();
    let check_count = agent.registry.check_count();

    let mut agent_stats = HashMap::new();
    agent_stats.insert(
        "check_monitors".to_string(),
        agent.registry.active_check_count().to_string(),
    );
    agent_stats.insert(
        "check_ttls".to_string(),
        agent.registry.ttl_check_count().to_string(),
    );
    agent_stats.insert("checks".to_string(), check_count.to_string());
    agent_stats.insert("services".to_string(), service_count.to_string());

    let mut runtime_stats = HashMap::new();
    runtime_stats.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    runtime_stats.insert("os".to_string(), std::env::consts::OS.to_string());
    runtime_stats.insert("version".to_string(), "rust".to_string());

    // Build stats (matching Consul's Stats().build section)
    let mut build_stats = HashMap::new();
    build_stats.insert("version".to_string(), dc_config.full_version());
    build_stats.insert("revision".to_string(), dc_config.batata_version.clone());
    build_stats.insert("prerelease".to_string(), String::new());

    // Consul stats section (matching Consul's delegate.Stats())
    let mut consul_stats = HashMap::new();
    consul_stats.insert("server".to_string(), "true".to_string());
    consul_stats.insert("leader".to_string(), member_manager.is_leader().to_string());
    consul_stats.insert(
        "leader_addr".to_string(),
        member_manager.leader_address().unwrap_or_default(),
    );
    consul_stats.insert(
        "bootstrap".to_string(),
        member_manager.is_standalone().to_string(),
    );
    // Count distinct datacenters: always includes self; picks up additional
    // names from member metadata (e.g. federated peer DCs propagated through
    // `ExtendedMemberInfo::extend_info`).
    let cm_arc: &Arc<dyn ClusterManager> = member_manager.as_ref();
    let known_datacenters = count_known_datacenters(cm_arc.as_ref(), &dc_config.datacenter);
    consul_stats.insert(
        "known_datacenters".to_string(),
        known_datacenters.to_string(),
    );

    // Serf LAN stats from cluster health
    let mut serf_lan_stats = HashMap::new();
    serf_lan_stats.insert("members".to_string(), health_summary.total.to_string());
    serf_lan_stats.insert("member_time".to_string(), "1".to_string());
    // Health score: 0 = healthy, higher = less healthy (based on failed members)
    let health_score = health_summary.down + health_summary.suspicious;
    serf_lan_stats.insert("health_score".to_string(), health_score.to_string());

    let stats = AgentStats {
        agent: agent_stats,
        runtime: runtime_stats,
        build: Some(build_stats),
        consul: Some(consul_stats),
        raft: None,
        serf_lan: Some(serf_lan_stats),
    };

    let mut meta = HashMap::new();
    meta.insert("consul-network-segment".to_string(), "".to_string());
    meta.insert("consul-version".to_string(), dc_config.full_version());

    // Provide basic DebugConfig with runtime port/address information
    // (Consul returns the full RuntimeConfig.Sanitized() with hundreds of fields)
    let grpc_port = dc_config.main_port.saturating_add(1000);
    let debug_config = serde_json::json!({
        "Datacenter": dc_config.datacenter,
        "PrimaryDatacenter": dc_config.primary_datacenter,
        "NodeName": dc_config.node_name,
        "NodeID": dc_config.node_id,
        "ServerMode": true,
        "Ports": {
            "HTTP": dc_config.main_port,
            "HTTPS": -1,
            "gRPC": grpc_port,
            "gRPCTLS": -1,
            "SerfLAN": dc_config.consul_port.saturating_sub(199),
            "Server": dc_config.raft_port(),
            "DNS": -1
        },
        "Addresses": {
            "HTTP": "0.0.0.0",
            "HTTPS": "",
            "gRPC": "0.0.0.0",
            "DNS": ""
        },
        "AdvertiseAddrLAN": addr.clone(),
        "BindAddr": "0.0.0.0",
        "Version": dc_config.full_version(),
        "Revision": dc_config.batata_version,
    });

    // Provide default network coordinate (8-dimensional Vivaldi, matching Consul's serf LAN)
    let coord = Coordinate {
        adjustment: 0.0,
        error: 1.5,
        height: 1e-05,
        vec: vec![0.0; 8],
    };

    let response = AgentSelf {
        config,
        debug_config: Some(debug_config),
        coord: Some(coord),
        member,
        meta,
        stats,
        xds: None,
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(response)
}

/// GET /v1/agent/host
/// Returns information about the host the agent is running on
pub async fn get_agent_host(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let mut sys = System::new_all();
    sys.refresh_all();

    // Memory info
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();
    let available_memory = sys.available_memory();
    let memory = HostMemory {
        total: total_memory,
        available: available_memory,
        used: used_memory,
        used_percent: if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        },
    };

    // CPU info
    let cpus: Vec<HostCPU> = sys
        .cpus()
        .iter()
        .enumerate()
        .map(|(i, cpu)| HostCPU {
            cpu: i as i32,
            vendor_id: cpu.vendor_id().to_string(),
            family: "".to_string(),
            model: cpu.brand().to_string(),
            physical_id: "0".to_string(),
            core_id: i.to_string(),
            cores: 1,
            mhz: cpu.frequency() as f64,
        })
        .collect();

    // Disk info (use root path)
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let disk = disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| HostDisk {
            path: d.mount_point().to_string_lossy().to_string(),
            total: d.total_space(),
            free: d.available_space(),
            used: d.total_space() - d.available_space(),
            used_percent: if d.total_space() > 0 {
                ((d.total_space() - d.available_space()) as f64 / d.total_space() as f64) * 100.0
            } else {
                0.0
            },
        })
        .unwrap_or(HostDisk {
            path: "/".to_string(),
            total: 0,
            free: 0,
            used: 0,
            used_percent: 0.0,
        });

    // Host info
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let host = HostInfo {
        hostname,
        os: System::name().unwrap_or_else(|| "unknown".to_string()),
        platform: std::env::consts::OS.to_string(),
        platform_version: System::os_version().unwrap_or_else(|| "unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "unknown".to_string()),
    };

    let response = AgentHostInfo {
        memory,
        cpu: cpus,
        disk,
        host,
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(response)
}

/// GET /v1/agent/version
/// Returns the Consul version of the agent
pub async fn get_agent_version(
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let full_version = dc_config.full_version();
    let response = AgentVersion {
        version: full_version.clone(),
        revision: dc_config.batata_version.clone(),
        prerelease: "".to_string(),
        human_version: full_version,
        build_date: option_env!("BATATA_BUILD_DATE")
            .unwrap_or("unknown")
            .to_string(),
        fips: "".to_string(),
    };
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(response)
}

/// PUT /v1/agent/join/{address}
/// Triggers the agent to join a cluster by address
pub async fn agent_join(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let address = path.into_inner();

    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Standalone/in-memory mode: no real cluster to join. In cluster mode the
    // ClusterManager-backed handler takes over gossip-level join semantics.
    tracing::warn!(
        address = %address,
        "Agent join: not supported in standalone mode (no Serf gossip layer)"
    );
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

/// PUT /v1/agent/leave
/// Triggers a graceful leave and shutdown of the agent
pub async fn agent_leave(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    tracing::warn!("Agent leave: not supported in standalone mode (no Serf gossip layer)");
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

/// PUT /v1/agent/force-leave/{node}
/// Forces a node into the left state
pub async fn agent_force_leave(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let node = path.into_inner();

    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    tracing::warn!(
        node = %node,
        "Agent force-leave: not supported in standalone mode (no Serf gossip layer)"
    );
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

/// PUT /v1/agent/reload
/// Triggers a reload of the agent's configuration
pub async fn agent_reload(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Batata reads config at startup; runtime reload is not yet supported.
    // Return 200 for compatibility, but warn that it's a no-op.
    tracing::warn!("Agent reload: config is read-only at runtime, restart required for changes");
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

/// PUT /v1/agent/maintenance
/// Toggles node maintenance mode
pub async fn agent_maintenance(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    query: web::Query<AgentMaintenanceRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let enable = query.enable;
    let reason = query
        .reason
        .clone()
        .unwrap_or_else(|| "Maintenance".to_string());

    let check_id = "_node_maintenance".to_string();

    if enable {
        // Create a critical node maintenance health check
        let registration = CheckRegistration {
            name: "Node Maintenance Mode".to_string(),
            check_id: Some(check_id),
            service_id: None,
            status: Some("critical".to_string()),
            notes: Some(reason.clone()),
            ..Default::default()
        };
        let _ = health_service.register_check(registration).await;
    } else {
        // Remove the maintenance check
        let _ = health_service.deregister_check(&check_id).await;
    }

    tracing::info!(
        "Agent maintenance mode: {} (reason: {})",
        if enable { "enabled" } else { "disabled" },
        reason
    );

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).finish()
}

/// GET /v1/agent/metrics
/// Returns metrics for the agent (Prometheus format compatible)
#[allow(clippy::vec_init_then_push)]
/// GET /v1/agent/metrics
/// Returns comprehensive metrics including service counts and cluster health.
#[allow(clippy::vec_init_then_push)]
pub async fn get_agent_metrics(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    agent: web::Data<ConsulAgentService>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Collect system metrics
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut gauges = Vec::new();
    let mut counters = Vec::new();
    let mut samples = Vec::new();

    // Runtime metrics (Consul-compatible names)
    gauges.push(GaugeMetric::new(
        "consul.runtime.num_goroutines",
        std::thread::available_parallelism()
            .map(|p| p.get() as f64)
            .unwrap_or(1.0),
    ));
    gauges.push(GaugeMetric::new(
        "consul.runtime.alloc_bytes",
        sys.used_memory() as f64,
    ));
    gauges.push(GaugeMetric::new(
        "consul.runtime.sys_bytes",
        sys.total_memory() as f64,
    ));

    // Batata-specific runtime metrics
    let cpu_usage = sys.global_cpu_usage();
    gauges.push(GaugeMetric::new(
        "batata.runtime.cpu_percent",
        cpu_usage as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.runtime.cpu_cores",
        sys.cpus().len() as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.runtime.memory_total",
        sys.total_memory() as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.runtime.memory_used",
        sys.used_memory() as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.runtime.memory_available",
        sys.available_memory() as f64,
    ));

    // Service metrics from ConsulNamingStore (use default namespace for metrics)
    let service_names = agent
        .naming_store
        .service_names(crate::namespace::DEFAULT_NAMESPACE);
    let total_services = service_names.len();

    gauges.push(
        GaugeMetric::new("consul.catalog.service_count", total_services as f64)
            .with_label("datacenter", &dc_config.datacenter),
    );
    gauges.push(
        GaugeMetric::new("batata.naming.service_count", total_services as f64)
            .with_label("namespace", CONSUL_INTERNAL_NAMESPACE)
            .with_label("group", CONSUL_INTERNAL_GROUP),
    );

    // Count total instances across all services
    let mut total_instances = 0u64;
    let mut healthy_instances = 0u64;
    let mut unhealthy_instances = 0u64;

    for service_name in &service_names {
        let entries = agent
            .naming_store
            .get_service_entries(crate::namespace::DEFAULT_NAMESPACE, service_name);
        let instance_count = entries.len() as u64;
        total_instances += instance_count;

        for entry_bytes in &entries {
            if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(entry_bytes) {
                if agent
                    .naming_store
                    .is_healthy(&reg.effective_address(), reg.effective_port() as i32)
                {
                    healthy_instances += 1;
                } else {
                    unhealthy_instances += 1;
                }
            }
        }

        // Per-service instance count
        gauges.push(
            GaugeMetric::new(
                "batata.naming.service_instance_count",
                instance_count as f64,
            )
            .with_label("service", service_name)
            .with_label("namespace", CONSUL_INTERNAL_NAMESPACE),
        );
    }

    gauges.push(GaugeMetric::new(
        "consul.catalog.service_instance_count",
        total_instances as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.naming.healthy_instance_count",
        healthy_instances as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.naming.unhealthy_instance_count",
        unhealthy_instances as f64,
    ));

    // Cluster metrics from ClusterManager
    let health_summary = member_manager.health_summary();
    gauges.push(GaugeMetric::new(
        "consul.serf.member.count",
        health_summary.total as f64,
    ));
    gauges.push(
        GaugeMetric::new("consul.serf.member.alive", health_summary.up as f64)
            .with_label("status", "alive"),
    );
    gauges.push(
        GaugeMetric::new("consul.serf.member.failed", health_summary.down as f64)
            .with_label("status", "failed"),
    );
    gauges.push(GaugeMetric::new(
        "batata.cluster.member_total",
        health_summary.total as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.cluster.member_up",
        health_summary.up as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.cluster.member_down",
        health_summary.down as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.cluster.member_suspicious",
        health_summary.suspicious as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.cluster.member_starting",
        health_summary.starting as f64,
    ));

    // Cluster state
    gauges.push(GaugeMetric::new(
        "batata.cluster.is_leader",
        if member_manager.is_leader() { 1.0 } else { 0.0 },
    ));
    gauges.push(GaugeMetric::new(
        "batata.cluster.is_standalone",
        if member_manager.is_standalone() {
            1.0
        } else {
            0.0
        },
    ));

    // Add service counter (cumulative service registrations - simulated)
    counters.push(CounterMetric::new(
        "consul.catalog.register.count",
        total_instances as i64,
        total_instances as f64,
    ));

    // Add sample metric for instance health distribution
    if total_instances > 0 {
        let health_ratio = healthy_instances as f64 / total_instances as f64;
        samples.push(SampleMetric::new(
            "batata.naming.health_ratio",
            total_instances as i64,
            health_ratio,
            0.0,
            1.0,
            0.0,
        ));
    }

    let response = MetricsResponse {
        timestamp: chrono::Utc::now().to_rfc3339(),
        gauges,
        counters,
        samples,
        points: Vec::new(),
    };

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta).json(response)
}

/// Query parameters for /v1/agent/monitor
#[derive(Debug, serde::Deserialize)]
pub struct MonitorQueryParams {
    #[serde(default = "default_log_level")]
    pub loglevel: String,
    #[serde(default)]
    pub logjson: Option<bool>,
}

fn default_log_level() -> String {
    "INFO".to_string()
}

/// GET /v1/agent/monitor
/// Streams log entries from the agent as a streaming HTTP response.
/// Supports ?loglevel=INFO and ?logjson query params.
///
/// Subscribes to the tracing broadcast channel (MonitorLayer) and streams
/// real log events filtered by the requested log level.
pub async fn agent_monitor(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    query: web::Query<MonitorQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let log_level = query.loglevel.to_uppercase();
    let as_json = query.logjson.unwrap_or(false);
    let _ = &index_provider;

    // Subscribe to the log broadcast channel for real tracing events.
    // Ownership moves into the stream closure, which mutates its own binding.
    let rx = crate::log_broadcast::subscribe();

    // Map log levels to numeric priority for filtering
    let min_level = match log_level.as_str() {
        "TRACE" => 0,
        "DEBUG" => 1,
        "INFO" => 2,
        "WARN" => 3,
        "ERROR" => 4,
        _ => 2, // default to INFO
    };

    // Stream real log entries from the tracing broadcast channel
    let stream = futures::stream::unfold(
        (rx, min_level, as_json, true),
        move |(mut rx, min_level, as_json, first)| async move {
            if first {
                // Emit initial message
                let msg = format_monitor_line(
                    "INFO",
                    as_json,
                    "agent.monitor",
                    "Log streaming active (connected to tracing broadcast)",
                );
                return Some((
                    Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(msg)),
                    (rx, min_level, as_json, false),
                ));
            }

            loop {
                match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
                    Ok(Ok(entry)) => {
                        // Filter by log level
                        let entry_level = match entry.level.as_str() {
                            "TRACE" => 0,
                            "DEBUG" => 1,
                            "INFO" => 2,
                            "WARN" => 3,
                            "ERROR" => 4,
                            _ => 2,
                        };
                        if entry_level < min_level {
                            continue;
                        }
                        let msg = if as_json {
                            entry.format_json()
                        } else {
                            entry.format_plain()
                        };
                        return Some((
                            Ok(actix_web::web::Bytes::from(msg)),
                            (rx, min_level, as_json, false),
                        ));
                    }
                    Ok(Err(_)) => {
                        // Channel closed or lagged — reconnect
                        rx = crate::log_broadcast::subscribe();
                        continue;
                    }
                    Err(_) => {
                        // Timeout — send heartbeat to keep connection alive
                        let msg =
                            format_monitor_line("DEBUG", as_json, "agent.monitor", "heartbeat");
                        return Some((
                            Ok(actix_web::web::Bytes::from(msg)),
                            (rx, min_level, as_json, false),
                        ));
                    }
                }
            }
        },
    );

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .insert_header(("Transfer-Encoding", "chunked"))
        .streaming(stream)
}

/// Format a single monitor log line (plain text or JSON, matching Consul format)
fn format_monitor_line(level: &str, as_json: bool, module: &str, message: &str) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();
    if as_json {
        serde_json::json!({
            "@level": level.to_lowercase(),
            "@message": message,
            "@module": module,
            "@timestamp": timestamp,
        })
        .to_string()
            + "\n"
    } else {
        format!("{}  [{}] {}: {}\n", timestamp, level, module, message)
    }
}

/// Collect current metrics snapshot from the ConsulNamingStore.
fn collect_metrics_snapshot(
    naming_store: &ConsulNamingStore,
    _dc_config: &ConsulDatacenterConfig,
) -> MetricsResponse {
    let service_names = naming_store.service_names(crate::namespace::DEFAULT_NAMESPACE);
    let service_count = service_names.len();

    let total_instances = naming_store.len();
    // Count healthy instances by scanning all entries
    let mut healthy_instances: usize = 0;
    for (_key, data) in naming_store.scan_ns(crate::namespace::DEFAULT_NAMESPACE) {
        if let Ok(reg) = serde_json::from_slice::<AgentServiceRegistration>(&data)
            && naming_store.is_healthy(&reg.effective_address(), reg.effective_port() as i32)
        {
            healthy_instances += 1;
        }
    }

    MetricsResponse {
        timestamp: chrono::Utc::now().to_rfc3339(),
        gauges: vec![
            GaugeMetric::new("consul.catalog.service.count", service_count as f64),
            GaugeMetric::new(
                "consul.catalog.service.instance.count",
                total_instances as f64,
            ),
            GaugeMetric::new(
                "batata.naming.healthy_instance_count",
                healthy_instances as f64,
            ),
        ],
        counters: Vec::new(),
        samples: Vec::new(),
        points: Vec::new(),
    }
}

/// GET /v1/agent/metrics/stream
/// Streams metrics with cluster member data every 10 seconds.
pub async fn agent_metrics_stream(
    req: HttpRequest,
    naming_store: web::Data<ConsulNamingStore>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    let store = naming_store.into_inner();
    let dc = dc_config.into_inner();
    let mm = member_manager.into_inner();
    let stream = futures::stream::unfold(0u64, move |tick| {
        let store = store.clone();
        let dc = dc.clone();
        let mm = mm.clone();
        async move {
            if tick > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
            let mut snapshot = collect_metrics_snapshot(&store, &dc);
            let health_summary = mm.health_summary();
            snapshot.gauges.push(GaugeMetric::new(
                "consul.serf.member.count",
                health_summary.total as f64,
            ));
            let mut bytes = serde_json::to_vec(&snapshot).unwrap_or_default();
            bytes.push(b'\n');
            Some((
                Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(bytes)),
                tick + 1,
            ))
        }
    });

    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta)
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Transfer-Encoding", "chunked"))
        .streaming(stream)
}

/// PUT /v1/agent/token/{type}
/// Updates the ACL token for the agent
pub async fn update_agent_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let token_type = path.into_inner();

    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(&authz.reason));
    }

    // Consul supports updating: default, agent, replication, config_file_service_registration tokens.
    // Batata uses its own auth system (JWT/RBAC) instead of Consul ACL tokens,
    // so token updates are acknowledged but not applied.
    // Return 200 OK (matching Consul's response format) to avoid breaking clients,
    // but include a warning header.
    tracing::info!(
        token_type = %token_type,
        "Agent token update: acknowledged (Batata uses its own auth system)"
    );
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::Catalog));
    consul_ok(&meta)
        .insert_header(("X-Consul-Default-ACL-Policy", "allow"))
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_naming::service::NamingService;

    #[test]
    fn test_consul_agent_service_creation() {
        let naming_service = Arc::new(NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming_service));
        let naming_store = Arc::new(ConsulNamingStore::new());
        let check_index = Arc::new(ConsulCheckIndex::new());
        let agent = ConsulAgentService::new(naming_store.clone(), registry, check_index);
        assert_eq!(agent.naming_store.len(), 0);
    }

    #[test]
    fn test_node_state_to_consul_status_all_variants() {
        assert_eq!(member_state_to_consul_status(&MemberState::Up), 1); // alive
        assert_eq!(member_state_to_consul_status(&MemberState::Down), 4); // failed
        assert_eq!(member_state_to_consul_status(&MemberState::Suspicious), 2); // leaving
    }

    #[test]
    fn test_create_health_check_healthy() {
        let reg = AgentServiceRegistration {
            id: Some("healthy-001".to_string()),
            name: "web".to_string(),
            address: Some("10.0.0.1".to_string()),
            port: Some(8080),
            ..Default::default()
        };

        let check = create_service_health_check_from_reg(&reg, true);
        assert_eq!(check.status, "passing");
        assert_eq!(check.check_id, "service:web:healthy-001");
        assert_eq!(check.name, "web Health Check");
        assert!(check.output.contains("healthy"));
        assert_eq!(check.service_id, "healthy-001");
        assert_eq!(check.service_name, "web");
        assert_eq!(check.check_type, "service");
    }

    #[test]
    fn test_create_health_check_unhealthy() {
        let reg = AgentServiceRegistration {
            id: Some("unhealthy-001".to_string()),
            name: "api".to_string(),
            address: Some("10.0.0.2".to_string()),
            port: Some(8080),
            ..Default::default()
        };

        let check = create_service_health_check_from_reg(&reg, false);
        assert_eq!(check.status, "critical");
        assert!(check.output.contains("unhealthy"));
    }

    #[test]
    fn test_create_health_check_id_format() {
        let reg = AgentServiceRegistration {
            id: Some("svc-abc-123".to_string()),
            name: "my-service".to_string(),
            address: Some("192.168.1.1".to_string()),
            port: Some(9090),
            ..Default::default()
        };

        let check = create_service_health_check_from_reg(&reg, true);
        assert_eq!(check.check_id, "service:my-service:svc-abc-123");
    }

    fn create_test_agent() -> (
        ConsulAgentService,
        Arc<ConsulNamingStore>,
        Arc<InstanceCheckRegistry>,
        Arc<ConsulCheckIndex>,
    ) {
        let naming_service = Arc::new(NamingService::new());
        let registry = Arc::new(InstanceCheckRegistry::with_naming_service(naming_service));
        let naming_store = Arc::new(ConsulNamingStore::new());
        let check_index = Arc::new(ConsulCheckIndex::new());
        let agent =
            ConsulAgentService::new(naming_store.clone(), registry.clone(), check_index.clone());
        (agent, naming_store, registry, check_index)
    }

    #[tokio::test]
    async fn test_consul_self_registration() {
        use batata_naming::healthcheck::registry::*;

        let (agent, _naming_store, registry, check_index) = create_test_agent();

        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        let result = agent.register_consul_service(&dc_config).await;
        assert!(result.is_ok(), "Self-registration should succeed");

        // Verify service is in ConsulNamingStore (service_id is node-unique)
        let expected_service_id = format!("consul-{}", dc_config.node_name);
        let data = agent
            .naming_store
            .get_by_service_id(crate::namespace::DEFAULT_NAMESPACE, &expected_service_id);
        assert!(data.is_some(), "Consul service should be in naming store");

        let reg: AgentServiceRegistration = serde_json::from_slice(&data.unwrap()).unwrap();
        assert_eq!(reg.service_id(), expected_service_id);
        assert_eq!(reg.effective_port(), dc_config.raft_port());
        assert_eq!(reg.name, "consul");

        // Verify metadata matches Consul's fields
        let meta = reg.meta.as_ref().unwrap();
        assert!(meta.contains_key("version"));
        assert!(meta.contains_key("raft_version"));

        // Verify serfHealth check is registered
        let check = registry.get_check("serfHealth");
        assert!(check.is_some(), "serfHealth check should be registered");
        let (config, status) = check.unwrap();
        assert_eq!(config.check_type, CheckType::None);
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "serfHealth should start as Passing"
        );
        // Verify consul service ID index (uses node-unique service_id)
        let lookup = check_index.lookup(&expected_service_id);
        assert!(lookup.is_some(), "consul service ID should be indexed");
    }

    #[tokio::test]
    async fn test_consul_self_deregistration() {
        let (agent, _naming_store, _registry, _check_index) = create_test_agent();

        // Register first
        let dc_config = ConsulDatacenterConfig::new("dc1".to_string());
        let service_id = format!("consul-{}", dc_config.node_name);
        agent.register_consul_service(&dc_config).await.unwrap();
        assert!(
            agent
                .naming_store
                .get_by_service_id(crate::namespace::DEFAULT_NAMESPACE, &service_id)
                .is_some()
        );

        // Deregister
        agent.deregister_consul_service(&dc_config).await;

        assert!(
            agent
                .naming_store
                .get_by_service_id(crate::namespace::DEFAULT_NAMESPACE, &service_id)
                .is_none(),
            "Consul service should be deregistered from naming store"
        );
    }

    #[test]
    fn test_agent_uses_fixed_constants() {
        // Verify the fixed constants have the expected values used as registry key prefixes.
        assert_eq!(CONSUL_INTERNAL_NAMESPACE, "consul");
        assert_eq!(CONSUL_INTERNAL_GROUP, "CONSUL_GROUP");
        assert_eq!(CONSUL_INTERNAL_CLUSTER, "DEFAULT");
    }

    // -----------------------------------------------------------------
    // count_known_datacenters tests (Gap 1)
    // -----------------------------------------------------------------

    use batata_common::{ClusterHealthSummary, ExtendedMemberInfo};

    struct DcTestClusterManager {
        members: Vec<ExtendedMemberInfo>,
    }

    impl ClusterManager for DcTestClusterManager {
        fn is_standalone(&self) -> bool {
            false
        }
        fn is_leader(&self) -> bool {
            true
        }
        fn is_cluster_healthy(&self) -> bool {
            true
        }
        fn leader_address(&self) -> Option<String> {
            Some("127.0.0.1:8848".to_string())
        }
        fn local_address(&self) -> &str {
            "127.0.0.1:8848"
        }
        fn member_count(&self) -> usize {
            self.members.len()
        }
        fn all_members_extended(&self) -> Vec<ExtendedMemberInfo> {
            self.members.clone()
        }
        fn healthy_members_extended(&self) -> Vec<ExtendedMemberInfo> {
            self.members.clone()
        }
        fn get_member(&self, _address: &str) -> Option<ExtendedMemberInfo> {
            self.members.first().cloned()
        }
        fn get_self_member(&self) -> ExtendedMemberInfo {
            self.members
                .first()
                .cloned()
                .unwrap_or_else(|| member_with_dc("127.0.0.1:8848", None))
        }
        fn health_summary(&self) -> ClusterHealthSummary {
            ClusterHealthSummary {
                total: self.members.len(),
                up: self.members.len(),
                ..Default::default()
            }
        }
        fn refresh_self(&self) {}
        fn is_self(&self, address: &str) -> bool {
            address == self.local_address()
        }
        fn update_member_state(&self, _a: &str, _s: &str) -> Result<String, String> {
            Ok("UP".to_string())
        }
    }

    fn member_with_dc(address: &str, dc: Option<&str>) -> ExtendedMemberInfo {
        let mut extend_info = std::collections::BTreeMap::new();
        if let Some(name) = dc {
            extend_info.insert("dc".to_string(), serde_json::json!(name));
        }
        ExtendedMemberInfo {
            ip: address.split(':').next().unwrap_or("").to_string(),
            port: 8848,
            address: address.to_string(),
            state: MemberState::Up,
            extend_info,
        }
    }

    #[test]
    fn test_count_known_datacenters_single_dc() {
        let cm = DcTestClusterManager {
            members: vec![member_with_dc("10.0.0.1:8848", None)],
        };
        assert_eq!(count_known_datacenters(&cm, "dc1"), 1);
    }

    #[test]
    fn test_count_known_datacenters_self_only_when_no_members() {
        let cm = DcTestClusterManager { members: vec![] };
        assert_eq!(count_known_datacenters(&cm, "dc-primary"), 1);
    }

    #[test]
    fn test_count_known_datacenters_multi_dc() {
        let cm = DcTestClusterManager {
            members: vec![
                member_with_dc("10.0.0.1:8848", Some("dc1")),
                member_with_dc("10.0.0.2:8848", Some("dc2")),
                member_with_dc("10.0.0.3:8848", Some("dc2")), // duplicate
                member_with_dc("10.0.0.4:8848", Some("dc3")),
            ],
        };
        // dc1 (self) + dc2 + dc3 = 3 unique (self happens to equal member DC)
        assert_eq!(count_known_datacenters(&cm, "dc1"), 3);
    }

    #[test]
    fn test_count_known_datacenters_self_dc_not_in_members() {
        let cm = DcTestClusterManager {
            members: vec![member_with_dc("10.0.0.1:8848", Some("peer-dc"))],
        };
        // self ("dc1") not advertised by any member, but always counted.
        assert_eq!(count_known_datacenters(&cm, "dc1"), 2);
    }

    #[test]
    fn test_count_known_datacenters_ignores_empty_dc_value() {
        let cm = DcTestClusterManager {
            members: vec![member_with_dc("10.0.0.1:8848", Some(""))],
        };
        assert_eq!(count_known_datacenters(&cm, "dc1"), 1);
    }

    #[test]
    fn test_count_known_datacenters_accepts_datacenter_key_alias() {
        let mut extend_info = std::collections::BTreeMap::new();
        extend_info.insert("datacenter".to_string(), serde_json::json!("aliased-dc"));
        let member = ExtendedMemberInfo {
            ip: "10.0.0.1".to_string(),
            port: 8848,
            address: "10.0.0.1:8848".to_string(),
            state: MemberState::Up,
            extend_info,
        };
        let cm = DcTestClusterManager {
            members: vec![member],
        };
        assert_eq!(count_known_datacenters(&cm, "dc1"), 2);
    }
}
