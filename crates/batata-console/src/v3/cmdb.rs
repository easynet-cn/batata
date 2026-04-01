//! Console CMDB management API endpoints
//!
//! Provides endpoints for managing CMDB entities, labels, and sync operations.

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, delete, get, post, put, web};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

use batata_common::CmdbPlugin;
use batata_common::model::plugin::cmdb::{CmdbEntity, CmdbEntityType, LabelMapping};
use batata_server_common::{ActionTypes, ApiType, Secured, SignType, model::AppState, secured};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityListQuery {
    #[serde(default, alias = "entityType")]
    pub entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntityRequest {
    pub name: String,
    #[serde(default = "default_entity_type")]
    pub entity_type: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

fn default_entity_type() -> String {
    "Service".to_string()
}

fn parse_entity_type(s: &str) -> CmdbEntityType {
    match s {
        "Instance" => CmdbEntityType::Instance,
        "Config" => CmdbEntityType::Config,
        "Namespace" => CmdbEntityType::Namespace,
        "Host" => CmdbEntityType::Host,
        "Application" => CmdbEntityType::Application,
        "Environment" => CmdbEntityType::Environment,
        _ => CmdbEntityType::Service,
    }
}

/// GET /v3/console/cmdb/entities - List CMDB entities
#[get("/entities")]
async fn list_entities(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    query: web::Query<EntityListQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let entity_type = query.entity_type.as_deref().map(parse_entity_type);

    match plugin.list_entities(entity_type).await {
        Ok(entities) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "data": entities,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// POST /v3/console/cmdb/entities - Register a CMDB entity
#[post("/entities")]
async fn create_entity(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    body: web::Json<CreateEntityRequest>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let entity_type = parse_entity_type(&body.entity_type);
    let mut entity = CmdbEntity::new(entity_type, &body.name);
    if !body.namespace.is_empty() {
        entity = entity.with_namespace(&body.namespace);
    }
    if !body.group.is_empty() {
        entity = entity.with_group(&body.group);
    }
    for (k, v) in &body.labels {
        entity = entity.with_label(k, v);
    }
    entity.attributes = body.attributes.clone();

    match plugin.register_entity(entity).await {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "data": { "id": id },
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// DELETE /v3/console/cmdb/entities - Delete a CMDB entity by id
#[delete("/entities")]
async fn delete_entity(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let id = query.get("id").cloned().unwrap_or_default();
    if id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "id parameter is required",
        }));
    }

    match plugin.delete_entity(&id).await {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": "Entity deleted",
        })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "code": 404,
            "message": "Entity not found",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// GET /v3/console/cmdb/labels - Get label mappings
#[get("/labels")]
async fn list_label_mappings(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match plugin.get_label_mappings().await {
        Ok(mappings) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "data": mappings,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// POST /v3/console/cmdb/labels - Add a label mapping
#[post("/labels")]
async fn add_label_mapping(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    body: web::Json<LabelMapping>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match plugin.add_label_mapping(body.into_inner()).await {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "data": { "id": id },
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// DELETE /v3/console/cmdb/labels - Remove a label mapping
#[delete("/labels")]
async fn remove_label_mapping(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let id = query.get("id").cloned().unwrap_or_default();
    if id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": 400,
            "message": "id parameter is required",
        }));
    }

    match plugin.remove_label_mapping(&id).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": "Label mapping removed",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// POST /v3/console/cmdb/sync - Trigger full CMDB sync
#[post("/sync")]
async fn trigger_sync(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match plugin.full_sync().await {
        Ok(result) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "data": result,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// GET /v3/console/cmdb/stats - Get CMDB statistics
#[get("/stats")]
async fn get_stats(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let stats = plugin.get_stats().await;
    HttpResponse::Ok().json(serde_json::json!({
        "code": 0,
        "data": stats,
    }))
}

/// GET /v3/console/cmdb/entities/search - Search entities by labels
#[get("/entities/search")]
async fn search_by_labels(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    // All query params (except reserved ones) are treated as label filters
    let labels: HashMap<String, String> = query
        .into_inner()
        .into_iter()
        .filter(|(k, _)| k != "pageNo" && k != "pageSize")
        .collect();

    match plugin.search_by_labels(&labels).await {
        Ok(entities) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "data": entities,
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// PUT /v3/console/cmdb/entities - Update a CMDB entity
#[put("/entities")]
async fn update_entity(
    req: HttpRequest,
    data: web::Data<AppState>,
    plugin: web::Data<Arc<dyn CmdbPlugin>>,
    body: web::Json<CmdbEntity>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Write)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match plugin.update_entity(body.into_inner()).await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "code": 0,
            "message": "Entity updated",
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "code": 500,
            "message": e.to_string(),
        })),
    }
}

/// Create the CMDB console routes
pub fn routes() -> Scope {
    web::scope("/cmdb")
        .service(list_entities)
        .service(search_by_labels)
        .service(create_entity)
        .service(update_entity)
        .service(delete_entity)
        .service(list_label_mappings)
        .service(add_label_mapping)
        .service(remove_label_mapping)
        .service(trigger_sync)
        .service(get_stats)
}
