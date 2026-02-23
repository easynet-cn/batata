//! V2 Config Listener API handlers
//!
//! Implements the Nacos V2 configuration change listener API endpoint:
//! - POST /nacos/v2/cs/config/listener - Listen for configuration changes

use actix_web::{HttpMessage, HttpRequest, Responder, post, web};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured,
};

/// Parse listener configs from the "Listening-Configs" form parameter
///
/// Format: dataId%01group%02md5%03timeout%01dataId%01group%02md5%03timeout...
/// %01, %02, %03 are delimiters (ASCII 1, 2, 3)
fn parse_listening_configs(listening_configs: &str) -> Vec<(String, String, String, u64)> {
    if listening_configs.is_empty() {
        return Vec::new();
    }

    let mut configs = Vec::new();
    // Split by %01 (config entries separator)
    let entries: Vec<&str> = listening_configs.split('\x01').collect();

    for entry in entries {
        if entry.is_empty() {
            continue;
        }

        // Split by %02 (fields separator)
        let fields: Vec<&str> = entry.split('\x02').collect();

        if fields.len() >= 4 {
            let data_id = fields[0].to_string();
            let group = fields[1].to_string();
            let md5 = fields[2].to_string();
            let timeout = fields[3].parse().unwrap_or(30000u64); // Default 30 seconds

            configs.push((data_id, group, md5, timeout));
        }
    }

    configs
}

/// Check if any configuration has changed
///
/// Returns a map of changed configs with their group key as key
async fn check_config_changes(
    db: &sea_orm::DatabaseConnection,
    configs: &[(String, String, String, u64)],
    namespace_id: &str,
) -> HashMap<String, String> {
    let mut changed_configs = HashMap::new();

    for (data_id, group, client_md5, _timeout) in configs {
        // Get current config from database
        match batata_config::service::config::find_one(db, data_id, group, namespace_id).await {
            Ok(Some(config)) => {
                let server_md5 = &config.config_info.config_info_base.md5;
                // MD5 mismatch means config has changed or doesn't exist
                if server_md5 != client_md5 {
                    let group_key = format!("{}+{}+{}", data_id, group, namespace_id);
                    changed_configs.insert(group_key, server_md5.clone());
                }
            }
            Ok(None) => {
                // Config doesn't exist, return empty MD5
                let group_key = format!("{}+{}+{}", data_id, group, namespace_id);
                changed_configs.insert(group_key, String::new());
            }
            Err(e) => {
                warn!(error = %e, "Failed to check config change");
            }
        }
    }

    changed_configs
}

/// Listen for configuration changes
///
/// POST /nacos/v2/cs/config/listener
///
/// Listens for configuration changes using long-polling mechanism.
/// Clients provide a list of configs with their current MD5 hashes,
/// and the server responds when any of the configs change.
#[post("listener")]
pub async fn config_listener(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Form<HashMap<String, String>>,
) -> impl Responder {
    // Parse the "Listening-Configs" parameter
    let listening_configs = match form.get("Listening-Configs") {
        Some(value) => value.clone(),
        None => {
            return Result::<String>::http_response(
                400,
                400,
                "Required parameter 'Listening-Configs' is missing".to_string(),
                String::new(),
            );
        }
    };

    // Parse listener configs
    let configs = parse_listening_configs(&listening_configs);

    if configs.is_empty() {
        // Empty config list (heartbeat) - just return success
        info!("Config listener heartbeat");
        return Result::<String>::http_success(String::new());
    }

    // Get namespace from request or use default
    // Note: tenant parameter can be passed via query string or form data
    let namespace_id = form
        .get("tenant")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| "public".to_string());

    // Check authorization for config read access
    let resource = format!("{}:*:config/*", namespace_id);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Check for config changes
    let db = data.db();
    let changed_configs = check_config_changes(db, &configs, &namespace_id).await;

    if !changed_configs.is_empty() {
        // Configs have changed, return immediately
        info!(count = changed_configs.len(), "Config changes detected");
        return Result::<HashMap<String, String>>::http_success(changed_configs);
    }

    // No changes yet - implement long polling
    // For simplicity, we check once more after a short delay
    // A full implementation would use a notification system
    let max_timeout = configs
        .iter()
        .map(|(_, _, _, timeout)| *timeout)
        .max()
        .unwrap_or(30000);

    // Poll with increasing intervals (adaptive polling)
    let intervals = vec![1000u64, 2000, 5000, 10000];
    let mut elapsed = 0u64;

    for interval in intervals {
        if elapsed + interval > max_timeout {
            tokio::time::sleep(Duration::from_millis(max_timeout - elapsed)).await;
            elapsed = max_timeout;
        } else {
            tokio::time::sleep(Duration::from_millis(interval)).await;
            elapsed += interval;
        }

        // Check for changes again
        let changed_configs = check_config_changes(db, &configs, &namespace_id).await;
        if !changed_configs.is_empty() {
            info!(
                count = changed_configs.len(),
                elapsed_ms = elapsed,
                "Config changes detected after polling"
            );
            return Result::<HashMap<String, String>>::http_success(changed_configs);
        }

        if elapsed >= max_timeout {
            break;
        }
    }

    // Timeout - return empty result
    info!(elapsed_ms = elapsed, "Config listener timeout, no changes");
    Result::<HashMap<String, String>>::http_success(HashMap::<String, String>::new())
}
