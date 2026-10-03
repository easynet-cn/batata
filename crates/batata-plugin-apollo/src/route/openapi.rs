use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use crate::persistence::shared::StoredRelease;
use crate::persistence::traits::{ApolloPersistenceService, ReleasePersistence, NamespacePersistence};
use crate::service::{AppService, NamespaceService, ItemService, ItemSetService, ReleaseService, ClusterService, InstanceService, AccessKeyService, GrayReleaseRuleService, AppNamespaceService, CommitService, ConsumerService, ConsumerTokenService, AuditService, FavoriteService, SearchService, UserTokenService};
use crate::api::dto::{AppDTO, NamespaceDTO, ItemDTO, ErrorResponse, ClusterDTO, GrayReleaseRuleDTO, AppNamespaceDTO, CommitDTO, ItemChangeSets, ConfigImportDTO, ConfigExportDTO};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `OpenCreateAppDTO` entity.
pub struct OpenCreateAppDTO {
    /// The `app` field.
    pub app: AppDTO,
    #[serde(default)]
    /// The `admins` field.
    pub admins: Vec<String>,
    #[serde(default)]
    /// The `assign_app_role_to_self` field.
    pub assign_app_role_to_self: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `OpenAppNamespaceDTO` entity.
pub struct OpenAppNamespaceDTO {
    /// The `app_id` field.
    pub app_id: String,
    /// The `name` field.
    pub name: String,
    #[serde(default = "default_format")]
    /// The `format` field.
    pub format: String,
    #[serde(default = "default_is_public")]
    /// The `is_public` field.
    pub is_public: bool,
    #[serde(default)]
    /// The `comment` field.
    pub comment: String,
    /// The `data_change_created_by` field.
    pub data_change_created_by: String,
}

fn default_format() -> String {
    "properties".to_string()
}

fn default_is_public() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `OpenItemDTO` entity.
pub struct OpenItemDTO {
    /// The `key` field.
    pub key: String,
    /// The `value` field.
    pub value: String,
    #[serde(default)]
    /// The `comment` field.
    pub comment: String,
    #[serde(default = "default_item_type")]
    /// The `type` field.
    pub r#type: i32,
    #[serde(default)]
    /// The `data_change_created_by` field.
    pub data_change_created_by: Option<String>,
    #[serde(default)]
    /// The `data_change_last_modified_by` field.
    pub data_change_last_modified_by: Option<String>,
}

fn default_item_type() -> i32 {
    0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `NamespaceReleaseDTO` entity.
pub struct NamespaceReleaseDTO {
    /// The `release_title` field.
    pub release_title: String,
    #[serde(default)]
    /// The `release_comment` field.
    pub release_comment: String,
    /// The `released_by` field.
    pub released_by: String,
    #[serde(default)]
    /// The `is_emergency_publish` field.
    pub is_emergency_publish: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `NamespaceGrayDelReleaseDTO` entity.
pub struct NamespaceGrayDelReleaseDTO {
    /// The `release_title` field.
    pub release_title: String,
    #[serde(default)]
    /// The `release_comment` field.
    pub release_comment: String,
    /// The `released_by` field.
    pub released_by: String,
    #[serde(default)]
    /// The `is_emergency_publish` field.
    pub is_emergency_publish: bool,
    #[serde(default)]
    /// The `create_items` field.
    pub create_items: Vec<OpenItemDTO>,
    #[serde(default)]
    /// The `update_items` field.
    pub update_items: Vec<OpenItemDTO>,
    #[serde(default)]
    /// The `delete_items` field.
    pub delete_items: Vec<OpenItemDTO>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `OpenNamespace` entity.
pub struct OpenNamespace {
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `format` field.
    pub format: String,
    /// The `comment` field.
    pub comment: String,
    /// The `is_public` field.
    pub is_public: bool,
    /// The `items` field.
    pub items: Vec<ItemDTO>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `OpenEnvCluster` entity.
pub struct OpenEnvCluster {
    /// The `env` field.
    pub env: String,
    /// The `clusters` field.
    pub clusters: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Represents the `OpenRelease` entity.
pub struct OpenRelease {
    /// The `id` field.
    pub id: i64,
    /// The `release_id` field.
    pub release_id: i64,
    /// The `app_id` field.
    pub app_id: String,
    /// The `cluster_name` field.
    pub cluster_name: String,
    /// The `namespace_name` field.
    pub namespace_name: String,
    /// The `name` field.
    pub name: String,
    /// The `configurations` field.
    pub configurations: Value,
    /// The `comment` field.
    pub comment: String,
}

async fn create_app(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<OpenCreateAppDTO>,
) -> impl Responder {
    let req = body.into_inner();
    let service = AppService::new(data.get_ref().clone());
    match service.create(req.app).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_apps(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let service = AppService::new(data.get_ref().clone());
    let ids: Option<Vec<String>> = query
        .get("appIds")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').map(|id| id.trim().to_string()).collect());
    let result = match ids {
        Some(ids) if !ids.is_empty() => service.get_by_ids(&ids).await,
        _ => service.list().await,
    };
    match result {
        Ok(apps) => HttpResponse::Ok().json(apps),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_app(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
) -> impl Responder {
    let service = AppService::new(data.get_ref().clone());
    match service.get(app_id.as_str()).await {
        Ok(Some(app)) => HttpResponse::Ok().json(app),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("App not found: {}", app_id),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_env_clusters(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let app_service = AppService::new(data.get_ref().clone());
    match app_service.get(&app_id).await {
        Ok(None) => return HttpResponse::NotFound().finish(),
        Ok(Some(_)) => {}
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
    let service = ClusterService::new(data.get_ref().clone());
    match service.list(&app_id).await {
        Ok(clusters) => {
            let cluster_names: Vec<String> = clusters.into_iter().map(|c| c.name).collect();
            let result = vec![OpenEnvCluster {
                env: "DEV".to_string(),
                clusters: cluster_names,
            }];
            HttpResponse::Ok().json(result)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_app_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
    body: web::Json<OpenAppNamespaceDTO>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let req = body.into_inner();

    let app_ns_service = AppNamespaceService::new(data.get_ref().clone());
    let app_ns_dto = AppNamespaceDTO {
        id: None,
        name: req.name.clone(),
        app_id: app_id.clone(),
        format: req.format.clone(),
        is_public: req.is_public,
        comment: req.comment.clone(),
        data_change_created_by: Some(req.data_change_created_by.clone()),
        data_change_created_time: None,
    };

    match app_ns_service.create(app_ns_dto).await {
        Ok(created) => {
            let ns_dto = NamespaceDTO {
                app_id: app_id.clone(),
                cluster_name: "default".to_string(),
                namespace_name: req.name.clone(),
                format: Some(req.format.clone()),
                is_public: Some(req.is_public),
                comment: if req.comment.is_empty() { None } else { Some(req.comment.clone()) },
                data_change_created_by: Some(req.data_change_created_by.clone()),
                data_change_last_modified_by: None,
                data_change_created_time: None,
                data_change_last_time: None,
            };
            let ns_service = NamespaceService::new(data.get_ref().clone());
            let _ = ns_service.create(&app_id, "default", ns_dto).await;
            HttpResponse::Ok().json(created)
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_app_namespaces_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.list_by_app(&app_id).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    body: web::Json<OpenAppNamespaceDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name) = path.into_inner();
    let req = body.into_inner();
    let ns_dto = NamespaceDTO {
        app_id: app_id.clone(),
        cluster_name: cluster_name.clone(),
        namespace_name: req.name,
        format: Some(req.format),
        is_public: Some(req.is_public),
        comment: if req.comment.is_empty() { None } else { Some(req.comment) },
        data_change_created_by: Some(req.data_change_created_by),
        data_change_last_modified_by: None,
        data_change_created_time: None,
        data_change_last_time: None,
    };
    let service = NamespaceService::new(data.get_ref().clone());
    match service.create(&app_id, &cluster_name, ns_dto).await {
        Ok(ns) => HttpResponse::Ok().json(OpenNamespace {
            app_id: ns.app_id,
            cluster_name: ns.cluster_name,
            namespace_name: ns.namespace_name,
            format: ns.format.unwrap_or_else(|| "properties".to_string()),
            comment: ns.comment.unwrap_or_default(),
            is_public: ns.is_public.unwrap_or(false),
            items: vec![],
        }),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_namespaces(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name) = path.into_inner();
    let ns_service = NamespaceService::new(data.get_ref().clone());
    let item_service = ItemService::new(data.get_ref().clone());
    match ns_service.list(&app_id, &cluster_name).await {
        Ok(namespaces) => {
            let mut result: Vec<OpenNamespace> = Vec::new();
            for ns in namespaces {
                let items = item_service
                    .list(&app_id, &cluster_name, &ns.namespace_name)
                    .await
                    .unwrap_or_default();
                result.push(OpenNamespace {
                    app_id: ns.app_id,
                    cluster_name: ns.cluster_name,
                    namespace_name: ns.namespace_name,
                    format: ns.format.unwrap_or_else(|| "properties".to_string()),
                    comment: ns.comment.unwrap_or_default(),
                    is_public: ns.is_public.unwrap_or(false),
                    items,
                });
            }
            HttpResponse::Ok().json(result)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let ns_service = NamespaceService::new(data.get_ref().clone());
    let item_service = ItemService::new(data.get_ref().clone());
    match ns_service.get(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(ns)) => {
            let items = item_service
                .list(&app_id, &cluster_name, &namespace_name)
                .await
                .unwrap_or_default();
            HttpResponse::Ok().json(OpenNamespace {
                app_id: ns.app_id,
                cluster_name: ns.cluster_name,
                namespace_name: ns.namespace_name,
                format: ns.format.unwrap_or_else(|| "properties".to_string()),
                comment: ns.comment.unwrap_or_default(),
                is_public: ns.is_public.unwrap_or(false),
                items,
            })
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!(
                "Namespace not found: {}/{}/{}",
                app_id, cluster_name, namespace_name
            ),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, key) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.get_by_key(&app_id, &cluster_name, &namespace_name, &key).await {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<OpenItemDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let req = body.into_inner();
    let item_dto = ItemDTO {
        id: None,
        key: req.key,
        value: req.value,
        r#type: Some(req.r#type),
        comment: if req.comment.is_empty() { None } else { Some(req.comment) },
        line_num: None,
        data_change_created_by: req.data_change_created_by,
        data_change_last_modified_by: req.data_change_last_modified_by,
        data_change_created_time: None,
        data_change_last_time: None,
    };
    let service = ItemService::new(data.get_ref().clone());
    match service.create(&app_id, &cluster_name, &namespace_name, item_dto).await {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn update_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    body: web::Json<OpenItemDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, _key) = path.into_inner();
    let req = body.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.get_by_key(&app_id, &cluster_name, &namespace_name, &req.key).await {
        Ok(Some(item)) => {
            let item_id = item.id.unwrap_or(0);
            let item_dto = ItemDTO {
                id: Some(item_id),
                key: req.key,
                value: req.value,
                r#type: Some(req.r#type),
                comment: if req.comment.is_empty() { None } else { Some(req.comment) },
                line_num: None,
                data_change_created_by: None,
                data_change_last_modified_by: req
                    .data_change_last_modified_by
                    .or(req.data_change_created_by),
                data_change_created_time: None,
                data_change_last_time: None,
            };
            match service
                .update(&app_id, &cluster_name, &namespace_name, item_id, item_dto)
                .await
            {
                Ok(updated) => HttpResponse::Ok().json(updated),
                Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
                    status: 400,
                    message: e.to_string(),
                }),
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Item not found: {}", req.key),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn delete_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    operator: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, key) = path.into_inner();
    let op = operator
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");
    let service = ItemService::new(data.get_ref().clone());
    match service.get_by_key(&app_id, &cluster_name, &namespace_name, &key).await {
        Ok(Some(item)) => {
            let item_id = item.id.unwrap_or(0);
            match service.delete(item_id, op).await {
                Ok(_) => HttpResponse::Ok().finish(),
                Err(e) => HttpResponse::NotFound().json(ErrorResponse {
                    status: 404,
                    message: e.to_string(),
                }),
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Item not found: {}", key),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn find_items_by_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(items) => {
            let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let total = items.len();
            let start = page * size;
            let end = (start + size).min(total);
            let paged: Vec<_> = if start < total {
                items[start..end].to_vec()
            } else {
                Vec::new()
            };
            HttpResponse::Ok().json(serde_json::json!({
                "content": paged,
                "page": page,
                "size": size,
                "total": total,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn publish_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<NamespaceReleaseDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let req = body.into_inner();
    let service = ReleaseService::new(data.get_ref().clone());
    match service
        .publish(
            &app_id,
            &cluster_name,
            &namespace_name,
            &req.release_title,
            if req.release_comment.is_empty() { None } else { Some(req.release_comment) },
            &req.released_by,
            req.is_emergency_publish,
        )
        .await
    {
        Ok(release) => {
            let configs: Value = serde_json::from_str(&release.configurations.unwrap_or_default())
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: release.id.unwrap_or(0),
                release_id: release.id.unwrap_or(0),
                app_id: release.app_id,
                cluster_name: release.cluster_name,
                namespace_name: release.namespace_name,
                name: release.name,
                configurations: configs,
                comment: release.comment.unwrap_or_default(),
            })
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_latest_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ReleaseService::new(data.get_ref().clone());
    match service.get_latest_active(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(release)) => {
            let configs: Value = serde_json::from_str(&release.configurations.unwrap_or_default())
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: release.id.unwrap_or(0),
                release_id: release.id.unwrap_or(0),
                app_id: release.app_id,
                cluster_name: release.cluster_name,
                namespace_name: release.namespace_name,
                name: release.name,
                configurations: configs,
                comment: release.comment.unwrap_or_default(),
            })
        }
        Ok(None) => HttpResponse::Ok().json(serde_json::Value::Null),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn find_active_releases(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(10);
    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_active_releases(&app_id, &cluster_name, &namespace_name, page, size).await {
        Ok((releases, total)) => {
            let open_releases: Vec<OpenRelease> = releases
                .into_iter()
                .map(|r| {
                    let configs: Value = serde_json::from_str(&r.configurations.unwrap_or_default())
                        .unwrap_or_else(|_| Value::Object(Default::default()));
                    OpenRelease {
                        id: r.id.unwrap_or(0),
                        release_id: r.id.unwrap_or(0),
                        app_id: r.app_id,
                        cluster_name: r.cluster_name,
                        namespace_name: r.namespace_name,
                        name: r.name,
                        configurations: configs,
                        comment: r.comment.unwrap_or_default(),
                    }
                })
                .collect();
            HttpResponse::Ok().json(serde_json::json!({
                "content": open_releases,
                "page": page,
                "size": size,
                "total": total,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn rollback_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, i64)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, release_id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ReleaseService::new(data.get_ref().clone());
    match service.rollback(&app_id, &cluster_name, &namespace_name, release_id, operator).await {
        Ok(release) => {
            let configs: Value = serde_json::from_str(&release.configurations.unwrap_or_default())
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: release.id.unwrap_or(0),
                release_id: release.id.unwrap_or(0),
                app_id: release.app_id,
                cluster_name: release.cluster_name,
                namespace_name: release.namespace_name,
                name: release.name,
                configurations: configs,
                comment: release.comment.unwrap_or_default(),
            })
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_cluster(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name) = path.into_inner();
    let service = ClusterService::new(data.get_ref().clone());
    match service.get(&app_id, &cluster_name).await {
        Ok(Some(cluster)) => HttpResponse::Ok().json(cluster),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Cluster not found: {}/{}", app_id, cluster_name),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_cluster(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
    body: web::Json<ClusterDTO>,
) -> impl Responder {
    let (_env, app_id) = path.into_inner();
    let service = ClusterService::new(data.get_ref().clone());
    match service.create(&app_id, body.into_inner()).await {
        Ok(cluster) => HttpResponse::Ok().json(cluster),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_envs(_data: web::Data<Arc<dyn ApolloPersistenceService>>) -> impl Responder {
    let envs = vec!["DEV", "FAT", "UAT", "PRO"];
    HttpResponse::Ok().json(envs)
}

async fn list_instances(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name) = path.into_inner();
    let service = InstanceService::new(data.get_ref().clone());
    match service.list_by_app_cluster(&app_id, &cluster_name).await {
        Ok(instances) => HttpResponse::Ok().json(instances),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_access_key(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
    body: web::Json<Value>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let operator = body.get("createdBy").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.create(&app_id, operator).await {
        Ok(key) => HttpResponse::Ok().json(key),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_access_keys(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.list_by_app(&app_id).await {
        Ok(keys) => HttpResponse::Ok().json(keys),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn delete_access_key(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, i64)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.delete(&app_id, id, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

/// Frontend uses `/apps/{app_id}/envs/{env}/accesskeys` path.
async fn create_access_key_with_env(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
    body: web::Json<Value>,
) -> impl Responder {
    let (app_id, _env) = path.into_inner();
    let operator = body.get("createdBy").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.create(&app_id, operator).await {
        Ok(key) => HttpResponse::Ok().json(key),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_access_keys_with_env(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (app_id, _env) = path.into_inner();
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.list_by_app(&app_id).await {
        Ok(keys) => HttpResponse::Ok().json(keys),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn delete_access_key_with_env(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, i64)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, _env, id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.delete(&app_id, id, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn get_release_history(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(50);
    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_release_history(&app_id, &cluster_name, &namespace_name, page, size).await {
        Ok((list, total)) => {
            let resp = serde_json::json!({
                "content": list,
                "total": total,
                "page": page,
                "size": size,
            });
            HttpResponse::Ok().json(resp)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// Frontend uses `/apps/{app_id}/envs/{env}/.../releases/histories` path order.
async fn get_release_history_frontend(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, _env, cluster_name, namespace_name) = path.into_inner();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(50);
    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_release_history(&app_id, &cluster_name, &namespace_name, page, size).await {
        Ok((list, total)) => {
            let resp = serde_json::json!({
                "content": list,
                "total": total,
                "page": page,
                "size": size,
            });
            HttpResponse::Ok().json(resp)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn list_branches(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.list_by_namespace(&app_id, &cluster_name, &namespace_name).await {
        Ok(rules) => HttpResponse::Ok().json(rules),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_branch(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");

    // Upstream createBranch: child Cluster (ParentClusterId>0, timestamp-named)
    // + child Namespace. Idempotent — returns the existing branch.
    let service = crate::service::namespace_branch_service::NamespaceBranchService::new(
        data.get_ref().clone(),
    );
    match service
        .create_branch(&app_id, &cluster_name, &namespace_name, operator)
        .await
    {
        Ok(branch_ns) => HttpResponse::Ok().json(branch_ns),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn update_branch_rule(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    body: web::Json<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());

    let rules = body.get("rules").and_then(|v| v.as_str()).map(|s| s.to_string());
    let release_id = body.get("releaseId").and_then(|v| v.as_i64()).unwrap_or(0);
    let branch_status = body.get("branchStatus").and_then(|v| v.as_i64()).map(|v| v as i32);
    let operator = body.get("dataChangeLastModifiedBy").and_then(|v| v.as_str()).unwrap_or("admin");

    let dto = GrayReleaseRuleDTO {
        id: None,
        app_id: app_id.clone(),
        cluster_name: cluster_name.clone(),
        namespace_name: namespace_name.clone(),
        branch_name: branch_name.clone(),
        rules,
        release_id,
        branch_status,
        priority: None,
        data_change_created_by: None,
        data_change_last_modified_by: Some(operator.to_string()),
        data_change_created_time: None,
    };

    match service.update(&app_id, &cluster_name, &namespace_name, &branch_name, dto).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok"})),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn delete_branch(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, _branch_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");

    // Upstream deleteBranch: tombstone rule + cascade soft-delete of the
    // child cluster/namespace/items; child releases are retained.
    let service = crate::service::namespace_branch_service::NamespaceBranchService::new(
        data.get_ref().clone(),
    );
    match service
        .delete_branch(&app_id, &cluster_name, &namespace_name, operator, false)
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn merge_branch(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    body: web::Json<NamespaceGrayDelReleaseDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let req = body.into_inner();

    let convert = |items: Vec<OpenItemDTO>| -> Vec<ItemDTO> {
        items.into_iter().map(|item| ItemDTO {
            id: None,
            key: item.key,
            value: item.value,
            r#type: Some(item.r#type),
            comment: if item.comment.is_empty() { None } else { Some(item.comment) },
            line_num: None,
            data_change_created_by: item.data_change_created_by,
            data_change_last_modified_by: item.data_change_last_modified_by,
            data_change_created_time: None,
            data_change_last_time: None,
        }).collect()
    };

    let change_sets = ItemChangeSets {
        create_items: convert(req.create_items),
        update_items: convert(req.update_items),
        delete_items: convert(req.delete_items),
    };

    let release_comment = if req.release_comment.is_empty() { None } else { Some(req.release_comment) };
    let service = ReleaseService::new(data.get_ref().clone());
    match service.merge_branch_and_release(
        &app_id,
        &cluster_name,
        &namespace_name,
        &branch_name,
        &req.release_title,
        release_comment,
        &req.released_by,
        req.is_emergency_publish,
        change_sets,
    ).await {
        Ok(release) => {
            let configs: Value = serde_json::from_str(&release.configurations.unwrap_or_default())
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: release.id.unwrap_or(0),
                release_id: release.id.unwrap_or(0),
                app_id: release.app_id,
                cluster_name: release.cluster_name,
                namespace_name: release.namespace_name,
                name: release.name,
                configurations: configs,
                comment: release.comment.unwrap_or_default(),
            })
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_branch_rule(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.get(&app_id, &cluster_name, &namespace_name, &branch_name).await {
        Ok(Some(rule)) => HttpResponse::Ok().json(rule),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Branch not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn list_commits_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let service = CommitService::new(data.get_ref().clone());
    match service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(commits) => HttpResponse::Ok().json(commits),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_commit_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<CommitDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let mut dto = body.into_inner();
    dto.app_id = app_id;
    dto.cluster_name = cluster_name;
    dto.namespace_name = namespace_name;
    
    let service = CommitService::new(data.get_ref().clone());
    match service.create(dto).await {
        Ok(commit) => HttpResponse::Ok().json(commit),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_commit_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    commit_id: web::Path<i64>,
) -> impl Responder {
    let id = commit_id.into_inner();
    let service = CommitService::new(data.get_ref().clone());
    match service.get(id).await {
        Ok(Some(commit)) => HttpResponse::Ok().json(commit),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Commit not found: {}", id),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn update_app(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
    body: web::Json<AppDTO>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let service = AppService::new(data.get_ref().clone());
    let mut dto = body.into_inner();
    let created_by = dto.data_change_created_by.clone();
    dto.data_change_last_modified_by = Some(
        dto.data_change_last_modified_by
            .or(created_by)
            .unwrap_or_else(|| "admin".to_string()),
    );
    match service.update(&app_id, dto).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_app(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
    query: web::Query<Value>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AppService::new(data.get_ref().clone());
    match service.delete(&app_id, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_app_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (app_id, namespace_name) = path.into_inner();
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.get(&app_id, &namespace_name).await {
        Ok(Some(ns)) => HttpResponse::Ok().json(ns),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("AppNamespace not found: {}/{}", app_id, namespace_name),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn delete_app_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, namespace_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.delete(&app_id, &namespace_name, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_cluster(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ClusterService::new(data.get_ref().clone());
    match service.delete(&app_id, &cluster_name, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = NamespaceService::new(data.get_ref().clone());
    match service.delete(&app_id, &cluster_name, &namespace_name, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_release_by_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, i64)>,
) -> impl Responder {
    let (_env, release_id) = path.into_inner();
    let service = ReleaseService::new(data.get_ref().clone());
    match service.get_by_id(release_id).await {
        Ok(Some(release)) => {
            let configs: Value = serde_json::from_str(&release.configurations.unwrap_or_default())
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: release.id.unwrap_or(0),
                release_id: release.id.unwrap_or(0),
                app_id: release.app_id,
                cluster_name: release.cluster_name,
                namespace_name: release.namespace_name,
                name: release.name,
                configurations: configs,
                comment: release.comment.unwrap_or_default(),
            })
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Release not found: {}", release_id),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn compare_releases(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let base = query.get("baseReleaseId").and_then(|v| v.as_i64()).unwrap_or(0);
    let to_compare = query.get("toCompareReleaseId").and_then(|v| v.as_i64()).unwrap_or(0);
    let service = ReleaseService::new(data.get_ref().clone());
    match service.compare(base, to_compare).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn rollback_release_by_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, i64)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, release_id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let to_release_id = query.get("toReleaseId").and_then(|v| v.as_i64());
    let service = ReleaseService::new(data.get_ref().clone());
    match service.rollback_by_id(release_id, to_release_id, operator).await {
        Ok(release) => {
            let configs: Value = serde_json::from_str(&release.configurations.unwrap_or_default())
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: release.id.unwrap_or(0),
                release_id: release.id.unwrap_or(0),
                app_id: release.app_id,
                cluster_name: release.cluster_name,
                namespace_name: release.namespace_name,
                name: release.name,
                configurations: configs,
                comment: release.comment.unwrap_or_default(),
            })
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn create_gray_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    body: web::Json<NamespaceGrayDelReleaseDTO>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let req = body.into_inner();

    let gray_service = GrayReleaseRuleService::new(data.get_ref().clone());
    match gray_service.get(&app_id, &cluster_name, &namespace_name, &branch_name).await {
        Ok(Some(_)) => {}
        Ok(None) => return HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Branch not found: {}", branch_name),
        }),
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }

    // Upstream publishBranchNamespace: base = the PARENT cluster's latest
    // active release configurations, overlaid by branch-namespace item edits.
    let release_service = ReleaseService::new(data.get_ref().clone());
    let mut configurations: HashMap<String, String> = release_service
        .get_configurations(&app_id, &cluster_name, &namespace_name)
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    let item_service = ItemService::new(data.get_ref().clone());
    let branch_items = item_service
        .list(&app_id, &branch_name, &namespace_name)
        .await
        .unwrap_or_default();
    for item in branch_items {
        configurations.insert(item.key, item.value);
    }

    let configurations_json = match serde_json::to_string(&configurations) {
        Ok(s) => s,
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    };

    let now = Utc::now().timestamp_millis();
    let release_id = now;
    let release_key = format!(
        "{}+{}+{}+{}+gray+{}",
        app_id, cluster_name, namespace_name, release_id, branch_name
    );

    let stored = StoredRelease {
        id: 0,
        release_key,
        name: req.release_title,
        comment: if req.release_comment.is_empty() {
            None
        } else {
            Some(req.release_comment)
        },
        app_id: app_id.clone(),
        cluster_name: branch_name.clone(),
        namespace_name: namespace_name.clone(),
        configurations: configurations_json,
        release_id: Some(release_id),
        is_abandoned: false,
        is_deleted: false,
        deleted_at: 0,
        data_change_created_by: req.released_by.clone(),
        data_change_created_time: now,
        data_change_last_modified_by: None,
        data_change_last_time: None,
    };

    match <dyn ReleasePersistence>::create(data.get_ref(), stored).await {
        Ok(created) => {
            // Upstream: gray publish notifies under the PARENT cluster key so
            // clients watching the master namespace refresh.
            let sender = crate::service::release_message_service::ReleaseMessageService::new(
                data.get_ref().clone(),
            );
            match sender.send_message(&app_id, &cluster_name, &namespace_name).await {
                Ok(stored) => {
                    crate::service::notification_hub::hub().notify(&stored.message);
                }
                Err(e) => tracing::error!("failed to persist gray release message: {}", e),
            }
            let _ = ReleaseService::new(data.get_ref().clone())
                .record_history(&app_id, &cluster_name, &namespace_name, &branch_name, created.id, 0, 2, &req.released_by, "")
                .await;
            let configs: Value = serde_json::from_str(&created.configurations)
                .unwrap_or_else(|_| Value::Object(Default::default()));
            HttpResponse::Ok().json(OpenRelease {
                id: created.id,
                release_id: created.id,
                app_id: created.app_id,
                cluster_name: created.cluster_name,
                namespace_name: created.namespace_name,
                name: created.name,
                configurations: configs,
                comment: created.comment.unwrap_or_default(),
            })
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// PMISC-014: GET /openapi/v1/consumers - consumer list
async fn list_consumers_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    let service = ConsumerService::new(data.get_ref().clone());
    match service.list().await {
        Ok(consumers) => HttpResponse::Ok().json(consumers),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// PMISC-014: GET /openapi/v1/consumers/{app_id} - single consumer lookup
async fn get_consumer_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let service = ConsumerService::new(data.get_ref().clone());
    match service.get_by_app(&app_id).await {
        Ok(Some(consumer)) => HttpResponse::Ok().json(consumer),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Consumer not found: {}", app_id),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// PMISC-015: GET /openapi/v1/configs/{app_id}/{cluster}/{namespace}/export - config export
async fn export_configs_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let item_service = ItemService::new(data.get_ref().clone());
    match item_service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(items) => {
            let export = ConfigExportDTO {
                app_id,
                cluster_name,
                namespace_name,
                items,
            };
            HttpResponse::Ok().json(export)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// PMISC-015: POST /openapi/v1/configs/import - config import
async fn import_configs_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<ConfigImportDTO>,
) -> impl Responder {
    let dto = body.into_inner();
    let item_set_service = ItemSetService::new(data.get_ref().clone());
    let change_sets = ItemChangeSets {
        create_items: dto.items,
        update_items: vec![],
        delete_items: vec![],
    };
    match item_set_service
        .update_set(&dto.app_id, &dto.cluster_name, &dto.namespace_name, change_sets)
        .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// PMISC-018: GET /openapi/v1/apollo/audit - audit log query
async fn list_audit_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20);
    let service = AuditService::new(data.get_ref().clone());
    match service.list(page, size).await {
        Ok((audits, total)) => {
            HttpResponse::Ok().json(serde_json::json!({
                "content": audits,
                "total": total,
                "page": page,
                "size": size,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// PMISC-018: GET /openapi/v1/apollo/audit/by-entity - audit log by entity
async fn list_audit_by_entity_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let entity_name = query
        .get("entityName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let entity_id = query
        .get("entityId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let service = AuditService::new(data.get_ref().clone());
    match service.list_by_entity(entity_name, entity_id).await {
        Ok(audits) => HttpResponse::Ok().json(audits),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// ===== Audit log additional endpoints =====
async fn openapi_audit_properties(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([
        "APP", "CLUSTER", "NAMESPACE", "ITEM", "RELEASE", "COMMIT"
    ]))
}

async fn openapi_audit_logs(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20);
    let service = AuditService::new(data.get_ref().clone());
    match service.list(page, size).await {
        Ok((audits, total)) => HttpResponse::Ok().json(serde_json::json!({
            "content": audits, "total": total, "page": page, "size": size,
        })),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_audit_logs_op_name(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20);
    let service = AuditService::new(data.get_ref().clone());
    match service.list(page, size).await {
        Ok((audits, total)) => HttpResponse::Ok().json(serde_json::json!({
            "content": audits, "total": total, "page": page, "size": size,
        })),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_audit_trace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _query: web::Query<Value>,
) -> impl Responder {
    let service = AuditService::new(data.get_ref().clone());
    match service.list(0, 100).await {
        Ok((audits, _)) => HttpResponse::Ok().json(audits),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_audit_logs_field(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20);
    let service = AuditService::new(data.get_ref().clone());
    match service.list(page, size).await {
        Ok((audits, total)) => HttpResponse::Ok().json(serde_json::json!({
            "content": audits, "total": total, "page": page, "size": size,
        })),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_audit_logs_search(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20);
    let service = AuditService::new(data.get_ref().clone());
    match service.list(page, size).await {
        Ok((audits, total)) => HttpResponse::Ok().json(serde_json::json!({
            "content": audits, "total": total, "page": page, "size": size,
        })),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

// ===== Consumer extension endpoints =====
async fn openapi_get_consumer_by_app(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let app_id = query.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let service = ConsumerService::new(data.get_ref().clone());
    match service.get_by_app(app_id).await {
        Ok(Some(c)) => HttpResponse::Ok().json(c),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: format!("Consumer not found: {}", app_id) }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_consumer_tokens_by_app(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let app_id = query.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let consumer_service = ConsumerService::new(data.get_ref().clone());
    match consumer_service.get_by_app(app_id).await {
        Ok(Some(c)) => {
            let token_service = ConsumerTokenService::new(data.get_ref().clone());
            match token_service.list_by_consumer(c.id.unwrap_or(0)).await {
                Ok(tokens) => HttpResponse::Ok().json(tokens),
                Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
            }
        }
        Ok(None) => HttpResponse::Ok().json(serde_json::json!([])),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_assign_consumer_role(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<String>,
    _body: web::Json<Value>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

// ===== User token endpoints =====
async fn openapi_list_user_tokens(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    let service = UserTokenService::new(data.get_ref().clone());
    match service.list("apollo").await {
        Ok(tokens) => HttpResponse::Ok().json(tokens),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_create_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<Value>,
) -> impl Responder {
    let description = body.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let service = UserTokenService::new(data.get_ref().clone());
    match service.create("apollo", description, "apollo", None).await {
        Ok((token, model)) => HttpResponse::Ok().json(serde_json::json!({
            "token": token,
            "model": model,
        })),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

async fn openapi_revoke_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    let service = UserTokenService::new(data.get_ref().clone());
    match service.revoke(id).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

async fn openapi_rotate_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    let service = UserTokenService::new(data.get_ref().clone());
    match service.revoke(id).await {
        Ok(_) => match service.create("apollo", "rotated", "apollo", None).await {
            Ok((token, model)) => HttpResponse::Ok().json(serde_json::json!({
                "token": token,
                "model": model,
            })),
            Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
        },
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

async fn openapi_delete_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    let service = UserTokenService::new(data.get_ref().clone());
    match service.revoke(id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

async fn openapi_user_token_capabilities(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "tokenSupported": true,
        "tokenRotationSupported": true,
    }))
}

async fn openapi_list_admin_user_tokens(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    let service = UserTokenService::new(data.get_ref().clone());
    match service.list("apollo").await {
        Ok(tokens) => HttpResponse::Ok().json(tokens),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

async fn openapi_create_admin_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<Value>,
) -> impl Responder {
    let user_id = body.get("userId").and_then(|v| v.as_str()).unwrap_or("apollo");
    let description = body.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let service = UserTokenService::new(data.get_ref().clone());
    match service.create(user_id, description, "apollo", None).await {
        Ok((token, model)) => HttpResponse::Ok().json(serde_json::json!({
            "token": token,
            "model": model,
        })),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

async fn openapi_revoke_admin_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, i64)>,
) -> impl Responder {
    let (_username, id) = path.into_inner();
    let service = UserTokenService::new(data.get_ref().clone());
    match service.revoke(id).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

async fn openapi_delete_admin_user_token(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, i64)>,
) -> impl Responder {
    let (_username, id) = path.into_inner();
    let service = UserTokenService::new(data.get_ref().clone());
    match service.revoke(id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

// ===== Server config endpoints =====
async fn openapi_server_portal_db_config(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({}))
}

async fn openapi_server_env_config_db_config(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<String>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({}))
}

async fn openapi_server_portal_db_config_find_all(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "content": [], "total": 0 }))
}

async fn openapi_server_env_config_db_config_find_all(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<String>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "content": [], "total": 0 }))
}

// PMISC-019: GET /openapi/v1/favorites - favorites list
async fn list_favorites_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let user_id = query
        .get("userId")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");
    let service = FavoriteService::new(data.get_ref().clone());
    match service.list_by_user(user_id).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// PMISC-020: GET /openapi/v1/global-search/item-info/by-key-or-value - global search
async fn search_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let app_id = query.get("appId").and_then(|v| v.as_str());
    let cluster_name = query.get("clusterName").and_then(|v| v.as_str());
    let key = query.get("key").and_then(|v| v.as_str());
    let value = query.get("value").and_then(|v| v.as_str());

    let service = SearchService::new(data.get_ref().clone());
    let result = match (app_id, cluster_name) {
        (Some(app), Some(cluster)) => service.search_items(app, cluster, key, value).await,
        _ => service.search_across_apps(key, value).await,
    };

    match result {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// PMISC-006: PUT /openapi/v1/apps/{app_id}/envs/{env}/accesskeys/{id}/activation - enable access key
async fn enable_access_key_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, i64)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, _env, id) = path.into_inner();
    let operator = query
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.enable(&app_id, id, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// PMISC-006: PUT /openapi/v1/apps/{app_id}/envs/{env}/accesskeys/{id}/deactivation - disable access key
async fn disable_access_key_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, i64)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, _env, id) = path.into_inner();
    let operator = query
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.disable(&app_id, id, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// PORT-026: GET /openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/missing-namespaces
// Find AppNamespaces defined for the app but not yet created as Namespaces in the given cluster
async fn find_missing_namespaces_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (app_id, _env, cluster_name) = path.into_inner();
    let app_ns_service = AppNamespaceService::new(data.get_ref().clone());
    let ns_service = NamespaceService::new(data.get_ref().clone());

    let app_namespaces = match app_ns_service.list_by_app(&app_id).await {
        Ok(list) => list,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                status: 500,
                message: e.to_string(),
            })
        }
    };

    let existing_namespaces = match ns_service.list(&app_id, &cluster_name).await {
        Ok(list) => list,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                status: 500,
                message: e.to_string(),
            })
        }
    };

    let existing_names: std::collections::HashSet<String> = existing_namespaces
        .into_iter()
        .map(|ns| ns.namespace_name)
        .collect();

    let missing: Vec<String> = app_namespaces
        .into_iter()
        .filter(|app_ns| !existing_names.contains(&app_ns.name))
        .map(|app_ns| app_ns.name)
        .collect();

    HttpResponse::Ok().json(missing)
}

// PORT-026: POST /openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/missing-namespaces
// Create the missing namespaces
async fn create_missing_namespaces_openapi(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, _env, cluster_name) = path.into_inner();
    let operator = query
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");

    let app_ns_service = AppNamespaceService::new(data.get_ref().clone());
    let ns_service = NamespaceService::new(data.get_ref().clone());

    let app_namespaces = match app_ns_service.list_by_app(&app_id).await {
        Ok(list) => list,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                status: 500,
                message: e.to_string(),
            })
        }
    };

    let existing_namespaces = match ns_service.list(&app_id, &cluster_name).await {
        Ok(list) => list,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                status: 500,
                message: e.to_string(),
            })
        }
    };

    let existing_names: std::collections::HashSet<String> = existing_namespaces
        .into_iter()
        .map(|ns| ns.namespace_name)
        .collect();

    let missing: Vec<AppNamespaceDTO> = app_namespaces
        .into_iter()
        .filter(|app_ns| !existing_names.contains(&app_ns.name))
        .collect();

    let mut created: Vec<String> = Vec::new();
    for app_ns in missing {
        let ns_dto = NamespaceDTO {
            app_id: app_id.clone(),
            cluster_name: cluster_name.clone(),
            namespace_name: app_ns.name.clone(),
            format: Some(app_ns.format.clone()),
            is_public: Some(app_ns.is_public),
            comment: if app_ns.comment.is_empty() {
                None
            } else {
                Some(app_ns.comment.clone())
            },
            data_change_created_by: Some(operator.to_string()),
            data_change_last_modified_by: None,
            data_change_created_time: None,
            data_change_last_time: None,
        };
        match ns_service.create(&app_id, &cluster_name, ns_dto).await {
            Ok(ns) => created.push(ns.namespace_name),
            Err(e) => {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    status: 400,
                    message: format!("Failed to create namespace {}: {}", app_ns.name, e),
                })
            }
        }
    }

    HttpResponse::Ok().json(serde_json::json!({
        "created": created,
        "count": created.len(),
    }))
}

/// Performs the `configure_openapi_routes` operation.
pub fn configure_openapi_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/openapi/v1/apps").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(create_app))
            .route(web::get().to(list_apps)),
    )
    .service(
        // PORT-003: authorized app listings
        web::resource("/openapi/v1/apps/authorized").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_apps_authorized)),
    )
    .service(
        web::resource("/openapi/v1/apps/by-self").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_apps_by_self)),
    )
    .service(
        // PORT-025: same payload as adminservice publish_info
        web::resource("/openapi/v1/apps/{app_id}/namespaces/releases/status").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_releases_status)),
    )
    .service(
        // PORT-021: POST /openapi/v1/namespaces (create namespace via OpenAPI)
        web::resource("/openapi/v1/namespaces").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_create_namespace)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(get_app))
            .route(web::put().to(update_app))
            .route(web::delete().to(delete_app)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envclusters").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(get_env_clusters)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/appnamespaces").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(create_app_namespace))
            .route(web::get().to(list_app_namespaces_openapi)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/appnamespaces/{namespace_name}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(get_app_namespace))
            .route(web::delete().to(delete_app_namespace)),
    )
    .service(
        web::resource("/openapi/v1/envs").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(list_envs)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(create_cluster)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_cluster))
        .route(web::delete().to(delete_cluster)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(create_namespace))
        .route(web::get().to(list_namespaces)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_namespace))
        .route(web::delete().to(delete_namespace)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/associated-public-namespace").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(openapi_associated_public_namespace)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(create_item))
        .route(web::get().to(find_items_by_namespace))
        // PITEM-003: bulk text update (full properties text, editor save)
        .route(web::put().to(openapi_bulk_update_items_by_text)),
    )
    // PITEM-011: syntax validation
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{ns_name}/items/validation").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_validate_items_text))
    )
    // PITEM-012: revert unpublished changes back to latest release
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{ns_name}/items/revocation").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_revert_items))
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/synchronize").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_items_synchronize_env))
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/{key}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_item))
        .route(web::put().to(update_item))
        .route(web::delete().to(delete_item)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(publish_release))
        .route(web::get().to(find_active_releases)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/latest").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_latest_release)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/active").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(find_active_releases)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/{release_id}/rollback").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(rollback_release)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/releases/{release_id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(get_release_by_id)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/releases/compare").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(compare_releases)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/releases/comparison").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(compare_releases)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/releases/{release_id}/rollback").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(rollback_release_by_id)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/history").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_release_history)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/histories").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_release_history_frontend)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/instances").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(list_instances)),
    )
    .service(
        // PORT-022: GET/POST /openapi/v1/appnamespaces
        web::resource("/openapi/v1/appnamespaces").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_list_appnamespaces))
            .route(web::post().to(openapi_create_appnamespace)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/appnamespaces/{ns_name}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::delete().to(openapi_delete_appnamespace)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/appnamespaces/{public_namespace_name}/instances").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_appnamespace_instances)),
    )
    .service(
        // PORT-026: public namespaces not yet linked to this cluster
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster}/missing-namespaces").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_missing_namespaces)),
    )
    .service(
        // PORT-020: GET .../namespaces/{namespaceName}/lock
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{ns_name}/lock").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_get_namespace_lock)),
    )
    // PITEM-007: base64-key item access
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{ns_name}/encodedItems/{b64key}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_get_encoded_item))
            .route(web::put().to(openapi_put_encoded_item))
            .route(web::delete().to(openapi_delete_encoded_item))
    )
    // PITEM-008: branch items
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{ns_name}/branches/{branch_name}/items").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_find_branch_items))
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/accesskeys").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(create_access_key))
            .route(web::get().to(list_access_keys)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/accesskeys/{id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::delete().to(delete_access_key)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/accesskeys").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(create_access_key_with_env))
            .route(web::get().to(list_access_keys_with_env)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/accesskeys/{id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::delete().to(delete_access_key_with_env)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(list_branches))
        .route(web::post().to(create_branch)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::delete().to(delete_branch)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}/rules").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(get_branch_rule))
        .route(web::put().to(update_branch_rule)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}/merge").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(merge_branch)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}/releases").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(create_gray_release)),
    )
    .service(
        web::resource("/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/commits").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(list_commits_openapi))
        .route(web::post().to(create_commit_openapi)),
    )
    .service(
        web::resource("/openapi/v1/commits/{commit_id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(get_commit_openapi)),
    )
    // PMISC-014: Consumer list and single consumer lookup
    .service(
        web::resource("/openapi/v1/consumers").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(list_consumers_openapi)),
    )
    .service(
        web::resource("/openapi/v1/consumers/by-appId").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_get_consumer_by_app)),
    )
    .service(
        web::resource("/openapi/v1/consumers/{app_id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(get_consumer_openapi)),
    )
    .service(
        web::resource("/openapi/v1/consumers/{token}/assign-role").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_assign_consumer_role)),
    )
    .service(
        web::resource("/openapi/v1/consumer-tokens/by-appId").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_consumer_tokens_by_app)),
    )
    // PMISC-015: Config export/import
    .service(
        web::resource("/openapi/v1/configs/{app_id}/{cluster_name}/{namespace_name}/export").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(export_configs_openapi)),
    )
    .service(
        web::resource("/openapi/v1/configs/import").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(import_configs_openapi)),
    )
    .service(
        web::resource("/openapi/v1/import").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(import_configs_openapi)),
    )
    // PMISC-018: Audit log query
    .service(
        web::resource("/openapi/v1/apollo/audit").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(list_audit_openapi)),
    )
    .service(
        web::resource("/openapi/v1/apollo/audit/by-entity").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(list_audit_by_entity_openapi)),
    )
    // PMISC-019: Favorites list
    .service(
        web::resource("/openapi/v1/favorites").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(list_favorites_openapi)),
    )
    .service(
        web::resource("/openapi/v1/favorites/{favorite_id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::delete().to(openapi_delete_favorite)),
    )
    // PMISC-020: Global search
    .service(
        web::resource("/openapi/v1/global-search/item-info/by-key-or-value").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(search_openapi)),
    )
    // PMISC-006: AccessKey activation/deactivation
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/accesskeys/{id}/activation").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(enable_access_key_openapi)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/accesskeys/{id}/deactivation").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(disable_access_key_openapi)),
    )
    // PORT-026: Missing namespaces
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/missing-namespaces").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(find_missing_namespaces_openapi))
        .route(web::post().to(create_missing_namespaces_openapi)),
    )
    // PMISC-001: paged instance configs of a namespace
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/instances").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(openapi_list_instances)),
    )
    // PMISC-002: instance configs bound to a release (by release ids)
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/instances/by-release").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(openapi_list_instances_by_release)),
    )
    // PMISC-003: instance count of a namespace
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/instances/by-namespace").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(openapi_count_instances)),
    )
    // PMISC-004: instance configs whose release is not in the given release ids
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/instances/by-release-not-in").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::get().to(openapi_list_instances_by_release_not_in)),
    )
    // PITEM-009: diff source items against each target namespace
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/items/diff").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::post().to(openapi_items_diff)),
    )
    // PITEM-010: synchronize source items into the target namespaces (PUT)
    .service(
        web::resource("/openapi/v1/apps/{app_id}/namespaces/{namespace_name}/items").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
        .route(web::put().to(openapi_items_synchronize)),
    )
    // PORT-009: env + cluster info of an app
    .service(
        web::resource("/openapi/v1/apps/{app_id}/env-cluster-info").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_env_cluster_info)),
    )
    // PORT-010: environments the app has NOT been created in
    .service(
        web::resource("/openapi/v1/apps/{app_id}/miss-envs").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_miss_envs)),
    )
    // PORT-011: create the app's namespaces in a given env
    .service(
        web::resource("/openapi/v1/apps/envs/{env}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_create_app_env)),
    )
    // PORT-024: where an app namespace is used
    .service(
        web::resource("/openapi/v1/apps/{app_id}/appnamespaces/{namespace_name}/usage").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_appnamespace_usage)),
    )
    // namespace usage with env/cluster context
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/usage").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_namespace_usage)),
    )
    // PMISC-007: current user is super admin (single-tenant: always true)
    .service(
        web::resource("/openapi/v1/permissions/root").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_permissions_root)),
    )
    // PMISC-008: whether the current user has a permission type on an app
    .service(
        web::resource("/openapi/v1/apps/{app_id}/permissions/{permission_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_permissions_app)),
    )
    // PMISC-009: role members of an app role type
    .service(
        web::resource("/openapi/v1/apps/{app_id}/roles/{role_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_roles)),
    )
    // permission-init
    .service(
        web::resource("/openapi/v1/apps/{app_id}/namespaces/{namespace_name}/permission-init").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_permission_init)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/permission-init").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::post().to(openapi_permission_init_cluster)),
    )
    // namespace-level permissions
    .service(
        web::resource("/openapi/v1/apps/{app_id}/namespaces/{namespace_name}/permissions/{permission_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_permissions_namespace)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/namespaces/{namespace_name}/permissions/{permission_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_permissions_namespace_env)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/permissions/{permission_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_permissions_cluster)),
    )
    // role-users
    .service(
        web::resource("/openapi/v1/apps/{app_id}/role-users").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_role_users)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/namespaces/{namespace_name}/role-users").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_role_users)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/namespaces/{namespace_name}/role-users").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_role_users_env)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/role-users").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_role_users_cluster)),
    )
    // namespace-level roles (GET/POST/DELETE)
    .service(
        web::resource("/openapi/v1/apps/{app_id}/namespaces/{namespace_name}/roles/{role_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_namespace_roles_get))
            .route(web::post().to(openapi_namespace_roles_post))
            .route(web::delete().to(openapi_namespace_roles_delete)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/namespaces/{namespace_name}/roles/{role_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_namespace_roles_env_get))
            .route(web::post().to(openapi_namespace_roles_env_post))
            .route(web::delete().to(openapi_namespace_roles_env_delete)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envs/{env}/clusters/{cluster_name}/roles/{role_type}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_cluster_roles_get))
            .route(web::post().to(openapi_cluster_roles_post))
            .route(web::delete().to(openapi_cluster_roles_delete)),
    )
    // system roles
    .service(
        web::resource("/openapi/v1/system/roles/create-application").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_system_roles_create_application))
            .route(web::post().to(openapi_system_roles_create_application_post)),
    )
    .service(
        web::resource("/openapi/v1/system/roles/create-application/role-users").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_system_role_users)),
    )
    .service(
        web::resource("/openapi/v1/system/role/manage-app-master").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_system_role_manage_app_master)),
    )
    // PMISC-010: organization list (single-tenant: default list)
    .service(
        web::resource("/openapi/v1/organizations").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_organizations)),
    )
    // PMISC-011: current user (single-tenant: fixed admin)
    .service(
        web::resource("/openapi/v1/user").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_current_user)),
    )
    // PMISC-012: user management
    .service(
        web::resource("/openapi/v1/users").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_list_users))
            .route(web::post().to(openapi_create_user)),
    )
    .service(
        web::resource("/openapi/v1/users/enabled").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(openapi_change_user_enabled)),
    )
    .service(
        web::resource("/openapi/v1/users/{username}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(openapi_update_user))
            .route(web::delete().to(openapi_delete_user)),
    )
    // PMISC-016: system information
    .service(
        web::resource("/openapi/v1/system-info").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_system_info)),
    )
    .service(
        web::resource("/openapi/v1/system-info/health").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_system_info_health)),
    )
    // Portal page settings
    .service(
        web::resource("/openapi/v1/page-settings").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_page_settings)),
    )
    // ===== Audit log extension endpoints =====
    .service(
        web::resource("/openapi/v1/apollo/audit/properties").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_audit_properties)),
    )
    .service(
        web::resource("/openapi/v1/logs").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_audit_logs)),
    )
    .service(
        web::resource("/openapi/v1/logs/opName").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_audit_logs_op_name)),
    )
    .service(
        web::resource("/openapi/v1/trace").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_audit_trace)),
    )
    .service(
        web::resource("/openapi/v1/logs/dataInfluences/field").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_audit_logs_field)),
    )
    .service(
        web::resource("/openapi/v1/logs/by-name-or-type-or-operator").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_audit_logs_search)),
    )
    // ===== User token endpoints =====
    .service(
        web::resource("/openapi/v1/user-tokens").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_list_user_tokens))
            .route(web::post().to(openapi_create_user_token)),
    )
    .service(
        web::resource("/openapi/v1/user-tokens/capabilities").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_user_token_capabilities)),
    )
    .service(
        web::resource("/openapi/v1/user-tokens/{id}/revoke").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(openapi_revoke_user_token)),
    )
    .service(
        web::resource("/openapi/v1/user-tokens/{id}/rotate").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(openapi_rotate_user_token)),
    )
    .service(
        web::resource("/openapi/v1/user-tokens/{id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::delete().to(openapi_delete_user_token)),
    )
    .service(
        web::resource("/openapi/v1/users/{username}/tokens").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_list_admin_user_tokens))
            .route(web::post().to(openapi_create_admin_user_token)),
    )
    .service(
        web::resource("/openapi/v1/users/{username}/tokens/{id}/revoke").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::put().to(openapi_revoke_admin_user_token)),
    )
    .service(
        web::resource("/openapi/v1/users/{username}/tokens/{id}").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::delete().to(openapi_delete_admin_user_token)),
    )
    // ===== Server config endpoints =====
    .service(
        web::resource("/openapi/v1/server/portal-db/config").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_server_portal_db_config)),
    )
    .service(
        web::resource("/openapi/v1/server/portal-db/config/find-all").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_server_portal_db_config_find_all)),
    )
    .service(
        web::resource("/openapi/v1/server/envs/{env}/config-db/config").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_server_env_config_db_config)),
    )
    .service(
        web::resource("/openapi/v1/server/envs/{env}/config-db/config/find-all").wrap(crate::middleware::auth::OpenApiAuthMiddleware::new())
            .route(web::get().to(openapi_server_env_config_db_config_find_all)),
    );
}

