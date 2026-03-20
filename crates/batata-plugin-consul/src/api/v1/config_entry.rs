//! Consul Config Entry API handlers with full-path route macros.
//!
//! These use `#[put("/v1/config")]` style macros to avoid actix-web scope
//! conflicts with other `/v1` scoped routes (KV, lock, snapshot, etc.).

use actix_web::{HttpRequest, HttpResponse, delete, get, put, web};

use crate::acl::{AclService, ResourceType};
use crate::config_entry::{
    ConfigEntryDeleteParams, ConfigEntryListParams, ConfigEntryRequest, ConsulConfigEntryService,
    ConsulConfigEntryServicePersistent, SUPPORTED_KINDS,
};
use crate::index_provider::ConsulIndexProvider;
use crate::model::ConsulError;

// ============================================================================
// In-memory handlers
// ============================================================================

#[put("/v1/config")]
pub async fn apply_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Bytes,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entry_req: ConfigEntryRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ConsulError::new(format!("Invalid request body: {}", e)));
        }
    };

    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            entry_req.kind
        )));
    }

    let cas: Option<u64> = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            if kv.next() == Some("cas") {
                kv.next().and_then(|v| v.parse().ok())
            } else {
                None
            }
        })
    });

    match config_service.apply_entry(entry_req, cas) {
        Ok(success) => HttpResponse::Ok()
            .insert_header(("X-Consul-Index", index_provider.current_index().to_string()))
            .json(success),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

#[get("/v1/config/{kind}/{name}")]
pub async fn get_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name) {
        Some(entry) => HttpResponse::Ok()
            .insert_header(("X-Consul-Index", index_provider.current_index().to_string()))
            .json(entry),
        None => HttpResponse::NotFound().json(ConsulError::new("Config entry not found")),
    }
}

#[delete("/v1/config/{kind}/{name}")]
pub async fn delete_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    query: web::Query<ConfigEntryDeleteParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.delete_entry(&kind, &name, query.cas) {
        Ok(success) => {
            if query.cas.is_some() {
                HttpResponse::Ok()
                    .insert_header((
                        "X-Consul-Index",
                        index_provider.current_index().to_string(),
                    ))
                    .json(success)
            } else {
                HttpResponse::Ok()
                    .insert_header((
                        "X-Consul-Index",
                        index_provider.current_index().to_string(),
                    ))
                    .json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

#[get("/v1/config/{kind}")]
pub async fn list_config_entries(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let kind = path.into_inner();
    if !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            kind
        )));
    }

    let entries = config_service.list_entries(&kind);
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index_provider.current_index().to_string()))
        .json(entries)
}

// ============================================================================
// Persistent handlers
// ============================================================================

#[put("/v1/config")]
pub async fn apply_config_entry_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Bytes,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let entry_req: ConfigEntryRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest()
                .json(ConsulError::new(format!("Invalid request body: {}", e)));
        }
    };

    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            entry_req.kind
        )));
    }

    let cas: Option<u64> = req.uri().query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            if kv.next() == Some("cas") {
                kv.next().and_then(|v| v.parse().ok())
            } else {
                None
            }
        })
    });

    match config_service.apply_entry(entry_req, cas).await {
        Ok(success) => HttpResponse::Ok()
            .insert_header(("X-Consul-Index", index_provider.current_index().to_string()))
            .json(success),
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

#[get("/v1/config/{kind}/{name}")]
pub async fn get_config_entry_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    path: web::Path<(String, String)>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name).await {
        Some(entry) => HttpResponse::Ok()
            .insert_header(("X-Consul-Index", index_provider.current_index().to_string()))
            .json(entry),
        None => HttpResponse::NotFound().json(ConsulError::new("Config entry not found")),
    }
}

#[delete("/v1/config/{kind}/{name}")]
pub async fn delete_config_entry_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    path: web::Path<(String, String)>,
    query: web::Query<ConfigEntryDeleteParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.delete_entry(&kind, &name, query.cas).await {
        Ok(success) => {
            if query.cas.is_some() {
                HttpResponse::Ok()
                    .insert_header((
                        "X-Consul-Index",
                        index_provider.current_index().to_string(),
                    ))
                    .json(success)
            } else {
                HttpResponse::Ok()
                    .insert_header((
                        "X-Consul-Index",
                        index_provider.current_index().to_string(),
                    ))
                    .json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(ConsulError::new(e)),
    }
}

#[get("/v1/config/{kind}")]
pub async fn list_config_entries_persistent(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryServicePersistent>,
    path: web::Path<String>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(authz.reason));
    }

    let kind = path.into_inner();
    if !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return HttpResponse::BadRequest().json(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            kind
        )));
    }

    let entries = config_service.list_entries(&kind).await;
    HttpResponse::Ok()
        .insert_header(("X-Consul-Index", index_provider.current_index().to_string()))
        .json(entries)
}
