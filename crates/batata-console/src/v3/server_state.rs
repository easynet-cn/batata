//! Server state console endpoints

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use actix_web::{HttpResponse, Responder, Scope, get, web};
use serde::Deserialize;

use crate::datasource::ConsoleDataSource;

use super::namespace::ApiResult;

pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
pub const GUIDE_FILE: &str = "console-guide.conf";

/// Server state keys
pub const SERVER_PORT_STATE: &str = "server_port";
pub const CONSOLE_UI_ENABLED: &str = "console_ui_enabled";
pub const STANDALONE_MODE: &str = "standalone_mode";
pub const FUNCTION_MODE: &str = "function_mode";
pub const AUTH_ENABLED: &str = "auth_enabled";
pub const AUTH_ADMIN_REQUEST: &str = "auth_admin_request";
pub const LOGIN_PAGE_ENABLED: &str = "login_page_enabled";
pub const AUTH_SYSTEM_TYPE: &str = "auth_system_type";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageParam {
    pub language: String,
}

/// Server state configuration
#[derive(Clone, Debug)]
pub struct ServerStateConfig {
    pub server_port: u16,
    pub console_ui_enabled: bool,
    pub function_mode: String,
    pub auth_enabled: bool,
    pub auth_system_type: String,
    pub login_page_enabled: bool,
}

impl Default for ServerStateConfig {
    fn default() -> Self {
        Self {
            server_port: 8848,
            console_ui_enabled: true,
            function_mode: "naming,config".to_string(),
            auth_enabled: false,
            auth_system_type: "nacos".to_string(),
            login_page_enabled: true,
        }
    }
}

/// Server state provider trait
pub trait ServerStateProvider: Send + Sync {
    fn config_state(&self) -> HashMap<String, Option<String>>;
    fn auth_state(&self, admin_request: bool) -> HashMap<String, Option<String>>;
    fn env_state(&self) -> HashMap<String, Option<String>>;
    fn console_state(&self) -> HashMap<String, Option<String>>;
    fn server_port(&self) -> u16;
}

/// Default implementation for getting server state
pub fn get_server_state<T: ServerStateProvider>(provider: &T) -> HashMap<String, Option<String>> {
    let config_state = provider.config_state();
    let auth_state = provider.auth_state(false);
    let env_state = provider.env_state();
    let console_state = provider.console_state();

    let mut state_map: HashMap<String, Option<String>> = HashMap::with_capacity(
        config_state.len() + auth_state.len() + env_state.len() + console_state.len() + 1,
    );

    state_map.insert(
        SERVER_PORT_STATE.to_string(),
        Some(format!("{}", provider.server_port())),
    );

    for (k, v) in config_state {
        state_map.insert(k, v);
    }

    for (k, v) in auth_state {
        state_map.insert(k, v);
    }

    for (k, v) in env_state {
        state_map.insert(k, v);
    }

    for (k, v) in console_state {
        state_map.insert(k, v);
    }

    state_map
}

/// Get server state endpoint
#[get("/state")]
pub async fn get_state(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    config: Option<web::Data<ServerStateConfig>>,
) -> impl Responder {
    let config = config.map(|c| c.get_ref().clone()).unwrap_or_default();
    let standalone = datasource.cluster_is_standalone();

    let mut state: HashMap<String, String> = HashMap::new();
    state.insert(
        SERVER_PORT_STATE.to_string(),
        config.server_port.to_string(),
    );
    state.insert(
        CONSOLE_UI_ENABLED.to_string(),
        config.console_ui_enabled.to_string(),
    );
    state.insert(STANDALONE_MODE.to_string(), standalone.to_string());
    state.insert(FUNCTION_MODE.to_string(), config.function_mode.clone());
    state.insert(AUTH_ENABLED.to_string(), config.auth_enabled.to_string());
    state.insert(
        AUTH_SYSTEM_TYPE.to_string(),
        config.auth_system_type.clone(),
    );
    state.insert(
        LOGIN_PAGE_ENABLED.to_string(),
        config.login_page_enabled.to_string(),
    );

    HttpResponse::Ok().json(state)
}

#[get("/announcement")]
pub async fn announcement(params: web::Query<LanguageParam>) -> web::Json<ApiResult<String>> {
    let file = format!(
        "conf/{}_{}.conf",
        &ANNOUNCEMENT_FILE[0..ANNOUNCEMENT_FILE.len() - 5],
        params.language
    );

    if let Ok(content) = fs::read_to_string(file) {
        web::Json(ApiResult::<String>::success(content))
    } else {
        web::Json(ApiResult::<String>::success(String::new()))
    }
}

#[get("/guide")]
pub async fn guide() -> web::Json<ApiResult<String>> {
    let file = format!("conf/{}", GUIDE_FILE);

    if let Ok(content) = fs::read_to_string(file) {
        web::Json(ApiResult::<String>::success(content))
    } else {
        web::Json(ApiResult::<String>::success(String::new()))
    }
}

pub fn routes() -> Scope {
    web::scope("/server")
        .service(get_state)
        .service(announcement)
        .service(guide)
}