/// PORT-021: create a namespace (upstream OpenApiNamespaceController).
async fn openapi_create_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<Value>,
) -> impl Responder {
    let app_id = body.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let cluster = body.get("clusterName").and_then(|v| v.as_str()).unwrap_or("default");
    let ns = body.get("namespaceName").and_then(|v| v.as_str()).unwrap_or("");
    let operator = body.get("dataChangeCreatedBy").and_then(|v| v.as_str()).unwrap_or("apollo");
    if app_id.is_empty() || ns.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: "appId and namespaceName are required".into() });
    }
    let dto = crate::api::dto::NamespaceDTO {
        app_id: app_id.to_string(),
        cluster_name: cluster.to_string(),
        namespace_name: ns.to_string(),
        format: Some(body.get("format").and_then(|v| v.as_str()).unwrap_or("properties").to_string()),
        is_public: Some(body.get("isPublic").and_then(|v| v.as_bool()).unwrap_or(false)),
        comment: body.get("comment").and_then(|v| v.as_str()).map(|s| s.to_string()),
        data_change_created_by: Some(operator.to_string()),
        data_change_last_modified_by: None,
        data_change_last_time: None,
        data_change_created_time: None,
    };
    match crate::service::NamespaceService::new(data.get_ref().clone())
        .create(app_id, cluster, dto)
        .await
    {
        Ok(ns) => HttpResponse::Ok().json(ns),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// PORT-022 GET: page through AppNamespaces (upstream OpenApiAppNamespaceController).
#[derive(serde::Deserialize)]
struct PageQuery {
    #[serde(default)]
    page: usize,
    #[serde(default = "default_page_size")]
    size: usize,
}
fn default_page_size() -> usize {
    20
}

async fn openapi_list_appnamespaces(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    q: web::Query<PageQuery>,
) -> impl Responder {
    let service = crate::service::AppNamespaceService::new(data.get_ref().clone());
    // OpenAPI surface exposes the PUBLIC namespaces catalog (upstream
    // /appnamespaces lists public ones); private ns are queried per app.
    let all = match service.list_public().await {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    };
    let total = all.len();
    let start = (q.page * q.size).min(total);
    let end = ((q.page + 1) * q.size).min(total);
    HttpResponse::Ok().json(serde_json::json!({
        "content": &all[start..end],
        "total": total,
        "page": q.page,
        "size": q.size,
    }))
}

/// PORT-022 POST: create an AppNamespace.
async fn openapi_create_appnamespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<AppNamespaceDTO>,
) -> impl Responder {
    let service = crate::service::AppNamespaceService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(ns) => HttpResponse::Ok().json(ns),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// PORT-022 DELETE.
async fn openapi_delete_appnamespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("apollo");
    let service = crate::service::AppNamespaceService::new(data.get_ref().clone());
    match service.delete(&app_id, &name, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

/// PORT-020: lock info for a namespace (upstream returns null when unlocked).
async fn openapi_get_namespace_lock(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    use crate::persistence::traits::NamespaceLockPersistence;
    let (_env, app_id, cluster, ns) = path.into_inner();
    match NamespaceLockPersistence::get(data.get_ref(), &app_id, &cluster, &ns).await {
        Ok(Some(lock)) => HttpResponse::Ok().json(serde_json::json!({
            "namespaceName": ns,
            "isLocked": true,
            "lockedBy": lock.data_change_created_by,
            "clusterName": cluster,
            "appId": app_id,
        })),
        Ok(None) => HttpResponse::Ok().json(serde_json::Value::Null),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

// ===================== P5 batch 2 handlers =====================

/// PORT-003 — no auth system in batata: every app is "authorized".
async fn openapi_apps_authorized(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    let service = crate::service::AppService::new(data.get_ref().clone());
    match service.list().await {
        Ok(apps) => HttpResponse::Ok().json(apps),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

/// PORT-004 — same as authorized under batata's single-tenant model.
async fn openapi_apps_by_self(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    openapi_apps_authorized(data).await
}

/// PORT-025 — identical payload to adminservice publish_info
/// (frontend getNamespacePublishInfo consumes the same map).
async fn openapi_releases_status(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<String>,
) -> impl Responder {
    let app_id = path.into_inner();
    // Reuse the adminservice handler logic by delegating through the service layer.
    match crate::route::admin::publish_info_map(data.get_ref(), &app_id).await {
        Ok(map) => HttpResponse::Ok().json(map),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// Parse a `.properties` document into kv pairs; returns Err(line, reason).
fn parse_properties_text(text: &str) -> Result<Vec<(String, String)>, (usize, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashMap::new();
    for (idx, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let sep = line.find(['=', ':']).map(|i| (i, line.as_bytes()[i]));
        let (k, v) = match sep {
            Some((i, b)) if b == b'=' || b == b':' => {
                (line[..i].trim(), line[i + 1..].trim())
            }
            _ => {
                // `key value` whitespace form
                match line.split_once(char::is_whitespace) {
                    Some((k, v)) => (k.trim(), v.trim()),
                    None => return Err((idx + 1, format!("invalid properties line: {}", line))),
                }
            }
        };
        if k.is_empty() {
            return Err((idx + 1, format!("empty key at line {}", idx + 1)));
        }
        if seen.insert(k.to_string(), idx + 1).is_some() {
            return Err((idx + 1, format!("duplicate key: {}", k)));
        }
        out.push((k.to_string(), v.to_string()));
    }
    Ok(out)
}

/// PITEM-011 — syntax check only.
async fn openapi_validate_items_text(
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let text = body.get("text").and_then(|v| v.as_str()).unwrap_or("");
    match parse_properties_text(text) {
        Ok(_) => HttpResponse::Ok().finish(),
        Err((line, reason)) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: format!("syntax error at line {}: {}", line, reason),
        }),
    }
}

fn decode_item_key(b64: &str) -> anyhow::Result<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(b64)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(b64))
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(b64))?;
    String::from_utf8(bytes).map_err(|e| anyhow::anyhow!("key is not utf-8: {}", e))
}

/// PITEM-007 GET encodedItems/{b64key}
async fn openapi_get_encoded_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, cluster, ns, b64) = path.into_inner();
    let key = match decode_item_key(&b64) {
        Ok(k) => k,
        Err(e) => return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    };
    let service = crate::service::ItemService::new(data.get_ref().clone());
    match service.get_by_key(&app_id, &cluster, &ns, &key).await {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: format!("Item not found: {}", key) }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

/// PITEM-007 PUT encodedItems/{b64key}?createIfNotExists=
async fn openapi_put_encoded_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    query: web::Query<Value>,
    body: web::Json<crate::api::dto::ItemDTO>,
) -> impl Responder {
    let (_env, app_id, cluster, ns, b64) = path.into_inner();
    let create_if_missing = query.get("createIfNotExists").and_then(|v| v.as_str()) != Some("false");
    let key = match decode_item_key(&b64) {
        Ok(k) => k,
        Err(e) => return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    };
    let mut dto = body.into_inner();
    dto.key = key.clone();
    let operator = dto.data_change_last_modified_by.clone().unwrap_or_else(|| "apollo".into());
    let create_dto = crate::api::dto::ItemDTO {
        id: None, key: key.clone(), value: dto.value.clone(), r#type: None,
        comment: None, line_num: None,
        data_change_created_by: Some(operator),
        data_change_last_modified_by: None, data_change_last_time: None,
        data_change_created_time: None,
    };
    let service = crate::service::ItemService::new(data.get_ref().clone());
    match service.update_by_key(&app_id, &cluster, &ns, &key, dto).await {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(_) if create_if_missing => match service.create(&app_id, &cluster, &ns, create_dto).await {
            Ok(item) => HttpResponse::Ok().json(item),
            Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
        },
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// PITEM-007 DELETE encodedItems/{b64key}
async fn openapi_delete_encoded_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster, ns, b64) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("apollo");
    let key = match decode_item_key(&b64) {
        Ok(k) => k,
        Err(e) => return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    };
    let service = crate::service::ItemService::new(data.get_ref().clone());
    match service.delete_by_key(&app_id, &cluster, &ns, &key, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

/// PITEM-008 GET branches/{branch}/items — branch namespace's own items.
async fn openapi_find_branch_items(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String, String)>,
) -> impl Responder {
    let (_env, app_id, _cluster, ns, branch) = path.into_inner();
    let service = crate::service::ItemService::new(data.get_ref().clone());
    match service.list(&app_id, &branch, &ns).await {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}


/// PITEM-003 PUT items with full properties text — diff against current
/// items and apply create/update/delete as one change set (+ one commit),
/// mirroring portal's editor save flow.
async fn openapi_bulk_update_items_by_text(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    use crate::persistence::traits::NamespacePersistence;
    let (_env, app_id, cluster, ns_name) = path.into_inner();
    let text = body.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("apollo");

    let parsed = match parse_properties_text(text) {
        Ok(p) => p,
        Err((line, reason)) => return HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: format!("syntax error at line {}: {}", line, reason),
        }),
    };

    if data.get_ref().get_by_app_cluster(&app_id, &cluster, &ns_name).await.unwrap_or(None).is_none() {
        return HttpResponse::NotFound().json(ErrorResponse { status: 404, message: format!("Namespace not found: {}/{}/{}", app_id, cluster, ns_name) });
    }

    let item_service = crate::service::ItemService::new(data.get_ref().clone());
    let current = item_service.list(&app_id, &cluster, &ns_name).await.unwrap_or_default();
    let desired: std::collections::HashMap<String, String> = parsed.into_iter().collect();

    let mut creates = Vec::new();
    let mut updates = Vec::new();
    let mut deletes = Vec::new();

    for it in &current {
        match desired.get(&it.key) {
            None => deletes.push(it.clone()),
            Some(v) if v != &it.value => updates.push((it.clone(), v.clone())),
            _ => {}
        }
    }
    let existing_keys: std::collections::HashSet<String> = current.iter().map(|i| i.key.clone()).collect();
    for (k, v) in &desired {
        if !existing_keys.contains(k) {
            creates.push((k.clone(), v.clone()));
        }
    }

    for (it, new_value) in &updates {
        let mut dto = crate::api::dto::ItemDTO {
            id: it.id, key: it.key.clone(), value: new_value.clone(),
            r#type: it.r#type, comment: it.comment.clone(), line_num: it.line_num,
            data_change_created_by: it.data_change_created_by.clone(),
            data_change_last_modified_by: Some(operator.to_string()),
            data_change_last_time: None, data_change_created_time: None,
        };
        dto.data_change_created_time = None;
        if let Err(e) = item_service.update(&app_id, &cluster, &ns_name, it.id.unwrap(), dto).await {
            return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() });
        }
    }
    for (k, v) in &creates {
        if let Err(e) = item_service.create(&app_id, &cluster, &ns_name, crate::api::dto::ItemDTO {
            id: None, key: k.clone(), value: v.clone(), r#type: None, comment: None, line_num: None,
            data_change_created_by: Some(operator.to_string()),
            data_change_last_modified_by: None, data_change_last_time: None, data_change_created_time: None,
        }).await {
            return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() });
        }
    }
    for it in &deletes {
        if let Err(e) = item_service.delete_by_key(&app_id, &cluster, &ns_name, &it.key, operator).await {
            return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() });
        }
    }

    HttpResponse::Ok().json(serde_json::json!({
        "created": creates.len(), "updated": updates.len(), "deleted": deletes.len(),
    }))
}

