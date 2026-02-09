//! Apollo advanced features API handlers
//!
//! HTTP handlers for namespace locking, gray release, access keys, and metrics.

use std::sync::Arc;

use actix_web::{HttpResponse, web};
use serde::Deserialize;

use crate::model::{AcquireLockRequest, CreateGrayReleaseRequest};
use crate::service::ApolloAdvancedService;

// ============================================================================
// Path Parameters
// ============================================================================

/// Path parameters for namespace operations
#[derive(Debug, Deserialize)]
pub struct NamespacePathParams {
    pub env: String,
    pub app_id: String,
    pub cluster: String,
    pub namespace: String,
}

/// Path parameters for access key operations
#[derive(Debug, Deserialize)]
pub struct AccessKeyPathParams {
    pub app_id: String,
    pub key_id: String,
}

/// Query parameters for release lock
#[derive(Debug, Deserialize)]
pub struct ReleaseLockQuery {
    pub user: String,
}

// ============================================================================
// Namespace Lock Handlers
// ============================================================================

/// Get namespace lock status
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock`
pub async fn get_lock_status(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
) -> HttpResponse {
    let lock = service
        .get_lock_status(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await;
    HttpResponse::Ok().json(lock)
}

/// Acquire namespace lock
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock`
pub async fn acquire_lock(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
    body: web::Json<AcquireLockRequest>,
) -> HttpResponse {
    match service
        .acquire_lock(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            body.into_inner(),
        )
        .await
    {
        Ok(lock) => HttpResponse::Ok().json(lock),
        Err(e) => HttpResponse::Conflict().json(serde_json::json!({
            "error": e
        })),
    }
}

/// Release namespace lock
///
/// `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock`
pub async fn release_lock(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
    query: web::Query<ReleaseLockQuery>,
) -> HttpResponse {
    match service
        .release_lock(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            &query.user,
        )
        .await
    {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Err(e) => HttpResponse::Forbidden().json(serde_json::json!({
            "error": e
        })),
    }
}

// ============================================================================
// Gray Release Handlers
// ============================================================================

/// Create gray release
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray`
pub async fn create_gray_release(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
    body: web::Json<CreateGrayReleaseRequest>,
) -> HttpResponse {
    match service
        .create_gray_release(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.env,
            body.into_inner(),
        )
        .await
    {
        Ok(rule) => HttpResponse::Created().json(rule),
        Err(e) => {
            tracing::error!("Failed to create gray release: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Get gray release
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray`
pub async fn get_gray_release(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
) -> HttpResponse {
    match service
        .get_gray_release(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await
    {
        Some(rule) => HttpResponse::Ok().json(rule),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "No gray release found"
        })),
    }
}

/// Merge gray release
///
/// `PUT /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray/merge`
pub async fn merge_gray_release(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
) -> HttpResponse {
    match service
        .merge_gray_release(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await
    {
        Ok(rule) => HttpResponse::Ok().json(rule),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": e
        })),
    }
}

/// Abandon gray release
///
/// `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray`
pub async fn abandon_gray_release(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<NamespacePathParams>,
) -> HttpResponse {
    match service
        .abandon_gray_release(&path.app_id, &path.cluster, &path.namespace, &path.env)
        .await
    {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Err(e) => HttpResponse::NotFound().json(serde_json::json!({
            "error": e
        })),
    }
}

// ============================================================================
// Access Key Handlers
// ============================================================================

/// Create access key
///
/// `POST /openapi/v1/apps/{appId}/accesskeys`
pub async fn create_access_key(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<AppIdPath>,
) -> HttpResponse {
    let key = service.create_access_key(&path.app_id).await;
    HttpResponse::Created().json(key)
}

/// List access keys
///
/// `GET /openapi/v1/apps/{appId}/accesskeys`
pub async fn list_access_keys(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<AppIdPath>,
) -> HttpResponse {
    let keys = service.list_access_keys(&path.app_id).await;
    HttpResponse::Ok().json(keys)
}

/// Get access key
///
/// `GET /openapi/v1/apps/{appId}/accesskeys/{keyId}`
pub async fn get_access_key(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<AccessKeyPathParams>,
) -> HttpResponse {
    match service.get_access_key(&path.key_id).await {
        Some(key) => HttpResponse::Ok().json(key),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Access key not found"
        })),
    }
}

/// Delete access key
///
/// `DELETE /openapi/v1/apps/{appId}/accesskeys/{keyId}`
pub async fn delete_access_key(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<AccessKeyPathParams>,
) -> HttpResponse {
    if service.delete_access_key(&path.key_id).await {
        HttpResponse::Ok().json(serde_json::json!({
            "success": true
        }))
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "error": "Access key not found"
        }))
    }
}

/// Enable/disable access key
///
/// `PUT /openapi/v1/apps/{appId}/accesskeys/{keyId}/enable`
pub async fn set_access_key_enabled(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<AccessKeyPathParams>,
    body: web::Json<EnableRequest>,
) -> HttpResponse {
    match service
        .set_access_key_enabled(&path.key_id, body.enabled)
        .await
    {
        Some(key) => HttpResponse::Ok().json(key),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Access key not found"
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct EnableRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct AppIdPath {
    pub app_id: String,
}

// ============================================================================
// Client Metrics Handlers
// ============================================================================

/// Get client metrics summary
///
/// `GET /openapi/v1/metrics/clients`
pub async fn get_client_metrics(service: web::Data<Arc<ApolloAdvancedService>>) -> HttpResponse {
    let metrics = service.get_metrics().await;
    HttpResponse::Ok().json(metrics)
}

/// Get clients for an app
///
/// `GET /openapi/v1/apps/{appId}/clients`
pub async fn get_app_clients(
    service: web::Data<Arc<ApolloAdvancedService>>,
    path: web::Path<AppIdPath>,
) -> HttpResponse {
    let clients = service.get_clients_by_app(&path.app_id).await;
    HttpResponse::Ok().json(clients)
}

/// Cleanup stale connections
///
/// `POST /openapi/v1/metrics/clients/cleanup`
pub async fn cleanup_stale_clients(service: web::Data<Arc<ApolloAdvancedService>>) -> HttpResponse {
    service.cleanup_stale_connections().await;
    HttpResponse::Ok().json(serde_json::json!({
        "success": true
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_path_params() {
        let json = r#"{"env":"DEV","app_id":"test","cluster":"default","namespace":"application"}"#;
        let params: NamespacePathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.env, "DEV");
        assert_eq!(params.namespace, "application");
    }
}
