//! Config fuzzy watch - pattern-based config watching
//!
//! Allows subscribing to config changes using wildcard patterns.
//! When a config matching the pattern changes, the watcher is notified.

use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, error, info};

use batata_api::config::model::{
    ConfigFuzzyWatchChangeNotifyRequest, ConfigFuzzyWatchChangeNotifyResponse,
    ConfigFuzzyWatchRequest, ConfigFuzzyWatchResponse, ConfigFuzzyWatchSyncRequest,
    ConfigFuzzyWatchSyncResponse, Context,
};
use batata_api::grpc::Payload;
use batata_api::remote::model::ResponseTrait;

use crate::error::Result;
use crate::grpc::{GrpcClient, ServerPushHandler};

/// Watch types for fuzzy watch requests
pub const WATCH_TYPE_WATCH: &str = "WATCH";
pub const WATCH_TYPE_UNWATCH: &str = "UN_WATCH";

/// Change types for fuzzy watch notifications
pub const CHANGE_TYPE_ADD: &str = "ADD_CONFIG";
pub const CHANGE_TYPE_DELETE: &str = "DELETE_CONFIG";

/// Trait for receiving config fuzzy watch events
pub trait ConfigFuzzyWatchListener: Send + Sync + 'static {
    /// Called when a config matching the watched pattern changes
    fn on_change(&self, event: ConfigFuzzyWatchEvent);
}

/// Event for config fuzzy watch changes
#[derive(Debug, Clone)]
pub struct ConfigFuzzyWatchEvent {
    /// The group key of the changed config (format: "dataId+group+tenant")
    pub group_key: String,
    /// Type of change (ADD_CONFIG, DELETE_CONFIG)
    pub change_type: String,
}

/// Closure-based fuzzy watch listener
pub struct FnConfigFuzzyWatchListener<F>
where
    F: Fn(ConfigFuzzyWatchEvent) + Send + Sync + 'static,
{
    f: F,
}