/// PITEM-012 — revert unpublished changes: restore items to the latest
/// release state (delete extras, restore modified values, recreate removed).
async fn openapi_revert_items(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (_env, app_id, cluster, ns_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("apollo");

    let latest = ReleasePersistence::get_latest(data.get_ref(), &app_id, &cluster, &ns_name).await.ok().flatten();
    let released: HashMap<String, String> = latest
        .as_ref()
        .and_then(|r| serde_json::from_str(&r.configurations).unwrap_or_default())
        .unwrap_or_default();

    let item_service = crate::service::ItemService::new(data.get_ref().clone());
    let current = item_service.list(&app_id, &cluster, &ns_name).await.unwrap_or_default();
    let mut reverted = 0usize;

    for it in &current {
        match released.get(&it.key) {
            None => {
                if item_service.delete_by_key(&app_id, &cluster, &ns_name, &it.key, operator).await.is_ok() {
                    reverted += 1;
                }
            }
            Some(v) if v != &it.value => {
                let mut dto = crate::api::dto::ItemDTO {
                    id: it.id, key: it.key.clone(), value: v.clone(),
                    r#type: it.r#type, comment: it.comment.clone(), line_num: it.line_num,
                    data_change_created_by: it.data_change_created_by.clone(),
                    data_change_last_modified_by: Some(operator.to_string()),
                    data_change_last_time: None, data_change_created_time: None,
                };
                dto.data_change_created_time = None;
                if item_service.update(&app_id, &cluster, &ns_name, it.id.unwrap(), dto).await.is_ok() {
                    reverted += 1;
                }
            }
            _ => {}
        }
    }
    for (k, v) in &released {
        if !current.iter().any(|i| &i.key == k)
            && item_service.create(&app_id, &cluster, &ns_name, crate::api::dto::ItemDTO {
                id: None, key: k.clone(), value: v.clone(), r#type: None, comment: None, line_num: None,
                data_change_created_by: Some(operator.to_string()),
                data_change_last_modified_by: None, data_change_last_time: None, data_change_created_time: None,
            }).await.is_ok() {
                reverted += 1;
            }
    }
    let _ = latest; // release row untouched; history recorded implicitly via items
    let _ = ns_name;
    HttpResponse::Ok().json(serde_json::json!({ "reverted": reverted }))
}

/// PORT-026 — public namespaces not yet instantiated under this cluster.
async fn openapi_missing_namespaces(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    use crate::persistence::traits::NamespacePersistence;
    let (app_id, cluster, _ns) = path.into_inner();
    let public_ns = crate::service::AppNamespaceService::new(data.get_ref().clone())
        .list_public().await.unwrap_or_default();
    let mut missing = Vec::new();
    for p in public_ns {
        let exists = NamespacePersistence::get_by_app_cluster(data.get_ref(), &app_id, &cluster, &p.name)
            .await.map(|v| v.is_some()).unwrap_or(false);
        if !exists && p.app_id != app_id {
            missing.push(serde_json::json!({
                "name": p.name, "appId": p.app_id, "format": p.format, "isPublic": true,
            }));
        }
    }
    HttpResponse::Ok().json(missing)
}

// ===========================================================================
// PMISC-001~004: OpenAPI instance endpoints
// ===========================================================================

/// PMISC-001 — list the instance configs of a namespace, paged.
///
/// Upstream `apollo-portal` `OpenApiController.listInstances` returns a
/// `PageDTO<OpenInstanceDTO>` (default page size 20). `instanceAppId` is an
/// optional query filter matching the owning `configAppId`.
async fn openapi_list_instances(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    q: web::Query<InstancePageQuery>,
) -> impl Responder {
    let (app_id, _env, cluster, namespace_name) = path.into_inner();
    // batata keys instance configs by (configAppId, clusterName, namespaceName)
    // and reuses `appId` as the config app id. `env` is accepted for upstream
    // parity but ignored (single env model).
    let service = InstanceService::new(data.get_ref().clone());
    let page = q.page.max(1) as u64;
    let size = if q.size == 0 { 20 } else { q.size } as u64;
    match service
        .list_open_instances(&app_id, &cluster, &namespace_name, q.instance_app_id.as_deref(), page, size)
        .await
    {
        Ok(dto) => HttpResponse::Ok().json(dto),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

/// PMISC-002 — list instance configs whose release is in `releaseIds`.
///
/// Upstream throws 404 (`findReleaseOrThrow`) if any requested release id does
/// not exist. `releaseIds` is a comma-separated query parameter.
async fn openapi_list_instances_by_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    q: web::Query<ReleaseIdsQuery>,
) -> impl Responder {
    let (_app_id, _env, _cluster, _ns) = path.into_inner();
    let release_ids = parse_release_ids(&q.release_ids);
    let service = InstanceService::new(data.get_ref().clone());
    match service.list_open_instances_by_release(&release_ids).await {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

/// PMISC-003 — distinct instance count of a namespace.
async fn openapi_count_instances(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (app_id, _env, cluster, namespace_name) = path.into_inner();
    let service = InstanceService::new(data.get_ref().clone());
    match service.count_open_instances(&app_id, &cluster, &namespace_name).await {
        Ok(count) => HttpResponse::Ok().json(count),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

/// PMISC-004 — instance configs of a namespace whose release is NOT in
/// `releaseIds`.
async fn openapi_list_instances_by_release_not_in(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    q: web::Query<ReleaseIdsQuery>,
) -> impl Responder {
    let (app_id, _env, cluster, namespace_name) = path.into_inner();
    let release_ids = parse_release_ids(&q.release_ids);
    let service = InstanceService::new(data.get_ref().clone());
    match service
        .list_open_instances_by_release_not_in(&app_id, &cluster, &namespace_name, &release_ids)
        .await
    {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse { status: 404, message: e.to_string() }),
    }
}

#[derive(serde::Deserialize)]
struct InstancePageQuery {
    #[serde(default = "one")]
    page: usize,
    #[serde(default = "twenty")]
    size: usize,
    #[serde(default)]
    instance_app_id: Option<String>,
}
fn one() -> usize { 1 }
fn twenty() -> usize { 20 }

#[derive(serde::Deserialize)]
struct ReleaseIdsQuery {
    #[serde(default)]
    release_ids: String,
}

/// Parses a comma-separated release id list into `i64` values, dropping any
/// non-numeric token (upstream ignores malformed ids; here we keep only valid
/// integers so an empty/garbage list yields an empty result rather than a 500).
fn parse_release_ids(raw: &str) -> Vec<i64> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i64>().ok())
        .collect()
}

// ===========================================================================
// PITEM-009 / PITEM-010: items diff and synchronize
// ===========================================================================

/// PITEM-009 — diff the source change sets against each target namespace.
///
/// Upstream `portal/controller/ItemController.java:180-203`
/// `configService.compare(syncToNamespaces, syncItems)` returns one diff per
/// target namespace (create/update/delete item lists). `appId`/`env`/`cluster`
/// identify the source namespace; the body is a `NamespaceSyncModel`.
async fn openapi_items_diff(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<crate::api::dto::NamespaceSyncModel>,
) -> impl Responder {
    let (app_id, _env, cluster, namespace_name) = path.into_inner();
    let model = body.into_inner();
    let service = crate::service::ConfigSyncService::new(data.get_ref().clone());
    match service
        .compare(&app_id, &cluster, &namespace_name, &model.sync_to_namespaces, &model.sync_items)
        .await
    {
        Ok(diffs) => HttpResponse::Ok().json(diffs),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// PITEM-010 — synchronize the source change sets into every target namespace.
///
/// Upstream `portal/controller/ItemController.java:205-230` is
/// `PUT /apps/{appId}/namespaces/{namespaceName}/items` (note: PUT, not POST,
/// and no env/cluster in the path). The body is a `NamespaceSyncModel`.
async fn openapi_items_synchronize(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
    body: web::Json<crate::api::dto::NamespaceSyncModel>,
) -> impl Responder {
    let (app_id, namespace_name) = path.into_inner();
    let model = body.into_inner();
    // batata stores namespaces per (appId, cluster); for the openapi sync we
    // target the `default` cluster (upstream synchronizes public app namespaces
    // which live under the default cluster of each app).
    let operator = "apollo";
    let service = crate::service::ConfigSyncService::new(data.get_ref().clone());
    match service
        .synchronize(&app_id, "default", &namespace_name, &model.sync_to_namespaces, &model.sync_items, operator)
        .await
    {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

// ===========================================================================
// PORT-009 / PORT-010 / PORT-011 / PORT-024: portal app & namespace endpoints
// ===========================================================================

/// PORT-009 — env + cluster info of an app.
///
/// Upstream `apollo-portal` `AppController.envClusterInfo` returns
/// `EnvClusterInfoDTO`. batata has no per-env cluster separation, so every
/// canonical env the app has namespaces in maps to the app's root clusters.
async fn openapi_env_cluster_info(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<String>,
) -> impl Responder {
    use crate::persistence::traits::ClusterPersistence;
    let app_id = path.into_inner();
    let clusters = data.get_ref().list(&app_id).await.unwrap_or_default();
    let root_clusters: Vec<crate::api::dto::ClusterInfoDTO> = clusters
        .into_iter()
        .filter(|c| !c.is_deleted && c.parent_cluster_id == 0)
        .map(|c| crate::api::dto::ClusterInfoDTO {
            cluster_name: c.name,
            parent_cluster_name: None,
            config_app_id: Some(app_id.clone()),
        })
        .collect();
    let envs = ["DEV", "FAT", "UAT", "PRO"];
    let env_cluster_info: Vec<crate::api::dto::EnvClusterInfo> = envs
        .iter()
        .map(|env| crate::api::dto::EnvClusterInfo {
            env: env.to_string(),
            clusters: root_clusters.clone(),
        })
        .collect();
    HttpResponse::Ok().json(crate::api::dto::EnvClusterInfoDTO { env_cluster_info })
}

/// PORT-010 — environments the app has NOT been created in yet.
///
/// Upstream returns the set of all envs minus the envs the app already has a
/// namespace in. batata keeps namespaces across all envs uniformly, so this is
/// the complement of the envs that contain at least one namespace of the app.
async fn openapi_miss_envs(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<String>,
) -> impl Responder {
    let app_id = path.into_inner();
    let namespaces = <dyn NamespacePersistence>::list_by_app(data.get_ref(), &app_id).await.unwrap_or_default();
    // batata does not persist an env per namespace; treat any namespace as
    // "present in all envs" so an app with at least one namespace reports no
    // missing envs.
    let present = !namespaces.is_empty();
    let all = ["DEV", "FAT", "UAT", "PRO"];
    let miss: Vec<String> = if present {
        Vec::new()
    } else {
        all.iter().map(|s| s.to_string()).collect()
    };
    HttpResponse::Ok().json(miss)
}

/// PORT-011 — create the app's public app namespaces under a given env.
///
/// Upstream `AppNamespaceController.createAppNamespace` instantiates a public
/// app namespace into the cluster. batata stores namespaces globally per app,
/// so we create each requested namespace (from the body) under the default
/// cluster. The body is a list of `AppNamespaceDTO`.
async fn openapi_create_app_env(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<String>,
    body: web::Json<Vec<crate::api::dto::AppNamespaceDTO>>,
) -> impl Responder {
    let items = body.into_inner();
    let service = crate::service::AppNamespaceService::new(data.get_ref().clone());
    let mut created = Vec::new();
    for mut dto in items {
        if dto.data_change_created_by.is_none() {
            dto.data_change_created_by = Some("apollo".to_string());
        }
        match service.create(dto).await {
            Ok(c) => created.push(c),
            Err(e) => return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
        }
    }
    HttpResponse::Ok().json(created)
}

/// PORT-024 — where an app namespace is used (which envs / clusters / apps).
///
/// Upstream `AppNamespaceController.usage`. batata tracks usage via the
/// namespaces table (appId + cluster + namespaceName). We collect the distinct
/// clusters and the owning app for the given app namespace name.
async fn openapi_appnamespace_usage(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (_app_id, namespace_name) = path.into_inner();
    // All namespaces sharing this name across apps/clusters.
    let all_ns = <dyn NamespacePersistence>::list_all(data.get_ref()).await.unwrap_or_default();
    let mut envs = Vec::new();
    let mut clusters = Vec::new();
    let mut used_by = Vec::new();
    for ns in all_ns {
        if ns.namespace_name == namespace_name && !ns.is_deleted {
            if !clusters.contains(&ns.cluster_name) {
                clusters.push(ns.cluster_name.clone());
            }
            if !used_by.contains(&ns.app_id) {
                used_by.push(ns.app_id.clone());
            }
        }
    }
    if !clusters.is_empty() {
        envs = vec!["DEV".to_string(), "FAT".to_string(), "UAT".to_string(), "PRO".to_string()];
    }
    HttpResponse::Ok().json(crate::api::dto::AppNamespaceUsageDTO {
        namespace_name,
        envs,
        clusters,
        used_by,
    })
}

/// GET .../namespaces/{namespace_name}/associated-public-namespace
async fn openapi_associated_public_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    use crate::persistence::traits::{NamespacePersistence, ReleasePersistence};
    let (_env, _app_id, cluster_name, namespace_name) = path.into_inner();

    let public_ns = crate::service::AppNamespaceService::new(data.get_ref().clone())
        .list_public()
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.name == namespace_name);
    let Some(owner) = public_ns else {
        return HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("public namespace not found. namespace:{}", namespace_name),
        });
    };
    let owner_app = owner.app_id.clone();

    let ns_json = |ns: Option<crate::persistence::shared::StoredNamespace>| -> serde_json::Value {
        match ns {
            Some(n) => serde_json::json!({
                "id": n.id, "appId": n.app_id, "clusterName": n.cluster_name,
                "namespaceName": n.namespace_name, "format": n.format, "isPublic": n.is_public,
            }),
            None => serde_json::Value::Null,
        }
    };

    if cluster_name == "default" {
        let ns = data.get_ref().get_by_app_cluster(&owner_app, &cluster_name, &namespace_name).await.unwrap_or(None);
        return HttpResponse::Ok().json(ns_json(ns));
    }

    let custom = data.get_ref().get_by_app_cluster(&owner_app, &cluster_name, &namespace_name).await.unwrap_or(None);
    let published = match custom.as_ref() {
        Some(_) => ReleasePersistence::get_latest(data.get_ref(), &owner_app, &cluster_name, &namespace_name)
            .await
            .ok()
            .flatten()
            .is_some(),
        None => false,
    };

    if published {
        return HttpResponse::Ok().json(ns_json(custom));
    }
    let fallback = data.get_ref().get_by_app_cluster(&owner_app, "default", &namespace_name).await.unwrap_or(None);
    HttpResponse::Ok().json(ns_json(fallback))
}

/// POST .../items/synchronize
async fn openapi_items_synchronize_env(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<crate::api::dto::NamespaceSyncModel>,
) -> impl Responder {
    let (_env, app_id, cluster_name, namespace_name) = path.into_inner();
    let model = body.into_inner();
    let operator = "apollo";
    let service = crate::service::ConfigSyncService::new(data.get_ref().clone());
    match service
        .synchronize(&app_id, &cluster_name, &namespace_name, &model.sync_to_namespaces, &model.sync_items, operator)
        .await
    {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// GET /envs/{env}/appnamespaces/{public_namespace_name}/instances
async fn openapi_appnamespace_instances(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
    q: web::Query<InstancePageQuery>,
) -> impl Responder {
    let (_env, namespace_name) = path.into_inner();
    // Find the owning app of this public namespace, then list instances.
    let public_ns = AppNamespaceService::new(data.get_ref().clone())
        .list_public()
        .await
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.name == namespace_name);
    let Some(owner) = public_ns else {
        return HttpResponse::Ok().json(serde_json::json!({
            "content": [],
            "page": q.page,
            "size": q.size,
            "total": 0,
        }));
    };
    let service = InstanceService::new(data.get_ref().clone());
    let page = q.page.max(1) as u64;
    let size = if q.size == 0 { 20 } else { q.size } as u64;
    match service
        .list_open_instances(&owner.app_id, "default", &namespace_name, q.instance_app_id.as_deref(), page, size)
        .await
    {
        Ok(dto) => HttpResponse::Ok().json(dto),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

/// GET /apps/{app_id}/envs/{env}/clusters/{cluster_name}/namespaces/{namespace_name}/usage
async fn openapi_namespace_usage(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (app_id, _env, cluster_name, namespace_name) = path.into_inner();
    let all_ns = <dyn NamespacePersistence>::list_all(data.get_ref()).await.unwrap_or_default();
    let mut used_by = Vec::new();
    for ns in all_ns {
        if ns.namespace_name == namespace_name && !ns.is_deleted {
            if !used_by.contains(&ns.app_id) {
                used_by.push(ns.app_id.clone());
            }
        }
    }
    HttpResponse::Ok().json(serde_json::json!({
        "appId": app_id,
        "clusterName": cluster_name,
        "namespaceName": namespace_name,
        "usedBy": used_by,
    }))
}

/// DELETE /favorites/{favorite_id}
async fn openapi_delete_favorite(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<i64>,
    query: web::Query<Value>,
) -> impl Responder {
    let id = path.into_inner();
    let user_id = query.get("userId").and_then(|v| v.as_str()).unwrap_or("apollo");
    let service = FavoriteService::new(data.get_ref().clone());
    match service.delete(id, user_id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

// ===========================================================================
// PMISC-007~013 / 016: single-tenant degraded permission / user / system
// ===========================================================================

/// PMISC-007 — whether the current user is a super admin.
///
/// Upstream `UserInfoHolder.isSuperAdmin()` gated by `@PreAuthorize
/// hasRootPermission`. batata is single-tenant with no auth system, so every
/// caller is treated as having root permission.
async fn openapi_permissions_root(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    // Upstream returns a boolean `hasRootPermission`.
    HttpResponse::Ok().json(serde_json::json!({ "hasRootPermission": true }))
}

/// PMISC-008 — whether the current user has `permissionType` on `appId`.
///
/// Upstream `PermissionController.isAppRolePermission` returns a
/// `PermissionDTO` with `hasPermission`. batata degrades to "allowed".
async fn openapi_permissions_app(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (app_id, permission_type) = path.into_inner();
    // Single-tenant: every permission type on every app is granted.
    HttpResponse::Ok().json(serde_json::json!({
        "hasPermission": true,
        "permissionType": permission_type,
        "appId": app_id,
        "targetId": app_id,
    }))
}

/// PMISC-009 — members of an app role type.
///
/// Upstream `RoleController.listAppRoles` returns `RoleDTO` per role type with
/// its `users`. batata has no role store populated, so we return a single role
/// with no members for the requested type.
async fn openapi_roles(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (app_id, role_type) = path.into_inner();
    let role_name = format!("{}-{}", role_type.to_uppercase(), app_id);
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_name,
        "users": [],
    }))
}

/// POST .../permission-init — initialize permissions for a namespace.
/// Single-tenant: no-op, always succeeds.
async fn openapi_permission_init(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String)>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

async fn openapi_permission_init_cluster(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String, String)>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

/// GET namespace-level permission check.
async fn openapi_permissions_namespace(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (app_id, namespace_name, permission_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "hasPermission": true,
        "permissionType": permission_type,
        "appId": app_id,
        "targetId": namespace_name,
    }))
}

async fn openapi_permissions_namespace_env(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (app_id, _env, namespace_name, permission_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "hasPermission": true,
        "permissionType": permission_type,
        "appId": app_id,
        "targetId": namespace_name,
    }))
}

async fn openapi_permissions_cluster(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (app_id, _env, cluster_name, permission_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "hasPermission": true,
        "permissionType": permission_type,
        "appId": app_id,
        "targetId": cluster_name,
    }))
}

