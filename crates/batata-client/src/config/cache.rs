//! Config cache data for tracking per-config state and listeners.
//!
//! Matches Nacos Java `CacheData` with per-listener MD5 tracking,
//! server consistency state, and notification coordination flags.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use md5::{Digest, Md5};

use super::change_parser::{self, ConfigChangeEvent};
use super::listener::{ConfigChangeEventListener, ConfigChangeListener, ConfigResponse};

/// Resets a listener's `in_notifying` flag when a pending notification is
/// dropped.
///
/// Holding the flag in an `Arc` lets the notification outlive the cache entry
/// guard, and resetting it on drop means a panicking callback cannot leave the
/// listener permanently marked as "being notified" (which would silently stop
/// all future deliveries to it).
struct NotifyingReset(Arc<AtomicBool>);

impl Drop for NotifyingReset {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Wraps a config listener with per-listener state tracking.
///
/// Matches Nacos Java `ManagerListenerWrap`:
/// - `last_call_md5`: the MD5 of the content last delivered to this listener
/// - `last_content`: the content last delivered (for change event diff computation)
/// - `in_notifying`: prevents concurrent notification of the same listener
pub struct ManagerListenerWrap {
    /// The wrapped config listener.
    pub listener: Arc<dyn ConfigChangeListener>,
    /// MD5 of the content last delivered to this listener.
    pub last_call_md5: String,
    /// Content last delivered to this listener.
    pub last_content: String,
    /// Whether a notification is currently in progress for this listener.
    ///
    /// Shared with [`PendingConfigNotification`] so the flag stays set until
    /// the callback has actually run, even though the callback is invoked
    /// after the cache entry has been released.
    pub in_notifying: Arc<AtomicBool>,
}

impl ManagerListenerWrap {
    /// Create a new listener wrap with the given listener and initial state.
    pub fn new(listener: Arc<dyn ConfigChangeListener>, md5: &str, content: &str) -> Self {
        Self {
            listener,
            last_call_md5: md5.to_string(),
            last_content: content.to_string(),
            in_notifying: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Wraps a change event listener with per-listener state tracking.
pub struct ManagerChangeEventListenerWrap {
    /// The wrapped change event listener.
    pub listener: Arc<dyn ConfigChangeEventListener>,
    /// MD5 of the content last delivered to this listener.
    pub last_call_md5: String,
    /// Content last delivered to this listener.
    pub last_content: String,
    /// Whether a notification is currently in progress for this listener.
    pub in_notifying: Arc<AtomicBool>,
}

impl ManagerChangeEventListenerWrap {
    /// Create a new change-event listener wrap with the given listener and initial state.
    pub fn new(listener: Arc<dyn ConfigChangeEventListener>, md5: &str, content: &str) -> Self {
        Self {
            listener,
            last_call_md5: md5.to_string(),
            last_content: content.to_string(),
            in_notifying: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Cache entry for a single config item, tracking content, MD5, and listeners.
///
/// Matches Nacos Java `CacheData` with all state flags for coordinating
/// between server push notifications and the listen loop.
pub struct CacheData {
    /// Config data ID.
    pub data_id: String,
    /// Config group.
    pub group: String,
    /// Config tenant/namespace ID.
    pub tenant: String,
    /// Current config content.
    pub content: String,
    /// MD5 hash of the current content.
    pub md5: String,
    /// Config type (e.g., "properties", "yaml", "json") for change parsing
    pub config_type: String,
    /// Encrypted data key for encrypted configs
    pub encrypted_data_key: String,
    /// Listeners wrapped with per-listener MD5 tracking
    pub listeners: Vec<ManagerListenerWrap>,
    /// Change event listeners wrapped with per-listener state
    pub change_event_listeners: Vec<ManagerChangeEventListenerWrap>,

    // --- State flags matching Nacos Java CacheData ---
    /// Whether the client's MD5 is consistent (synced) with the server.
    /// Set to `false` when a listener is added, on server push, or on disconnect.
    /// Set to `true` after a successful batch listen confirms no changes.
    pub is_consistent_with_server: AtomicBool,

    /// Set by the server push handler (`ConfigChangeNotifyRequest`).
    /// The listen loop checks this flag and re-fetches if set.
    /// Reset to `false` at the start of each listen cycle.
    pub receive_notify_changed: AtomicBool,

    /// `true` during the first listen cycle (before any response from server).
    /// While initializing, listeners are NOT notified to avoid spurious events.
    pub is_initializing: bool,

    /// Marked for removal when all listeners have been removed.
    /// The listen loop will send an unsubscribe for discarded entries.
    pub is_discard: bool,

    /// Task ID for parallel listen grouping (future optimization).
    pub task_id: u32,
}

impl CacheData {
    /// Create a new CacheData for the given config key.
    pub fn new(data_id: &str, group: &str, tenant: &str) -> Self {
        Self {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: String::new(),
            md5: String::new(),
            config_type: String::new(),
            encrypted_data_key: String::new(),
            listeners: Vec::new(),
            change_event_listeners: Vec::new(),
            is_consistent_with_server: AtomicBool::new(false),
            receive_notify_changed: AtomicBool::new(false),
            is_initializing: true,
            is_discard: false,
            task_id: 0,
        }
    }

    /// Update the content and recompute the MD5 hash.
    /// Returns `true` if the content actually changed.
    /// Also marks as inconsistent with server when content changes.
    pub fn update_content(&mut self, content: &str) -> bool {
        let new_md5 = compute_md5(content);
        if new_md5 != self.md5 {
            self.content = content.to_string();
            self.md5 = new_md5;
            self.is_consistent_with_server
                .store(false, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Add a listener to this cache entry.
    ///
    /// The listener is wrapped with the current MD5 and content so that
    /// `check_listener_md5()` won't trigger a spurious first notification.
    pub fn add_listener(&mut self, listener: Arc<dyn ConfigChangeListener>) {
        let wrap = ManagerListenerWrap::new(listener, &self.md5, &self.content);
        self.listeners.push(wrap);
        // Mark inconsistent so the listen loop will re-check this config
        self.is_consistent_with_server
            .store(false, Ordering::Relaxed);
    }

    /// Add a change event listener.
    pub fn add_change_event_listener(&mut self, listener: Arc<dyn ConfigChangeEventListener>) {
        let wrap = ManagerChangeEventListenerWrap::new(listener, &self.md5, &self.content);
        self.change_event_listeners.push(wrap);
        self.is_consistent_with_server
            .store(false, Ordering::Relaxed);
    }

    /// Remove a specific listener by pointer equality.
    /// Returns true if the listener was found and removed.
    pub fn remove_listener(&mut self, listener: &Arc<dyn ConfigChangeListener>) -> bool {
        let before = self.listeners.len();
        self.listeners
            .retain(|w| !Arc::ptr_eq(&w.listener, listener));
        let removed = self.listeners.len() < before;
        if removed && !self.has_listeners() {
            self.is_discard = true;
            self.is_consistent_with_server
                .store(false, Ordering::Relaxed);
        }
        removed
    }

    /// Remove all listeners (returns the count of removed listeners).
    pub fn remove_all_listeners(&mut self) -> usize {
        let count = self.listeners.len() + self.change_event_listeners.len();
        self.listeners.clear();
        self.change_event_listeners.clear();
        if count > 0 {
            self.is_discard = true;
            self.is_consistent_with_server
                .store(false, Ordering::Relaxed);
        }
        count
    }

    /// Mark this entry as "the config does not exist on the server".
    ///
    /// The MD5 falls back to the empty "no content" sentinel — the same value a
    /// client sends for a config it has never seen. The server treats that
    /// sentinel as up to date for a missing config, so the entry converges
    /// instead of being reported as changed on every listen cycle.
    ///
    /// Listeners are deliberately NOT notified: there is no content to deliver.
    pub fn mark_missing(&mut self) {
        self.content.clear();
        self.md5.clear();
        self.config_type.clear();
        self.encrypted_data_key.clear();
        self.is_initializing = false;
        self.is_consistent_with_server
            .store(true, Ordering::Relaxed);
    }

    /// Check if there are any registered listeners.
    pub fn has_listeners(&self) -> bool {
        !self.listeners.is_empty() || !self.change_event_listeners.is_empty()
    }

    /// Collect the content notifications that are due, without invoking any
    /// listener.
    ///
    /// This is the core notification mechanism matching Nacos
    /// `CacheData.checkListenerMd5()`: a listener fires when its
    /// `last_call_md5` differs from the current MD5. Here the listener is
    /// marked as notified up front and handed back as a
    /// [`PendingConfigNotification`], which the caller delivers **after**
    /// releasing the cache entry — see
    /// [`PendingConfigNotification::deliver`] for why that matters.
    pub fn take_content_notifications(&mut self) -> Vec<PendingConfigNotification> {
        let response = ConfigResponse {
            data_id: self.data_id.clone(),
            group: self.group.clone(),
            tenant: self.tenant.clone(),
            content: self.content.clone(),
        };

        let mut pending = Vec::new();
        for wrap in &mut self.listeners {
            if wrap.last_call_md5 == self.md5 {
                continue;
            }
            // Skip if already notifying (prevent concurrent notification)
            if wrap
                .in_notifying
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                continue;
            }

            pending.push(PendingConfigNotification {
                listener: Arc::clone(&wrap.listener),
                response: response.clone(),
                in_notifying: Arc::clone(&wrap.in_notifying),
            });

            wrap.last_call_md5 = self.md5.clone();
            wrap.last_content = self.content.clone();
        }

        pending
    }

    /// Collect the change-event notifications that are due, without invoking
    /// any listener.
    ///
    /// Mirrors [`Self::take_content_notifications`]: the field-level diff is
    /// computed here (from the first listener's last content, matching the
    /// existing behavior) and the delivery itself is deferred to the caller.
    pub fn take_change_event_notifications(&mut self) -> Vec<PendingChangeEventNotification> {
        if self.change_event_listeners.is_empty() {
            return Vec::new();
        }

        let old_content = self
            .change_event_listeners
            .first()
            .map(|w| w.last_content.clone())
            .unwrap_or_default();

        let items =
            change_parser::parse_change_items(&self.config_type, &old_content, &self.content);
        if items.is_empty() {
            return Vec::new();
        }

        let event = ConfigChangeEvent {
            data_id: self.data_id.clone(),
            group: self.group.clone(),
            tenant: self.tenant.clone(),
            items,
        };

        let mut pending = Vec::new();
        for wrap in &mut self.change_event_listeners {
            // Skip if already notifying (prevent concurrent notification)
            if wrap
                .in_notifying
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                continue;
            }

            pending.push(PendingChangeEventNotification {
                listener: Arc::clone(&wrap.listener),
                event: event.clone(),
                in_notifying: Arc::clone(&wrap.in_notifying),
            });

            wrap.last_call_md5 = self.md5.clone();
            wrap.last_content = self.content.clone();
        }

        pending
    }

    /// Check if all listeners have been notified with the current MD5.
    pub fn check_listeners_md5_consistent(&self) -> bool {
        self.listeners.iter().all(|w| w.last_call_md5 == self.md5)
            && self
                .change_event_listeners
                .iter()
                .all(|w| w.last_call_md5 == self.md5)
    }

    /// Build the cache key for this config.
    pub fn key(&self) -> String {
        build_cache_key(&self.data_id, &self.group, &self.tenant)
    }
}

/// A content notification detached from its cache entry.
///
/// Listener callbacks must never run while the cache entry guard is held: the
/// callback runs on the client's listen-loop task, so a callback that
/// re-enters the same `BatataConfigService` (a `get_config`, another
/// `add_listener`, ...) deadlocks on the very DashMap shard being held, and a
/// callback that blocks stalls every other config on the client.
///
/// [`CacheData::take_content_notifications`] therefore marks the listener as
/// notified and returns everything needed to invoke it later.
pub struct PendingConfigNotification {
    /// The listener to invoke.
    pub listener: Arc<dyn ConfigChangeListener>,
    /// The config payload to hand to the listener.
    pub response: ConfigResponse,
    in_notifying: Arc<AtomicBool>,
}

impl PendingConfigNotification {
    /// Invoke the listener. Call only after the cache entry guard is released.
    pub fn deliver(self) {
        let _reset = NotifyingReset(Arc::clone(&self.in_notifying));
        self.listener.receive_config_info(self.response);
    }
}

/// A change-event notification detached from its cache entry.
///
/// See [`PendingConfigNotification`] for why delivery is deferred.
pub struct PendingChangeEventNotification {
    /// The listener to invoke.
    pub listener: Arc<dyn ConfigChangeEventListener>,
    /// The field-level change event to hand to the listener.
    pub event: ConfigChangeEvent,
    in_notifying: Arc<AtomicBool>,
}

impl PendingChangeEventNotification {
    /// Invoke the listener. Call only after the cache entry guard is released.
    pub fn deliver(self) {
        let _reset = NotifyingReset(Arc::clone(&self.in_notifying));
        self.listener.receive_config_change(self.event);
    }
}

/// Build a cache key from config identifiers.
pub fn build_cache_key(data_id: &str, group: &str, tenant: &str) -> String {
    if tenant.is_empty() {
        format!("{}+{}", data_id, group)
    } else {
        format!("{}+{}+{}", data_id, group, tenant)
    }
}

/// Compute MD5 hash of a string, returning the hex digest.
pub fn compute_md5(content: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    const_hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_md5() {
        let md5 = compute_md5("hello world");
        assert_eq!(md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_compute_md5_empty() {
        let md5 = compute_md5("");
        assert_eq!(md5, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_build_cache_key() {
        assert_eq!(build_cache_key("data-id", "group", ""), "data-id+group");
        assert_eq!(
            build_cache_key("data-id", "group", "tenant"),
            "data-id+group+tenant"
        );
    }

    #[test]
    fn test_cache_data_update_content() {
        let mut cache = CacheData::new("test", "DEFAULT_GROUP", "");
        assert!(cache.md5.is_empty());

        // First update: content changes
        assert!(cache.update_content("hello"));
        assert_eq!(cache.content, "hello");
        assert!(!cache.md5.is_empty());

        // Same content: no change
        assert!(!cache.update_content("hello"));

        // Different content: change
        assert!(cache.update_content("world"));
        assert_eq!(cache.content, "world");
    }

    #[test]
    fn test_cache_data_key() {
        let cache = CacheData::new("my-config", "DEFAULT_GROUP", "public");
        assert_eq!(cache.key(), "my-config+DEFAULT_GROUP+public");
    }

    #[test]
    fn test_cache_data_key_no_tenant() {
        let cache = CacheData::new("my-config", "DEFAULT_GROUP", "");
        assert_eq!(cache.key(), "my-config+DEFAULT_GROUP");
    }

    #[test]
    fn test_cache_data_new_defaults() {
        let cache = CacheData::new("id", "group", "tenant");
        assert_eq!(cache.data_id, "id");
        assert_eq!(cache.group, "group");
        assert_eq!(cache.tenant, "tenant");
        assert!(cache.content.is_empty());
        assert!(cache.md5.is_empty());
        assert!(cache.listeners.is_empty());
        assert!(!cache.is_consistent_with_server.load(Ordering::Relaxed));
        assert!(cache.is_initializing);
        assert!(!cache.is_discard);
    }

    #[test]
    fn test_cache_data_listeners() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let mut cache = CacheData::new("id", "group", "");
        assert!(!cache.has_listeners());

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let listener = Arc::new(super::super::listener::FnConfigChangeListener::new(
            move |_info: ConfigResponse| {
                called_clone.store(true, Ordering::SeqCst);
            },
        ));

        cache.add_listener(listener.clone());
        assert!(cache.has_listeners());
        assert_eq!(cache.listeners.len(), 1);

        // Add another
        cache.add_listener(listener);
        assert_eq!(cache.listeners.len(), 2);

        // Remove all
        let removed = cache.remove_all_listeners();
        assert_eq!(removed, 2);
        assert!(!cache.has_listeners());
    }

    #[test]
    fn test_cache_data_md5_consistency() {
        let mut cache = CacheData::new("id", "group", "");
        cache.update_content("test content");

        let expected_md5 = compute_md5("test content");
        assert_eq!(cache.md5, expected_md5);
    }

    #[test]
    fn test_compute_md5_different_content() {
        let md5a = compute_md5("content-a");
        let md5b = compute_md5("content-b");
        assert_ne!(md5a, md5b);
    }

    #[test]
    fn test_build_cache_key_special_chars() {
        assert_eq!(
            build_cache_key("data.id", "my-group", "ns:public"),
            "data.id+my-group+ns:public"
        );
    }

    #[test]
    fn test_check_listener_md5_notifies_on_change() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let mut cache = CacheData::new("id", "group", "");
        cache.update_content("initial");

        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();
        let listener = Arc::new(super::super::listener::FnConfigChangeListener::new(
            move |_: ConfigResponse| {
                cc.fetch_add(1, Ordering::SeqCst);
            },
        ));

        // Add listener with current MD5 — should NOT trigger
        cache.add_listener(listener);
        assert_eq!(deliver(cache.take_content_notifications()), 0);
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        // Change content — listener's last_call_md5 differs from cache.md5
        cache.update_content("changed");
        assert_eq!(deliver(cache.take_content_notifications()), 1);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Call again — should NOT trigger (md5 now matches)
        assert_eq!(deliver(cache.take_content_notifications()), 0);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_take_content_notifications_releases_before_delivery() {
        // Regression guard: the pending notification must carry everything
        // needed to invoke the listener, so the caller can drop the cache
        // entry guard first. A callback that re-enters the config service
        // (for example another `get_config`) would otherwise deadlock.
        let mut cache = CacheData::new("id", "group", "");
        cache.update_content("v1");

        let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let calls_clone = calls.clone();
        let listener = Arc::new(super::super::listener::FnConfigChangeListener::new(
            move |info: ConfigResponse| {
                assert_eq!(info.content, "v2");
                calls_clone.fetch_add(1, Ordering::SeqCst);
            },
        ));
        cache.add_listener(listener);
        assert_eq!(deliver(cache.take_content_notifications()), 0);

        cache.update_content("v2");
        let pending = cache.take_content_notifications();
        assert_eq!(pending.len(), 1);
        // Drop the (borrowed) cache before delivering, mimicking the caller.
        drop(cache);
        assert_eq!(deliver(pending), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// Deliver every pending notification, returning how many listeners ran.
    fn deliver(pending: Vec<PendingConfigNotification>) -> usize {
        let count = pending.len();
        for notification in pending {
            notification.deliver();
        }
        count
    }

    #[test]
    fn test_consistent_with_server_flag() {
        let mut cache = CacheData::new("id", "group", "");

        // Initially not consistent
        assert!(!cache.is_consistent_with_server.load(Ordering::Relaxed));

        // Set consistent
        cache
            .is_consistent_with_server
            .store(true, Ordering::Relaxed);
        assert!(cache.is_consistent_with_server.load(Ordering::Relaxed));

        // Update content resets to inconsistent
        cache.update_content("new");
        assert!(!cache.is_consistent_with_server.load(Ordering::Relaxed));
    }

    #[test]
    fn test_receive_notify_changed_flag() {
        let cache = CacheData::new("id", "group", "");
        assert!(!cache.receive_notify_changed.load(Ordering::Relaxed));

        cache.receive_notify_changed.store(true, Ordering::Relaxed);
        assert!(cache.receive_notify_changed.load(Ordering::Relaxed));
    }

    #[test]
    fn test_discard_on_remove_all_listeners() {
        use std::sync::Arc;

        let mut cache = CacheData::new("id", "group", "");
        let listener = Arc::new(super::super::listener::FnConfigChangeListener::new(
            |_: ConfigResponse| {},
        ));
        cache.add_listener(listener);
        assert!(!cache.is_discard);

        cache.remove_all_listeners();
        assert!(cache.is_discard);
    }
}
