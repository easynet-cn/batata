//! Consul Namespace API handlers with scope-relative route macros.

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, put, web};

use crate::acl::AclService;
use crate::index_provider::ConsulIndexProvider;
use crate::namespace::{ConsulNamespaceService, Namespace};

#[get("")]
async fn list_namespaces(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::namespace::list_namespaces(req, ns_service, acl_service, index_provider).await
}

#[get("/{name}")]
async fn read_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::namespace::read_namespace(req, ns_service, acl_service, path, index_provider).await
}

#[put("")]
async fn create_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    body: web::Json<Namespace>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::namespace::create_namespace(req, ns_service, acl_service, body, index_provider).await
}

#[put("/{name}")]
async fn update_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<Namespace>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::namespace::update_namespace(req, ns_service, acl_service, path, body, index_provider)
        .await
}

#[delete("/{name}")]
async fn delete_namespace(
    req: HttpRequest,
    ns_service: web::Data<ConsulNamespaceService>,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::namespace::delete_namespace(req, ns_service, acl_service, path, index_provider).await
}

/// Singular namespace scope: /v1/namespace
pub fn namespace_routes() -> Scope {
    web::scope("/namespace")
        .service(create_namespace)
        .service(read_namespace)
        .service(update_namespace)
        .service(delete_namespace)
}

/// Plural namespaces scope: /v1/namespaces
pub fn namespaces_routes() -> Scope {
    web::scope("/namespaces").service(list_namespaces)
}
