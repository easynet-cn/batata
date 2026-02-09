// Consul Agent API HTTP handlers
// Implements Consul-compatible service registration endpoints

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use sysinfo::System;

use batata_api::model::NodeState;
use batata_api::naming::model::Instance as NacosInstance;
use batata_core::service::cluster::ServerMemberManager;
use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::health::ConsulHealthService;
use crate::model::{
    AgentConfig, AgentHostInfo, AgentMaintenanceRequest, AgentMember, AgentMembersParams,
    AgentSelf, AgentService, AgentServiceRegistration, AgentServiceWithChecks, AgentStats,
    AgentVersion, CheckRegistration, ConsulError, CounterMetric, GaugeMetric, HostCPU, HostDisk,
    HostInfo, HostMemory, MaintenanceRequest, MetricsResponse, SampleMetric, ServiceQueryParams,
};

/// Consul Agent service adapter
/// Wraps NamingService to provide Consul-compatible API
#[derive(Clone)]
pub struct ConsulAgentService {
    naming_service: Arc<NamingService>,
}

impl ConsulAgentService {
    pub fn new(naming_service: Arc<NamingService>) -> Self {
        Self { naming_service }
    }
}

/// PUT /v1/agent/service/register
/// Register a new service with the local agent
pub async fn register_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    health_service: web::Data<ConsulHealthService>,
    query: web::Query<ServiceQueryParams>,
    body: web::Json<AgentServiceRegistration>,
) -> HttpResponse {
    let registration = body.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    // Check ACL authorization for service write
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &registration.name,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let service_id = registration.service_id();

    // Extract embedded checks before converting to NacosInstance
    let mut embedded_checks: Vec<CheckRegistration> = Vec::new();

    // Single check
    if let Some(ref check) = registration.check {
        embedded_checks.push(CheckRegistration {
            name: check
                .name
                .clone()
                .unwrap_or_else(|| format!("Service '{}' check", registration.name)),
            check_id: check.check_id.clone(),
            service_id: Some(service_id.clone()),
            notes: check.notes.clone(),
            ttl: check.ttl.clone(),
            http: check.http.clone(),
            method: check.method.clone(),
            header: check.header.clone(),
            tcp: check.tcp.clone(),
            grpc: check.grpc.clone(),
            interval: check.interval.clone(),
            timeout: check.timeout.clone(),
            deregister_critical_service_after: check.deregister_critical_service_after.clone(),
            status: check.status.clone(),
        });
    }

    // Multiple checks
    if let Some(ref checks) = registration.checks {
        for check in checks {
            embedded_checks.push(CheckRegistration {
                name: check
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Service '{}' check", registration.name)),
                check_id: check.check_id.clone(),
                service_id: Some(service_id.clone()),
                notes: check.notes.clone(),
                ttl: check.ttl.clone(),
                http: check.http.clone(),
                method: check.method.clone(),
                header: check.header.clone(),
                tcp: check.tcp.clone(),
                grpc: check.grpc.clone(),
                interval: check.interval.clone(),
                timeout: check.timeout.clone(),
                deregister_critical_service_after: check.deregister_critical_service_after.clone(),
                status: check.status.clone(),
            });
        }
    }

    // Convert Consul registration to Nacos Instance
    let nacos_instance: NacosInstance = (&registration).into();

    // Register with naming service
    // Consul doesn't have a concept of group, use DEFAULT_GROUP
    let success = agent.naming_service.register_instance(
        &namespace,
        "DEFAULT_GROUP",
        &registration.name,
        nacos_instance,
    );

    if success {
        // Register embedded checks with health service
        for check_reg in embedded_checks {
            if let Err(e) = health_service.register_check(check_reg) {
                tracing::warn!("Failed to register embedded check: {}", e);
            }
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::InternalServerError().json(ConsulError::new("Failed to register service"))
    }
}

/// PUT /v1/agent/service/deregister/{service_id}
/// Deregister a service from the local agent
pub async fn deregister_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    health_service: web::Data<ConsulHealthService>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    // Check ACL authorization for service write (deregister requires write)
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_id,
        true, // write access required
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Find the service by ID across all services
    // We need to iterate through services to find the one with matching ID
    let mut deregistered = false;

    // Get all services and find the one with matching instance_id
    // Since we don't know the service name, we need to search
    let services = agent
        .naming_service
        .list_services(&namespace, "DEFAULT_GROUP", 1, 10000);

    for service_name in services.1 {
        let instances = agent.naming_service.get_instances(
            &namespace,
            "DEFAULT_GROUP",
            &service_name,
            "",
            false,
        );

        for instance in instances {
            if instance.instance_id == service_id {
                let success = agent.naming_service.deregister_instance(
                    &namespace,
                    "DEFAULT_GROUP",
                    &service_name,
                    &instance,
                );
                if success {
                    deregistered = true;
                    break;
                }
            }
        }
        if deregistered {
            break;
        }
    }

    if deregistered {
        // Clean up any associated health checks
        let service_checks = health_service.get_service_checks(&service_id);
        for check in &service_checks {
            let _ = health_service.deregister_check(&check.check_id);
        }
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::NotFound().json(ConsulError::new(format!(
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
    query: web::Query<ServiceQueryParams>,
) -> HttpResponse {
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    // Check ACL authorization for service read (list requires read on all services)
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        "",    // empty prefix means all services
        false, // read access only
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get all services from naming service
    let (_, service_names) =
        agent
            .naming_service
            .list_services(&namespace, "DEFAULT_GROUP", 1, 10000);

    let mut services: std::collections::HashMap<String, AgentService> =
        std::collections::HashMap::new();

    for service_name in service_names {
        let instances = agent.naming_service.get_instances(
            &namespace,
            "DEFAULT_GROUP",
            &service_name,
            "",
            false,
        );

        // Each instance becomes a separate service entry in Consul format
        for instance in instances {
            let agent_service = AgentService::from(&instance);
            services.insert(agent_service.id.clone(), agent_service);
        }
    }

    HttpResponse::Ok().json(services)
}

/// GET /v1/agent/service/{service_id}
/// Returns the full service definition for a single service instance
pub async fn get_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let namespace = query.ns.clone().unwrap_or_else(|| "public".to_string());

    // Check ACL authorization for service read
    let authz = acl_service.authorize_request(
        &req,
        ResourceType::Service,
        &service_id,
        false, // read access only
    );
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Find the service by ID
    let (_, service_names) =
        agent
            .naming_service
            .list_services(&namespace, "DEFAULT_GROUP", 1, 10000);

    for service_name in service_names {
        let instances = agent.naming_service.get_instances(
            &namespace,
            "DEFAULT_GROUP",
            &service_name,
            "",
            false,
        );

        for instance in instances {
            if instance.instance_id == service_id {
                let agent_service = AgentService::from(&instance);
                // Generate health check based on instance status
                let checks = vec![create_service_health_check(&instance, &service_name)];
                let response = AgentServiceWithChecks {
                    service: agent_service,
                    checks: Some(checks),
                };
                return HttpResponse::Ok().json(response);
            }
        }
    }

    HttpResponse::NotFound().json(ConsulError::new(format!(
        "Service not found: {}",
        service_id
    )))
}

// ============================================================================
// Health Check Helper Functions
// ============================================================================

/// Create a health check for a service instance based on its status
/// Integrates with Nacos naming service health status
fn create_service_health_check(
    instance: &NacosInstance,
    service_name: &str,
) -> crate::model::AgentCheck {
    use crate::model::AgentCheck;

    // Determine health status based on instance health and enabled state
    let (status, output, notes) = if instance.healthy && instance.enabled {
        (
            "passing",
            format!("Service '{}' is healthy", service_name),
            "Service is running and accepting connections".to_string(),
        )
    } else if !instance.enabled {
        (
            "critical",
            format!("Service '{}' is in maintenance mode", service_name),
            "Service has been disabled or is in maintenance".to_string(),
        )
    } else {
        (
            "critical",
            format!("Service '{}' is unhealthy", service_name),
            "Service is not responding or health check failed".to_string(),
        )
    };

    AgentCheck {
        check_id: format!("service:{}:{}", service_name, instance.instance_id),
        name: format!("{} Health Check", service_name),
        status: status.to_string(),
        notes,
        output,
        service_id: instance.instance_id.clone(),
        service_name: service_name.to_string(),
        check_type: "service".to_string(),
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
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
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
        let _ = health_service.register_check(registration);
    } else {
        // Remove the maintenance check
        let _ = health_service.deregister_check(&check_id);
    }

    HttpResponse::Ok().finish()
}

// ============================================================================
// Agent Core API Handlers
// ============================================================================

/// GET /v1/agent/self
/// Returns the configuration and member information of the local agent
pub async fn get_agent_self(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let node_id = uuid::Uuid::new_v4().to_string();
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "batata-node".to_string());

    let config = AgentConfig {
        datacenter: "dc1".to_string(),
        node_name: hostname.clone(),
        node_id: node_id.clone(),
        server: true,
        revision: env!("CARGO_PKG_VERSION").to_string(),
        version: format!("1.15.0-batata-{}", env!("CARGO_PKG_VERSION")),
        primary_datacenter: "dc1".to_string(),
    };

    let mut tags = HashMap::new();
    tags.insert("role".to_string(), "consul".to_string());
    tags.insert("dc".to_string(), "dc1".to_string());
    tags.insert("port".to_string(), "8300".to_string());
    tags.insert("build".to_string(), env!("CARGO_PKG_VERSION").to_string());

    let member = AgentMember {
        name: hostname.clone(),
        addr: "127.0.0.1".to_string(),
        port: 8301,
        tags,
        status: 1, // alive
        ..Default::default()
    };

    let mut agent_stats = HashMap::new();
    agent_stats.insert("check_monitors".to_string(), "0".to_string());
    agent_stats.insert("check_ttls".to_string(), "0".to_string());
    agent_stats.insert("checks".to_string(), "0".to_string());
    agent_stats.insert("services".to_string(), "0".to_string());

    let mut runtime_stats = HashMap::new();
    runtime_stats.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    runtime_stats.insert("os".to_string(), std::env::consts::OS.to_string());
    runtime_stats.insert("version".to_string(), "rust".to_string());

    let stats = AgentStats {
        agent: agent_stats,
        runtime: runtime_stats,
        raft: None,
        serf_lan: None,
    };

    let mut meta = HashMap::new();
    meta.insert("consul-network-segment".to_string(), "".to_string());

    let response = AgentSelf {
        config,
        coord: None,
        member,
        meta,
        stats,
    };

    HttpResponse::Ok().json(response)
}

/// GET /v1/agent/members
/// Returns the members the agent sees in the cluster gossip pool
pub async fn get_agent_members(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _query: web::Query<AgentMembersParams>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "batata-node".to_string());

    let mut tags = HashMap::new();
    tags.insert("role".to_string(), "consul".to_string());
    tags.insert("dc".to_string(), "dc1".to_string());
    tags.insert("port".to_string(), "8300".to_string());
    tags.insert("vsn".to_string(), "2".to_string());
    tags.insert("vsn_min".to_string(), "1".to_string());
    tags.insert("vsn_max".to_string(), "3".to_string());
    tags.insert("build".to_string(), env!("CARGO_PKG_VERSION").to_string());

    // Return self as the only member (single node mode)
    let member = AgentMember {
        name: hostname,
        addr: "127.0.0.1".to_string(),
        port: 8301,
        tags,
        status: 1, // alive
        ..Default::default()
    };

    HttpResponse::Ok().json(vec![member])
}

// ============================================================================
// Real Cluster Integration Handlers (Using ServerMemberManager)
// ============================================================================

/// Convert NodeState to Consul member status
fn node_state_to_consul_status(state: &NodeState) -> i32 {
    match state {
        NodeState::Up => 1,         // alive
        NodeState::Down => 4,       // failed
        NodeState::Suspicious => 2, // leaving
        NodeState::Starting => 3,   // left
        NodeState::Isolation => 4,  // failed
    }
}

/// GET /v1/agent/members (Real cluster version)
/// Returns the actual cluster members from ServerMemberManager
pub async fn get_agent_members_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    _query: web::Query<AgentMembersParams>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Get all members from ServerMemberManager
    let members: Vec<AgentMember> = member_manager
        .all_members()
        .iter()
        .map(|m| {
            let mut tags = HashMap::new();
            tags.insert("role".to_string(), "consul".to_string());
            tags.insert("dc".to_string(), "dc1".to_string());
            tags.insert("port".to_string(), "8300".to_string());
            tags.insert("vsn".to_string(), "2".to_string());
            tags.insert("vsn_min".to_string(), "1".to_string());
            tags.insert("vsn_max".to_string(), "3".to_string());
            tags.insert("build".to_string(), env!("CARGO_PKG_VERSION").to_string());

            // Add node state as tag
            let state_str = match m.state {
                NodeState::Up => "alive",
                NodeState::Down => "failed",
                NodeState::Suspicious => "suspicious",
                NodeState::Starting => "starting",
                NodeState::Isolation => "isolation",
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
                status: node_state_to_consul_status(&m.state),
                ..Default::default()
            }
        })
        .collect();

    HttpResponse::Ok().json(members)
}