impl<F> FnConfigFuzzyWatchListener<F>
where
    F: Fn(ConfigFuzzyWatchEvent) + Send + Sync + 'static,
{
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> ConfigFuzzyWatchListener for FnConfigFuzzyWatchListener<F>
where
    F: Fn(ConfigFuzzyWatchEvent) + Send + Sync + 'static,
{
    fn on_change(&self, event: ConfigFuzzyWatchEvent) {
        (self.f)(event);
    }
}

/// Config fuzzy watch data for a pattern
struct FuzzyWatchData {
    pattern: String,
    listeners: Vec<Arc<dyn ConfigFuzzyWatchListener>>,
    /// Group keys that the client has already received
    received_group_keys: HashSet<String>,
    is_initializing: bool,
}

/// Config fuzzy watch service
pub struct ConfigFuzzyWatchService {
    grpc_client: Arc<GrpcClient>,
    /// Watched patterns -> data
    watches: DashMap<String, FuzzyWatchData>,
}

impl ConfigFuzzyWatchService {
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self {
            grpc_client,
            watches: DashMap::new(),
        }
    }

    /// Register server push handlers on the gRPC client.
    /// Must be called after wrapping in `Arc`.
    pub fn register_push_handlers(self: &Arc<Self>) {
        self.grpc_client.register_push_handler(
            "ConfigFuzzyWatchChangeNotifyRequest",
            ConfigFuzzyWatchChangeNotifyHandler::new(self.clone()),
        );
        self.grpc_client.register_push_handler(
            "ConfigFuzzyWatchSyncRequest",
            ConfigFuzzyWatchSyncHandler::new(self.clone()),
        );
    }

    /// Start watching configs matching the given pattern
    pub async fn watch(
        &self,
        group_key_pattern: &str,
        listener: Arc<dyn ConfigFuzzyWatchListener>,
    ) -> Result<()> {
        let should_subscribe;
        {
            let mut entry = self
                .watches
                .entry(group_key_pattern.to_string())
                .or_insert_with(|| FuzzyWatchData {
                    pattern: group_key_pattern.to_string(),
                    listeners: Vec::new(),
                    received_group_keys: HashSet::new(),
                    is_initializing: true,
                });
            entry.listeners.push(listener);
            should_subscribe = entry.listeners.len() == 1;
        }

        if should_subscribe {
            self.send_watch_request(group_key_pattern, true).await?;
            debug!("Started fuzzy watch for pattern: {}", group_key_pattern);
        }

        Ok(())
    }

    /// Watch and wait for initial sync to complete, then return all matched group keys.
    ///
    /// Equivalent to Nacos `fuzzyWatchWithGroupKeys()`.
    /// The returned set contains all group keys matching the pattern that exist on the server.
    pub async fn watch_with_keys(
        &self,
        group_key_pattern: &str,
        listener: Arc<dyn ConfigFuzzyWatchListener>,
        timeout: std::time::Duration,
    ) -> Result<HashSet<String>> {
        self.watch(group_key_pattern, listener).await?;

        // Wait for initialization to complete (server sends FINISH_FUZZY_WATCH_INIT_NOTIFY)
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(entry) = self.watches.get(group_key_pattern) {
                if !entry.is_initializing {
                    return Ok(entry.received_group_keys.clone());
                }
            } else {
                return Ok(HashSet::new());
            }
            if tokio::time::Instant::now() > deadline {
                // Timeout: return whatever keys we have so far
                return Ok(self
                    .watches
                    .get(group_key_pattern)
                    .map(|e| e.received_group_keys.clone())
                    .unwrap_or_default());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Stop watching configs matching the given pattern
    pub async fn unwatch(&self, group_key_pattern: &str) -> Result<()> {
        if self.watches.remove(group_key_pattern).is_some() {
            self.send_watch_request(group_key_pattern, false).await?;
            debug!("Stopped fuzzy watch for pattern: {}", group_key_pattern);
        }
        Ok(())
    }

    /// Handle a fuzzy watch change notification from the server.
    ///
    /// `group_key` is in Nacos GroupKey format: `dataId+group+namespace`.
    pub fn handle_change_notify(&self, group_key: &str, change_type: &str) {
        let event = ConfigFuzzyWatchEvent {
            group_key: group_key.to_string(),
            change_type: change_type.to_string(),
        };

        // Parse group_key (format: dataId+group+namespace)
        let (data_id, group, namespace) = match parse_group_key(group_key) {
            Some(parts) => parts,
            None => {
                debug!("Cannot parse group_key: {}", group_key);
                return;
            }
        };

        // Track the received key for each matching watch pattern
        for mut entry in self.watches.iter_mut() {
            if matches_structured_pattern(&entry.pattern, &namespace, &group, &data_id) {
                entry.received_group_keys.insert(group_key.to_string());
                for listener in &entry.listeners {
                    listener.on_change(event.clone());
                }
            }
        }
    }

    /// Handle a fuzzy watch sync request from the server (batch notification)
    pub fn handle_sync_request(&self, pattern: &str, contexts: &HashSet<Context>) {
        if let Some(mut entry) = self.watches.get_mut(pattern) {
            for ctx in contexts {
                entry.received_group_keys.insert(ctx.group_key.clone());
                let event = ConfigFuzzyWatchEvent {
                    group_key: ctx.group_key.clone(),
                    change_type: ctx.change_type.clone(),
                };
                for listener in &entry.listeners {
                    listener.on_change(event.clone());
                }
            }
            entry.is_initializing = false;
        }
    }

    /// Re-establish all fuzzy watch subscriptions (called after reconnect)
    pub async fn redo(&self) -> Result<()> {
        let patterns: Vec<String> = self.watches.iter().map(|e| e.key().clone()).collect();

        for pattern in &patterns {
            if let Err(e) = self.send_watch_request(pattern, true).await {
                error!("Failed to redo fuzzy watch for pattern {}: {}", pattern, e);
            }
        }

        if !patterns.is_empty() {
            info!(
                "Re-established {} config fuzzy watch subscriptions",
                patterns.len()
            );
        }

        Ok(())
    }

    async fn send_watch_request(&self, pattern: &str, watch: bool) -> Result<()> {
        let received_keys = self
            .watches
            .get(pattern)
            .map(|e| e.received_group_keys.clone())
            .unwrap_or_default();

        let is_initializing = self
            .watches
            .get(pattern)
            .map(|e| e.is_initializing)
            .unwrap_or(true);

        let mut req = ConfigFuzzyWatchRequest::new();
        req.request.request_id = uuid::Uuid::new_v4().to_string();
        req.group_key_pattern = pattern.to_string();
        req.watch_type = if watch {
            WATCH_TYPE_WATCH.to_string()
        } else {
            WATCH_TYPE_UNWATCH.to_string()
        };
        req.received_group_keys = received_keys;
        req.initializing = is_initializing;

        let _resp: ConfigFuzzyWatchResponse = self.grpc_client.request_typed(&req).await?;
        Ok(())
    }
}

/// Parse a Nacos GroupKey (format: "dataId+group+namespace") into components.
fn parse_group_key(group_key: &str) -> Option<(String, String, String)> {
    // Nacos GroupKey format: dataId+group+namespace
    let parts: Vec<&str> = group_key.splitn(3, '+').collect();
    if parts.len() >= 2 {
        let data_id = parts[0].to_string();
        let group = parts[1].to_string();
        let namespace = if parts.len() > 2 {
            parts[2].to_string()
        } else {
            "public".to_string()
        };
        Some((data_id, group, namespace))
    } else {
        None
    }
}

/// Structured pattern matching against a fuzzy watch pattern.
///
/// Pattern format: "namespace>>group>>dataIdPattern" (Nacos FuzzyGroupKeyPattern)
/// Each segment is matched independently. `*` means match anything.
fn matches_structured_pattern(pattern: &str, namespace: &str, group: &str, data_id: &str) -> bool {
    let parts: Vec<&str> = pattern.splitn(3, ">>").collect();
    if parts.len() != 3 {
        return false;
    }
    let pat_ns = parts[0];
    let pat_group = parts[1];
    let pat_data_id = parts[2];

    // Namespace: exact match (normalized)
    let ns = if namespace.is_empty() {
        "public"
    } else {
        namespace
    };
    let pns = if pat_ns.is_empty() { "public" } else { pat_ns };
    if pns != ns {
        return false;
    }

    // Group: wildcard or exact
    if !item_matches(pat_group, group) {
        return false;
    }

    // DataId: wildcard or pattern match
    item_matches(pat_data_id, data_id)
}

/// Match a single segment with simple glob pattern (supports `*`)
fn item_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    matches_pattern(pattern, value)
}

/// Simple pattern matching: supports * as wildcard
fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == value;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(found) = value[pos..].find(part) {
            if i == 0 && found != 0 {
                return false;
            }
            pos += found + part.len();
        } else {
            return false;
        }
    }

    if let Some(last) = parts.last()
        && !last.is_empty()
    {
        return value.ends_with(last);
    }

    true
}

