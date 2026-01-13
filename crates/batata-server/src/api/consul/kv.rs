// Consul KV Store API - Local extensions
// Re-exports from batata_plugin_consul with export/import handlers that need AppState

// Re-export all types from plugin
pub use batata_plugin_consul::kv::*;

// Local export/import handlers that require AppState
use actix_web::{HttpRequest, HttpResponse, web};

use super::acl::{AclService, ResourceType};
use super::model::ConsulError;
use crate::{
    config::export_model::{
        ConsulExportRequest, ConsulImportRequest, ConsulKVExportItem, ImportResult,
    },
    model::common::AppState,
    service::{config_export, config_import},
};

/// GET /v1/kv/export
/// Export all configurations in Consul format
pub async fn export_kv(
    data: web::Data<AppState>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<ConsulExportRequest>,
) -> HttpResponse {
    // Check ACL authorization for key read
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "", false);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let namespace_id = query
        .namespace_id
        .clone()
        .unwrap_or_else(|| "public".to_string());

    // Find configs for export
    let configs =
        match config_export::find_configs_for_export(data.db(), &namespace_id, None, None, None)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                return HttpResponse::InternalServerError().json(ConsulError::new(e.to_string()));
            }
        };

    if configs.is_empty() {
        return HttpResponse::Ok().json(Vec::<ConsulKVExportItem>::new());
    }

    // Create Consul JSON
    let json_data = match config_export::create_consul_export_json(configs, &namespace_id) {
        Ok(j) => j,
        Err(e) => return HttpResponse::InternalServerError().json(ConsulError::new(e.to_string())),
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .body(json_data)
}

/// PUT /v1/kv/import
/// Import configurations from Consul format
pub async fn import_kv(
    data: web::Data<AppState>,
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    query: web::Query<ConsulImportRequest>,
    body: web::Json<Vec<ConsulKVExportItem>>,
) -> HttpResponse {
    // Check ACL authorization for key write
    let authz = acl_service.authorize_request(&req, ResourceType::Key, "", true);
    if !authz.allowed {
        return HttpResponse::Forbidden().json(ConsulError::new(&authz.reason));
    }

    let namespace_id = query
        .namespace_id
        .clone()
        .unwrap_or_else(|| "public".to_string());
    let items = body.into_inner();

    if items.is_empty() {
        return HttpResponse::Ok().json(ImportResult::default());
    }

    // Serialize items back to JSON for parsing
    let json_data = match serde_json::to_vec(&items) {
        Ok(j) => j,
        Err(e) => return HttpResponse::BadRequest().json(ConsulError::new(e.to_string())),
    };

    // Parse Consul JSON to config items
    let config_items = match config_import::parse_consul_import_json(&json_data, &namespace_id) {
        Ok(i) => i,
        Err(e) => return HttpResponse::BadRequest().json(ConsulError::new(e.to_string())),
    };

    // Get user info from connection (Consul API doesn't have auth context like Nacos)
    let src_user = "consul-import".to_string();
    let src_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    // Import configs with overwrite policy (Consul behavior)
    let result = match config_import::import_configs(
        data.db(),
        config_items,
        &namespace_id,
        crate::config::export_model::SameConfigPolicy::Overwrite,
        &src_user,
        &src_ip,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return HttpResponse::InternalServerError().json(ConsulError::new(e.to_string())),
    };

    HttpResponse::Ok().json(result)
}
