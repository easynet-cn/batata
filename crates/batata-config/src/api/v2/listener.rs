//! V2 Config Listener API handlers
//!
//! Implements the Batata V2 configuration change listener API endpoint (Nacos-compatible):
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
use tracing::{debug, info, warn};

use crate::service::notifier::{ConfigChangeNotifier, NotifyEntry};

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

/// Compare the client-supplied MD5s against one batch of server MD5s and return
/// the configs that differ. Kept as a pure helper so the diff logic is unit
/// testable without a persistence backend.
fn compute_changed_configs(
    configs: &[(String, String, String, u64)],
    server_md5s: &HashMap<String, String>,
    namespace_id: &str,
) -> HashMap<String, String> {
    let mut changed_configs = HashMap::new();
    for (data_id, group, client_md5, _) in configs {
        let group_key = batata_common::build_config_key(data_id, group, namespace_id);
        match server_md5s.get(&group_key) {
            Some(server_md5) if server_md5 != client_md5 => {
                changed_configs.insert(group_key, server_md5.clone());
            }
            None => {
                // Config doesn't exist on the server; treat as a change.
                changed_configs.insert(group_key, String::new());
            }
            _ => {} // MD5 matches, no change
        }
    }
    changed_configs
}

/// Check if any configuration has changed using a single batch MD5 query.
///
/// Returns a map of changed configs with their group key as key.
///
/// This is the persistence-backed fallback: `AppState` does not expose the
/// in-memory `ConfigCacheService`, and wiring one in would mean changing
/// files outside this crate's ownership. It therefore does one
/// `config_find_md5_batch` round-trip per call; the long-poll caller keeps
/// the number of invocations to the minimum needed for correctness.
async fn check_config_changes(
    persistence: &dyn PersistenceService,
    configs: &[(String, String, String, u64)],
    namespace_id: &str,
) -> HashMap<String, String> {
    let keys: Vec<(String, String, String)> = configs
        .iter()
        .map(|(data_id, group, _, _)| (data_id.clone(), group.clone(), namespace_id.to_string()))
        .collect();

    // ONE batched md5 read per call. The long-poll handler reuses this result and
    // limits the number of calls to the minimum needed for correctness.
    let server_md5s = match persistence.config_find_md5_batch(&keys).await {
        Ok(md5s) => md5s,
        Err(e) => {
            warn!(error = %e, "Failed to batch check config changes");
            return HashMap::new();
        }
    };

    compute_changed_configs(configs, &server_md5s, namespace_id)
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
        debug!("Config listener heartbeat");
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

    let max_timeout = configs
        .iter()
        .map(|(_, _, _, timeout)| *timeout)
        .max()
        .unwrap_or(30000);
    let timeout_duration = Duration::from_millis(max_timeout);

    // Collect Notify entries for all listened configs so we wake up on any change.
    let entries: Vec<Arc<NotifyEntry>> = configs
        .iter()
        .map(|(data_id, group, _, _)| {
            let key = ConfigChangeNotifier::build_key(&namespace_id, group, data_id);
            notifier.get_or_create(&key)
        })
        .collect();

    // Capture generations BEFORE the first config check. Every publish bumps a
    // generation before calling notify_waiters(), so comparing generations below
    // covers the whole window between a config check and waiter registration -
    // a lost-wakeup race that notify_waiters() alone cannot close. This is what
    // makes the former loop-top re-check unnecessary.
    let mut last_generations: Vec<u64> = entries.iter().map(|e| e.generation()).collect();

    // Initial check: return immediately if anything already changed. This is one
    // of the two batched md5 reads a request performs; the other is after the
    // wait. The redundant loop-top check has been removed.
    let changed_configs = check_config_changes(persistence, &configs, &namespace_id).await;
    if !changed_configs.is_empty() {
        info!(count = changed_configs.len(), "Config changes detected");
        return Result::<HashMap<String, String>>::http_success(changed_configs);
    }

    loop {
        // Register waiters, then re-read generations to catch a missed wakeup.
        let notified_futures = entries
            .iter()
            .map(|e| e.notify.notified())
            .collect::<Vec<_>>();
        let current_generations: Vec<u64> = entries.iter().map(|e| e.generation()).collect();
        if current_generations != last_generations {
            last_generations = current_generations;
            // A publish happened between our last check and waiter registration.
            // Re-check config immediately (no wait) to recover the missed wakeup;
            // if nothing relevant changed, loop again and keep waiting.
            let changed_configs = check_config_changes(persistence, &configs, &namespace_id).await;
            if !changed_configs.is_empty() {
                info!(count = changed_configs.len(), "Config changes detected");
                return Result::<HashMap<String, String>>::http_success(changed_configs);
            }
            continue;
        }

        // Wait for any config change notification or the client timeout.
        let _was_notified = tokio::time::timeout(timeout_duration, async {
            futures::future::select_all(notified_futures.into_iter().map(|f| {
                Box::pin(f) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            }))
            .await;
        })
        .await;

        // One fresh batched md5 read after the wait decides the response.
        let changed_configs = check_config_changes(persistence, &configs, &namespace_id).await;
        if !changed_configs.is_empty() {
            info!(
                count = changed_configs.len(),
                "Config changes detected after notification"
            );
            return Result::<HashMap<String, String>>::http_success(changed_configs);
        }

        // Timeout (or a spurious wakeup) with no change.
        debug!(
            timeout_ms = max_timeout,
            "Config listener timeout, no changes"
        );
        return Result::<HashMap<String, String>>::http_success(HashMap::<String, String>::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(data_id: &str, group: &str, md5: &str) -> (String, String, String, u64) {
        (
            data_id.to_string(),
            group.to_string(),
            md5.to_string(),
            30000,
        )
    }

    fn key(data_id: &str, group: &str, namespace: &str) -> String {
        batata_common::build_config_key(data_id, group, namespace)
    }

    // Mutant-checkable: removing either the mismatch arm or the missing-config
    // arm makes this test fail, and the unchanged/matching config guards the
    // success path (no spurious "changed" push).
    #[test]
    fn test_compute_changed_configs_reports_only_mismatch_and_missing() {
        let configs = vec![
            cfg("a.yaml", "G", "same-md5"),
            cfg("b.yaml", "G", "old-md5"),
            cfg("c.yaml", "G", "client-md5"),
        ];
        let mut server = HashMap::new();
        server.insert(key("a.yaml", "G", "public"), "same-md5".to_string());
        server.insert(key("b.yaml", "G", "public"), "new-md5".to_string());
        // c.yaml is intentionally absent -> treated as a change with an empty md5.

        let changed = compute_changed_configs(&configs, &server, "public");

        assert_eq!(
            changed.len(),
            2,
            "only mismatched and missing configs must be reported"
        );
        assert_eq!(
            changed.get(&key("b.yaml", "G", "public")),
            Some(&"new-md5".to_string())
        );
        assert_eq!(
            changed.get(&key("c.yaml", "G", "public")),
            Some(&String::new())
        );
        assert!(!changed.contains_key(&key("a.yaml", "G", "public")));
    }

    #[test]
    fn test_compute_changed_configs_success_path_no_changes() {
        let configs = vec![cfg("a.yaml", "G", "md5"), cfg("b.yaml", "G", "md5-2")];
        let mut server = HashMap::new();
        server.insert(key("a.yaml", "G", "public"), "md5".to_string());
        server.insert(key("b.yaml", "G", "public"), "md5-2".to_string());

        let changed = compute_changed_configs(&configs, &server, "public");
        assert!(
            changed.is_empty(),
            "matching md5s must not produce any changes"
        );
    }
}
