//! V3 Console server state API endpoints.
//!
//! Provides endpoints for server announcements, guides, and runtime state.

// actix-web route macros (`#[get]`, `#[post]`, `#[delete]`, ...) expand to a
// struct that cannot carry a doc comment, which trips `missing_docs`. The
// generated struct is an internal implementation detail, so the lint is allowed
// for this module.
#![allow(missing_docs)]

use std::{collections::HashMap, fs};

use actix_web::{Scope, get, web};
use serde::Deserialize;

use batata_server_common::model::{AppState, common};

/// File name of the server announcement message (loaded from `conf/`).
pub const ANNOUNCEMENT_FILE: &str = "announcement.conf";
/// File name of the console guide message (loaded from `conf/`).
pub const GUIDE_FILE: &str = "console-guide.conf";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageParam {
    #[serde(default = "default_language")]
    language: String,
}

fn default_language() -> String {
    "zh-CN".to_string()
}

/// Server state configuration
#[derive(Clone, Debug)]
pub struct ServerStateConfig {
    /// TCP port the console server listens on.
    pub server_port: u16,
    /// Whether the console UI is enabled.
    pub console_ui_enabled: bool,
    /// Enabled function modes (e.g. `naming,config`).
    pub function_mode: String,
    /// Whether authentication is enabled.
    pub auth_enabled: bool,
    /// Authentication system type (e.g. `nacos`).
    pub auth_system_type: String,
    /// Whether the login page is enabled.
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

/// Register the server state routes under `/server`.
pub fn routes() -> Scope {
    web::scope("/server")
        .service(state)
        .service(announcement)
        .service(guide)
}