/// GET /v1/agent/self (Real cluster version)
/// Returns real cluster information from ServerMemberManager
pub async fn get_agent_self_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let self_member = member_manager.get_self();
    let health_summary = member_manager.health_summary();

    let node_id = uuid::Uuid::new_v4().to_string();
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "batata-node".to_string());

    let config = AgentConfig {
        datacenter: "dc1".to_string(),
        node_name: hostname.clone(),
        node_id: node_id.clone(),
        server: true,
        revision: env!("CARGO_PKG_VERSION").to_string(),
        version: format!("1.15.0-batata-{}", env!("CARGO_PKG_VERSION")),
        primary_datacenter: "dc1".to_string(),
    };

    let mut tags = HashMap::new();
    tags.insert("role".to_string(), "consul".to_string());
    tags.insert("dc".to_string(), "dc1".to_string());
    tags.insert("port".to_string(), "8300".to_string());
    tags.insert("build".to_string(), env!("CARGO_PKG_VERSION").to_string());

    // Parse self_member address
    let (addr, port) = if let Some(pos) = self_member.address.rfind(':') {
        let ip = &self_member.address[..pos];
        let port: u16 = self_member.address[pos + 1..].parse().unwrap_or(8301);
        (ip.to_string(), port)
    } else {
        (self_member.address.clone(), 8301)
    };

    let member = AgentMember {
        name: hostname.clone(),
        addr,
        port,
        tags,
        status: node_state_to_consul_status(&self_member.state),
        ..Default::default()
    };

    let mut agent_stats = HashMap::new();
    agent_stats.insert("check_monitors".to_string(), "0".to_string());
    agent_stats.insert("check_ttls".to_string(), "0".to_string());
    agent_stats.insert("checks".to_string(), "0".to_string());
    agent_stats.insert("services".to_string(), "0".to_string());

    let mut runtime_stats = HashMap::new();
    runtime_stats.insert("arch".to_string(), std::env::consts::ARCH.to_string());
    runtime_stats.insert("os".to_string(), std::env::consts::OS.to_string());
    runtime_stats.insert("version".to_string(), "rust".to_string());

    // Add cluster health stats
    let mut cluster_stats = HashMap::new();
    cluster_stats.insert("total".to_string(), health_summary.total.to_string());
    cluster_stats.insert("up".to_string(), health_summary.up.to_string());
    cluster_stats.insert("down".to_string(), health_summary.down.to_string());
    cluster_stats.insert(
        "suspicious".to_string(),
        health_summary.suspicious.to_string(),
    );
    cluster_stats.insert("starting".to_string(), health_summary.starting.to_string());
    cluster_stats.insert(
        "standalone".to_string(),
        member_manager.is_standalone().to_string(),
    );
    cluster_stats.insert(
        "is_leader".to_string(),
        member_manager.is_leader().to_string(),
    );

    let stats = AgentStats {
        agent: agent_stats,
        runtime: runtime_stats,
        raft: None,
        serf_lan: Some(cluster_stats),
    };

    let mut meta = HashMap::new();
    meta.insert("consul-network-segment".to_string(), "".to_string());

    let response = AgentSelf {
        config,
        coord: None,
        member,
        meta,
        stats,
    };

    HttpResponse::Ok().json(response)
}

