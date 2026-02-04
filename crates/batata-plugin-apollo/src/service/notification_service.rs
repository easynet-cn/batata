//! Apollo notification service
//!
//! Provides long-polling notification mechanism for configuration updates.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use tokio::sync::broadcast;
use tokio::time::timeout;

use crate::mapping::ApolloMappingContext;
use crate::model::{
    ApolloConfigNotification, ApolloNotificationMessages, NotificationRequest, WatchedKey,
};

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
        env: Option<&str>,
        notifications: Vec<NotificationRequest>,
    ) -> Vec<ApolloConfigNotification> {
        // First check if any config has already changed
        let mut changed = Vec::new();
        let mut watched_keys = Vec::new();

        for req in &notifications {
            let ctx = ApolloMappingContext::new(app_id, cluster, &req.namespace_name, env);
            let watched_key = WatchedKey::new(
                app_id.to_string(),
                cluster.to_string(),
                ctx.namespace.clone(),
            );
            let key_str = watched_key.to_key_string();

            // Get or initialize notification ID
            let current_id = self.get_or_init_notification_id(&key_str, &ctx).await;

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

        match poll_result {
            Ok(notifications) => notifications,
            Err(_) => {
                // Timeout, return empty (client will retry)
                Vec::new()
            }
        }
    }

    /// Get or initialize notification ID for a watched key
    async fn get_or_init_notification_id(&self, key: &str, ctx: &ApolloMappingContext) -> i64 {
        // Check if we already have an ID
        if let Some(id_ref) = self.notification_ids.get(key) {
            return id_ref.load(Ordering::SeqCst);
        }

        // Initialize from database (use config's last modified timestamp as initial ID)
        let id = self.fetch_notification_id_from_db(ctx).await.unwrap_or(1);

        self.notification_ids
            .entry(key.to_string())
            .or_insert_with(|| AtomicI64::new(id));

        id
    }

    /// Fetch notification ID from database (based on config's modify_time)
    async fn fetch_notification_id_from_db(&self, ctx: &ApolloMappingContext) -> Option<i64> {
        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await
        .ok()??;

        // Use modify_time (already in milliseconds) as notification ID
        Some(config.modify_time)
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

    /// Notify change from Nacos identifiers
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
}
