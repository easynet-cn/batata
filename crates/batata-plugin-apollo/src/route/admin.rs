use actix_web::{web, HttpResponse, Responder};
use base64::Engine;
use serde_json::Value;
use std::sync::Arc;
use crate::persistence::traits::ApolloPersistenceService;
use crate::service::{
    AppService, NamespaceService, ItemService, ItemSetService, ReleaseService,
    ClusterService, CommitService, GrayReleaseRuleService, ServerConfigService,
    AppNamespaceService, NamespaceLockService, InstanceConfigService,
    AuditService, ConsumerService, ConsumerTokenService,
    PermissionService, RoleService, FavoriteService, SearchService,
    AccessKeyService, NamespaceBranchService,
};
use crate::api::dto::{
    AppDTO, NamespaceDTO, ItemDTO, ItemChangeSets, ErrorResponse, ClusterDTO,
    CommitDTO, GrayReleaseRuleDTO, ServerConfigDTO, AppNamespaceDTO,
    ConsumerDTO, RoleDTO, FavoriteDTO, ConfigImportDTO, NamespaceGrayReleaseDTO,
    UserDTO,
};

async fn list_apps(data: web::Data<Arc<dyn ApolloPersistenceService>>) -> impl Responder {
    let service = AppService::new(data.get_ref().clone());
    match service.list().await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_app(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<AppDTO>) -> impl Responder {
    let service = AppService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(app) => HttpResponse::Ok().json(app),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_app(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>) -> impl Responder {
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

async fn update_app(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>, body: web::Json<AppDTO>) -> impl Responder {
    let service = AppService::new(data.get_ref().clone());
    match service.update(app_id.as_str(), body.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_app(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>, operator: web::Query<Value>) -> impl Responder {
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AppService::new(data.get_ref().clone());
    match service.delete(app_id.as_str(), op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn create_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>, body: web::Json<NamespaceDTO>) -> impl Responder {
    let (app_id, cluster_name) = path.into_inner();
    let service = NamespaceService::new(data.get_ref().clone());
    match service.create(&app_id, &cluster_name, body.into_inner()).await {
        Ok(ns) => HttpResponse::Ok().json(ns),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = NamespaceService::new(data.get_ref().clone());
    match service.get(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(ns)) => HttpResponse::Ok().json(ns),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Namespace not found: {}/{}/{}", app_id, cluster_name, namespace_name),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn list_namespaces(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>) -> impl Responder {
    let (app_id, cluster_name) = path.into_inner();
    let service = NamespaceService::new(data.get_ref().clone());
    match service.list(&app_id, &cluster_name).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn delete_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, operator: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = NamespaceService::new(data.get_ref().clone());
    match service.delete(&app_id, &cluster_name, &namespace_name, op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn update_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, body: web::Json<NamespaceDTO>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = NamespaceService::new(data.get_ref().clone());
    match service.update(&app_id, &cluster_name, &namespace_name, body.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn create_item(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, body: web::Json<ItemDTO>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.create(&app_id, &cluster_name, &namespace_name, body.into_inner()).await {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_items(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_item(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, key) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.get_by_key(&app_id, &cluster_name, &namespace_name, &key).await {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
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

async fn update_item(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>, body: web::Json<ItemDTO>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, key) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    // Upstream ItemController PUT /items/{itemId} addresses items BY ID;
    // keep by-key as primary for compatibility and fall back to numeric id.
    let dto = body.into_inner();
    match service.update_by_key(&app_id, &cluster_name, &namespace_name, &key, dto.clone()).await {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(e) => {
            if let Ok(item_id) = key.parse::<i64>() {
                match service.update(&app_id, &cluster_name, &namespace_name, item_id, dto).await {
                    Ok(item) => return HttpResponse::Ok().json(item),
                    Err(id_err) => {
                        return HttpResponse::BadRequest().json(ErrorResponse {
                            status: 400,
                            message: format!("by-key: {}; by-id: {}", e, id_err),
                        });
                    }
                }
            }
            HttpResponse::BadRequest().json(ErrorResponse {
                status: 400,
                message: e.to_string(),
            })
        }
    }
}

async fn delete_item(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>, operator: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, key) = path.into_inner();
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ItemService::new(data.get_ref().clone());
    match service.delete_by_key(&app_id, &cluster_name, &namespace_name, &key, op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn publish_release(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let release_name = query.get("name").and_then(|v| v.as_str()).unwrap_or_default();
    let release_comment = query.get("comment").and_then(|v| v.as_str()).map(|s| s.to_string());
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let is_emergency_publish = query.get("isEmergencyPublish").and_then(|v| v.as_bool()).unwrap_or(false);

    let service = ReleaseService::new(data.get_ref().clone());
    match service.publish(&app_id, &cluster_name, &namespace_name, release_name, release_comment, operator, is_emergency_publish).await {
        Ok(release) => HttpResponse::Ok().json(release),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_clusters(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>) -> impl Responder {
    let service = ClusterService::new(data.get_ref().clone());
    match service.list(app_id.as_str()).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_cluster(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>, body: web::Json<ClusterDTO>) -> impl Responder {
    let service = ClusterService::new(data.get_ref().clone());
    match service.create(app_id.as_str(), body.into_inner()).await {
        Ok(cluster) => HttpResponse::Ok().json(cluster),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_cluster(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>) -> impl Responder {
    let (app_id, cluster_name) = path.into_inner();
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

async fn delete_cluster(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>, operator: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name) = path.into_inner();
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ClusterService::new(data.get_ref().clone());
    match service.delete(&app_id, &cluster_name, op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn list_commits(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = CommitService::new(data.get_ref().clone());
    match service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_commit(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<CommitDTO>) -> impl Responder {
    let service = CommitService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(commit) => HttpResponse::Ok().json(commit),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_commit(data: web::Data<Arc<dyn ApolloPersistenceService>>, id: web::Path<i64>) -> impl Responder {
    let commit_id = id.into_inner();
    let service = CommitService::new(data.get_ref().clone());
    match service.get(commit_id).await {
        Ok(Some(commit)) => HttpResponse::Ok().json(commit),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Commit not found: {}", commit_id),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn list_gray_release_rules(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.list_by_namespace(&app_id, &cluster_name, &namespace_name).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_gray_release_rule(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<GrayReleaseRuleDTO>) -> impl Responder {
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(rule) => HttpResponse::Ok().json(rule),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_gray_release_rule(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.get(&app_id, &cluster_name, &namespace_name, &branch_name).await {
        Ok(Some(rule)) => HttpResponse::Ok().json(rule),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Gray release rule not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn update_gray_release_rule(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>, body: web::Json<GrayReleaseRuleDTO>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.update(&app_id, &cluster_name, &namespace_name, &branch_name, body.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_gray_release_rule(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>, operator: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.delete(&app_id, &cluster_name, &namespace_name, &branch_name, op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

// ---- NamespaceBranch (gray release branch) admin endpoints ----

async fn create_branch_admin(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = NamespaceBranchService::new(data.get_ref().clone());
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

async fn get_branch_admin(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = NamespaceBranchService::new(data.get_ref().clone());
    match service.find_branch(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(ns)) => HttpResponse::Ok().json(ns),
        Ok(None) => HttpResponse::Ok().json(serde_json::Value::Null),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_branch_rules_admin(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.get(&app_id, &cluster_name, &namespace_name, &branch_name).await {
        Ok(Some(rule)) => HttpResponse::Ok().json(rule),
        Ok(None) => HttpResponse::Ok().json(serde_json::Value::Null),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn update_branch_rules_admin(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    body: web::Json<GrayReleaseRuleDTO>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name, branch_name) = path.into_inner();
    let service = GrayReleaseRuleService::new(data.get_ref().clone());
    match service.update(&app_id, &cluster_name, &namespace_name, &branch_name, body.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_branch_admin(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name, _branch_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = NamespaceBranchService::new(data.get_ref().clone());
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

// ---- FR-2: Release variant endpoints ----

/// GET /releases/{releaseId} — upstream ReleaseController.findOne.
async fn get_release_by_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    release_id: web::Path<i64>,
) -> impl Responder {
    let service = ReleaseService::new(data.get_ref().clone());
    match service.get_by_id(release_id.into_inner()).await {
        Ok(Some(release)) => HttpResponse::Ok().json(release),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Release not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// GET .../releases/all — all releases (incl. abandoned) for a namespace.
async fn get_all_releases(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_all_releases(&app_id, &cluster_name, &namespace_name).await {
        Ok(all) => {
            let total = all.len();
            let content: Vec<_> = all.into_iter().skip(page * size).take(size).collect();
            HttpResponse::Ok().json(serde_json::json!({
                "content": content,
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

/// GET .../releases/active — the current effective (non-abandoned) release.
async fn get_active_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ReleaseService::new(data.get_ref().clone());
    match service.get_latest_active(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(release)) => HttpResponse::Ok().json(release),
        Ok(None) => HttpResponse::Ok().json(serde_json::Value::Null),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// GET .../releases/latest — the latest release (same as active in batata).
async fn get_latest_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ReleaseService::new(data.get_ref().clone());
    match service.get_latest_active(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(release)) => HttpResponse::Ok().json(release),
        Ok(None) => HttpResponse::Ok().json(serde_json::Value::Null),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// POST .../gray-del-releases — upstream gray release deletion (rollback gray).
/// Marks the gray release abandoned and clears the active gray rule.
async fn gray_del_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let release_id = query.get("releaseId").and_then(|v| v.as_i64());

    let service = NamespaceBranchService::new(data.get_ref().clone());
    match service
        .delete_branch(&app_id, &cluster_name, &namespace_name, operator, true)
        .await
    {
        Ok(_) => {
            let _ = release_id;
            HttpResponse::Ok().finish()
        }
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// ---- FR-3: ReleaseHistory endpoints ----

/// GET .../releases/histories — release history for a namespace.
async fn get_release_histories(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20);
    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_release_history(&app_id, &cluster_name, &namespace_name, page, size).await {
        Ok((histories, total)) => HttpResponse::Ok().json(serde_json::json!({
            "content": histories,
            "total": total,
            "page": page,
            "size": size,
        })),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// GET /releases/histories/by_release_id_and_operation — filter by release id + operation.
async fn get_release_history_by_release_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let release_id = query.get("releaseId").and_then(|v| v.as_i64()).unwrap_or(0);
    let operation = query.get("operation").and_then(|v| v.as_i64()).map(|v| v as i32);
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let service = ReleaseService::new(data.get_ref().clone());
    // Fetch a large page and filter in memory (release history is per-namespace small).
    match service.find_release_history("", "", "", 0, 1000).await {
        Ok((all, _)) => {
            let filtered: Vec<_> = all
                .into_iter()
                .filter(|h| h.release_id == release_id)
                .filter(|h| operation.map_or(true, |op| h.operation == op))
                .collect();
            let total = filtered.len();
            let content: Vec<_> = filtered.into_iter().skip(page * size).take(size).collect();
            HttpResponse::Ok().json(serde_json::json!({
                "content": content,
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

/// GET /releases/histories/by_previous_release_id_and_operation — filter by previous release id.
async fn get_release_history_by_previous_release_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let previous_release_id = query.get("previousReleaseId").and_then(|v| v.as_i64()).unwrap_or(0);
    let operation = query.get("operation").and_then(|v| v.as_i64()).map(|v| v as i32);
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_release_history("", "", "", 0, 1000).await {
        Ok((all, _)) => {
            let filtered: Vec<_> = all
                .into_iter()
                .filter(|h| h.previous_release_id == previous_release_id)
                .filter(|h| operation.map_or(true, |op| h.operation == op))
                .collect();
            let total = filtered.len();
            let content: Vec<_> = filtered.into_iter().skip(page * size).take(size).collect();
            HttpResponse::Ok().json(serde_json::json!({
                "content": content,
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

// ---- FR-4: Item by ID endpoints ----

/// GET /items/{itemId} — upstream ItemController.findOne by id.
async fn get_item_by_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    item_id: web::Path<i64>,
) -> impl Responder {
    let service = ItemService::new(data.get_ref().clone());
    match service.get_by_id(item_id.into_inner()).await {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Item not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// DELETE /items/{itemId} — upstream ItemController.delete by id.
async fn delete_item_by_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    item_id: web::Path<i64>,
    query: web::Query<Value>,
) -> impl Responder {
    let op = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ItemService::new(data.get_ref().clone());
    match service.delete(item_id.into_inner(), op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

/// POST .../comment_items — upstream batch item update with a release comment.
/// Body: { createItems, updateItems, deleteItems, comment, operator }
async fn comment_items(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    body: web::Json<Value>,
) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let comment = body.get("comment").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let _operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");

    let change_sets = ItemChangeSets {
        create_items: body
            .get("createItems")
            .and_then(|v| serde_json::from_value::<Vec<ItemDTO>>(v.clone()).ok())
            .unwrap_or_default(),
        update_items: body
            .get("updateItems")
            .and_then(|v| serde_json::from_value::<Vec<ItemDTO>>(v.clone()).ok())
            .unwrap_or_default(),
        delete_items: body
            .get("deleteItems")
            .and_then(|v| serde_json::from_value::<Vec<ItemDTO>>(v.clone()).ok())
            .unwrap_or_default(),
    };

    let service = ItemSetService::new(data.get_ref().clone());
    match service
        .update_set(&app_id, &cluster_name, &namespace_name, change_sets)
        .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "comment": comment })),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// ---- FR-5: Namespace by ID & App Unique ----

/// GET /namespaces/{namespaceId} — upstream NamespaceController.findOne by id.
async fn get_namespace_by_id(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    namespace_id: web::Path<i64>,
) -> impl Responder {
    use crate::persistence::traits::NamespacePersistence;
    match NamespacePersistence::get(data.get_ref(), namespace_id.into_inner()).await {
        Ok(Some(ns)) => HttpResponse::Ok().json(NamespaceDTO {
            app_id: ns.app_id,
            cluster_name: ns.cluster_name,
            namespace_name: ns.namespace_name,
            format: Some(ns.format),
            is_public: Some(ns.is_public),
            comment: ns.comment,
            data_change_created_by: Some(ns.data_change_created_by),
            data_change_created_time: Some(ns.data_change_created_time.to_string()),
            data_change_last_modified_by: ns.data_change_last_modified_by,
            data_change_last_time: ns.data_change_last_time.map(|t| t.to_string()),
        }),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Namespace not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// GET /apps/{appId}/unique — upstream AppController.isAppIdUnique.
async fn check_app_unique(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
) -> impl Responder {
    use crate::persistence::traits::AppPersistence;
    match AppPersistence::get(data.get_ref(), app_id.as_str()).await {
        Ok(Some(_)) => HttpResponse::Ok().json(false),
        Ok(None) => HttpResponse::Ok().json(true),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// ---- FR-6: AppNamespace association queries ----

/// GET /appnamespaces/{publicNamespaceName}/namespaces — list all namespaces
/// (across apps) associated with a public namespace.
async fn list_associated_namespaces(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    public_namespace_name: web::Path<String>,
) -> impl Responder {
    use crate::persistence::traits::NamespacePersistence;
    let name = public_namespace_name.into_inner();
    let all = match NamespacePersistence::list_all(data.get_ref()).await {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    };
    let associated: Vec<_> = all
        .into_iter()
        .filter(|ns| !ns.is_deleted && ns.namespace_name == name)
        .map(|ns| serde_json::json!({
            "id": ns.id,
            "appId": ns.app_id,
            "clusterName": ns.cluster_name,
            "namespaceName": ns.namespace_name,
            "isPublic": ns.is_public,
            "format": ns.format,
        }))
        .collect();
    HttpResponse::Ok().json(associated)
}

/// GET /appnamespaces/{publicNamespaceName}/associated-namespaces/count
async fn count_associated_namespaces(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    public_namespace_name: web::Path<String>,
) -> impl Responder {
    use crate::persistence::traits::NamespacePersistence;
    let name = public_namespace_name.into_inner();
    let all = match NamespacePersistence::list_all(data.get_ref()).await {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    };
    let count = all
        .into_iter()
        .filter(|ns| !ns.is_deleted && ns.namespace_name == name)
        .count();
    HttpResponse::Ok().json(serde_json::json!({ "count": count }))
}

async fn list_server_configs(data: web::Data<Arc<dyn ApolloPersistenceService>>) -> impl Responder {
    let service = ServerConfigService::new(data.get_ref().clone());
    match service.list().await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_server_config(data: web::Data<Arc<dyn ApolloPersistenceService>>, key: web::Path<String>) -> impl Responder {
    let service = ServerConfigService::new(data.get_ref().clone());
    match service.get(key.as_str()).await {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("Server config not found: {}", key),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_server_config(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<ServerConfigDTO>) -> impl Responder {
    let service = ServerConfigService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(config) => HttpResponse::Ok().json(config),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn update_server_config(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<ServerConfigDTO>) -> impl Responder {
    let dto = body.into_inner();
    let service = ServerConfigService::new(data.get_ref().clone());
    let operator = dto.data_change_created_by.clone().unwrap_or_else(|| "admin".to_string());
    match service.update(&dto.key, &dto.value, &operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_server_config(data: web::Data<Arc<dyn ApolloPersistenceService>>, key: web::Path<String>, operator: web::Query<Value>) -> impl Responder {
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ServerConfigService::new(data.get_ref().clone());
    match service.delete(key.as_str(), op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn list_app_namespaces(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>) -> impl Responder {
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.list_by_app(app_id.as_str()).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn get_app_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>) -> impl Responder {
    let (app_id, name) = path.into_inner();
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.get(&app_id, &name).await {
        Ok(Some(ns)) => HttpResponse::Ok().json(ns),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!("AppNamespace not found: {}/{}", app_id, name),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_app_namespace_admin(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<AppNamespaceDTO>) -> impl Responder {
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(ns) => HttpResponse::Ok().json(ns),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_app_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>, operator: web::Query<Value>) -> impl Responder {
    let (app_id, name) = path.into_inner();
    let op = operator.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.delete(&app_id, &name, op).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn lock_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let locked_by = query.get("lockedBy").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = NamespaceLockService::new(data.get_ref().clone());
    match service.lock(&app_id, &cluster_name, &namespace_name, locked_by).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn unlock_namespace(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let locked_by = query.get("lockedBy").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = NamespaceLockService::new(data.get_ref().clone());
    match service.unlock(&app_id, &cluster_name, &namespace_name, locked_by).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_namespace_lock(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = NamespaceLockService::new(data.get_ref().clone());
    match service.get_lock(&app_id, &cluster_name, &namespace_name).await {
        Ok(Some(lock)) => HttpResponse::Ok().json(serde_json::json!({
            "appId": lock.app_id,
            "clusterName": lock.cluster_name,
            "namespaceName": lock.namespace_name,
            "isLocked": true,
            "lockedBy": lock.locked_by.clone(),
        })),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Namespace lock not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn update_item_set(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, body: web::Json<ItemChangeSets>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ItemSetService::new(data.get_ref().clone());
    match service.update_set(&app_id, &cluster_name, &namespace_name, body.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn rollback_release_admin(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, i64)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, release_id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ReleaseService::new(data.get_ref().clone());
    match service.rollback(&app_id, &cluster_name, &namespace_name, release_id, operator).await {
        Ok(release) => HttpResponse::Ok().json(release),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_releases(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0);
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(10);
    let service = ReleaseService::new(data.get_ref().clone());
    match service.find_active_releases(&app_id, &cluster_name, &namespace_name, page, size).await {
        Ok((releases, total)) => {
            HttpResponse::Ok().json(serde_json::json!({
                "content": releases,
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

async fn merge_branch_and_release(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, body: web::Json<NamespaceGrayReleaseDTO>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let req = body.into_inner();
    let service = ReleaseService::new(data.get_ref().clone());
    let release_comment = if req.release_comment.is_empty() { None } else { Some(req.release_comment) };
    match service.merge_branch_and_release_full(
        &app_id,
        &cluster_name,
        &namespace_name,
        &req.branch_name,
        &req.release_title,
        release_comment,
        &req.released_by,
        req.is_emergency_publish,
        req.change_sets,
        req.delete_branch,
    ).await {
        Ok(release) => HttpResponse::Ok().json(release),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_instance_configs(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
    let instance_id = query.get("instanceId").and_then(|v| v.as_i64()).unwrap_or(0);
    let service = InstanceConfigService::new(data.get_ref().clone());
    match service.get_by_instance(instance_id).await {
        Ok(configs) => HttpResponse::Ok().json(configs),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// ADMSVC-001: instances that fetched a given release.
/// Upstream resolves the Release → its releaseKey → matching configs.
async fn instances_by_release(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let release_id = query.get("releaseId").and_then(|v| v.as_i64()).unwrap_or(0);
    let release = match <dyn crate::persistence::traits::ReleasePersistence>::get_by_release_id(
        data.get_ref(), release_id,
    ).await {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::Ok().json(Vec::<Value>::new()),
        Err(e) => return internal_error(e),
    };
    let service = InstanceConfigService::new(data.get_ref().clone());
    let all = match service.list_by_app_cluster(&release.app_id, &release.cluster_name, &release.namespace_name).await {
        Ok(v) => v, Err(e) => return internal_error(e),
    };
    let matched: Vec<&crate::api::dto::InstanceConfigDTO> = all.iter().filter(|c| c.release_key == release.release_key).collect();
    HttpResponse::Ok().json(with_instances(data, matched).await)
}

/// ADMSVC-004: configs whose delivered releaseKey is NOT in `releaseIds`
/// (instances that have not synced to one of these releases).
async fn instances_not_in_releases(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let app_id = query.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let cluster = query.get("clusterName").and_then(|v| v.as_str()).unwrap_or("default");
    let ns = query.get("namespaceName").and_then(|v| v.as_str()).unwrap_or("");
    let release_ids = query
        .get("releaseIds")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect::<Vec<_>>();

    let release_svc = ReleaseService::new(data.get_ref().clone());
    let mut keys = std::collections::HashSet::new();
    for id in release_ids {
        if let Ok(Some(r)) = release_svc.get_by_id(id).await {
            keys.insert(r.release_key);
        }
    }
    let service = InstanceConfigService::new(data.get_ref().clone());
    let all = match service.list_by_app_cluster(app_id, cluster, ns).await {
        Ok(v) => v, Err(e) => return internal_error(e),
    };
    let matched: Vec<&crate::api::dto::InstanceConfigDTO> =
        all.iter().filter(|c| !keys.contains(&c.release_key)).collect();
    HttpResponse::Ok().json(with_instances(data, matched).await)
}

/// ADMSVC-003: count of distinct instances using a namespace.
async fn instances_by_namespace_count(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster, ns) = namespace_params(&query);
    let service = InstanceConfigService::new(data.get_ref().clone());
    let all = match service.list_by_app_cluster(&app_id, &cluster, &ns).await {
        Ok(v) => v, Err(e) => return internal_error(e),
    };
    let count = all.iter().map(|c| c.instance_id).collect::<std::collections::HashSet<_>>().len();
    HttpResponse::Ok().json(serde_json::json!({ "count": count }))
}

/// ADMSVC-002: full instance-config rows for one namespace.
async fn instances_by_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let (app_id, cluster, ns) = namespace_params(&query);
    let service = InstanceConfigService::new(data.get_ref().clone());
    let all = match service.list_by_app_cluster(&app_id, &cluster, &ns).await {
        Ok(v) => v, Err(e) => return internal_error(e),
    };
    HttpResponse::Ok().json(with_instances(data, all.iter().collect()).await)
}

fn namespace_params(query: &web::Query<Value>) -> (String, String, String) {
    (
        query.get("appId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        query.get("clusterName").and_then(|v| v.as_str()).unwrap_or("default").to_string(),
        query.get("namespaceName").and_then(|v| v.as_str()).unwrap_or("application").to_string(),
    )
}

fn internal_error(e: anyhow::Error) -> actix_web::HttpResponse {
    HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() })
}

/// Join instance rows so responses carry appId/ip/dataCenter like upstream.
async fn with_instances(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    configs: Vec<&crate::api::dto::InstanceConfigDTO>,
) -> Vec<Value> {
    use crate::persistence::traits::InstancePersistence;
    let instance_map: std::collections::HashMap<i64, crate::persistence::shared::StoredInstance> =
        InstancePersistence::list_all(data.get_ref())
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|i| (i.id, i))
            .collect();
    let mut out = Vec::with_capacity(configs.len());
    for c in configs {
        let instance = instance_map.get(&c.instance_id);
        out.push(serde_json::json!({
            "instance": instance.as_ref().map(|i| serde_json::json!({
                "appId": i.app_id,
                "clusterName": i.cluster_name,
                "dataCenter": i.data_center,
                "ip": i.ip,
            })),
            "configAppId": c.config_app_id.clone().unwrap_or_default(),
            "namespaceName": c.namespace_name,
            "clusterName": c.cluster_name,
            "releaseKey": c.release_key,
            "fetchTime": c.data_change_last_time.clone(),
        }));
    }
    out
}

/// PORT-012 — upstream SearchController.search: filter apps by id/name.
async fn search_apps_by_id_or_name(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let q = query.get("query").and_then(|v| v.as_str()).unwrap_or("").trim().to_lowercase();
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let service = crate::service::AppService::new(data.get_ref().clone());
    let all = match service.list().await {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError().json(ErrorResponse { status: 500, message: e.to_string() }),
    };
    let mut filtered: Vec<_> = all
        .into_iter()
        .filter(|a| {
            q.is_empty()
                || a.app_id.to_lowercase().contains(&q)
                || a.name.to_lowercase().contains(&q)
        })
        .map(|a| serde_json::json!({
            "appId": a.app_id, "name": a.name, "orgId": a.org_id,
            "orgName": a.org_name, "ownerName": a.owner_name, "ownerEmail": a.owner_email,
        }))
        .collect();
    let total = filtered.len();
    let start = (page * size).min(total);
    let end = ((page + 1) * size).min(total);
    let content = filtered.split_off(start); // reuse alloc; end clamped
    let content: Vec<_> = content.into_iter().take(end - start).collect();
    HttpResponse::Ok().json(serde_json::json!({
        "content": content,
        "total": total,
        "page": page,
        "size": size,
    }))
}

/// ADM-011 — upstream ClusterController.isAppIdUnique.
async fn cluster_name_unique(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    use crate::persistence::traits::ClusterPersistence;
    let (app_id, cluster_name) = path.into_inner();
    let exists = ClusterPersistence::list(data.get_ref(), &app_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .any(|c| !c.is_deleted && c.name == cluster_name);
    HttpResponse::Ok().json(!exists)
}

/// ADM-017 — upstream NamespaceController.findByItem:
/// items with this key → their namespaces, paged.
async fn find_namespaces_by_item(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    use crate::persistence::traits::{ItemPersistence, NamespacePersistence};
    let item_key = query.get("itemKey").and_then(|v| v.as_str()).unwrap_or("");
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    if item_key.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: "itemKey is required".into() });
    }
    let ns_ids = ItemPersistence::find_namespace_ids_by_item_key(data.get_ref(), item_key)
        .await
        .unwrap_or_default();
    let mut namespaces = Vec::new();
    for id in ns_ids {
        if let Ok(Some(ns)) = NamespacePersistence::get(data.get_ref(), id).await {
            namespaces.push(serde_json::json!({
                "id": ns.id,
                "appId": ns.app_id,
                "clusterName": ns.cluster_name,
                "namespaceName": ns.namespace_name,
            }));
        }
    }
    let total = namespaces.len();
    let start = (page * size).min(total);
    let end = ((page + 1) * size).min(total);
    HttpResponse::Ok().json(serde_json::json!({
        "content": &namespaces[start..end],
        "total": total,
        "page": page,
        "size": size,
    }))
}

/// ADM-019 — upstream namespacePublishInfo: clusterName → true when any of
/// its namespaces has unpublished item changes.
async fn namespaces_publish_info(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<String>,
) -> impl Responder {
    let app_id = path.into_inner();
    match publish_info_map(data.get_ref(), &app_id).await {
        Ok(map) => HttpResponse::Ok().json(map),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse { status: 400, message: e.to_string() }),
    }
}

/// Core of ADM-019 / PORT-025: clusterName → has-unpublished-changes.
pub(crate) async fn publish_info_map(
    persistence: &Arc<dyn ApolloPersistenceService>,
    app_id: &str,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    use crate::persistence::traits::{ClusterPersistence, ItemPersistence, NamespacePersistence, ReleasePersistence};
    let clusters: Vec<_> = ClusterPersistence::list(persistence, app_id)
        .await?
        .into_iter()
        .filter(|c| !c.is_deleted && c.parent_cluster_id == 0)
        .collect();
    if clusters.is_empty() {
        return Err(anyhow::anyhow!("App not found: {}", app_id));
    }
    let mut result = serde_json::Map::new();
    for cluster in clusters {
        let mut has_unpublished = false;
        'outer: for ns in NamespacePersistence::list_by_app(persistence, app_id).await.unwrap_or_default() {
            if ns.is_deleted || ns.cluster_name != cluster.name {
                continue;
            }
            let latest = ReleasePersistence::get_latest(persistence, app_id, &ns.cluster_name, &ns.namespace_name).await.ok().flatten();
            match latest {
                None => {
                    if !ItemPersistence::list_by_namespace(persistence, ns.id).await.unwrap_or_default().is_empty() {
                        has_unpublished = true;
                        break 'outer;
                    }
                }
                Some(rel) => {
                    let publish_ms = rel.data_change_last_time.unwrap_or(rel.data_change_created_time);
                    for item in ItemPersistence::list_by_namespace(persistence, ns.id).await.unwrap_or_default() {
                        if item.data_change_last_time.unwrap_or(item.data_change_created_time) > publish_ms {
                            has_unpublished = true;
                            break 'outer;
                        }
                    }
                }
            }
        }
        result.insert(cluster.name.clone(), serde_json::Value::Bool(has_unpublished));
    }
    Ok(result)
}


/// ADM-018 — findPublicNamespaceForAssociatedNamespace:
/// default cluster → the owner's namespace; custom cluster without a release
/// → fall back to owner's default-cluster namespace; else the custom one.
async fn associated_public_namespace(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
) -> impl Responder {
    use crate::persistence::traits::{NamespacePersistence, ReleasePersistence};
    let (_app_id, cluster_name, namespace_name) = path.into_inner();

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

async fn list_public_app_namespaces(data: web::Data<Arc<dyn ApolloPersistenceService>>) -> impl Responder {
    let service = AppNamespaceService::new(data.get_ref().clone());
    match service.list_public().await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn list_audit(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
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

async fn list_audit_by_entity(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
    let entity_name = query.get("entityName").and_then(|v| v.as_str()).unwrap_or("");
    let entity_id = query.get("entityId").and_then(|v| v.as_str()).unwrap_or("");
    let service = AuditService::new(data.get_ref().clone());
    match service.list_by_entity(entity_name, entity_id).await {
        Ok(audits) => HttpResponse::Ok().json(audits),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn list_permissions(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
    let target_id = query.get("targetId").and_then(|v| v.as_str()).unwrap_or("");
    let service = PermissionService::new(data.get_ref().clone());
    match service.list_by_target(target_id).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_role(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<RoleDTO>) -> impl Responder {
    let service = RoleService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(role) => HttpResponse::Ok().json(role),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_role(data: web::Data<Arc<dyn ApolloPersistenceService>>, id: web::Path<i64>) -> impl Responder {
    let service = RoleService::new(data.get_ref().clone());
    match service.delete(id.into_inner()).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn list_user_roles(data: web::Data<Arc<dyn ApolloPersistenceService>>, user_id: web::Path<String>) -> impl Responder {
    let service = RoleService::new(data.get_ref().clone());
    match service.list_user_roles(user_id.as_str()).await {
        Ok(roles) => HttpResponse::Ok().json(roles),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn assign_role_to_user(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i64)>, query: web::Query<Value>) -> impl Responder {
    let (user_id, role_id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = RoleService::new(data.get_ref().clone());
    match service.assign_role_to_user(&user_id, role_id, operator).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn remove_role_from_user(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i64)>) -> impl Responder {
    let (user_id, role_id) = path.into_inner();
    let service = RoleService::new(data.get_ref().clone());
    match service.remove_role_from_user(&user_id, role_id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn create_consumer(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<ConsumerDTO>) -> impl Responder {
    let service = ConsumerService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(consumer) => HttpResponse::Ok().json(consumer),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn get_consumer(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>) -> impl Responder {
    let service = ConsumerService::new(data.get_ref().clone());
    match service.get_by_app(app_id.as_str()).await {
        Ok(Some(consumer)) => HttpResponse::Ok().json(consumer),
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: "Consumer not found".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_consumer_token(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<i64>, query: web::Query<Value>) -> impl Responder {
    let consumer_id = path.into_inner();
    let created_by = query.get("createdBy").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = ConsumerTokenService::new(data.get_ref().clone());
    match service.create(consumer_id, created_by).await {
        Ok(token) => HttpResponse::Ok().json(token),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_consumer_tokens(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<i64>) -> impl Responder {
    let consumer_id = path.into_inner();
    let service = ConsumerTokenService::new(data.get_ref().clone());
    match service.list_by_consumer(consumer_id).await {
        Ok(tokens) => HttpResponse::Ok().json(tokens),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn delete_consumer_token(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(i64, i64)>) -> impl Responder {
    let (consumer_id, token_id) = path.into_inner();
    let _ = consumer_id;
    let service = ConsumerTokenService::new(data.get_ref().clone());
    match service.delete(token_id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn export_configs(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let item_service = ItemService::new(data.get_ref().clone());
    match item_service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(items) => {
            let export = crate::api::dto::ConfigExportDTO {
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

async fn import_configs(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<ConfigImportDTO>) -> impl Responder {
    let dto = body.into_inner();
    let item_set_service = ItemSetService::new(data.get_ref().clone());
    let change_sets = ItemChangeSets {
        create_items: dto.items,
        update_items: vec![],
        delete_items: vec![],
    };
    match item_set_service.update_set(&dto.app_id, &dto.cluster_name, &dto.namespace_name, change_sets).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_favorites(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
    let user_id = query.get("userId").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = FavoriteService::new(data.get_ref().clone());
    match service.list_by_user(user_id).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn create_favorite(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<FavoriteDTO>) -> impl Responder {
    let service = FavoriteService::new(data.get_ref().clone());
    match service.create(body.into_inner()).await {
        Ok(favorite) => HttpResponse::Ok().json(favorite),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn delete_favorite(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<i64>, query: web::Query<Value>) -> impl Responder {
    let id = path.into_inner();
    let user_id = query.get("userId").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = FavoriteService::new(data.get_ref().clone());
    match service.delete(id, user_id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn search(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
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

// F-APO-ITEM-011: `/items-search/key-and-value`.
//
// Upstream `ItemController.getItemInfoBySearch` (lines 233-239) wraps the
// matches in a `PageDTO<ItemInfoDTO>` (`{total, content, page, size}`) rather
// than returning a bare array, which is what the `/search` alias keeps doing.
async fn search_key_and_value(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
    let app_id = query.get("appId").and_then(|v| v.as_str());
    let cluster_name = query.get("clusterName").and_then(|v| v.as_str());
    let key = query.get("key").and_then(|v| v.as_str());
    let value = query.get("value").and_then(|v| v.as_str());
    // Upstream `Pageable` defaults to page 0 with a size of 20.
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let service = SearchService::new(data.get_ref().clone());
    let result = match (app_id, cluster_name) {
        (Some(app), Some(cluster)) => service.search_items(app, cluster, key, value).await,
        _ => service.search_across_apps(key, value).await,
    };

    match result {
        Ok(results) => {
            let total = results.len();
            let content: Vec<_> = results.into_iter().skip(page * size).take(size).collect();

            HttpResponse::Ok().json(serde_json::json!({
                "total": total,
                "content": content,
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

async fn sync_configs(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<Value>) -> impl Responder {
    let source_app_id = body.get("sourceAppId").and_then(|v| v.as_str()).unwrap_or("");
    let source_cluster = body.get("sourceCluster").and_then(|v| v.as_str()).unwrap_or("default");
    let source_namespace = body.get("sourceNamespace").and_then(|v| v.as_str()).unwrap_or("application");
    let target_app_id = body.get("targetAppId").and_then(|v| v.as_str()).unwrap_or("");
    let target_cluster = body.get("targetCluster").and_then(|v| v.as_str()).unwrap_or("default");
    let target_namespace = body.get("targetNamespace").and_then(|v| v.as_str()).unwrap_or("application");
    let operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let overwrite = body.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);

    let service = crate::service::ConfigSyncService::new(data.get_ref().clone());
    match service.sync_configs(source_app_id, source_cluster, source_namespace, target_app_id, target_cluster, target_namespace, operator, overwrite).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn sync_app_all_namespaces(data: web::Data<Arc<dyn ApolloPersistenceService>>, body: web::Json<Value>) -> impl Responder {
    let source_app_id = body.get("sourceAppId").and_then(|v| v.as_str()).unwrap_or("");
    let source_cluster = body.get("sourceCluster").and_then(|v| v.as_str()).unwrap_or("default");
    let target_app_id = body.get("targetAppId").and_then(|v| v.as_str()).unwrap_or("");
    let target_cluster = body.get("targetCluster").and_then(|v| v.as_str()).unwrap_or("default");
    let operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let overwrite = body.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);

    let service = crate::service::ConfigSyncService::new(data.get_ref().clone());
    match service.sync_app_all_namespaces(source_app_id, source_cluster, target_app_id, target_cluster, operator, overwrite).await {
        Ok(results) => HttpResponse::Ok().json(results),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

// F-APO-ADM-011: Cluster name uniqueness check
async fn check_cluster_unique(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String)>) -> impl Responder {
    let (app_id, cluster_name) = path.into_inner();
    let service = ClusterService::new(data.get_ref().clone());
    match service.get(&app_id, &cluster_name).await {
        Ok(Some(_)) => HttpResponse::Ok().json(false),
        Ok(None) => HttpResponse::Ok().json(true),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// F-APO-ITEM-008: Get item by base64 URL-encoded key
async fn get_item_encoded(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name, encoded_key) = path.into_inner();
    let key = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&encoded_key)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&encoded_key)) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return HttpResponse::BadRequest().json(ErrorResponse {
                status: 400,
                message: "Invalid base64 encoded key".to_string(),
            }),
        },
        Err(_) => return HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: "Invalid base64 encoded key".to_string(),
        }),
    };
    let service = ItemService::new(data.get_ref().clone());
    match service.get_by_key(&app_id, &cluster_name, &namespace_name, &key).await {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
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

// F-APO-ITEM-009: Paged items listing
async fn list_items_with_page(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    // Upstream `Pageable` defaults to page 0 with a size of 20.
    let page = query.get("page").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let size = query.get("size").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let service = ItemService::new(data.get_ref().clone());
    match service.list(&app_id, &cluster_name, &namespace_name).await {
        Ok(all_items) => {
            let total = all_items.len();
            let content: Vec<ItemDTO> = all_items.into_iter().skip(page * size).take(size).collect();
            HttpResponse::Ok().json(serde_json::json!({
                "page": page,
                "size": size,
                "total": total,
                "content": content,
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// F-APO-ITEM-006: items deleted since the latest active release.
//
// Upstream `ItemController.findDeletedItems` (lines 207-231): the result is
// derived from the commits created at/after the latest release, NOT from the
// set of currently soft-deleted items.
async fn get_deleted_items(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String)>) -> impl Responder {
    let (app_id, cluster_name, namespace_name) = path.into_inner();
    let service = ItemService::new(data.get_ref().clone());
    match service.list_deleted_since_last_release(&app_id, &cluster_name, &namespace_name).await {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

// F-APO-ADMSVC-005: Access key CRUD handlers
async fn create_access_key_admin(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>, body: web::Json<Value>) -> impl Responder {
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

async fn list_access_keys_admin(data: web::Data<Arc<dyn ApolloPersistenceService>>, app_id: web::Path<String>) -> impl Responder {
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

async fn delete_access_key_admin(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i64)>, query: web::Query<Value>) -> impl Responder {
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

// F-APO-ADMSVC-006: Enable/disable access key handlers
async fn enable_access_key(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i64)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.enable(&app_id, id, operator).await {
        Ok(key) => HttpResponse::Ok().json(key),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

async fn disable_access_key(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i64)>, query: web::Query<Value>) -> impl Responder {
    let (app_id, id) = path.into_inner();
    let operator = query.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let service = AccessKeyService::new(data.get_ref().clone());
    match service.disable(&app_id, id, operator).await {
        Ok(key) => HttpResponse::Ok().json(key),
        Err(e) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: e.to_string(),
        }),
    }
}

// ---- FR-8: Portal auxiliary endpoints ----

/// GET /system-info — upstream SystemInfoController.
async fn system_info(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
) -> impl Responder {
    let _ = data;
    HttpResponse::Ok().json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "gitCommit": "unknown",
        "buildTime": "unknown",
    }))
}

/// GET /system-info/health
async fn system_health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "UP" }))
}

/// GET /organizations — upstream OrganizationController.
async fn list_organizations() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!([
        { "orgId": "DEFAULT", "orgName": "Default Organization" }
    ]))
}

/// GET /page-settings — upstream PageSettingController.
async fn page_settings() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "wikiAddress": "",
        "createAppFeatureEnabled": true,
    }))
}

/// GET /prefix-path — upstream returns the configured context path prefix.
async fn prefix_path() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "prefixPath": "" }))
}

/// POST /users — upstream UserController.createOrUpdate.
async fn create_user(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<UserDTO>,
) -> impl Responder {
    use crate::persistence::traits::UserPersistence;
    let dto = body.into_inner();
    match UserPersistence::create_user(data.get_ref(), dto).await {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

/// PUT /users/enabled — upstream UserController.enableUser / disableUser.
async fn update_user_enabled(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    use crate::persistence::traits::UserPersistence;
    let username = query.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let enabled = query.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    match UserPersistence::get_user(data.get_ref(), username).await {
        Ok(Some(mut user)) => {
            user.enabled = enabled;
            let dto = UserDTO {
                id: Some(user.id),
                username: user.username.clone(),
                password: String::new(),
                email: Some(user.email.clone()),
                enabled: user.enabled,
                data_change_created_by: None,
                data_change_created_time: None,
            };
            match UserPersistence::update_user(data.get_ref(), username, dto).await {
                Ok(u) => HttpResponse::Ok().json(u),
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

/// POST /apps/{appId}/initPermission — upstream PermissionController.initAppPermission.
async fn init_app_permission(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    app_id: web::Path<String>,
    body: web::Json<Value>,
) -> impl Responder {
    let app_id = app_id.into_inner();
    let operator = body.get("operator").and_then(|v| v.as_str()).unwrap_or("admin");
    let permission_service = PermissionService::new(data.get_ref().clone());
    // Upstream creates AppMaster + CreateCluster + CreateNamespace per app.
    let target_id = format!("App+{}", app_id);
    let types = [1, 2, 3];
    for pt in types {
        let _ = permission_service.create(pt, &target_id, operator).await;
    }
    HttpResponse::Ok().json(serde_json::json!({ "appId": app_id }))
}

/// GET /apps/{appId}/permissions/{permissionType} — upstream PermissionController.
async fn get_app_permissions(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String)>,
) -> impl Responder {
    let (app_id, permission_type) = path.into_inner();
    let pt = permission_type.parse::<i32>().unwrap_or(0);
    let service = PermissionService::new(data.get_ref().clone());
    let target_id = format!("App+{}", app_id);
    match service.list_by_target(&target_id).await {
        Ok(perms) => {
            let filtered: Vec<_> = perms.into_iter().filter(|p| p.permission_type == pt).collect();
            HttpResponse::Ok().json(filtered)
        }
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

/// Performs the `configure_admin_routes` operation.
pub fn configure_admin_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        web::resource("/apps").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_app))
            .route(web::get().to(list_apps))
    )
    .service(
        web::resource("/apps/{app_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_app))
            .route(web::put().to(update_app))
            .route(web::delete().to(delete_app))
    )
    .service(
        web::resource("/apps/{app_id}/clusters").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_clusters))
            .route(web::post().to(create_cluster))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_cluster))
            .route(web::delete().to(delete_cluster))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_namespace))
            .route(web::get().to(list_namespaces))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_namespace))
            .route(web::put().to(update_namespace))
            .route(web::delete().to(delete_namespace))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_item))
            .route(web::get().to(list_items))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/deleted").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_deleted_items))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/{key}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_item))
            .route(web::put().to(update_item))
            .route(web::delete().to(delete_item))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/commits").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_commits))
            .route(web::post().to(create_commit))
    )
    .service(
        web::resource("/commits/{id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_commit))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(publish_release))
            .route(web::get().to(list_releases))
    )
    // FR-2: Release variant endpoints — register static paths before {release_id}.
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/all").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_all_releases))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/active").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_active_release))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/latest").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_latest_release))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/histories").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_release_histories))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/gray-del-releases").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(gray_del_release))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/comment_items").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(comment_items))
    )
    // FR-3: Release history global filter endpoints
    .service(
        web::resource("/releases/histories/by_release_id_and_operation").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_release_history_by_release_id))
    )
    .service(
        web::resource("/releases/histories/by_previous_release_id_and_operation").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_release_history_by_previous_release_id))
    )
    .service(
        web::resource("/releases/{release_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_release_by_id))
    )
    // FR-4: Item by ID endpoints
    .service(
        web::resource("/items/{item_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_item_by_id))
            .route(web::delete().to(delete_item_by_id))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/gray-release-rules").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_gray_release_rules))
            .route(web::post().to(create_gray_release_rule))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/gray-release-rules/{branch_name}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_gray_release_rule))
            .route(web::put().to(update_gray_release_rule))
            .route(web::delete().to(delete_gray_release_rule))
    )
    // FR-1: NamespaceBranch (gray release branch) admin endpoints
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_branch_admin))
            .route(web::get().to(get_branch_admin))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}/rules").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_branch_rules_admin))
            .route(web::put().to(update_branch_rules_admin))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/branches/{branch_name}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::delete().to(delete_branch_admin))
    )
    .service(
        web::resource("/serverconfigs").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_server_configs))
            .route(web::post().to(create_server_config))
            .route(web::put().to(update_server_config))
    )
    .service(
        web::resource("/serverconfigs/{key}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_server_config))
            .route(web::delete().to(delete_server_config))
    )
    .service(
        web::resource("/apps/{app_id}/appnamespaces").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_app_namespaces))
            .route(web::post().to(create_app_namespace_admin))
    )
    .service(
        web::resource("/apps/{app_id}/appnamespaces/{name}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_app_namespace))
            .route(web::delete().to(delete_app_namespace))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/lock").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_namespace_lock))
            .route(web::post().to(lock_namespace))
            .route(web::delete().to(unlock_namespace))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/itemset").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(update_item_set))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/{release_id}/rollback").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(rollback_release_admin))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/updateAndPublish").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(merge_branch_and_release))
    )
    // PORT-012 (upstream portal SearchController, root scope)
    .service(
        web::resource("/apps/search/by-appid-or-name").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(search_apps_by_id_or_name))
    )
    // ADM-011: cluster name uniqueness
    .service(
        web::resource("/apps/{app_id}/cluster/{cluster_name}/unique").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(cluster_name_unique))
    )
    // ADM-017: reverse lookup namespaces holding an item key
    .service(
        web::resource("/namespaces/find-by-item").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(find_namespaces_by_item))
    )
    // ADM-019: per-cluster "has unpublished changes" map
    .service(
        web::resource("/apps/{app_id}/namespaces/publish_info").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(namespaces_publish_info))
    )
    // ADM-018
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/associated-public-namespace").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(associated_public_namespace))
    )
    .service(
        web::resource("/instance-configs").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_instance_configs))
    )
    // Upstream adminservice InstanceConfigController (ADMSVC-001..004)
    .service(
        web::resource("/instances/by-release").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(instances_by_release))
    )
    .service(
        web::resource("/instances/by-namespace-and-releases-not-in").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(instances_not_in_releases))
    )
    .service(
        web::resource("/instances/by-namespace/count").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(instances_by_namespace_count))
    )
    .service(
        web::resource("/instances/by-namespace").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(instances_by_namespace))
    )
    .service(
        web::resource("/appnamespaces").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_public_app_namespaces))
    )
    .service(
        web::resource("/audit").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_audit))
    )
    .service(
        web::resource("/audit/by-entity").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_audit_by_entity))
    )
    .service(
        web::resource("/permissions").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_permissions))
    )
    .service(
        web::resource("/roles").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_role))
    )
    .service(
        web::resource("/roles/{id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::delete().to(delete_role))
    )
    .service(
        web::resource("/users/{user_id}/roles").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_user_roles))
    )
    .service(
        web::resource("/users/{user_id}/roles/{role_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(assign_role_to_user))
            .route(web::delete().to(remove_role_from_user))
    )
    .service(
        web::resource("/consumers").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_consumer))
    )
    .service(
        web::resource("/consumers/{app_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_consumer))
    )
    .service(
        web::resource("/consumers/{consumer_id}/tokens").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_consumer_token))
            .route(web::get().to(list_consumer_tokens))
    )
    .service(
        web::resource("/consumers/{consumer_id}/tokens/{token_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::delete().to(delete_consumer_token))
    )
    .service(
        web::resource("/configs/{app_id}/{cluster_name}/{namespace_name}/export").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(export_configs))
    )
    .service(
        web::resource("/configs/import").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(import_configs))
    )
    .service(
        web::resource("/favorites").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_favorites))
            .route(web::post().to(create_favorite))
    )
    .service(
        web::resource("/favorites/{id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::delete().to(delete_favorite))
    )
    .service(
        web::resource("/search").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(search))
    )
    .service(
        web::resource("/configs/sync").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(sync_configs))
    )
    .service(
        web::resource("/configs/sync/app").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(sync_app_all_namespaces))
    )
    // F-APO-ADM-011: Cluster name uniqueness check (singular "cluster")
    .service(
        web::resource("/apps/{app_id}/cluster/{cluster_name}/unique").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(check_cluster_unique))
    )
    // F-APO-ITEM-008: Get item by base64 URL-encoded key
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/encodedItems/{key}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_item_encoded))
    )
    // F-APO-ITEM-009: Paged items listing
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items-with-page").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_items_with_page))
    )
    // F-APO-ITEM-011: upstream-standard search path; returns PageDTO.
    .service(
        web::resource("/items-search/key-and-value").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(search_key_and_value))
    )
    // F-APO-ADMSVC-005: Access key CRUD
    .service(
        web::resource("/apps/{app_id}/accesskeys").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_access_key_admin))
            .route(web::get().to(list_access_keys_admin))
    )
    .service(
        web::resource("/apps/{app_id}/accesskeys/{id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::delete().to(delete_access_key_admin))
    )
    // F-APO-ADMSVC-006: Enable/disable access key
    .service(
        web::resource("/apps/{app_id}/accesskeys/{id}/enable").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::put().to(enable_access_key))
    )
    .service(
        web::resource("/apps/{app_id}/accesskeys/{id}/disable").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::put().to(disable_access_key))
    )
    // FR-5: Namespace by ID & App unique check
    .service(
        web::resource("/namespaces/{namespace_id}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_namespace_by_id))
    )
    .service(
        web::resource("/apps/{app_id}/unique").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(check_app_unique))
    )
    // FR-6: AppNamespace association queries
    .service(
        web::resource("/appnamespaces/{public_namespace_name}/namespaces").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_associated_namespaces))
    )
    .service(
        web::resource("/appnamespaces/{public_namespace_name}/associated-namespaces/count").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(count_associated_namespaces))
    )
    // FR-8: Portal auxiliary endpoints
    .service(
        web::resource("/system-info").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(system_info))
    )
    .service(
        web::resource("/system-info/health").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(system_health))
    )
    .service(
        web::resource("/organizations").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(list_organizations))
    )
    .service(
        web::resource("/page-settings").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(page_settings))
    )
    .service(
        web::resource("/prefix-path").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(prefix_path))
    )
    .service(
        web::resource("/users").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(create_user))
    )
    .service(
        web::resource("/users/enabled").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::put().to(update_user_enabled))
    )
    .service(
        web::resource("/apps/{app_id}/initPermission").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::post().to(init_app_permission))
    )
    .service(
        web::resource("/apps/{app_id}/permissions/{permission_type}").wrap(crate::middleware::auth::AdminAuthMiddleware::new())
            .route(web::get().to(get_app_permissions))
    );
}