/// GET role-users — returns the fixed admin as the role member.
async fn openapi_role_users(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String)>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([
        { "userId": "apollo", "name": "Apollo" }
    ]))
}

async fn openapi_role_users_env(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String, String)>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([
        { "userId": "apollo", "name": "Apollo" }
    ]))
}

async fn openapi_role_users_cluster(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String, String)>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([
        { "userId": "apollo", "name": "Apollo" }
    ]))
}

/// GET/POST/DELETE namespace-level roles.
async fn openapi_namespace_roles_get(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (_app_id, _namespace_name, role_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_type.to_uppercase(),
        "users": [],
    }))
}

async fn openapi_namespace_roles_post(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    _body: web::Json<Value>,
) -> impl Responder {
    let (_app_id, _namespace_name, role_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_type.to_uppercase(),
        "users": [],
    }))
}

async fn openapi_namespace_roles_delete(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String, String)>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

async fn openapi_namespace_roles_env_get(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (_app_id, _env, _namespace_name, role_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_type.to_uppercase(),
        "users": [],
    }))
}

async fn openapi_namespace_roles_env_post(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    _body: web::Json<Value>,
) -> impl Responder {
    let (_app_id, _env, _namespace_name, role_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_type.to_uppercase(),
        "users": [],
    }))
}

