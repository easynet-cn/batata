//! Server state console endpoints

use std::collections::HashMap;
use std::fs;

use actix_web::{Scope, get, web};
use serde::Deserialize;

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
    web::scope("/server").service(announcement).service(guide)
}
