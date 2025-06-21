use std::{collections::HashMap, fs};

use actix_web::{Scope, get, web};
use serde::Deserialize;

use crate::model::{common, common::AppState};

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

    // auth module state
    let auth_state = data.auth_state(false);

    for (k, v) in auth_state {
        state_map.insert(k, v);
    }

    // env module state
    let env_state = data.env_state();

    for (k, v) in env_state {
        state_map.insert(k, v);
    }

    //console module state
    let console_state = data.console_state();

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