async fn openapi_namespace_roles_env_delete(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

async fn openapi_cluster_roles_get(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (_app_id, _env, _cluster_name, role_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_type.to_uppercase(),
        "users": [],
    }))
}

async fn openapi_cluster_roles_post(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    _body: web::Json<Value>,
) -> impl Responder {
    let (_app_id, _env, _cluster_name, role_type) = path.into_inner();
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": role_type.to_uppercase(),
        "users": [],
    }))
}

async fn openapi_cluster_roles_delete(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    HttpResponse::Ok().finish()
}

/// System roles: create-application.
async fn openapi_system_roles_create_application(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": "CreateApplication",
        "users": [{ "userId": "apollo", "name": "Apollo" }],
    }))
}

async fn openapi_system_roles_create_application_post(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
    _body: web::Json<Value>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": "CreateApplication",
        "users": [],
    }))
}

async fn openapi_system_role_users(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([
        { "userId": "apollo", "name": "Apollo" }
    ]))
}

async fn openapi_system_role_manage_app_master(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "roleName": "ManageAppMaster",
        "users": [{ "userId": "apollo", "name": "Apollo" }],
    }))
}

/// PMISC-010 — the organization list.
///
/// Upstream `OrganizationController.findAllOrganizations` returns
/// `List<OrganizationDTO>`. batata is single-tenant; we return a fixed default
/// organization so portal UI can render without a user directory.
async fn openapi_organizations(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([{ "orgId": "1", "orgName": "default" }]))
}

