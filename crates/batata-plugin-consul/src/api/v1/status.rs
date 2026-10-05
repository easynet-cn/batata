//! Consul Status API handlers with scope-relative route macros.
//!
//! Delegates to the real cluster-aware handlers in `crate::status`.

// actix-web route macros (`#[get]`, `#[post]`, `#[delete]`, ...) expand to a
// struct that cannot carry a doc comment, which trips `missing_docs`. The
// generated struct is an internal implementation detail, so the lint is allowed
// for this module.
#![allow(missing_docs)]

use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, Scope, get, web};

use batata_common::ClusterManager;

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::model::ConsulDatacenterConfig;

#[get("/leader")]
async fn get_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    member_manager: web::Data<Arc<dyn ClusterManager>>,
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::status::get_leader(req, acl_service, member_manager, dc_config, index_provider).await
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

/// The `routes` function.
pub fn routes() -> Scope {
    web::scope("/status").service(get_leader).service(get_peers)
}