/// GET /v1/agent/host
/// Returns information about the host the agent is running on
pub async fn get_agent_host(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
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

    HttpResponse::Ok().json(response)
}

/// GET /v1/agent/version
/// Returns the Consul version of the agent
pub async fn get_agent_version() -> HttpResponse {
    let version = env!("CARGO_PKG_VERSION");
    let response = AgentVersion {
        version: format!("1.15.0-batata-{}", version),
        revision: version.to_string(),
        prerelease: "".to_string(),
        human_version: format!("1.15.0-batata-{}", version),
        build_date: "2024-01-01T00:00:00Z".to_string(),
        fips: "".to_string(),
    };
    HttpResponse::Ok().json(response)
}

/// PUT /v1/agent/join/{address}
/// Triggers the agent to join a cluster by address
pub async fn agent_join(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let address = path.into_inner();

    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // In compatibility mode, just return success
    // Real cluster join would be handled by Batata's cluster management
    tracing::info!("Agent join requested for address: {}", address);
    HttpResponse::Ok().finish()
}

/// PUT /v1/agent/leave
/// Triggers a graceful leave and shutdown of the agent
pub async fn agent_leave(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // In compatibility mode, just return success
    // Real leave would trigger Batata's graceful shutdown
    tracing::info!("Agent leave requested");
    HttpResponse::Ok().finish()
}