/// PMISC-011 — the current user.
///
/// Upstream `UserInfoController.currentUser`. batata is single-tenant, so the
/// fixed admin user is returned.
async fn openapi_current_user(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "username": "apollo",
        "email": "apollo@localhost",
        "realName": "Apollo",
        "roles": ["ROLE_ADMIN"],
    }))
}

/// PMISC-012 — list users.
async fn openapi_list_users(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    // Single-tenant: always include the fixed admin. The persistence layer may
    // be empty, so we synthesize the admin entry directly.
    let mut users = vec![serde_json::json!({
        "username": "apollo",
        "email": "apollo@localhost",
        "realName": "Apollo",
        "roles": ["ROLE_ADMIN"],
    })];
    if let Ok(persisted) = data.list_users().await {
        for u in persisted {
            if u.username == "apollo" {
                continue;
            }
            users.push(serde_json::json!({
                "username": u.username,
                "email": u.email,
                "realName": u.username,
                "roles": [],
            }));
        }
    }
    HttpResponse::Ok().json(users)
}

/// PMISC-012 — create a user.
async fn openapi_create_user(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<crate::api::dto::UserDTO>,
) -> impl Responder {
    let dto = body.into_inner();
    match data.create_user(dto).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// PMISC-012 — update a user.
async fn openapi_update_user(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<String>,
    body: web::Json<crate::api::dto::UserDTO>,
) -> impl Responder {
    let username = path.into_inner();
    let dto = body.into_inner();
    match data.update_user(&username, dto).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// PMISC-012 — delete a user.
async fn openapi_delete_user(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<String>,
) -> impl Responder {
    let username = path.into_inner();
    match data.delete_user(&username).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    }
}

/// PMISC-016 — system information.
///
/// Upstream `SystemInfoController.getSystemInfo` returns
/// `SystemInfoDTO` (`apolloVersion` + `gitCommitId`). batata fills these from
/// the build-time cargo environment.
async fn openapi_system_info(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let commit = option_env!("GIT_COMMIT_ID").unwrap_or("unknown").to_string();
    HttpResponse::Ok().json(serde_json::json!({
        "apolloVersion": version,
        "gitCommitId": commit,
    }))
}

/// Portal page settings (single-tenant defaults).
async fn openapi_page_settings(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "prefix": "/",
        "supportCustomTitle": true,
    }))
}

/// System health check.
async fn openapi_system_info_health(
    _data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "UP" }))
}

/// Enable/disable a user (PUT /users/enabled).
async fn openapi_change_user_enabled(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<Value>,
) -> impl Responder {
    use crate::persistence::traits::UserPersistence;
    let username = body.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    match UserPersistence::get_user(data.get_ref(), username).await {
        Ok(Some(mut user)) => {
            user.enabled = enabled;
            let dto = crate::api::dto::UserDTO {
                id: Some(user.id),
                username: user.username.clone(),
                password: String::new(),
                email: Some(user.email.clone()),
                enabled: user.enabled,
                data_change_created_by: None,
                data_change_created_time: None,
            };
            match UserPersistence::update_user(data.get_ref(), username, dto).await {
                Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })),
                Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
                    status: 400,
                    message: e.to_string(),
                }),
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("User not found: {}", username),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}
