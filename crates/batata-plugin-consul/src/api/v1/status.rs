//! Consul Status API handlers with scope-relative route macros.
//!
//! Delegates to the real cluster-aware handlers in `crate::status`.

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Scope, get, web};

use batata_common::ClusterManager;

use crate::acl::AclService;
use crate::model::ConsulDatacenterConfig;

#[get("/leader")]
async fn get_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::status::get_leader(req, acl_service, member_manager, dc_config).await
}

#[get("/peers")]
async fn get_peers(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
) -> HttpResponse {
    crate::status::get_peers(req, acl_service, member_manager, dc_config).await
}

pub fn routes() -> Scope {
    web::scope("/status").service(get_leader).service(get_peers)
}
