//! Consul Config Entry API handlers with full-path route macros.
//!
//! These use `#[put("/v1/config")]` style macros to avoid actix-web scope
//! conflicts with other `/v1` scoped routes (KV, lock, snapshot, etc.).

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, put, web};

use crate::acl::{AclService, ResourceType};
use crate::config_entry::{
    ConfigEntryDeleteParams, ConfigEntryListParams, ConfigEntryRequest, ConsulConfigEntryService,
    SUPPORTED_KINDS,
};
use crate::consul_meta::{ConsulResponseMeta, consul_ok};
use crate::index_provider::{ConsulIndexProvider, ConsulTable};
use crate::model::ConsulError;
use crate::model::ConsulErrorBody;

// ============================================================================
// In-memory handlers
// ============================================================================

#[put("")]
async fn apply_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    index_provider: web::Data<ConsulIndexProvider>,
    body: web::Bytes,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let entry_req: ConfigEntryRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest()
                .consul_error(ConsulError::new(format!("Invalid request body: {}", e)));
        }
    };

    if !SUPPORTED_KINDS.contains(&entry_req.kind.as_str()) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
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
        Ok(success) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            consul_ok(&meta).json(success)
        }
        Err(e) => HttpResponse::InternalServerError().consul_error(ConsulError::new(e)),
    }
}

#[get("/{kind}/{name}")]
async fn get_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    _query: web::Query<ConfigEntryListParams>,
    _index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.get_entry(&kind, &name) {
        Some(entry) => {
            // Use entry's ModifyIndex as X-Consul-Index so CAS works correctly
            let meta = ConsulResponseMeta::new(entry.modify_index);
            consul_ok(&meta).json(entry)
        }
        None => HttpResponse::NotFound().consul_error(ConsulError::new("Config entry not found")),
    }
}

#[delete("/{kind}/{name}")]
async fn delete_config_entry(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<(String, String)>,
    query: web::Query<ConfigEntryDeleteParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let (kind, name) = path.into_inner();
    match config_service.delete_entry(&kind, &name, query.cas).await {
        Ok(success) => {
            let meta =
                ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
            if query.cas.is_some() {
                consul_ok(&meta).json(success)
            } else {
                consul_ok(&meta).json(serde_json::json!({}))
            }
        }
        Err(e) => HttpResponse::InternalServerError().consul_error(ConsulError::new(e)),
    }
}

#[get("/{kind}")]
async fn list_config_entries(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    config_service: web::Data<ConsulConfigEntryService>,
    path: web::Path<String>,
    _query: web::Query<ConfigEntryListParams>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    let authz = acl_service.authorize_request(&req, ResourceType::Service, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().consul_error(ConsulError::new(authz.reason));
    }

    let kind = path.into_inner();
    if !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return HttpResponse::BadRequest().consul_error(ConsulError::new(format!(
            "Unsupported config entry kind: {}",
            kind
        )));
    }

    let entries = config_service.list_entries(&kind);
    let meta = ConsulResponseMeta::new(index_provider.current_index(ConsulTable::ConfigEntries));
    consul_ok(&meta).json(entries)
}

pub fn routes() -> Scope {
    web::scope("/config")
        .service(apply_config_entry)
        .service(get_config_entry)
        .service(delete_config_entry)
        .service(list_config_entries)
}
