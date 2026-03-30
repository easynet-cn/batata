//! Naming fuzzy watch - pattern-based service watching
//!
//! Allows subscribing to service changes using wildcard patterns
//! for service name and group name. When a service matching the pattern
//! changes, the watcher is notified.

use std::collections::HashSet;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, error, info};

use batata_api::grpc::Payload;
use batata_api::naming::model::{
    NamingContext, NamingFuzzyWatchChangeNotifyRequest, NamingFuzzyWatchChangeNotifyResponse,
    NamingFuzzyWatchRequest, NamingFuzzyWatchResponse, NamingFuzzyWatchSyncRequest,
    NamingFuzzyWatchSyncResponse,
};
use batata_api::remote::model::ResponseTrait;

use crate::error::Result;
use crate::grpc::{GrpcClient, ServerPushHandler};

/// Watch types
pub const WATCH_TYPE_WATCH: &str = "WATCH";
pub const WATCH_TYPE_UNWATCH: &str = "UN_WATCH";

/// Change types for naming fuzzy watch
pub const CHANGE_TYPE_ADD: &str = "ADD_SERVICE";
pub const CHANGE_TYPE_DELETE: &str = "DELETE_SERVICE";

/// Trait for receiving naming fuzzy watch events
pub trait NamingFuzzyWatchListener: Send + Sync + 'static {
    /// Called when a service matching the watched pattern changes
    fn on_change(&self, event: NamingFuzzyWatchEvent);
}

/// Event for naming fuzzy watch changes
#[derive(Debug, Clone)]
pub struct NamingFuzzyWatchEvent {
    /// The service key of the changed service (format: "groupName@@serviceName")
    pub service_key: String,
    /// Type of change (ADD_SERVICE, DELETE_SERVICE)
    pub change_type: String,
}

/// Closure-based fuzzy watch listener
pub struct FnNamingFuzzyWatchListener<F>
where
    F: Fn(NamingFuzzyWatchEvent) + Send + Sync + 'static,
{
    f: F,
}

