// Console plugin management API endpoints
// This module provides console endpoints for listing available plugins

use actix_web::{HttpRequest, Responder, Scope, get, web};
use serde::Serialize;

use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response as common_response;
use batata_server_common::secured::Secured;
use batata_server_common::{ActionTypes, ApiType, SignType, secured};

/// Plugin information
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub name: String,
    pub category: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
}

/// Get static list of known plugins
fn get_known_plugins() -> Vec<PluginInfo> {
    vec![
        PluginInfo {
            name: "control".to_string(),
            category: "auth".to_string(),
            description: "Access control plugin for authentication and authorization".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
        },
        PluginInfo {
            name: "webhook".to_string(),
            category: "notification".to_string(),
            description: "Webhook notification plugin for config change events".to_string(),
            version: "1.0.0".to_string(),
            enabled: false,
        },
        PluginInfo {
            name: "cmdb".to_string(),
            category: "naming".to_string(),
            description: "CMDB plugin for instance metadata enrichment".to_string(),
            version: "1.0.0".to_string(),
            enabled: false,
        },
    ]
}

/// List all plugins
#[get("list")]
async fn list_plugins(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/core/plugin")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let plugins = get_known_plugins();
    common_response::Result::<Vec<PluginInfo>>::http_success(plugins)
}

/// Get a specific plugin by name
#[get("")]
async fn get_plugin(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<PluginQuery>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console/core/plugin")
            .action(ActionTypes::Read)
            .sign_type(SignType::Console)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let plugins = get_known_plugins();
    match plugins.into_iter().find(|p| p.name == params.name) {
        Some(plugin) => common_response::Result::<PluginInfo>::http_success(plugin),
        None => common_response::Result::<String>::http_response(
            404,
            404,
            format!("Plugin '{}' not found", params.name),
            String::new(),
        ),
    }
}

#[derive(Debug, serde::Deserialize)]
struct PluginQuery {
    name: String,
}

pub fn routes() -> Scope {
    web::scope("/core/plugin")
        .service(list_plugins)
        .service(get_plugin)
}