/// PUT /v1/agent/force-leave/{node}
/// Forces a node into the left state
pub async fn agent_force_leave(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let node = path.into_inner();

    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // In compatibility mode, just return success
    tracing::info!("Agent force-leave requested for node: {}", node);
    HttpResponse::Ok().finish()
}

/// PUT /v1/agent/reload
/// Triggers a reload of the agent's configuration
pub async fn agent_reload(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // In compatibility mode, return success (config reload not actually supported)
    tracing::info!("Agent reload requested");
    HttpResponse::Ok().finish()
}

/// PUT /v1/agent/maintenance
/// Toggles node maintenance mode
pub async fn agent_maintenance(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    query: web::Query<AgentMaintenanceRequest>,
) -> HttpResponse {
    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
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
        let _ = health_service.register_check(registration);
    } else {
        // Remove the maintenance check
        let _ = health_service.deregister_check(&check_id);
    }

    tracing::info!(
        "Agent maintenance mode: {} (reason: {})",
        if enable { "enabled" } else { "disabled" },
        reason
    );

    HttpResponse::Ok().finish()
}

/// GET /v1/agent/metrics
/// Returns metrics for the agent (Prometheus format compatible)
#[allow(clippy::vec_init_then_push)]
pub async fn get_agent_metrics(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Collect basic system metrics
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut gauges = Vec::new();

    // Runtime metrics
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
    gauges.push(GaugeMetric::new(
        "consul.runtime.heap_objects",
        0.0, // Not directly available in Rust
    ));

    // CPU metrics
    let cpu_usage = sys.global_cpu_usage();
    gauges.push(GaugeMetric::new(
        "batata.runtime.cpu_percent",
        cpu_usage as f64,
    ));
    gauges.push(GaugeMetric::new(
        "batata.runtime.cpu_cores",
        sys.cpus().len() as f64,
    ));

    // Memory metrics
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

    let response = MetricsResponse {
        timestamp: chrono::Utc::now().to_rfc3339(),
        gauges,
        counters: Vec::new(),
        samples: Vec::new(),
        points: Vec::new(),
    };

    HttpResponse::Ok().json(response)
}