impl<F> FnNamingFuzzyWatchListener<F>
where
    F: Fn(NamingFuzzyWatchEvent) + Send + Sync + 'static,
{
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> NamingFuzzyWatchListener for FnNamingFuzzyWatchListener<F>
where
    F: Fn(NamingFuzzyWatchEvent) + Send + Sync + 'static,
{
    fn on_change(&self, event: NamingFuzzyWatchEvent) {
        (self.f)(event);
    }
}

/// Key for a fuzzy watch subscription
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct WatchKey {
    namespace: String,
    group_name_pattern: String,
    service_name_pattern: String,
}

impl WatchKey {
    fn to_string_key(&self) -> String {
        format!(
            "{}#{}#{}",
            self.namespace, self.group_name_pattern, self.service_name_pattern
        )
    }
}

/// Fuzzy watch data for a pattern
struct FuzzyWatchData {
    key: WatchKey,
    listeners: Vec<Arc<dyn NamingFuzzyWatchListener>>,
    received_service_keys: HashSet<String>,
    is_initializing: bool,
}

/// Naming fuzzy watch service
pub struct NamingFuzzyWatchService {
    grpc_client: Arc<GrpcClient>,
    watches: DashMap<String, FuzzyWatchData>,
}

impl NamingFuzzyWatchService {
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self {
            grpc_client,
            watches: DashMap::new(),
        }
    }

    /// Start watching services matching the given patterns
    pub async fn watch(
        &self,
        namespace: &str,
        group_name_pattern: &str,
        service_name_pattern: &str,
        listener: Arc<dyn NamingFuzzyWatchListener>,
    ) -> Result<()> {
        let key = WatchKey {
            namespace: namespace.to_string(),
            group_name_pattern: group_name_pattern.to_string(),
            service_name_pattern: service_name_pattern.to_string(),
        };
        let string_key = key.to_string_key();

        let should_subscribe;
        {
            let mut entry =
                self.watches
                    .entry(string_key.clone())
                    .or_insert_with(|| FuzzyWatchData {
                        key: key.clone(),
                        listeners: Vec::new(),
                        received_service_keys: HashSet::new(),
                        is_initializing: true,
                    });
            entry.listeners.push(listener);
            should_subscribe = entry.listeners.len() == 1;
        }

        if should_subscribe {
            self.send_watch_request(&key, true).await?;
            debug!(
                "Started naming fuzzy watch: ns={}, group={}, service={}",
                namespace, group_name_pattern, service_name_pattern
            );
        }

        Ok(())
    }

    /// Watch and wait for initial sync to complete, then return all matched service keys.
    ///
    /// Equivalent to Nacos `fuzzyWatchWithServiceKeys()`.
    /// The returned set contains all service keys matching the pattern that exist on the server.
    pub async fn watch_with_keys(
        &self,
        namespace: &str,
        group_name_pattern: &str,
        service_name_pattern: &str,
        listener: Arc<dyn NamingFuzzyWatchListener>,
        timeout: std::time::Duration,
    ) -> Result<HashSet<String>> {
        self.watch(
            namespace,
            group_name_pattern,
            service_name_pattern,
            listener,
        )
        .await?;

        let string_key = WatchKey {
            namespace: namespace.to_string(),
            group_name_pattern: group_name_pattern.to_string(),
            service_name_pattern: service_name_pattern.to_string(),
        }
        .to_string_key();

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(entry) = self.watches.get(&string_key) {
                if !entry.is_initializing {
                    return Ok(entry.received_service_keys.clone());
                }
            } else {
                return Ok(HashSet::new());
            }
            if tokio::time::Instant::now() > deadline {
                return Ok(self
                    .watches
                    .get(&string_key)
                    .map(|e| e.received_service_keys.clone())
                    .unwrap_or_default());
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Stop watching services matching the given patterns
    pub async fn unwatch(
        &self,
        namespace: &str,
        group_name_pattern: &str,
        service_name_pattern: &str,
    ) -> Result<()> {
        let key = WatchKey {
            namespace: namespace.to_string(),
            group_name_pattern: group_name_pattern.to_string(),
            service_name_pattern: service_name_pattern.to_string(),
        };
        let string_key = key.to_string_key();

        if self.watches.remove(&string_key).is_some() {
            self.send_watch_request(&key, false).await?;
            debug!(
                "Stopped naming fuzzy watch: ns={}, group={}, service={}",
                namespace, group_name_pattern, service_name_pattern
            );
        }
        Ok(())
    }

    /// Handle a fuzzy watch change notification from the server
    pub fn handle_change_notify(&self, service_key: &str, change_type: &str) {
        let event = NamingFuzzyWatchEvent {
            service_key: service_key.to_string(),
            change_type: change_type.to_string(),
        };

        for mut entry in self.watches.iter_mut() {
            if matches_service_pattern(
                &entry.key.group_name_pattern,
                &entry.key.service_name_pattern,
                service_key,
            ) {
                entry.received_service_keys.insert(service_key.to_string());
                for listener in &entry.listeners {
                    listener.on_change(event.clone());
                }
            }
        }
    }

    /// Handle a fuzzy watch sync request from the server
    pub fn handle_sync_request(
        &self,
        namespace: &str,
        group_name_pattern: &str,
        service_name_pattern: &str,
        contexts: &HashSet<NamingContext>,
    ) {
        let key = WatchKey {
            namespace: namespace.to_string(),
            group_name_pattern: group_name_pattern.to_string(),
            service_name_pattern: service_name_pattern.to_string(),
        };
        let string_key = key.to_string_key();

        if let Some(mut entry) = self.watches.get_mut(&string_key) {
            for ctx in contexts {
                entry.received_service_keys.insert(ctx.service_key.clone());
                let event = NamingFuzzyWatchEvent {
                    service_key: ctx.service_key.clone(),
                    change_type: ctx.changed_type.clone(),
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
        let keys: Vec<WatchKey> = self.watches.iter().map(|e| e.key.clone()).collect();

        for key in &keys {
            if let Err(e) = self.send_watch_request(key, true).await {
                error!(
                    "Failed to redo naming fuzzy watch for {}: {}",
                    key.to_string_key(),
                    e
                );
            }
        }

        if !keys.is_empty() {
            info!(
                "Re-established {} naming fuzzy watch subscriptions",
                keys.len()
            );
        }

        Ok(())
    }

    async fn send_watch_request(&self, key: &WatchKey, watch: bool) -> Result<()> {
        let string_key = key.to_string_key();
        let received_keys = self
            .watches
            .get(&string_key)
            .map(|e| e.received_service_keys.clone())
            .unwrap_or_default();

        let is_initializing = self
            .watches
            .get(&string_key)
            .map(|e| e.is_initializing)
            .unwrap_or(true);

        let mut req = NamingFuzzyWatchRequest::new();
        req.request.request_id = uuid::Uuid::new_v4().to_string();
        req.namespace = key.namespace.clone();
        req.group_name_pattern = key.group_name_pattern.clone();
        req.service_name_pattern = key.service_name_pattern.clone();
        req.watch_type = if watch {
            WATCH_TYPE_WATCH.to_string()
        } else {
            WATCH_TYPE_UNWATCH.to_string()
        };
        req.received_service_keys = received_keys;
        req.initializing = is_initializing;

        let _resp: NamingFuzzyWatchResponse = self.grpc_client.request_typed(&req).await?;
        Ok(())
    }
}

/// Check if a service key matches the group/service name patterns
fn matches_service_pattern(group_pattern: &str, service_pattern: &str, service_key: &str) -> bool {
    // service_key format: "groupName@@serviceName"
    let parts: Vec<&str> = service_key.splitn(2, "@@").collect();
    if parts.len() != 2 {
        return false;
    }
    let group_name = parts[0];
    let service_name = parts[1];

    matches_wildcard(group_pattern, group_name) && matches_wildcard(service_pattern, service_name)
}

/// Simple wildcard matching: supports * as wildcard
fn matches_wildcard(pattern: &str, value: &str) -> bool {
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

/// Server push handler for NamingFuzzyWatchChangeNotifyRequest
pub struct NamingFuzzyWatchChangeNotifyHandler {
    fuzzy_watch_service: Arc<NamingFuzzyWatchService>,
}

impl NamingFuzzyWatchChangeNotifyHandler {
    pub fn new(fuzzy_watch_service: Arc<NamingFuzzyWatchService>) -> Self {
        Self {
            fuzzy_watch_service,
        }
    }
}

impl ServerPushHandler for NamingFuzzyWatchChangeNotifyHandler {
    fn handle(&self, payload: &Payload) -> Option<Payload> {
        let req: NamingFuzzyWatchChangeNotifyRequest = crate::grpc::deserialize_payload(payload);
        self.fuzzy_watch_service
            .handle_change_notify(&req.service_key, &req.changed_type);

        let resp = NamingFuzzyWatchChangeNotifyResponse::new();
        Some(resp.build_payload())
    }
}

/// Server push handler for NamingFuzzyWatchSyncRequest
pub struct NamingFuzzyWatchSyncHandler {
    fuzzy_watch_service: Arc<NamingFuzzyWatchService>,
}

impl NamingFuzzyWatchSyncHandler {
    pub fn new(fuzzy_watch_service: Arc<NamingFuzzyWatchService>) -> Self {
        Self {
            fuzzy_watch_service,
        }
    }
}

impl ServerPushHandler for NamingFuzzyWatchSyncHandler {
    fn handle(&self, payload: &Payload) -> Option<Payload> {
        let req: NamingFuzzyWatchSyncRequest = crate::grpc::deserialize_payload(payload);
        self.fuzzy_watch_service.handle_sync_request(
            &req.pattern_namespace,
            &req.pattern_service_name,
            &req.pattern_group_name,
            &req.contexts,
        );

        let resp = NamingFuzzyWatchSyncResponse::new();
        Some(resp.build_payload())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use batata_api::naming::model::NamingContext;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[test]
    fn test_matches_wildcard_exact() {
        assert!(matches_wildcard("hello", "hello"));
        assert!(!matches_wildcard("hello", "world"));
    }

    #[test]
    fn test_matches_wildcard_star() {
        assert!(matches_wildcard("*", "anything"));
        assert!(matches_wildcard("hello*", "hello_world"));
        assert!(matches_wildcard("*world", "hello_world"));
        assert!(!matches_wildcard("hello*", "world_hello"));
    }

    #[test]
    fn test_matches_wildcard_middle() {
        assert!(matches_wildcard("a*c", "abc"));
        assert!(matches_wildcard("a*c", "aXYZc"));
        assert!(!matches_wildcard("a*c", "aXYZd"));
    }

    #[test]
    fn test_matches_wildcard_multiple() {
        assert!(matches_wildcard("a*b*c", "axbxc"));
        assert!(matches_wildcard("a*b*c", "aXXbYYc"));
        assert!(!matches_wildcard("a*b*c", "axcxb"));
    }

    #[test]
    fn test_matches_wildcard_empty() {
        assert!(matches_wildcard("*", ""));
        assert!(matches_wildcard("", ""));
        assert!(!matches_wildcard("a", ""));
    }

    #[test]
    fn test_matches_service_pattern() {
        assert!(matches_service_pattern(
            "*",
            "*",
            "DEFAULT_GROUP@@my-service"
        ));
        assert!(matches_service_pattern(
            "DEFAULT_GROUP",
            "my-*",
            "DEFAULT_GROUP@@my-service"
        ));
        assert!(!matches_service_pattern(
            "OTHER_GROUP",
            "*",
            "DEFAULT_GROUP@@my-service"
        ));
    }

    #[test]
    fn test_matches_service_pattern_invalid_key() {
        // Missing @@ separator
        assert!(!matches_service_pattern("*", "*", "no-separator"));
    }

    #[test]
    fn test_matches_service_pattern_specific() {
        assert!(matches_service_pattern(
            "DEFAULT_GROUP",
            "user-service",
            "DEFAULT_GROUP@@user-service"
        ));
        assert!(!matches_service_pattern(
            "DEFAULT_GROUP",
            "user-service",
            "DEFAULT_GROUP@@order-service"
        ));
    }

    #[test]
    fn test_fn_listener() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let listener = FnNamingFuzzyWatchListener::new(move |event: NamingFuzzyWatchEvent| {
            assert_eq!(event.change_type, "ADD_SERVICE");
            called_clone.store(true, Ordering::SeqCst);
        });

        listener.on_change(NamingFuzzyWatchEvent {
            service_key: "DEFAULT_GROUP@@test".to_string(),
            change_type: "ADD_SERVICE".to_string(),
        });

        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_watch_key_to_string() {
        let key = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "DEFAULT_GROUP".to_string(),
            service_name_pattern: "my-*".to_string(),
        };
        assert_eq!(key.to_string_key(), "public#DEFAULT_GROUP#my-*");
    }

    #[test]
    fn test_watch_key_equality() {
        let key1 = WatchKey {
            namespace: "ns".to_string(),
            group_name_pattern: "g".to_string(),
            service_name_pattern: "s".to_string(),
        };
        let key2 = key1.clone();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_handle_change_notify_matching() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = NamingFuzzyWatchService::new(client);

        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let listener: Arc<dyn NamingFuzzyWatchListener> =
            Arc::new(FnNamingFuzzyWatchListener::new(move |event| {
                assert_eq!(event.service_key, "DEFAULT_GROUP@@my-service");
                assert_eq!(event.change_type, CHANGE_TYPE_ADD);
                c.fetch_add(1, Ordering::SeqCst);
            }));

        let key = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "*".to_string(),
            service_name_pattern: "*".to_string(),
        };
        service.watches.insert(
            key.to_string_key(),
            FuzzyWatchData {
                key,
                listeners: vec![listener],
                received_service_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        service.handle_change_notify("DEFAULT_GROUP@@my-service", CHANGE_TYPE_ADD);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_handle_change_notify_non_matching() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = NamingFuzzyWatchService::new(client);

        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let listener: Arc<dyn NamingFuzzyWatchListener> =
            Arc::new(FnNamingFuzzyWatchListener::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }));

        let key = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "OTHER_GROUP".to_string(),
            service_name_pattern: "*".to_string(),
        };
        service.watches.insert(
            key.to_string_key(),
            FuzzyWatchData {
                key,
                listeners: vec![listener],
                received_service_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        service.handle_change_notify("DEFAULT_GROUP@@my-service", CHANGE_TYPE_ADD);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_handle_change_notify_tracks_keys() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = NamingFuzzyWatchService::new(client);

        let key = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "*".to_string(),
            service_name_pattern: "*".to_string(),
        };
        let string_key = key.to_string_key();
        service.watches.insert(
            string_key.clone(),
            FuzzyWatchData {
                key,
                listeners: vec![],
                received_service_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        service.handle_change_notify("DEFAULT_GROUP@@svc1", CHANGE_TYPE_ADD);
        service.handle_change_notify("DEFAULT_GROUP@@svc2", CHANGE_TYPE_ADD);

        let entry = service.watches.get(&string_key).unwrap();
        assert!(entry.received_service_keys.contains("DEFAULT_GROUP@@svc1"));
        assert!(entry.received_service_keys.contains("DEFAULT_GROUP@@svc2"));
    }

    #[test]
    fn test_handle_sync_request() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = NamingFuzzyWatchService::new(client);

        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let listener: Arc<dyn NamingFuzzyWatchListener> =
            Arc::new(FnNamingFuzzyWatchListener::new(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }));

        let key = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "*".to_string(),
            service_name_pattern: "*".to_string(),
        };
        let string_key = key.to_string_key();
        service.watches.insert(
            string_key.clone(),
            FuzzyWatchData {
                key,
                listeners: vec![listener],
                received_service_keys: HashSet::new(),
                is_initializing: true,
            },
        );

        let mut contexts = HashSet::new();
        contexts.insert(NamingContext {
            service_key: "DEFAULT_GROUP@@svc1".to_string(),
            changed_type: CHANGE_TYPE_ADD.to_string(),
        });
        contexts.insert(NamingContext {
            service_key: "DEFAULT_GROUP@@svc2".to_string(),
            changed_type: CHANGE_TYPE_ADD.to_string(),
        });

        service.handle_sync_request("public", "*", "*", &contexts);

        assert_eq!(count.load(Ordering::SeqCst), 2);

        let entry = service.watches.get(&string_key).unwrap();
        assert!(!entry.is_initializing);
        assert_eq!(entry.received_service_keys.len(), 2);
    }

    #[test]
    fn test_handle_sync_request_wrong_key() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = NamingFuzzyWatchService::new(client);

        let key = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "*".to_string(),
            service_name_pattern: "*".to_string(),
        };
        let string_key = key.to_string_key();
        service.watches.insert(
            string_key.clone(),
            FuzzyWatchData {
                key,
                listeners: vec![],
                received_service_keys: HashSet::new(),
                is_initializing: true,
            },
        );

        // Different namespace
        service.handle_sync_request("other-ns", "*", "*", &HashSet::new());

        let entry = service.watches.get(&string_key).unwrap();
        assert!(entry.is_initializing); // Still initializing
    }

    #[test]
    fn test_change_type_constants() {
        assert_eq!(CHANGE_TYPE_ADD, "ADD_SERVICE");
        assert_eq!(CHANGE_TYPE_DELETE, "DELETE_SERVICE");
        assert_eq!(WATCH_TYPE_WATCH, "WATCH");
        assert_eq!(WATCH_TYPE_UNWATCH, "UN_WATCH");
    }

    #[test]
    fn test_event_debug() {
        let event = NamingFuzzyWatchEvent {
            service_key: "DEFAULT_GROUP@@svc".to_string(),
            change_type: CHANGE_TYPE_DELETE.to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("DEFAULT_GROUP@@svc"));
        assert!(debug_str.contains("DELETE_SERVICE"));
    }

    #[test]
    fn test_multiple_watches_notified() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let service = NamingFuzzyWatchService::new(client);

        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));
        let c1 = count1.clone();
        let c2 = count2.clone();

        // Watch 1: all services
        let key1 = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "*".to_string(),
            service_name_pattern: "*".to_string(),
        };
        service.watches.insert(
            key1.to_string_key(),
            FuzzyWatchData {
                key: key1,
                listeners: vec![Arc::new(FnNamingFuzzyWatchListener::new(move |_| {
                    c1.fetch_add(1, Ordering::SeqCst);
                }))],
                received_service_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        // Watch 2: specific group
        let key2 = WatchKey {
            namespace: "public".to_string(),
            group_name_pattern: "DEFAULT_GROUP".to_string(),
            service_name_pattern: "*".to_string(),
        };
        service.watches.insert(
            key2.to_string_key(),
            FuzzyWatchData {
                key: key2,
                listeners: vec![Arc::new(FnNamingFuzzyWatchListener::new(move |_| {
                    c2.fetch_add(1, Ordering::SeqCst);
                }))],
                received_service_keys: HashSet::new(),
                is_initializing: false,
            },
        );

        service.handle_change_notify("DEFAULT_GROUP@@my-service", CHANGE_TYPE_ADD);

        // Both watches should match
        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }
}
