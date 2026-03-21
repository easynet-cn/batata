//! Consul Status API handlers with scope-relative route macros.
//!
//! These delegate to the original handlers in `crate::status`.
//! Includes both fixed/fallback and real cluster variants.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Scope, get, web};

use batata_core::service::cluster::ServerMemberManager;

use crate::acl::AclService;
use crate::model::ConsulDatacenterConfig;

// ============================================================================
// Fixed/fallback handlers
// ============================================================================

#[get("/leader")]
async fn get_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::status::get_leader(req, acl_service, dc_config).await
}

#[get("/peers")]
async fn get_peers(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::status::get_peers(req, acl_service, dc_config).await
}

// ============================================================================
// Real cluster handlers
// ============================================================================

#[get("/leader")]
async fn get_leader_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::status::get_leader_real(req, acl_service, member_manager, dc_config).await
}

#[get("/peers")]
async fn get_peers_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<ServerMemberManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::status::get_peers_real(req, acl_service, member_manager, dc_config).await
}

// ============================================================================
// Scope builders
// ============================================================================

/// Fixed/fallback status routes
pub fn routes() -> Scope {
    web::scope("/status").service(get_leader).service(get_peers)
}

/// Real cluster status routes (using ServerMemberManager)
pub fn routes_real() -> Scope {
    web::scope("/status")
        .service(get_leader_real)
        .service(get_peers_real)
}
