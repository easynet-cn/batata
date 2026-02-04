//! Apollo configfiles endpoint handlers
//!
//! GET `/configfiles/{appId}/{clusterName}/{namespace}` - Plain text
//! GET `/configfiles/json/{appId}/{clusterName}/{namespace}` - JSON format

use actix_web::{HttpRequest, HttpResponse, web};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::model::ConfigFormat;
use crate::service::ApolloConfigService;

/// Path parameters for configfiles endpoint
#[derive(Debug, serde::Deserialize)]
pub struct ConfigFilesPath {
    pub app_id: String,
    pub cluster_name: String,
    pub namespace: String,
}

/// Get configuration as plain text
///
/// Returns raw configuration content without parsing.
pub async fn get_configfiles(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<ConfigFilesPath>,
    req: HttpRequest,
) -> HttpResponse {
    let service = ApolloConfigService::new(db.get_ref().clone());

    let env = req
        .headers()
        .get("X-Apollo-Env")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let namespace = path
        .namespace
        .strip_suffix(".properties")
        .unwrap_or(&path.namespace);

    match service
        .get_config_content(&path.app_id, &path.cluster_name, namespace, env.as_deref())
        .await
    {
        Ok(Some(content)) => {
            let format = ConfigFormat::from_namespace(&path.namespace);
            HttpResponse::Ok()
                .content_type(format.content_type())
                .body(content)
        }
        Ok(None) => HttpResponse::NotFound().body("Config not found"),
        Err(e) => {
            tracing::error!("Failed to get Apollo configfiles: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

/// Get configuration as JSON
///
/// Returns configuration content wrapped in JSON format.
pub async fn get_configfiles_json(
    db: web::Data<Arc<DatabaseConnection>>,
    path: web::Path<ConfigFilesPath>,
    req: HttpRequest,
) -> HttpResponse {
    let service = ApolloConfigService::new(db.get_ref().clone());

    let env = req
        .headers()
        .get("X-Apollo-Env")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let namespace = path
        .namespace
        .strip_suffix(".properties")
        .unwrap_or(&path.namespace);

    match service
        .get_config(
            &path.app_id,
            &path.cluster_name,
            namespace,
            env.as_deref(),
            None,
        )
        .await
    {
        Ok(Some(config)) => HttpResponse::Ok().json(config.configurations),
        Ok(None) => HttpResponse::NotModified().finish(),
        Err(e) => {
            tracing::error!("Failed to get Apollo configfiles/json: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": 500,
                "message": format!("Error: {}", e)
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configfiles_path_deserialize() {
        let json = r#"{"app_id":"app1","cluster_name":"default","namespace":"config.json"}"#;
        let path: ConfigFilesPath = serde_json::from_str(json).unwrap();
        assert_eq!(path.namespace, "config.json");
    }
}
