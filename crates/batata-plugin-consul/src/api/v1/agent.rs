//! Consul Agent API handlers with scope-relative route macros.
//!
//! Thin wrappers that delegate to the original handler functions in
//! `crate::agent` and `crate::health` (for check endpoints).

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Scope, get, put, web};

use batata_core::service::cluster::ServerMemberManager;
use batata_naming::service::NamingService;

use crate::acl::AclService;
use crate::agent::{ConsulAgentService, MonitorQueryParams};
use crate::health::ConsulHealthService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::{
    AgentMaintenanceRequest, AgentMembersParams, AgentServiceRegistration, CheckRegistration,
    CheckStatusUpdate, CheckUpdateParams, ConsulDatacenterConfig, MaintenanceRequest,
    ServiceQueryParams,
};

// ============================================================================
// Agent core endpoints (in-memory / persistent shared)
// ============================================================================

#[get("/self")]
async fn get_agent_self(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_self(req, acl_service, dc_config, index_provider).await
}

#[get("/members")]
async fn get_agent_members(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    _query: web::Query<AgentMembersParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_members(req, acl_service, dc_config, _query, index_provider).await
}

#[get("/host")]
async fn get_agent_host(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_host(req, acl_service, index_provider).await
}

#[get("/version")]
async fn get_agent_version(
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_version(dc_config, index_provider).await
}

#[put("/join/{address}")]
async fn agent_join(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_join(req, acl_service, path, index_provider).await
}

#[put("/leave")]
async fn agent_leave(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_leave(req, acl_service, index_provider).await
}

#[put("/force-leave/{node}")]
async fn agent_force_leave(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_force_leave(req, acl_service, path, index_provider).await
}

#[put("/reload")]
async fn agent_reload(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_reload(req, acl_service, index_provider).await
}

#[put("/maintenance")]
async fn agent_maintenance(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    query: web::Query<AgentMaintenanceRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_maintenance(req, health_service, acl_service, query, index_provider).await
}

#[get("/metrics")]
async fn get_agent_metrics(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_metrics(req, acl_service, index_provider).await
}

#[get("/metrics/stream")]
async fn agent_metrics_stream(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_metrics_stream(req, naming_service, acl_service, dc_config, index_provider)
        .await
}

#[get("/monitor")]
async fn agent_monitor(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    query: web::Query<MonitorQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_monitor(req, acl_service, query, index_provider).await
}

#[put("/token/{token_type}")]
async fn update_agent_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::update_agent_token(req, acl_service, path, index_provider).await
}

// ============================================================================
// Service registration endpoints
// ============================================================================

#[put("/service/register")]
async fn register_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    health_service: web::Data<ConsulHealthService>,
    index_provider: web::Data<ConsulIndexProvider>,
    query: web::Query<ServiceQueryParams>,
    body: web::Json<AgentServiceRegistration>,
) -> HttpResponse {
    crate::agent::register_service(
        req,
        agent,
        acl_service,
        dc_config,
        health_service,
        index_provider,
        query,
        body,
    )
    .await
}

#[put("/service/deregister/{service_id}")]
async fn deregister_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    health_service: web::Data<ConsulHealthService>,
    index_provider: web::Data<ConsulIndexProvider>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
) -> HttpResponse {
    crate::agent::deregister_service(
        req,
        agent,
        acl_service,
        dc_config,
        health_service,
        index_provider,
        path,
        query,
    )
    .await
}

#[get("/services")]
async fn list_services(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::list_services(req, agent, acl_service, dc_config, query, index_provider).await
}

#[get("/service/{service_id}")]
async fn get_service(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_service(
        req,
        agent,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[put("/service/maintenance/{service_id}")]
async fn set_service_maintenance(
    req: HttpRequest,
    _agent: web::Data<ConsulAgentService>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<MaintenanceRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::set_service_maintenance(
        req,
        _agent,
        health_service,
        acl_service,
        path,
        query,
        index_provider,
    )
    .await
}

// ============================================================================
// Check endpoints (delegating to crate::health)
// ============================================================================

#[put("/check/register")]
async fn register_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    body: web::Json<CheckRegistration>,
) -> HttpResponse {
    crate::health::register_check(req, health_service, acl_service, body).await
}

#[put("/check/deregister/{check_id}")]
async fn deregister_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
) -> HttpResponse {
    crate::health::deregister_check(req, health_service, acl_service, path).await
}

#[put("/check/pass/{check_id}")]
async fn pass_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    crate::health::pass_check(req, health_service, acl_service, path, query).await
}

#[put("/check/warn/{check_id}")]
async fn warn_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    crate::health::warn_check(req, health_service, acl_service, path, query).await
}

