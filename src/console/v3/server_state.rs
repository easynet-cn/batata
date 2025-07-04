use std::{collections::HashMap, fs};

use actix_web::{HttpMessage, HttpRequest, Scope, get, web};
use serde::Deserialize;

use crate::{
    model::{
        self,
        common::{self, AppState},
    },
    service,
};

pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
pub const GUIDE_FILE: &str = "console-guide.conf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageParam {
    language: String,
}

#[get("/state")]
pub async fn state(
    req: HttpRequest,
    data: web::Data<AppState>,
) -> web::Json<HashMap<String, Option<String>>> {
    // config module state
    let config_state = data.config_state();

    // auth module state
    let auth_enabled = data.configuration.auth_enabled();
    let mut has_global_admin_role = false;

    if let Some(auth_context) = req.extensions().get::<model::auth::AuthContext>() {
        if auth_context.jwt_error.is_none() {
            has_global_admin_role =
                service::role::find_by_username(&data.database_connection, &auth_context.username)
                    .await
                    .ok()
                    .unwrap()
                    .iter()
                    .any(|role| role.role == model::auth::GLOBAL_ADMIN_ROLE);
        }
    }

    let auth_state = data.auth_state(auth_enabled && !has_global_admin_role);

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