/// GET /v1/agent/metrics (Real version with service and cluster metrics)
/// Returns comprehensive metrics including service counts and cluster health
#[allow(clippy::vec_init_then_push)]
pub async fn get_agent_metrics_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    agent: web::Data<ConsulAgentService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
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

    // Service metrics from NamingService
    let (total_services, service_names) =
        agent
            .naming_service
            .list_services("public", "DEFAULT_GROUP", 1, 10000);

    gauges.push(
        GaugeMetric::new("consul.catalog.service_count", total_services as f64)
            .with_label("datacenter", "dc1"),
    );
    gauges.push(
        GaugeMetric::new("batata.naming.service_count", total_services as f64)
            .with_label("namespace", "public")
            .with_label("group", "DEFAULT_GROUP"),
    );

    // Count total instances across all services
    let mut total_instances = 0u64;
    let mut healthy_instances = 0u64;
    let mut unhealthy_instances = 0u64;

    for service_name in &service_names {
        let instances =
            agent
                .naming_service
                .get_instances("public", "DEFAULT_GROUP", service_name, "", false);

        total_instances += instances.len() as u64;
        for instance in &instances {
            if instance.healthy && instance.enabled {
                healthy_instances += 1;
            } else {
                unhealthy_instances += 1;
            }
        }

        // Per-service instance count
        gauges.push(
            GaugeMetric::new(
                "batata.naming.service_instance_count",
                instances.len() as f64,
            )
            .with_label("service", service_name)
            .with_label("namespace", "public"),
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

    // Cluster metrics from ServerMemberManager
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

    HttpResponse::Ok().json(response)
}

/// GET /v1/agent/monitor
/// Streams logs from the agent (stub - returns empty for compatibility)
pub async fn agent_monitor(req: HttpRequest, acl_service: web::Data<AclService>) -> HttpResponse {
    // Check ACL authorization for agent read
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    // Return empty response - log streaming not supported in compatibility mode
    HttpResponse::Ok().finish()
}

/// PUT /v1/agent/token/{type}
/// Updates the ACL token for the agent
pub async fn update_agent_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    let token_type = path.into_inner();

    // Check ACL authorization for agent write
    let authz = acl_service.authorize_request(&req, ResourceType::Agent, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    tracing::info!("Agent token update requested for type: {}", token_type);
    HttpResponse::Ok().finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consul_agent_service_creation() {
        let naming_service = Arc::new(NamingService::new());
        let naming_service_clone = naming_service.clone();
        let agent = ConsulAgentService::new(naming_service);
        // Verify the service was stored correctly
        assert!(Arc::ptr_eq(&agent.naming_service, &naming_service_clone));
    }

    #[test]
    fn test_node_state_to_consul_status_all_variants() {
        assert_eq!(node_state_to_consul_status(&NodeState::Up), 1); // alive
        assert_eq!(node_state_to_consul_status(&NodeState::Down), 4); // failed
        assert_eq!(node_state_to_consul_status(&NodeState::Suspicious), 2); // leaving
        assert_eq!(node_state_to_consul_status(&NodeState::Starting), 3); // left
        assert_eq!(node_state_to_consul_status(&NodeState::Isolation), 4); // failed
    }

    #[test]
    fn test_create_health_check_healthy_instance() {
        let mut instance = NacosInstance::new("10.0.0.1".to_string(), 8080);
        instance.instance_id = "healthy-001".to_string();
        instance.service_name = "web".to_string();
        instance.healthy = true;
        instance.enabled = true;

        let check = create_service_health_check(&instance, "web");
        assert_eq!(check.status, "passing");
        assert_eq!(check.check_id, "service:web:healthy-001");
        assert_eq!(check.name, "web Health Check");
        assert!(check.output.contains("healthy"));
        assert_eq!(check.service_id, "healthy-001");
        assert_eq!(check.service_name, "web");
        assert_eq!(check.check_type, "service");
    }

    #[test]
    fn test_create_health_check_unhealthy_instance() {
        let mut instance = NacosInstance::new("10.0.0.2".to_string(), 8080);
        instance.instance_id = "unhealthy-001".to_string();
        instance.service_name = "api".to_string();
        instance.healthy = false;
        instance.enabled = true;

        let check = create_service_health_check(&instance, "api");
        assert_eq!(check.status, "critical");
        assert!(check.output.contains("unhealthy"));
    }

    #[test]
    fn test_create_health_check_disabled_instance() {
        let mut instance = NacosInstance::new("10.0.0.3".to_string(), 8080);
        instance.instance_id = "disabled-001".to_string();
        instance.service_name = "worker".to_string();
        instance.healthy = true;
        instance.enabled = false;

        let check = create_service_health_check(&instance, "worker");
        assert_eq!(check.status, "critical");
        assert!(check.output.contains("maintenance"));
    }

    #[test]
    fn test_create_health_check_disabled_and_unhealthy() {
        let mut instance = NacosInstance::new("10.0.0.4".to_string(), 8080);
        instance.instance_id = "bad-001".to_string();
        instance.service_name = "cache".to_string();
        instance.healthy = false;
        instance.enabled = false;

        let check = create_service_health_check(&instance, "cache");
        // Disabled takes precedence over unhealthy
        assert_eq!(check.status, "critical");
        assert!(check.output.contains("maintenance"));
    }

    #[test]
    fn test_create_health_check_id_format() {
        let mut instance = NacosInstance::new("192.168.1.1".to_string(), 9090);
        instance.instance_id = "svc-abc-123".to_string();
        instance.service_name = "my-service".to_string();
        instance.healthy = true;
        instance.enabled = true;

        let check = create_service_health_check(&instance, "my-service");
        assert_eq!(check.check_id, "service:my-service:svc-abc-123");
    }
}
