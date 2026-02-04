//! Apollo API route configuration
//!
//! Configures HTTP routes for Apollo Config Service API compatibility.

use actix_web::web;

use super::{advanced, config, configfiles, notification, openapi};

/// Configure Apollo Config Service routes
///
/// These routes implement the Apollo client SDK protocol.
///
/// ## Endpoints
/// - `GET /configs/{appId}/{clusterName}/{namespace}` - Get configuration
/// - `GET /configfiles/{appId}/{clusterName}/{namespace}` - Get config as plain text
/// - `GET /configfiles/json/{appId}/{clusterName}/{namespace}` - Get config as JSON
/// - `GET /notifications/v2` - Long polling for updates
///
/// ## Usage
///
/// ```ignore
/// use batata_plugin_apollo::apollo_config_routes;
///
/// // In your actix-web app configuration:
/// App::new()
///     .service(apollo_config_routes())
/// ```
pub fn apollo_config_routes() -> actix_web::Scope {
    web::scope("")
        // Config endpoints
        .route(
            "/configs/{app_id}/{cluster_name}/{namespace}",
            web::get().to(config::get_config),
        )
        // Configfiles endpoints (plain text)
        .route(
            "/configfiles/{app_id}/{cluster_name}/{namespace}",
            web::get().to(configfiles::get_configfiles),
        )
        // Configfiles JSON endpoint
        .route(
            "/configfiles/json/{app_id}/{cluster_name}/{namespace}",
            web::get().to(configfiles::get_configfiles_json),
        )
        // Notifications (long polling)
        .route(
            "/notifications/v2",
            web::get().to(notification::get_notifications),
        )
}

/// Configure Apollo routes under a custom prefix
///
/// This allows mounting Apollo routes under a specific path prefix.
///
/// ## Example
///
/// ```ignore
/// // Mount under /apollo prefix
/// App::new()
///     .service(apollo_config_routes_with_prefix("/apollo"))
/// ```
pub fn apollo_config_routes_with_prefix(prefix: &str) -> actix_web::Scope {
    web::scope(prefix)
        .route(
            "/configs/{app_id}/{cluster_name}/{namespace}",
            web::get().to(config::get_config),
        )
        .route(
            "/configfiles/{app_id}/{cluster_name}/{namespace}",
            web::get().to(configfiles::get_configfiles),
        )
        .route(
            "/configfiles/json/{app_id}/{cluster_name}/{namespace}",
            web::get().to(configfiles::get_configfiles_json),
        )
        .route(
            "/notifications/v2",
            web::get().to(notification::get_notifications),
        )
}

/// Configure function for use with actix-web service configuration
///
/// ## Example
///
/// ```ignore
/// use batata_plugin_apollo::configure_apollo_routes;
///
/// // In your App configuration:
/// App::new()
///     .configure(configure_apollo_routes)
/// ```
pub fn configure_apollo_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(apollo_config_routes());
    cfg.service(apollo_openapi_routes());
    cfg.service(apollo_advanced_routes());
}

/// Configure Apollo Advanced routes
///
/// These routes implement Apollo advanced features.
///
/// ## Namespace Lock Endpoints
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock` - Get lock status
/// - `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock` - Acquire lock
/// - `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/lock` - Release lock
///
/// ## Gray Release Endpoints
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray` - Get gray release
/// - `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray` - Create gray release
/// - `PUT /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray/merge` - Merge gray release
/// - `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/gray` - Abandon gray release
///
/// ## Access Key Endpoints
/// - `GET /openapi/v1/apps/{appId}/accesskeys` - List access keys
/// - `POST /openapi/v1/apps/{appId}/accesskeys` - Create access key
/// - `GET /openapi/v1/apps/{appId}/accesskeys/{keyId}` - Get access key
/// - `PUT /openapi/v1/apps/{appId}/accesskeys/{keyId}/enable` - Enable/disable access key
/// - `DELETE /openapi/v1/apps/{appId}/accesskeys/{keyId}` - Delete access key
///
/// ## Client Metrics Endpoints
/// - `GET /openapi/v1/metrics/clients` - Get client metrics summary
/// - `GET /openapi/v1/apps/{appId}/clients` - Get clients for an app
/// - `POST /openapi/v1/metrics/clients/cleanup` - Cleanup stale connections
pub fn apollo_advanced_routes() -> actix_web::Scope {
    web::scope("/openapi/v1")
        // Namespace Lock
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/lock",
            web::get().to(advanced::get_lock_status),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/lock",
            web::post().to(advanced::acquire_lock),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/lock",
            web::delete().to(advanced::release_lock),
        )
        // Gray Release
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/gray",
            web::get().to(advanced::get_gray_release),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/gray",
            web::post().to(advanced::create_gray_release),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/gray/merge",
            web::put().to(advanced::merge_gray_release),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/gray",
            web::delete().to(advanced::abandon_gray_release),
        )
        // Access Keys
        .route(
            "/apps/{app_id}/accesskeys",
            web::get().to(advanced::list_access_keys),
        )
        .route(
            "/apps/{app_id}/accesskeys",
            web::post().to(advanced::create_access_key),
        )
        .route(
            "/apps/{app_id}/accesskeys/{key_id}",
            web::get().to(advanced::get_access_key),
        )
        .route(
            "/apps/{app_id}/accesskeys/{key_id}/enable",
            web::put().to(advanced::set_access_key_enabled),
        )
        .route(
            "/apps/{app_id}/accesskeys/{key_id}",
            web::delete().to(advanced::delete_access_key),
        )
        // Client Metrics
        .route(
            "/metrics/clients",
            web::get().to(advanced::get_client_metrics),
        )
        .route(
            "/apps/{app_id}/clients",
            web::get().to(advanced::get_app_clients),
        )
        .route(
            "/metrics/clients/cleanup",
            web::post().to(advanced::cleanup_stale_clients),
        )
}

/// Configure Apollo Open API routes
///
/// These routes implement the Apollo Open API management protocol.
///
/// ## App Endpoints
/// - `GET /openapi/v1/apps` - Get all apps
/// - `GET /openapi/v1/apps/{appId}/envclusters` - Get env clusters
///
/// ## Namespace Endpoints
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces` - List namespaces
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}` - Get namespace
///
/// ## Item Endpoints
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` - List items
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}` - Get item
/// - `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items` - Create item
/// - `PUT /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}` - Update item
/// - `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/items/{key}` - Delete item
///
/// ## Release Endpoints
/// - `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases` - Publish release
/// - `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/releases/latest` - Get latest release
pub fn apollo_openapi_routes() -> actix_web::Scope {
    web::scope("/openapi/v1")
        // App Management
        .route("/apps", web::get().to(openapi::get_apps))
        .route(
            "/apps/{app_id}/envclusters",
            web::get().to(openapi::get_env_clusters),
        )
        // Namespace Management
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces",
            web::get().to(openapi::get_namespaces),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}",
            web::get().to(openapi::get_namespace),
        )
        // Item Management
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/items",
            web::get().to(openapi::get_items),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/items",
            web::post().to(openapi::create_item),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/items/{key}",
            web::get().to(openapi::get_item),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/items/{key}",
            web::put().to(openapi::update_item),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/items/{key}",
            web::delete().to(openapi::delete_item),
        )
        // Release Management
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/releases",
            web::post().to(openapi::publish_release),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/releases/latest",
            web::get().to(openapi::get_latest_release),
        )
}
