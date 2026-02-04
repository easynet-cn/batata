//! Apollo Open API handlers
//!
//! HTTP handlers for Apollo Open API management endpoints.

use std::sync::Arc;

use actix_web::{HttpResponse, web};
use sea_orm::DatabaseConnection;
use serde::Deserialize;

use crate::model::{
    AppPathParams, CreateItemRequest, ItemPathParams, OpenApiPathParams, PublishReleaseRequest,
    UpdateItemRequest,
};
use crate::service::ApolloOpenApiService;

/// Query parameters for delete item
#[derive(Debug, Deserialize)]
pub struct DeleteItemQuery {
    pub operator: String,
}

// ============================================================================
// App Management
// ============================================================================

/// Get all apps
///
/// `GET /openapi/v1/apps`
pub async fn get_apps(db: web::Data<Arc<DatabaseConnection>>) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service.get_apps().await {
        Ok(apps) => HttpResponse::Ok().json(apps),
        Err(e) => {
            tracing::error!("Failed to get apps: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Get environment clusters for an app
///
/// `GET /openapi/v1/apps/{appId}/envclusters`
pub async fn get_env_clusters(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<AppPathParams>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service.get_env_clusters(&path.app_id).await {
        Ok(env_clusters) => HttpResponse::Ok().json(env_clusters),
        Err(e) => {
            tracing::error!("Failed to get env clusters: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

// ============================================================================
// Namespace Management
// ============================================================================

/// Get namespaces for an app
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces`
pub async fn get_namespaces(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<NamespaceListPathParams>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .get_namespaces(&path.app_id, &path.cluster, &path.env)
        .await
    {
        Ok(namespaces) => HttpResponse::Ok().json(namespaces),
        Err(e) => {
            tracing::error!("Failed to get namespaces: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Get a specific namespace
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}`
pub async fn get_namespace(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<OpenApiPathParams>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .get_namespace(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await
    {
        Ok(Some(ns)) => HttpResponse::Ok().json(ns),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Namespace not found"
        })),
        Err(e) => {
            tracing::error!("Failed to get namespace: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

// ============================================================================
// Item Management
// ============================================================================

/// Get all items in a namespace
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items`
pub async fn get_items(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<OpenApiPathParams>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .get_items(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await
    {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => {
            tracing::error!("Failed to get items: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Get a specific item
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}`
pub async fn get_item(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<ItemPathParams>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .get_item(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            &path.key,
        )
        .await
    {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Item not found"
        })),
        Err(e) => {
            tracing::error!("Failed to get item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Create a new item
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items`
pub async fn create_item(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<OpenApiPathParams>,
    body: web::Json<CreateItemRequest>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .create_item(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            body.into_inner(),
        )
        .await
    {
        Ok(item) => HttpResponse::Created().json(item),
        Err(e) => {
            tracing::error!("Failed to create item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Update an item
///
/// `PUT /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}`
pub async fn update_item(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<ItemPathParams>,
    body: web::Json<UpdateItemRequest>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .update_item(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            &path.key,
            body.into_inner(),
        )
        .await
    {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Item not found"
        })),
        Err(e) => {
            tracing::error!("Failed to update item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Delete an item
///
/// `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}`
pub async fn delete_item(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<ItemPathParams>,
    query: web::Query<DeleteItemQuery>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .delete_item(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            &path.key,
            &query.operator,
        )
        .await
    {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Item not found"
        })),
        Err(e) => {
            tracing::error!("Failed to delete item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

// ============================================================================
// Release Management
// ============================================================================

/// Publish a release
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases`
pub async fn publish_release(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<OpenApiPathParams>,
    body: web::Json<PublishReleaseRequest>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .publish_release(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            body.into_inner(),
        )
        .await
    {
        Ok(Some(release)) => HttpResponse::Ok().json(release),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Namespace not found"
        })),
        Err(e) => {
            tracing::error!("Failed to publish release: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Get the latest release
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases/latest`
pub async fn get_latest_release(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<OpenApiPathParams>,
) -> HttpResponse {
    let service = ApolloOpenApiService::new(db.get_ref().clone());

    match service
        .get_latest_release(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await
    {
        Ok(Some(release)) => HttpResponse::Ok().json(release),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "No release found"
        })),
        Err(e) => {
            tracing::error!("Failed to get latest release: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Path parameters for namespace list
#[derive(Debug, Deserialize)]
pub struct NamespaceListPathParams {
    pub env: String,
    pub app_id: String,
    pub cluster: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_list_path_params() {
        let json = r#"{"env":"DEV","app_id":"test-app","cluster":"default"}"#;
        let params: NamespaceListPathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.env, "DEV");
        assert_eq!(params.app_id, "test-app");
        assert_eq!(params.cluster, "default");
    }
}
