// Consul Agent API HTTP handlers
// Implements Consul-compatible service registration endpoints

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};

use batata_api::naming::model::Instance as NacosInstance;
use batata_naming::service::NamingService;

use crate::acl::{AclService, ResourceType};
use crate::model::{
    AgentService, AgentServiceRegistration, AgentServiceWithChecks, ConsulError,
    MaintenanceRequest, ServiceQueryParams,
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
                let response = AgentServiceWithChecks {
                    service: agent_service,
                    checks: None, // Health checks not implemented yet
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

/// PUT /v1/agent/service/maintenance/{service_id}
/// Places a service into maintenance mode
pub async fn set_service_maintenance(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<MaintenanceRequest>,
) -> HttpResponse {
    let service_id = path.into_inner();
    let enable = query.enable;
    let _reason = query.reason.clone();

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

    // Find and update the service
    // In Nacos, we can simulate maintenance by setting enabled = false
    let namespace = "public";
    let (_, service_names) =
        agent
            .naming_service
            .list_services(namespace, "DEFAULT_GROUP", 1, 10000);

    for service_name in service_names {
        let instances = agent.naming_service.get_instances(
            namespace,
            "DEFAULT_GROUP",
            &service_name,
            "",
            false,
        );

        for instance in &instances {
            if instance.instance_id == service_id {
                // Create updated instance with enabled status toggled
                let mut updated_instance = instance.clone();
                updated_instance.enabled = !enable;

                // Deregister old and register new
                agent.naming_service.deregister_instance(
                    namespace,
                    "DEFAULT_GROUP",
                    &service_name,
                    instance,
                );
                agent.naming_service.register_instance(
                    namespace,
                    "DEFAULT_GROUP",
                    &service_name,
                    updated_instance,
                );

                return HttpResponse::Ok().finish();
            }
        }
    }

    HttpResponse::NotFound().json(ConsulError::new(format!(
        "Service not found: {}",
        service_id
    )))
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
}