/// Server push handler for ConfigFuzzyWatchChangeNotifyRequest
pub struct ConfigFuzzyWatchChangeNotifyHandler {
    fuzzy_watch_service: Arc<ConfigFuzzyWatchService>,
}

impl ConfigFuzzyWatchChangeNotifyHandler {
    pub fn new(fuzzy_watch_service: Arc<ConfigFuzzyWatchService>) -> Self {
        Self {
            fuzzy_watch_service,
        }
    }
}

impl ServerPushHandler for ConfigFuzzyWatchChangeNotifyHandler {
    fn handle(&self, payload: &Payload) -> Option<Payload> {
        let req: ConfigFuzzyWatchChangeNotifyRequest = crate::grpc::deserialize_payload(payload);
        self.fuzzy_watch_service
            .handle_change_notify(&req.group_key, &req.change_type);

        let resp = ConfigFuzzyWatchChangeNotifyResponse::new();
        Some(resp.build_payload())
    }
}

/// Server push handler for ConfigFuzzyWatchSyncRequest
pub struct ConfigFuzzyWatchSyncHandler {
    fuzzy_watch_service: Arc<ConfigFuzzyWatchService>,
}

impl ConfigFuzzyWatchSyncHandler {
    pub fn new(fuzzy_watch_service: Arc<ConfigFuzzyWatchService>) -> Self {
        Self {
            fuzzy_watch_service,
        }
    }
}

