//! Config change notification service
//!
//! Enables server-side waiting for config changes in long-polling listeners.
//! When a config is published/updated/deleted, all waiting listeners for that
//! config key are notified immediately.

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Notify;

/// Global config change notifier for long-polling support.
///
/// Listeners register interest in specific config keys and wait for notifications.
/// When a config changes, all waiting listeners for that key are woken up.
pub struct ConfigChangeNotifier {
    /// Map of config key -> notification handle
    /// Key format: "{tenant}+{group}+{dataId}"
    notifiers: DashMap<String, Arc<Notify>>,
}

impl ConfigChangeNotifier {
    pub fn new() -> Self {
        Self {
            notifiers: DashMap::new(),
        }
    }

    /// Build a config key from its components
    pub fn build_key(tenant: &str, group: &str, data_id: &str) -> String {
        format!("{}+{}+{}", tenant, group, data_id)
    }

    /// Get or create a Notify handle for a config key
    pub fn get_or_create(&self, key: &str) -> Arc<Notify> {
        self.notifiers
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Notify::new()))
            .clone()
    }

    /// Notify all waiters for a specific config key
    pub fn notify_change(&self, tenant: &str, group: &str, data_id: &str) {
        let key = Self::build_key(tenant, group, data_id);
        if let Some(notify) = self.notifiers.get(&key) {
            notify.notify_waiters();
        }
    }

    /// Notify changes for multiple config keys at once
    pub fn notify_changes(&self, keys: &[(String, String, String)]) {
        for (tenant, group, data_id) in keys {
            self.notify_change(tenant, group, data_id);
        }
    }

    /// Remove a notifier entry (cleanup for rarely accessed configs)
    pub fn remove(&self, key: &str) {
        self.notifiers.remove(key);
    }

    /// Get current number of tracked config keys
    pub fn len(&self) -> usize {
        self.notifiers.len()
    }

    /// Check if there are no tracked keys
    pub fn is_empty(&self) -> bool {
        self.notifiers.is_empty()
    }
}

impl Default for ConfigChangeNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_key() {
        assert_eq!(
            ConfigChangeNotifier::build_key("public", "DEFAULT_GROUP", "app.yaml"),
            "public+DEFAULT_GROUP+app.yaml"
        );
        assert_eq!(
            ConfigChangeNotifier::build_key("", "DEFAULT_GROUP", "app.yaml"),
            "+DEFAULT_GROUP+app.yaml"
        );
    }

    #[test]
    fn test_get_or_create() {
        let notifier = ConfigChangeNotifier::new();
        let n1 = notifier.get_or_create("key1");
        let n2 = notifier.get_or_create("key1");
        // Same key should return the same Arc (same pointer)
        assert!(Arc::ptr_eq(&n1, &n2));
        assert_eq!(notifier.len(), 1);
    }

    #[test]
    fn test_notify_change_no_waiters() {
        let notifier = ConfigChangeNotifier::new();
        // Should not panic even if no one is waiting
        notifier.notify_change("public", "DEFAULT_GROUP", "app.yaml");
    }

    #[tokio::test]
    async fn test_notify_wakes_waiter() {
        let notifier = Arc::new(ConfigChangeNotifier::new());
        let key = "public+DEFAULT_GROUP+app.yaml";
        let notify = notifier.get_or_create(key);

        let notifier_clone = notifier.clone();
        let handle = tokio::spawn(async move {
            // Small delay then notify
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            notifier_clone.notify_change("public", "DEFAULT_GROUP", "app.yaml");
        });

        // Wait for notification with timeout
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(1), notify.notified()).await;

        assert!(result.is_ok(), "Should have been notified");
        handle.await.unwrap();
    }

    #[test]
    fn test_remove() {
        let notifier = ConfigChangeNotifier::new();
        notifier.get_or_create("key1");
        assert_eq!(notifier.len(), 1);
        notifier.remove("key1");
        assert_eq!(notifier.len(), 0);
    }

    #[test]
    fn test_notify_changes_batch() {
        let notifier = ConfigChangeNotifier::new();
        let keys = vec![
            ("public".to_string(), "G1".to_string(), "d1".to_string()),
            ("public".to_string(), "G2".to_string(), "d2".to_string()),
        ];
        // Should not panic
        notifier.notify_changes(&keys);
    }
}
