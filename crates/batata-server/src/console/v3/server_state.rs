use std::{collections::HashMap, fs};

use actix_web::{Scope, get, web};
use serde::Deserialize;

use crate::model::common::{self, AppState};

pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
pub const GUIDE_FILE: &str = "console-guide.conf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageParam {
    language: String,
}

#[get("/state")]
async fn state(data: web::Data<AppState>) -> web::Json<HashMap<String, Option<String>>> {
    let mut state_map = data.console_datasource.server_state().await;

    // When the remote datasource cannot authenticate (e.g. no admin user yet in
    // standalone_embedded mode), it returns an empty map.  Fall back to building
    // critical state fields from AppState so the frontend can still detect
    // auth_admin_request and show the admin-init page.
    if state_map.is_empty() {
        let has_admin = match data.persistence.as_ref() {
            Some(p) => p.role_has_global_admin().await.unwrap_or(false),
            None => false,
        };

        let is_admin_request = data.configuration.auth_enabled() && !has_admin;
        state_map.extend(data.auth_state(is_admin_request));
        state_map.extend(data.env_state());
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
