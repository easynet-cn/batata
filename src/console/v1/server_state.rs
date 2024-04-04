use crate::api::model::AppState;
use crate::common::model::RestResult;
use std::collections::HashMap;

use actix_web::{get, web};

#[get("/state")]
pub async fn state(data: web::Data<AppState>) -> web::Json<HashMap<String, String>> {
    let mut state_map: HashMap<String, String> = HashMap::new();

    // Auth state
    state_map.insert(
        "auth_enabled".to_string(),
        data.app_config
            .get_string("nacos.core.auth.enabled")
            .unwrap_or("false".to_string()),
    );
    state_map.insert(
        "login_page_enabled".to_string(),
        data.app_config
            .get_string("nacos.core.auth.enabled")
            .unwrap_or("false".to_string()),
    );
    state_map.insert(
        "auth_system_type".to_string(),
        data.app_config
            .get_string("nacos.core.auth.system.type")
            .unwrap_or_default(),
    );

    // Config state
    state_map.insert(
        "datasource_platform".to_string(),
        data.app_config
            .get_string("spring.sql.init.platform")
            .unwrap_or_default(),
    );
    state_map.insert(
        "plugin_datasource_log_enabled".to_string(),
        data.app_config
            .get_string("nacos.plugin.datasource.log.enabled")
            .unwrap_or("false".to_string()),
    );
    state_map.insert(
        "notifyConnectTimeout".to_string(),
        data.app_config
            .get_string("notifyConnectTimeout")
            .unwrap_or("100".to_string()),
    );
    state_map.insert(
        "notifySocketTimeout".to_string(),
        data.app_config
            .get_string("notifySocketTimeout")
            .unwrap_or("200".to_string()),
    );
    state_map.insert(
        "isHealthCheck".to_string(),
        data.app_config
            .get_string("isHealthCheck")
            .unwrap_or("true".to_string()),
    );
    state_map.insert(
        "maxHealthCheckFailCount".to_string(),
        data.app_config
            .get_string("maxHealthCheckFailCount")
            .unwrap_or("12".to_string()),
    );
    state_map.insert(
        "maxContent".to_string(),
        data.app_config
            .get_string("maxContent")
            .unwrap_or("12".to_string()),
    );
    state_map.insert(
        "isManageCapacity".to_string(),
        data.app_config
            .get_string("isManageCapacity")
            .unwrap_or("true".to_string()),
    );
    state_map.insert(
        "isCapacityLimitCheck".to_string(),
        data.app_config
            .get_string("isCapacityLimitCheck")
            .unwrap_or("false".to_string()),
    );
    state_map.insert(
        "defaultClusterQuota".to_string(),
        data.app_config
            .get_string("defaultClusterQuota")
            .unwrap_or("200".to_string()),
    );
    state_map.insert(
        "defaultMaxSize".to_string(),
        data.app_config
            .get_string("defaultMaxSize")
            .unwrap_or((100 * 1024).to_string()),
    );
    state_map.insert(
        "defaultMaxAggrCount".to_string(),
        data.app_config
            .get_string("defaultMaxAggrCount")
            .unwrap_or("10000".to_string()),
    );
    state_map.insert(
        "defaultMaxAggrSize".to_string(),
        data.app_config
            .get_string("defaultMaxAggrSize")
            .unwrap_or("1024".to_string()),
    );

    // Console state
    state_map.insert(
        "console_ui_enabled".to_string(),
        data.app_config
            .get_string("nacos.console.ui.enabled")
            .unwrap_or("true".to_string()),
    );

    // Env state
    state_map.insert(
        "startup_mode".to_string(),
        data.app_config
            .get_string("nacos.standalone")
            .unwrap_or("standalone".to_string()),
    );
    state_map.insert(
        "function_mode".to_string(),
        data.app_config
            .get_string("nacos.functionMode")
            .unwrap_or_default(),
    );
    state_map.insert(
        "version".to_string(),
        (env!("CARGO_PKG_VERSION")).to_string(),
    );
    state_map.insert(
        "server_port".to_string(),
        data.app_config
            .get_string("server.port")
            .unwrap_or("8848".to_string()),
    );

    web::Json(state_map)
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
