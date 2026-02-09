//! Apollo API route configuration
//!
//! Configures HTTP routes for Apollo Config Service API compatibility.

use actix_web::web;

use super::{advanced, branch, config, configfiles, notification, openapi, services};

/// Configure Apollo Config Service routes
///
/// These routes implement the Apollo client SDK protocol.
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
        // Service discovery
        .route(
            "/services/config",
            web::get().to(services::get_config_service),
        )
        .route(
            "/services/admin",
            web::get().to(services::get_admin_service),
        )
}

/// Configure Apollo routes under a custom prefix
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
        .route(
            "/services/config",
            web::get().to(services::get_config_service),
        )
        .route(
            "/services/admin",
            web::get().to(services::get_admin_service),
        )
}

/// Configure function for use with actix-web service configuration
pub fn configure_apollo_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(apollo_config_routes());
    cfg.service(apollo_openapi_routes());
    cfg.service(apollo_advanced_routes());
}

/// Configure Apollo Advanced routes
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
pub fn apollo_openapi_routes() -> actix_web::Scope {
    web::scope("/openapi/v1")
        // App Management
        .route("/apps", web::get().to(openapi::get_apps))
        .route("/apps", web::post().to(openapi::create_app))
        .route("/apps/{app_id}", web::get().to(openapi::get_app))
        .route("/apps/{app_id}", web::put().to(openapi::update_app))
        .route("/apps/{app_id}", web::delete().to(openapi::delete_app))
        .route(
            "/apps/{app_id}/envclusters",
            web::get().to(openapi::get_env_clusters),
        )
        // Cluster Management
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}",
            web::get().to(openapi::get_cluster),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters",
            web::post().to(openapi::create_cluster),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}",
            web::delete().to(openapi::delete_cluster),
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
        .route(
            "/apps/{app_id}/appnamespaces",
            web::post().to(openapi::create_namespace),
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
        .route(
            "/envs/{env}/releases/{release_id}/rollback",
            web::put().to(openapi::rollback_release),
        )
        // Branch Management
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches",
            web::get().to(branch::get_branches),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches",
            web::post().to(branch::create_branch),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}",
            web::delete().to(branch::delete_branch),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/rules",
            web::get().to(branch::get_gray_rules),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/rules",
            web::put().to(branch::update_gray_rules),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/releases",
            web::post().to(branch::gray_release),
        )
        .route(
            "/envs/{env}/apps/{app_id}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/merge",
            web::post().to(branch::merge_branch),
        )
}
