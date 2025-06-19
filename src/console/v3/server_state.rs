use std::{collections::HashMap, fs};

use actix_web::{Scope, get, web};
use serde::Deserialize;

use crate::model::{auth, common, common::AppState};

pub const AUTH_ENABLED: &str = "auth_enabled";
pub const LOGIN_PAGE_ENABLED: &str = "login_page_enabled";
pub const AUTH_SYSTEM_TYPE: &str = "auth_system_type";
pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
pub const GUIDE_FILE: &str = "console-guide.conf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageParam {
    language: String,
}

#[get("/state")]
pub async fn state(data: web::Data<AppState>) -> web::Json<HashMap<String, Option<String>>> {
    let mut state_map: HashMap<String, Option<String>> = HashMap::new();

    // config module state
    let config_state = data.config_state();

    for (k, v) in config_state {
        state_map.insert(k, v);
    }

    // console auth
    state_map.insert(
        AUTH_ENABLED.to_string(),
        Some(
            data.app_config
                .get_string(auth::NACOS_CORE_AUTH_ENABLED)
                .unwrap_or("true".to_string()),
        ),
    );
    state_map.insert(
        LOGIN_PAGE_ENABLED.to_string(),
        Some(
            data.app_config
                .get_string(auth::NACOS_CORE_AUTH_CONSOLE_ENABLED)
                .unwrap_or("false".to_string()),
        ),
    );
    state_map.insert(
        "auth_system_type".to_string(),
        Some(
            data.app_config
                .get_string("nacos.core.auth.system.type")
                .unwrap_or_default(),
        ),
    );

    // Console state
    state_map.insert(
        "console_ui_enabled".to_string(),
        Some(
            data.app_config
                .get_string("nacos.console.ui.enabled")
                .unwrap_or("true".to_string()),
        ),
    );

    // Env state
    state_map.insert(
        "startup_mode".to_string(),
        Some(
            data.app_config
                .get_string("nacos.standalone")
                .unwrap_or("standalone".to_string()),
        ),
    );
    state_map.insert(
        "function_mode".to_string(),
        data.app_config.get_string("nacos.functionMode").ok(),
    );
    state_map.insert(
        "version".to_string(),
        Some((env!("CARGO_PKG_VERSION")).to_string()),
    );
    state_map.insert(
        common::SERVER_PORT_STATE.to_string(),
        Some(format!("{}", data.server_port())),
    );

    web::Json(state_map)
}

#[get("/announcement")]
pub async fn announcement(params: web::Query<LanguageParam>) -> web::Json<common::Result<String>> {
    let file = format!(
        "conf/{}_{}.conf",
        ANNOUNCEMENT_FILE[0..ANNOUNCEMENT_FILE.len() - 5].to_string(),
        params.language
    );

    if let Ok(content) = fs::read_to_string(file) {
        web::Json(common::Result::<String>::success(content))
    } else {
        web::Json(common::Result::<String>::success("".to_string()))
    }
}

#[get("/guide")]
pub async fn guide() -> web::Json<common::Result<String>> {
    let file = format!("conf/{}", GUIDE_FILE.to_string());

    if let Ok(content) = fs::read_to_string(file) {
        web::Json(common::Result::<String>::success(content))
    } else {
        web::Json(common::Result::<String>::success("".to_string()))
    }
}

pub fn routers() -> Scope {
    web::scope("/server")
        .service(state)
        .service(announcement)
        .service(guide)
}
