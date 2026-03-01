//! V2 Config API routing configuration

use actix_web::{Scope, web};

use super::{capacity, config, history, listener};

/// Create the V2 Config Service routes
///
/// Routes:
/// - GET /nacos/v2/cs/config - Get config
/// - POST /nacos/v2/cs/config - Publish config
/// - DELETE /nacos/v2/cs/config - Delete config
/// - GET /nacos/v2/cs/config/searchDetail - Search config detail
/// - POST /nacos/v2/cs/config/listener - Listen for config changes
/// - GET /nacos/v2/cs/history/list - Get history list
/// - GET /nacos/v2/cs/history - Get specific history entry
/// - GET /nacos/v2/cs/history/detail - Get history detail with version comparison
/// - GET /nacos/v2/cs/history/previous - Get previous history entry
/// - GET /nacos/v2/cs/history/configs - Get all configs in namespace
/// - GET /nacos/v2/cs/capacity - Get capacity
/// - POST /nacos/v2/cs/capacity - Update capacity
/// - DELETE /nacos/v2/cs/capacity - Delete capacity
pub fn config_routes() -> Scope {
    web::scope("/v2/cs")
        .service(
            web::scope("/config")
                .service(config::get_config)
                .service(config::publish_config)
                .service(config::delete_config)
                .service(config::search_config_detail)
                .service(listener::config_listener),
        )
        .service(
            web::scope("/history")
                .service(history::get_history_list)
                .service(history::get_history)
                .service(history::get_history_detail)
                .service(history::get_previous_history)
                .service(history::get_namespace_configs),
        )
        .service(
            web::scope("/capacity")
                .service(capacity::get_capacity)
                .service(capacity::update_capacity)
                .service(capacity::delete_capacity),
        )
}
