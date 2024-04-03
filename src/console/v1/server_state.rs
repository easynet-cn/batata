use crate::api::model::AppState;
use crate::common::model::RestResult;
use std::collections::HashMap;

use actix_web::{get, web};

#[get("/state")]
pub async fn state(data: web::Data<AppState>) -> web::Json<HashMap<String, String>> {
    let mut config_map: HashMap<String, String> = HashMap::new();

    config_map.insert(
        "datasource_platform".to_string(),
        data.app_config
            .get_string("spring.sql.init.platform")
            .unwrap_or_default(),
    );
    config_map.insert(
        "console_ui_enabled".to_string(),
        data.app_config
            .get_string("nacos.console.ui.enabled")
            .unwrap_or("true".to_string()),
    );

    web::Json(config_map)
}

#[get("/announcement")]
pub async fn announcement() -> web::Json<RestResult<String>> {
    let rest_result = RestResult::<String>::success("".to_string());

    web::Json(rest_result)
}

#[get("/guide")]
pub async fn guide() -> web::Json<RestResult<String>> {
    let rest_result = RestResult::<String>::success("".to_string());

    web::Json(rest_result)
}
