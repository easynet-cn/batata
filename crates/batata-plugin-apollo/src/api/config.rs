//! Apollo config endpoint handler
//!
//! GET `/configs/{appId}/{clusterName}/{namespace}`

use actix_web::{HttpRequest, HttpResponse, web};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::model::ConfigQueryParams;
use crate::service::ApolloConfigService;

/// Path parameters for config endpoint
#[derive(Debug, serde::Deserialize)]
pub struct ConfigPath {
    pub app_id: String,
    pub cluster_name: String,
    /// Namespace with optional format suffix (e.g., "application", "config.json")
    pub namespace: String,
}

/// Get configuration
///
/// Apollo client SDK uses this endpoint to fetch configurations.
///
/// ## Response
/// - 200 OK: Configuration found, returns ApolloConfig JSON
/// - 304 Not Modified: Client's releaseKey matches server version
/// - 404 Not Found: Namespace not found (Apollo returns empty config instead)
pub async fn get_config(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<ConfigPath>,
    query: web::Query<ConfigQueryParams>,
    req: HttpRequest,
) -> HttpResponse {
    let service = ApolloConfigService::new(db.get_ref().clone());

    // Get env from header or query (Apollo convention)
    let env = req
        .headers()
        .get("X-Apollo-Env")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Normalize namespace (remove .properties suffix)
    let namespace = path
        .namespace
        .strip_suffix(".properties")
        .unwrap_or(&path.namespace);

    let client_release_key = if query.release_key == "-1" {
        None
    } else {
        Some(query.release_key.as_str())
    };

    match service
        .get_config(
            &path.app_id,
            &path.cluster_name,
            namespace,
            env.as_deref(),
            client_release_key,
        )
        .await
    {
        Ok(Some(config)) => HttpResponse::Ok().json(config),
        Ok(None) => {
            // 304 Not Modified
            HttpResponse::NotModified().finish()
        }
        Err(e) => {
            tracing::error!("Failed to get Apollo config: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": 500,
                "message": format!("Failed to get config: {}", e)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_deserialize() {
        let json = r#"{"app_id":"app1","cluster_name":"default","namespace":"application"}"#;
        let path: ConfigPath = serde_json::from_str(json).unwrap();
        assert_eq!(path.app_id, "app1");
        assert_eq!(path.cluster_name, "default");
        assert_eq!(path.namespace, "application");
    }
}
