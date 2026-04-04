//! Long-polling config change notifier

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Notify;

/// Config change notifier for long-polling listeners
///
/// When a config changes, all waiters for that config key are notified.
/// Listeners use `tokio::sync::Notify` for efficient async wake-up.
#[derive(Clone)]
pub struct ConfigChangeNotifier {
    /// Key: "namespace+group+dataId", Value: notification handle
    notifiers: DashMap<String, Arc<Notify>>,
}

impl ConfigChangeNotifier {
    pub fn new() -> Self {
        Self {
            notifiers: DashMap::new(),
        }
    }

    /// Build notification key
    pub fn build_key(namespace: &str, group: &str, data_id: &str) -> String {
        format!("{namespace}+{group}+{data_id}")
    }

    /// Get or create a notification handle for a config key
    pub fn get_or_create(&self, key: &str) -> Arc<Notify> {
        self.notifiers
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Notify::new()))
            .clone()
    }

    /// Notify all waiters for a specific config
    pub fn notify_change(&self, namespace: &str, group: &str, data_id: &str) {
        let key = Self::build_key(namespace, group, data_id);
        if let Some(notify) = self.notifiers.get(&key) {
            notify.notify_waiters();
        }
    }

    /// Batch notify changes
    pub fn notify_changes(&self, keys: &[(String, String, String)]) {
        for (ns, group, data_id) in keys {
            self.notify_change(ns, group, data_id);
        }
    }

    /// Remove a notifier (for cleanup)
    pub fn remove(&self, key: &str) {
        self.notifiers.remove(key);
    }

    /// Number of active notifiers
    pub fn len(&self) -> usize {
        self.notifiers.len()
    }

    /// Whether no notifiers exist
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

    #[tokio::test]
    async fn test_notify_wakes_waiters() {
        let notifier = ConfigChangeNotifier::new();
        let key = ConfigChangeNotifier::build_key("public", "DEFAULT_GROUP", "app.yaml");

        let handle = notifier.get_or_create(&key);

        // Spawn a waiter
        let handle_clone = handle.clone();
        let waiter = tokio::spawn(async move {
            handle_clone.notified().await;
            true
        });

        // Give waiter time to start waiting
        tokio::task::yield_now().await;

        // Notify
        notifier.notify_change("public", "DEFAULT_GROUP", "app.yaml");

        let result = tokio::time::timeout(std::time::Duration::from_millis(100), waiter).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_or_create_reuses() {
        let notifier = ConfigChangeNotifier::new();
        let key = "public+DEFAULT_GROUP+app.yaml";

        let h1 = notifier.get_or_create(key);
        let h2 = notifier.get_or_create(key);

        // Should be the same Arc
        assert!(Arc::ptr_eq(&h1, &h2));
        assert_eq!(notifier.len(), 1);
    }
}
