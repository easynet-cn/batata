use std::{collections::HashMap, fs};

use actix_web::{Scope, get, web};
use serde::Deserialize;

use batata_server_common::model::{AppState, common};

pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
pub const GUIDE_FILE: &str = "console-guide.conf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageParam {
    language: String,
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

#[get("/state")]
async fn state(data: web::Data<AppState>) -> web::Json<HashMap<String, Option<String>>> {
    let state_map = data.console_datasource.server_state().await;
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
