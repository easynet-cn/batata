//! Apollo notification service
//!
//! Provides long-polling notification mechanism for configuration updates.
//! Uses Apollo release_message table for notification IDs.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use tokio::sync::broadcast;
use tokio::time::timeout;

use crate::model::{
    ApolloConfigNotification, ApolloNotificationMessages, NotificationRequest, WatchedKey,
};
use crate::repository::release_repository;

/// Default long polling timeout in seconds
pub const DEFAULT_LONG_POLLING_TIMEOUT: u64 = 60;

/// Apollo notification service
///
/// Manages long-polling connections and sends notifications when configs change.
pub struct ApolloNotificationService {
    db: Arc<DatabaseConnection>,
    /// Notification ID counter per watched key
    notification_ids: DashMap<String, AtomicI64>,
    /// Broadcast channel for config change events
    change_sender: broadcast::Sender<ConfigChangeEvent>,
    /// Long polling timeout in seconds
    timeout_seconds: u64,
}

/// Config change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Watched key (appId+cluster+namespace)
    pub watched_key: String,
    /// New notification ID
    pub notification_id: i64,
}

impl ApolloNotificationService {
    /// Create a new notification service
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            db,
            notification_ids: DashMap::new(),
            change_sender: sender,
            timeout_seconds: DEFAULT_LONG_POLLING_TIMEOUT,
        }
    }

    /// Create with custom timeout
    pub fn with_timeout(db: Arc<DatabaseConnection>, timeout_seconds: u64) -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self {
            db,
            notification_ids: DashMap::new(),
            change_sender: sender,
            timeout_seconds,
        }
    }

    /// Get the broadcast sender for publishing change events
    pub fn change_sender(&self) -> broadcast::Sender<ConfigChangeEvent> {
        self.change_sender.clone()
    }

    /// Wait for notifications (long polling)
    ///
    /// Returns immediately if any config has changed since the client's last known notification ID.
    /// Otherwise, waits up to `timeout_seconds` for a change.
    pub async fn poll_notifications(
        &self,
        app_id: &str,
        cluster: &str,
        _env: Option<&str>,
        notifications: Vec<NotificationRequest>,
    ) -> Vec<ApolloConfigNotification> {
        // First check if any config has already changed
        let mut changed = Vec::new();
        let mut watched_keys = Vec::new();

        for req in &notifications {
            let watched_key = WatchedKey::new(
                app_id.to_string(),
                cluster.to_string(),
                req.namespace_name.clone(),
            );
            let key_str = watched_key.to_key_string();

            // Get or initialize notification ID
            let current_id = self
                .get_or_init_notification_id(&key_str, app_id, cluster, &req.namespace_name)
                .await;

            if current_id > req.notification_id {
                // Config has changed since client's last known version
                let mut messages = ApolloNotificationMessages::new();
                messages.add(key_str.clone(), current_id);

                changed.push(ApolloConfigNotification::with_messages(
                    req.namespace_name.clone(),
                    current_id,
                    messages,
                ));
            } else {
                watched_keys.push((key_str, req.namespace_name.clone()));
            }
        }

        // If any config has changed, return immediately
        if !changed.is_empty() {
            return changed;
        }

        // Otherwise, wait for changes via long polling
        let mut receiver = self.change_sender.subscribe();
        let watched_set: HashMap<String, String> = watched_keys.into_iter().collect();

        let poll_result = timeout(Duration::from_secs(self.timeout_seconds), async {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let Some(namespace_name) = watched_set.get(&event.watched_key) {
                            let mut messages = ApolloNotificationMessages::new();
                            messages.add(event.watched_key.clone(), event.notification_id);

                            return vec![ApolloConfigNotification::with_messages(
                                namespace_name.clone(),
                                event.notification_id,
                                messages,
                            )];
                        }
                    }
                    Err(_) => {
                        // Channel closed or lagged, return empty
                        return Vec::new();
                    }
                }
            }
        })
        .await;

        poll_result.unwrap_or_default()
    }

    /// Get or initialize notification ID for a watched key
    async fn get_or_init_notification_id(
        &self,
        key: &str,
        app_id: &str,
        cluster: &str,
        namespace: &str,
    ) -> i64 {
        // Check if we already have an ID
        if let Some(id_ref) = self.notification_ids.get(key) {
            return id_ref.load(Ordering::SeqCst);
        }

        // Initialize from database (use latest release message ID)
        let id = self
            .fetch_notification_id_from_db(app_id, cluster, namespace)
            .await
            .unwrap_or(1);

        self.notification_ids
            .entry(key.to_string())
            .or_insert_with(|| AtomicI64::new(id));

        id
    }

    /// Fetch notification ID from database (based on release message)
    async fn fetch_notification_id_from_db(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
    ) -> Option<i64> {
        // Use latest release message ID as notification ID
        let message_key = format!("{}+{}+{}", app_id, cluster, namespace);
        release_repository::find_latest_message_id_by_key(&self.db, &message_key)
            .await
            .ok()?
    }

    /// Notify that a config has changed
    ///
    /// Called when a config is published/updated.
    pub fn notify_change(&self, app_id: &str, cluster: &str, namespace: &str) {
        let watched_key = WatchedKey::new(
            app_id.to_string(),
            cluster.to_string(),
            namespace.to_string(),
        );
        let key_str = watched_key.to_key_string();

        // Increment notification ID
        let new_id = self
            .notification_ids
            .entry(key_str.clone())
            .or_insert_with(|| AtomicI64::new(0))
            .fetch_add(1, Ordering::SeqCst)
            + 1;

        // Broadcast change event
        let _ = self.change_sender.send(ConfigChangeEvent {
            watched_key: key_str,
            notification_id: new_id,
        });
    }

    /// Notify change from Nacos identifiers (backward compatibility)
    pub fn notify_change_nacos(&self, data_id: &str, group: &str, _namespace_id: &str) {
        // Parse dataId to extract appId and namespace
        if let Some((app_id, namespace)) = crate::mapping::from_nacos_data_id(data_id) {
            self.notify_change(&app_id, group, &namespace);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watched_key_format() {
        let key = WatchedKey::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
        );
        assert_eq!(key.to_key_string(), "app1+default+application");
    }

    #[test]
    fn test_config_change_event() {
        let event = ConfigChangeEvent {
            watched_key: "app1+default+application".to_string(),
            notification_id: 100,
        };
        assert_eq!(event.notification_id, 100);
    }

    #[test]
    fn test_config_change_event_clone() {
        let event = ConfigChangeEvent {
            watched_key: "app1+default+ns1".to_string(),
            notification_id: 42,
        };
        let cloned = event.clone();
        assert_eq!(cloned.watched_key, "app1+default+ns1");
        assert_eq!(cloned.notification_id, 42);
    }

    #[test]
    fn test_notification_request_parsing() {
        let json = r#"[
            {"namespaceName": "application", "notificationId": -1},
            {"namespaceName": "config.yaml", "notificationId": 100}
        ]"#;
        let requests: Vec<NotificationRequest> = serde_json::from_str(json).unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].namespace_name, "application");
        assert_eq!(requests[0].notification_id, -1);
        assert_eq!(requests[1].notification_id, 100);
    }

    #[test]
    fn test_notification_messages() {
        let mut messages = ApolloNotificationMessages::new();
        messages.add("app1+default+ns1".to_string(), 10);
        messages.add("app1+default+ns2".to_string(), 20);

        let json = serde_json::to_string(&messages).unwrap();
        assert!(json.contains("app1+default+ns1"));
        assert!(json.contains("app1+default+ns2"));
    }

    #[test]
    fn test_notification_response_serialization() {
        let notification = ApolloConfigNotification::new("application".to_string(), 42);
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"namespaceName\":\"application\""));
        assert!(json.contains("\"notificationId\":42"));
    }

    #[test]
    fn test_notification_with_messages_serialization() {
        let mut messages = ApolloNotificationMessages::new();
        messages.add("app1+default+application".to_string(), 42);

        let notification =
            ApolloConfigNotification::with_messages("application".to_string(), 42, messages);
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"messages\""));
        assert!(json.contains("\"details\""));
    }

    #[test]
    fn test_default_long_polling_timeout() {
        assert_eq!(DEFAULT_LONG_POLLING_TIMEOUT, 60);
    }
}
