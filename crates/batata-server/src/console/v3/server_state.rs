use std::{collections::HashMap, fs};

use actix_web::{Scope, get, web};
use serde::Deserialize;

use crate::{
    auth,
    model::common::{self, AppState},
};

pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
pub const GUIDE_FILE: &str = "console-guide.conf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageParam {
    language: String,
}

#[get("/state")]
async fn state(data: web::Data<AppState>) -> web::Json<HashMap<String, Option<String>>> {
    // config module state
    let config_state = data.config_state();

    // auth module state
    let auth_enabled = data.configuration.auth_enabled();
    let global_admin = auth::service::role::has_global_admin_role(data.db())
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to check global admin role: {}", e);
            false
        });

    let auth_state = data.auth_state(auth_enabled && !global_admin);

    // env module state
    let env_state = data.env_state();

    //console module state
    let console_state = data.console_state();

    let mut state_map: HashMap<String, Option<String>> = HashMap::with_capacity(
        config_state.len() + auth_state.len() + env_state.len() + console_state.len() + 1,
    );

    state_map.insert(
        common::SERVER_PORT_STATE.to_string(),
        Some(format!("{}", data.configuration.console_server_port())),
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

    web::Json(state_map)
}

#[get("/announcement")]
async fn announcement(params: web::Query<LanguageParam>) -> web::Json<common::Result<String>> {
    let file = format!(
        "conf/{}_{}.conf",
        &ANNOUNCEMENT_FILE[0..ANNOUNCEMENT_FILE.len() - 5],
        params.language
    );

    if let Ok(content) = fs::read_to_string(file) {
        web::Json(common::Result::<String>::success(content))
    } else {
        web::Json(common::Result::<String>::success("".to_string()))
    }
}

#[get("/guide")]
async fn guide() -> web::Json<common::Result<String>> {
    let file = format!("conf/{}", GUIDE_FILE);

    if let Ok(content) = fs::read_to_string(file) {
        web::Json(common::Result::<String>::success(content))
    } else {
        web::Json(common::Result::<String>::success("".to_string()))
    }
}

pub fn routes() -> Scope {
    web::scope("/server")
        .service(state)
        .service(announcement)
        .service(guide)
}
