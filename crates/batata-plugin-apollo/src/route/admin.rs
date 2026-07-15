use actix_web::{web, HttpResponse, Responder};
use serde_json::Value;
use std::sync::Arc;
use crate::persistence::traits::ApolloPersistenceService;
use crate::service::{
    AppService, NamespaceService, ItemService, ItemSetService, ReleaseService, 
    ClusterService, CommitService, GrayReleaseRuleService, ServerConfigService, 
    AppNamespaceService, NamespaceLockService, InstanceConfigService,
    AuditService, ConsumerService, ConsumerTokenService, 
    PermissionService, RoleService, FavoriteService, SearchService,
};
use crate::api::dto::{
    AppDTO, NamespaceDTO, ItemDTO, ItemChangeSets, ErrorResponse, ClusterDTO,
    CommitDTO, GrayReleaseRuleDTO, ServerConfigDTO, AppNamespaceDTO,
    ConsumerDTO, RoleDTO, FavoriteDTO, ConfigImportDTO, NamespaceGrayReleaseDTO,
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
    match service.update_by_key(&app_id, &cluster_name, &namespace_name, &key, body.into_inner()).await {
        Ok(item) => HttpResponse::Ok().json(item),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
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

async fn get_commit(data: web::Data<Arc<dyn ApolloPersistenceService>>, id: web::Path<i32>) -> impl Responder {
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
            message: format!("Gray release rule not found"),
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
        Ok(Some(lock)) => HttpResponse::Ok().json(lock),
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

async fn rollback_release_admin(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, String, String, i32)>, query: web::Query<Value>) -> impl Responder {
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
    match service.merge_branch_and_release(
        &app_id,
        &cluster_name,
        &namespace_name,
        &req.branch_name,
        &req.release_title,
        release_comment,
        &req.released_by,
        req.is_emergency_publish,
        req.change_sets,
    ).await {
        Ok(release) => HttpResponse::Ok().json(release),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn list_instance_configs(data: web::Data<Arc<dyn ApolloPersistenceService>>, query: web::Query<Value>) -> impl Responder {
    let instance_id = query.get("instanceId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let service = InstanceConfigService::new(data.get_ref().clone());
    match service.get_by_instance(instance_id).await {
        Ok(configs) => HttpResponse::Ok().json(configs),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
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

async fn delete_role(data: web::Data<Arc<dyn ApolloPersistenceService>>, id: web::Path<i32>) -> impl Responder {
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

async fn assign_role_to_user(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i32)>, query: web::Query<Value>) -> impl Responder {
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

async fn remove_role_from_user(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(String, i32)>) -> impl Responder {
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

async fn create_consumer_token(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<i32>, query: web::Query<Value>) -> impl Responder {
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

async fn list_consumer_tokens(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<i32>) -> impl Responder {
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

async fn delete_consumer_token(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<(i32, i32)>) -> impl Responder {
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

async fn delete_favorite(data: web::Data<Arc<dyn ApolloPersistenceService>>, path: web::Path<i32>, query: web::Query<Value>) -> impl Responder {
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

pub fn configure_admin_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        web::resource("/apps")
            .route(web::post().to(create_app))
            .route(web::get().to(list_apps))
    )
    .service(
        web::resource("/apps/{app_id}")
            .route(web::get().to(get_app))
            .route(web::put().to(update_app))
            .route(web::delete().to(delete_app))
    )
    .service(
        web::resource("/apps/{app_id}/clusters")
            .route(web::get().to(list_clusters))
            .route(web::post().to(create_cluster))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}")
            .route(web::get().to(get_cluster))
            .route(web::delete().to(delete_cluster))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces")
            .route(web::post().to(create_namespace))
            .route(web::get().to(list_namespaces))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}")
            .route(web::get().to(get_namespace))
            .route(web::put().to(update_namespace))
            .route(web::delete().to(delete_namespace))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items")
            .route(web::post().to(create_item))
            .route(web::get().to(list_items))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/items/{key}")
            .route(web::get().to(get_item))
            .route(web::put().to(update_item))
            .route(web::delete().to(delete_item))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/commits")
            .route(web::get().to(list_commits))
            .route(web::post().to(create_commit))
    )
    .service(
        web::resource("/commits/{id}")
            .route(web::get().to(get_commit))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases")
            .route(web::post().to(publish_release))
            .route(web::get().to(list_releases))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/gray-release-rules")
            .route(web::get().to(list_gray_release_rules))
            .route(web::post().to(create_gray_release_rule))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/gray-release-rules/{branch_name}")
            .route(web::get().to(get_gray_release_rule))
            .route(web::put().to(update_gray_release_rule))
            .route(web::delete().to(delete_gray_release_rule))
    )
    .service(
        web::resource("/serverconfigs")
            .route(web::get().to(list_server_configs))
            .route(web::post().to(create_server_config))
            .route(web::put().to(update_server_config))
    )
    .service(
        web::resource("/serverconfigs/{key}")
            .route(web::get().to(get_server_config))
            .route(web::delete().to(delete_server_config))
    )
    .service(
        web::resource("/apps/{app_id}/appnamespaces")
            .route(web::get().to(list_app_namespaces))
            .route(web::post().to(create_app_namespace_admin))
    )
    .service(
        web::resource("/apps/{app_id}/appnamespaces/{name}")
            .route(web::get().to(get_app_namespace))
            .route(web::delete().to(delete_app_namespace))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/lock")
            .route(web::get().to(get_namespace_lock))
            .route(web::post().to(lock_namespace))
            .route(web::delete().to(unlock_namespace))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/itemset")
            .route(web::post().to(update_item_set))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/releases/{release_id}/rollback")
            .route(web::post().to(rollback_release_admin))
    )
    .service(
        web::resource("/apps/{app_id}/clusters/{cluster_name}/namespaces/{namespace_name}/updateAndPublish")
            .route(web::post().to(merge_branch_and_release))
    )
    .service(
        web::resource("/instance-configs")
            .route(web::get().to(list_instance_configs))
    )
    .service(
        web::resource("/appnamespaces")
            .route(web::get().to(list_public_app_namespaces))
    )
    .service(
        web::resource("/audit")
            .route(web::get().to(list_audit))
    )
    .service(
        web::resource("/audit/by-entity")
            .route(web::get().to(list_audit_by_entity))
    )
    .service(
        web::resource("/permissions")
            .route(web::get().to(list_permissions))
    )
    .service(
        web::resource("/roles")
            .route(web::post().to(create_role))
    )
    .service(
        web::resource("/roles/{id}")
            .route(web::delete().to(delete_role))
    )
    .service(
        web::resource("/users/{user_id}/roles")
            .route(web::get().to(list_user_roles))
    )
    .service(
        web::resource("/users/{user_id}/roles/{role_id}")
            .route(web::post().to(assign_role_to_user))
            .route(web::delete().to(remove_role_from_user))
    )
    .service(
        web::resource("/consumers")
            .route(web::post().to(create_consumer))
    )
    .service(
        web::resource("/consumers/{app_id}")
            .route(web::get().to(get_consumer))
    )
    .service(
        web::resource("/consumers/{consumer_id}/tokens")
            .route(web::post().to(create_consumer_token))
            .route(web::get().to(list_consumer_tokens))
    )
    .service(
        web::resource("/consumers/{consumer_id}/tokens/{token_id}")
            .route(web::delete().to(delete_consumer_token))
    )
    .service(
        web::resource("/configs/{app_id}/{cluster_name}/{namespace_name}/export")
            .route(web::get().to(export_configs))
    )
    .service(
        web::resource("/configs/import")
            .route(web::post().to(import_configs))
    )
    .service(
        web::resource("/favorites")
            .route(web::get().to(list_favorites))
            .route(web::post().to(create_favorite))
    )
    .service(
        web::resource("/favorites/{id}")
            .route(web::delete().to(delete_favorite))
    )
    .service(
        web::resource("/search")
            .route(web::get().to(search))
    )
    .service(
        web::resource("/configs/sync")
            .route(web::post().to(sync_configs))
    )
    .service(
        web::resource("/configs/sync/app")
            .route(web::post().to(sync_app_all_namespaces))
    );
}