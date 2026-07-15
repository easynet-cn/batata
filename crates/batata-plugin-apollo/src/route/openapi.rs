use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use crate::persistence::traits::ApolloPersistenceService;
use crate::service::{AppService, NamespaceService, ItemService, ReleaseService, ClusterService, InstanceService, AccessKeyService, GrayReleaseRuleService, AppNamespaceService, CommitService};
use crate::api::dto::{AppDTO, NamespaceDTO, ItemDTO, ErrorResponse, ClusterDTO, GrayReleaseRuleDTO, AppNamespaceDTO, CommitDTO, ItemChangeSets};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCreateAppDTO {
    pub app: AppDTO,
    #[serde(default)]
    pub admins: Vec<String>,
    #[serde(default)]
    pub assign_app_role_to_self: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAppNamespaceDTO {
    pub app_id: String,
    pub name: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_is_public")]
    pub is_public: bool,
    #[serde(default)]
    pub comment: String,
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
pub struct OpenItemDTO {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default = "default_item_type")]
    pub r#type: i8,
    #[serde(default)]
    pub data_change_created_by: Option<String>,
    #[serde(default)]
    pub data_change_last_modified_by: Option<String>,
}

fn default_item_type() -> i8 {
    0
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceReleaseDTO {
    pub release_title: String,
    #[serde(default)]
    pub release_comment: String,
    pub released_by: String,
    #[serde(default)]
    pub is_emergency_publish: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceGrayDelReleaseDTO {
    pub release_title: String,
    #[serde(default)]
    pub release_comment: String,
    pub released_by: String,
    #[serde(default)]
    pub is_emergency_publish: bool,
    #[serde(default)]
    pub create_items: Vec<OpenItemDTO>,
    #[serde(default)]
    pub update_items: Vec<OpenItemDTO>,
    #[serde(default)]
    pub delete_items: Vec<OpenItemDTO>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenNamespace {
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub format: String,
    pub comment: String,
    pub is_public: bool,
    pub items: Vec<ItemDTO>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenEnvCluster {
    pub env: String,
    pub clusters: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRelease {
    pub id: i32,
    pub app_id: String,
    pub cluster_name: String,
    pub namespace_name: String,
    pub name: String,
    pub configurations: Value,
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
    path: web::Path<(String, String, String, String, i32)>,
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
    path: web::Path<(String, i32)>,
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
    let branch_name = body.get("branchName").and_then(|v| v.as_str()).unwrap_or("gray");
    let operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");

    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    let dto = GrayReleaseRuleDTO {
        id: None,
        app_id: app_id.clone(),
        cluster_name: cluster_name.clone(),
        namespace_name: namespace_name.clone(),
        branch_name: branch_name.to_string(),
        rules: Some("[]".to_string()),
        release_id: 0,
        branch_status: Some(1),
        priority: None,
        data_change_created_by: Some(operator.to_string()),
        data_change_last_modified_by: None,
        data_change_created_time: None,
    };

    match service.create(dto).await {
        Ok(rule) => HttpResponse::Ok().json(rule),
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
    let branch_status = body.get("branchStatus").and_then(|v| v.as_i64()).map(|v| v as i16);
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
    let (_env, app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.delete(&app_id, &cluster_name, &namespace_name, &branch_name, operator).await {
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
    commit_id: web::Path<i32>,
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

pub fn configure_openapi_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/openapi/v1/apps")
            .route(web::post().to(create_app))
            .route(web::get().to(list_apps)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}")
            .route(web::get().to(get_app)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/envclusters")
            .route(web::get().to(get_env_clusters)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/appnamespaces")
            .route(web::post().to(create_app_namespace))
            .route(web::get().to(list_app_namespaces_openapi)),
    )
    .service(
        web::resource("/openapi/v1/envs")
            .route(web::get().to(list_envs)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters",
        )
        .route(web::post().to(create_cluster)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}",
        )
        .route(web::get().to(get_cluster)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces",
        )
        .route(web::post().to(create_namespace))
        .route(web::get().to(list_namespaces)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}",
        )
        .route(web::get().to(get_namespace)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items",
        )
        .route(web::post().to(create_item))
        .route(web::get().to(find_items_by_namespace)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/{key}",
        )
        .route(web::get().to(get_item))
        .route(web::put().to(update_item))
        .route(web::delete().to(delete_item)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases",
        )
        .route(web::post().to(publish_release))
        .route(web::get().to(find_active_releases)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/latest",
        )
        .route(web::get().to(get_latest_release)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/{release_id}/rollback",
        )
        .route(web::post().to(rollback_release)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/history",
        )
        .route(web::get().to(get_release_history)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/instances",
        )
        .route(web::get().to(list_instances)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/accesskeys")
            .route(web::post().to(create_access_key))
            .route(web::get().to(list_access_keys)),
    )
    .service(
        web::resource("/openapi/v1/apps/{app_id}/accesskeys/{id}")
            .route(web::delete().to(delete_access_key)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches",
        )
        .route(web::get().to(list_branches))
        .route(web::post().to(create_branch)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}",
        )
        .route(web::get().to(get_branch_rule))
        .route(web::put().to(update_branch_rule))
        .route(web::delete().to(delete_branch)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}/merge",
        )
        .route(web::post().to(merge_branch)),
    )
    .service(
        web::resource(
            "/openapi/v1/envs/{env}/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/commits",
        )
        .route(web::get().to(list_commits_openapi))
        .route(web::post().to(create_commit_openapi)),
    )
    .service(
        web::resource("/openapi/v1/commits/{commit_id}")
            .route(web::get().to(get_commit_openapi)),
    );
}
