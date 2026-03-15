//! V2 Config Listener API handlers
//!
//! Implements the Nacos V2 configuration change listener API endpoint:
//! - POST /nacos/v2/cs/config/listener - Listen for configuration changes

use actix_web::{HttpRequest, Responder, post, web};
use batata_persistence::PersistenceService;
use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType,
    model::{AppState, response::Result},
    secured,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use crate::service::notifier::ConfigChangeNotifier;

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

/// Check if any configuration has changed using batch MD5 query.
///
/// Returns a map of changed configs with their group key as key.
async fn check_config_changes(
    persistence: &dyn PersistenceService,
    configs: &[(String, String, String, u64)],
    namespace_id: &str,
) -> HashMap<String, String> {
    let keys: Vec<(String, String, String)> = configs
        .iter()
        .map(|(data_id, group, _, _)| (data_id.clone(), group.clone(), namespace_id.to_string()))
        .collect();

    let server_md5s = match persistence.config_find_md5_batch(&keys).await {
        Ok(md5s) => md5s,
        Err(e) => {
            warn!(error = %e, "Failed to batch check config changes");
            return HashMap::new();
        }
    };

    let mut changed_configs = HashMap::new();
    for (data_id, group, client_md5, _) in configs {
        let group_key = format!("{}+{}+{}", data_id, group, namespace_id);
        match server_md5s.get(&group_key) {
            Some(server_md5) if server_md5 != client_md5 => {
                changed_configs.insert(group_key, server_md5.clone());
            }
            None => {
                // Config doesn't exist
                changed_configs.insert(group_key, String::new());
            }
            _ => {} // MD5 matches, no change
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
///
/// When no changes are detected on the initial check, the server waits
/// for a notification from `ConfigChangeNotifier` (triggered on config
/// publish/update/delete) or until the client-specified timeout expires.
/// This avoids wasteful adaptive polling and delivers changes immediately.
#[post("listener")]
pub async fn config_listener(
    req: HttpRequest,
    data: web::Data<AppState>,
    notifier: web::Data<Arc<ConfigChangeNotifier>>,
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
    let persistence = data.persistence();
    let changed_configs = check_config_changes(persistence, &configs, &namespace_id).await;

    if !changed_configs.is_empty() {
        // Configs have changed, return immediately
        info!(count = changed_configs.len(), "Config changes detected");
        return Result::<HashMap<String, String>>::http_success(changed_configs);
    }

    // No changes yet - wait for notifications using ConfigChangeNotifier.
    // Collect Notify handles for all listened configs so we wake up on any change.
    let max_timeout = configs
        .iter()
        .map(|(_, _, _, timeout)| *timeout)
        .max()
        .unwrap_or(30000);

    // Build notify futures for each config key
    let notify_handles: Vec<_> = configs
        .iter()
        .map(|(data_id, group, _, _)| {
            let key = ConfigChangeNotifier::build_key(&namespace_id, group, data_id);
            notifier.get_or_create(&key)
        })
        .collect();

    // Wait for any config change notification or timeout
    let timeout_duration = Duration::from_millis(max_timeout);

    let notified_futures = notify_handles
        .iter()
        .map(|n| n.notified())
        .collect::<Vec<_>>();

    // Use tokio::select! with a timeout: wake on the first notification or timeout
    let _was_notified = tokio::time::timeout(timeout_duration, async {
        // Wait for ANY of the notified futures to complete
        // We use a select-like approach: spawn a future that completes when any notify fires
        futures::future::select_all(notified_futures.into_iter().map(|f| {
            Box::pin(f) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        }))
        .await;
    })
    .await;

    // Re-check for changes after being notified (or after timeout)
    let changed_configs = check_config_changes(persistence, &configs, &namespace_id).await;

    if !changed_configs.is_empty() {
        info!(
            count = changed_configs.len(),
            "Config changes detected after notification"
        );
        return Result::<HashMap<String, String>>::http_success(changed_configs);
    }

    // Timeout - return empty result
    info!(
        timeout_ms = max_timeout,
        "Config listener timeout, no changes"
    );
    Result::<HashMap<String, String>>::http_success(HashMap::<String, String>::new())
}