impl ServerPushHandler for ConfigFuzzyWatchSyncHandler {
    fn handle(&self, payload: &Payload) -> Option<Payload> {
        let req: ConfigFuzzyWatchSyncRequest = crate::grpc::deserialize_payload(payload);
        self.fuzzy_watch_service
            .handle_sync_request(&req.group_key_pattern, &req.contexts);

        let resp = ConfigFuzzyWatchSyncResponse::new();
        Some(resp.build_payload())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn test_matches_pattern_exact() {
        assert!(matches_pattern("abc", "abc"));
        assert!(!matches_pattern("abc", "abcd"));
    }

    #[test]
    fn test_matches_pattern_wildcard_all() {
        assert!(matches_pattern("*", "anything"));
        assert!(matches_pattern("*", ""));
    }

    #[test]
    fn test_matches_pattern_prefix() {
        assert!(matches_pattern("prefix*", "prefix_hello"));
        assert!(!matches_pattern("prefix*", "hello_prefix"));
    }

    #[test]
    fn test_matches_pattern_suffix() {
        assert!(matches_pattern("*suffix", "hello_suffix"));
        assert!(!matches_pattern("*suffix", "suffix_hello"));
    }

    #[test]
    fn test_matches_pattern_middle() {
        assert!(matches_pattern("a*c", "abc"));
        assert!(matches_pattern("a*c", "aXYZc"));
        assert!(!matches_pattern("a*c", "aXYZd"));
    }

    #[test]
    fn test_matches_pattern_multiple_wildcards() {
        assert!(matches_pattern("a*b*c", "axbxc"));
        assert!(matches_pattern("a*b*c", "aXXbYYc"));
        assert!(!matches_pattern("a*b*c", "aXXcYYb"));
    }

    #[test]
    fn test_matches_pattern_empty() {
        assert!(matches_pattern("*", ""));
        assert!(!matches_pattern("a*", ""));
        assert!(matches_pattern("", ""));
        assert!(!matches_pattern("", "something"));
    }

    #[test]
    fn test_matches_pattern_complex() {
        // dataId+group+tenant pattern
        assert!(matches_pattern(
            "config-*+DEFAULT_GROUP*",
            "config-db+DEFAULT_GROUP+public"
        ));
        assert!(matches_pattern(
            "*+DEFAULT_GROUP+*",
            "anything+DEFAULT_GROUP+public"
        ));
    }

    #[test]
    fn test_fn_listener() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let listener = FnConfigFuzzyWatchListener::new(move |event: ConfigFuzzyWatchEvent| {
            assert_eq!(event.change_type, "ADD_CONFIG");
            called_clone.store(true, Ordering::SeqCst);
        });

        listener.on_change(ConfigFuzzyWatchEvent {
            group_key: "test+group".to_string(),
            change_type: "ADD_CONFIG".to_string(),
        });

        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_handle_change_notify_matching() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = ConfigFuzzyWatchService::new(client);

        // Pattern: namespace>>group>>dataIdPattern (Nacos structured format)
        let pattern = "public>>DEFAULT_GROUP>>config-*";
        service.watches.insert(
            pattern.to_string(),
            FuzzyWatchData {
                pattern: pattern.to_string(),
                listeners: vec![],
                received_group_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        // group_key format: dataId+group+namespace
        let group_key = "config-db+DEFAULT_GROUP+public";
        service.handle_change_notify(group_key, CHANGE_TYPE_ADD);

        // Check that the key was tracked
        let entry = service.watches.get(pattern).unwrap();
        assert!(entry.received_group_keys.contains(group_key));
    }

    #[test]
    fn test_handle_change_notify_non_matching() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = ConfigFuzzyWatchService::new(client);

        let pattern = "public>>DEFAULT_GROUP>>config-*";
        service.watches.insert(
            pattern.to_string(),
            FuzzyWatchData {
                pattern: pattern.to_string(),
                listeners: vec![],
                received_group_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        // Notify with non-matching dataId
        service.handle_change_notify("other-key+DEFAULT_GROUP+public", CHANGE_TYPE_ADD);

        let entry = service.watches.get(pattern).unwrap();
        assert!(entry.received_group_keys.is_empty());
    }

    #[test]
    fn test_handle_change_notify_with_listener() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = ConfigFuzzyWatchService::new(client);

        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();
        let group_key = "config-db+DEFAULT_GROUP+public";
        let listener: Arc<dyn ConfigFuzzyWatchListener> =
            Arc::new(FnConfigFuzzyWatchListener::new(move |event| {
                assert_eq!(event.group_key, "config-db+DEFAULT_GROUP+public");
                assert_eq!(event.change_type, CHANGE_TYPE_ADD);
                count_clone.fetch_add(1, Ordering::SeqCst);
            }));

        // Wildcard pattern: match all in public namespace, any group, any dataId
        let pattern = "public>>*>>*";
        service.watches.insert(
            pattern.to_string(),
            FuzzyWatchData {
                pattern: pattern.to_string(),
                listeners: vec![listener],
                received_group_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        service.handle_change_notify(group_key, CHANGE_TYPE_ADD);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_handle_sync_request() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = ConfigFuzzyWatchService::new(client);

        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();
        let listener: Arc<dyn ConfigFuzzyWatchListener> =
            Arc::new(FnConfigFuzzyWatchListener::new(move |_event| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }));

        service.watches.insert(
            "test-*".to_string(),
            FuzzyWatchData {
                pattern: "test-*".to_string(),
                listeners: vec![listener],
                received_group_keys: HashSet::new(),
                is_initializing: true,
            },
        );

        let mut contexts = HashSet::new();
        contexts.insert(Context {
            group_key: "test-1".to_string(),
            change_type: CHANGE_TYPE_ADD.to_string(),
        });
        contexts.insert(Context {
            group_key: "test-2".to_string(),
            change_type: CHANGE_TYPE_ADD.to_string(),
        });

        service.handle_sync_request("test-*", &contexts);

        // Listener called for each context
        assert_eq!(call_count.load(Ordering::SeqCst), 2);

        // Keys tracked
        let entry = service.watches.get("test-*").unwrap();
        assert!(entry.received_group_keys.contains("test-1"));
        assert!(entry.received_group_keys.contains("test-2"));
        // Initializing flag cleared
        assert!(!entry.is_initializing);
    }

    #[test]
    fn test_handle_sync_request_wrong_pattern() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = ConfigFuzzyWatchService::new(client);

        service.watches.insert(
            "test-*".to_string(),
            FuzzyWatchData {
                pattern: "test-*".to_string(),
                listeners: vec![],
                received_group_keys: HashSet::new(),
                is_initializing: true,
            },
        );

        // Sync for a different pattern - should not affect existing entry
        service.handle_sync_request("other-*", &HashSet::new());

        let entry = service.watches.get("test-*").unwrap();
        assert!(entry.is_initializing); // Still initializing
    }

    #[test]
    fn test_change_type_constants() {
        assert_eq!(CHANGE_TYPE_ADD, "ADD_CONFIG");
        assert_eq!(CHANGE_TYPE_DELETE, "DELETE_CONFIG");
        assert_eq!(WATCH_TYPE_WATCH, "WATCH");
        assert_eq!(WATCH_TYPE_UNWATCH, "UN_WATCH");
    }

    #[test]
    fn test_event_clone() {
        let event = ConfigFuzzyWatchEvent {
            group_key: "key1".to_string(),
            change_type: CHANGE_TYPE_ADD.to_string(),
        };
        let cloned = event.clone();
        assert_eq!(cloned.group_key, "key1");
        assert_eq!(cloned.change_type, CHANGE_TYPE_ADD);
    }

    #[test]
    fn test_multiple_listeners_notified() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = ConfigFuzzyWatchService::new(client);

        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));
        let c1 = count1.clone();
        let c2 = count2.clone();

        let l1: Arc<dyn ConfigFuzzyWatchListener> =
            Arc::new(FnConfigFuzzyWatchListener::new(move |_| {
                c1.fetch_add(1, Ordering::SeqCst);
            }));
        let l2: Arc<dyn ConfigFuzzyWatchListener> =
            Arc::new(FnConfigFuzzyWatchListener::new(move |_| {
                c2.fetch_add(1, Ordering::SeqCst);
            }));

        let pattern = "public>>*>>*";
        service.watches.insert(
            pattern.to_string(),
            FuzzyWatchData {
                pattern: pattern.to_string(),
                listeners: vec![l1, l2],
                received_group_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        service.handle_change_notify("any-key+DEFAULT_GROUP+public", CHANGE_TYPE_DELETE);

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }
}
