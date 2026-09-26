//! Consul Partition API handlers with scope-relative route macros.

// actix-web route macros (`#[get]`, `#[post]`, `#[delete]`, ...) expand to a
// struct that cannot carry a doc comment, which trips `missing_docs`. The
// generated struct is an internal implementation detail, so the lint is allowed
// for this module.
#![allow(missing_docs)]

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, put, web};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::partition::{ConsulPartitionService, Partition};

#[get("")]
async fn list_partitions(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::partition::list_partitions(req, partition_service, acl_service, index_provider).await
}

#[get("/{name}")]
async fn read_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::partition::read_partition(req, partition_service, acl_service, path, index_provider).await
}

#[put("")]
async fn create_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    body: web::Json<Partition>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::partition::create_partition(req, partition_service, acl_service, body, index_provider)
        .await
}

#[put("/{name}")]
async fn update_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<Partition>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::partition::update_partition(req, partition_service, acl_service, path, body, index_provider)
        .await
}

#[delete("/{name}")]
async fn delete_partition(
    req: HttpRequest,
    partition_service: web::Data<ConsulPartitionService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::partition::delete_partition(req, partition_service, acl_service, path, index_provider)
        .await
}

/// Singular partition scope: /v1/partition
pub fn partition_routes() -> Scope {
    web::scope("/partition")
        .service(create_partition)
        .service(read_partition)
        .service(update_partition)
        .service(delete_partition)
}

/// Plural partitions scope: /v1/partitions
pub fn partitions_routes() -> Scope {
    web::scope("/partitions").service(list_partitions)
}