#[put("/check/fail/{check_id}")]
async fn fail_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    query: web::Query<CheckUpdateParams>,
) -> HttpResponse {
    crate::health::fail_check(req, health_service, acl_service, path, query).await
}

#[put("/check/update/{check_id}")]
async fn update_check(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<CheckStatusUpdate>,
) -> HttpResponse {
    crate::health::update_check(req, health_service, acl_service, path, body).await
}

#[get("/checks")]
async fn list_agent_checks(
    req: HttpRequest,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    _query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::health::list_agent_checks(req, health_service, acl_service, _query, index_provider).await
}

// ============================================================================
// Agent health service endpoints
// ============================================================================

#[get("/health/service/id/{service_id}")]
async fn agent_health_service_by_id(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_health_service_by_id(
        req,
        agent,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

#[get("/health/service/name/{service_name}")]
async fn agent_health_service_by_name(
    req: HttpRequest,
    agent: web::Data<ConsulAgentService>,
    health_service: web::Data<ConsulHealthService>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    path: web::Path<String>,
    query: web::Query<ServiceQueryParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_health_service_by_name(
        req,
        agent,
        health_service,
        acl_service,
        dc_config,
        path,
        query,
        index_provider,
    )
    .await
}

// ============================================================================
// Real cluster variant endpoints
// ============================================================================

#[get("/self")]
async fn get_agent_self_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_self_real(req, acl_service, dc_config, member_manager, index_provider)
        .await
}

#[get("/members")]
async fn get_agent_members_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    _query: web::Query<AgentMembersParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_members_real(
        req,
        acl_service,
        dc_config,
        member_manager,
        _query,
        index_provider,
    )
    .await
}

#[get("/metrics")]
async fn get_agent_metrics_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    agent: web::Data<ConsulAgentService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::get_agent_metrics_real(
        req,
        acl_service,
        dc_config,
        agent,
        member_manager,
        index_provider,
    )
    .await
}

#[get("/metrics/stream")]
async fn agent_metrics_stream_real(
    req: HttpRequest,
    naming_service: web::Data<Arc<NamingService>>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::agent::agent_metrics_stream_real(
        req,
        naming_service,
        member_manager,
        acl_service,
        dc_config,
        index_provider,
    )
    .await
}

// ============================================================================
// Route registration
// ============================================================================

/// In-memory agent routes
pub fn routes() -> Scope {
    web::scope("/agent")
        // Agent core
        .service(get_agent_self)
        .service(get_agent_members)
        .service(get_agent_host)
        .service(get_agent_version)
        .service(agent_join)
        .service(agent_leave)
        .service(agent_force_leave)
        .service(agent_reload)
        .service(agent_maintenance)
        .service(get_agent_metrics)
        .service(agent_metrics_stream)
        .service(agent_monitor)
        .service(update_agent_token)
        // Service registration
        .service(register_service)
        .service(deregister_service)
        .service(list_services)
        .service(get_service)
        .service(set_service_maintenance)
        // Check endpoints
        .service(register_check)
        .service(deregister_check)
        .service(pass_check)
        .service(warn_check)
        .service(fail_check)
        .service(update_check)
        .service(list_agent_checks)
        // Agent health service endpoints
        .service(agent_health_service_by_id)
        .service(agent_health_service_by_name)
        // Agent connect routes (leaf cert, authorize)
        .service(crate::api::v1::connect_ca::agent_connect_routes())
}

/// Real cluster agent routes (uses ServerMemberManager for /self, /members, /metrics)
pub fn routes_real() -> Scope {
    web::scope("/agent")
        // Agent core - real cluster variants
        .service(get_agent_self_real)
        .service(get_agent_members_real)
        .service(get_agent_host)
        .service(get_agent_version)
        .service(agent_join)
        .service(agent_leave)
        .service(agent_force_leave)
        .service(agent_reload)
        .service(agent_maintenance)
        .service(get_agent_metrics_real)
        .service(agent_metrics_stream_real)
        .service(agent_monitor)
        .service(update_agent_token)
        // Service registration
        .service(register_service)
        .service(deregister_service)
        .service(list_services)
        .service(get_service)
        .service(set_service_maintenance)
        // Check endpoints
        .service(register_check)
        .service(deregister_check)
        .service(pass_check)
        .service(warn_check)
        .service(fail_check)
        .service(update_check)
        .service(list_agent_checks)
        // Agent health service endpoints
        .service(agent_health_service_by_id)
        .service(agent_health_service_by_name)
        // Agent connect routes (leaf cert, authorize)
        .service(crate::api::v1::connect_ca::agent_connect_routes())
}
