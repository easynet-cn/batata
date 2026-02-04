//! Apollo notification models
//!
//! These models support the long-polling notification mechanism.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Apollo configuration notification
///
/// Used for long-polling notification responses from `/notifications/v2`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApolloConfigNotification {
    /// Namespace name
    pub namespace_name: String,

    /// Notification ID (monotonically increasing)
    pub notification_id: i64,

    /// Optional messages map
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<ApolloNotificationMessages>,
}

impl ApolloConfigNotification {
    /// Create a new notification
    pub fn new(namespace_name: String, notification_id: i64) -> Self {
        Self {
            namespace_name,
            notification_id,
            messages: None,
        }
    }

    /// Create with messages
    pub fn with_messages(
        namespace_name: String,
        notification_id: i64,
        messages: ApolloNotificationMessages,
    ) -> Self {
        Self {
            namespace_name,
            notification_id,
            messages: Some(messages),
        }
    }
}

/// Notification messages containing details about watched keys
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApolloNotificationMessages {
    /// Map of watched keys to their notification IDs
    pub details: HashMap<String, i64>,
}

impl ApolloNotificationMessages {
    /// Create a new empty messages container
    pub fn new() -> Self {
        Self {
            details: HashMap::new(),
        }
    }

    /// Add a detail entry
    pub fn add(&mut self, key: String, notification_id: i64) {
        self.details.insert(key, notification_id);
    }

    /// Merge another messages container
    pub fn merge(&mut self, other: ApolloNotificationMessages) {
        for (key, id) in other.details {
            self.details
                .entry(key)
                .and_modify(|e| {
                    if id > *e {
                        *e = id;
                    }
                })
                .or_insert(id);
        }
    }
}

/// Watched key format for Apollo
///
/// Format: `{appId}+{cluster}+{namespace}`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchedKey {
    pub app_id: String,
    pub cluster: String,
    pub namespace: String,
}

impl WatchedKey {
    /// Create a new watched key
    pub fn new(app_id: String, cluster: String, namespace: String) -> Self {
        Self {
            app_id,
            cluster,
            namespace,
        }
    }

    /// Parse from string format: `{appId}+{cluster}+{namespace}`
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.len() == 3 {
            Some(Self {
                app_id: parts[0].to_string(),
                cluster: parts[1].to_string(),
                namespace: parts[2].to_string(),
            })
        } else {
            None
        }
    }

    /// Convert to string format: `{appId}+{cluster}+{namespace}`
    pub fn to_key_string(&self) -> String {
        format!("{}+{}+{}", self.app_id, self.cluster, self.namespace)
    }
}

impl std::fmt::Display for WatchedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}+{}+{}", self.app_id, self.cluster, self.namespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_serialization() {
        let notification = ApolloConfigNotification::new("application".to_string(), 100);
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("namespaceName"));
        assert!(json.contains("notificationId"));
        assert!(!json.contains("messages")); // None should be skipped
    }

    #[test]
    fn test_notification_with_messages() {
        let mut messages = ApolloNotificationMessages::new();
        messages.add("app1+default+application".to_string(), 100);

        let notification =
            ApolloConfigNotification::with_messages("application".to_string(), 100, messages);

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("messages"));
        assert!(json.contains("details"));
    }

    #[test]
    fn test_watched_key_parse() {
        let key = WatchedKey::parse("app1+default+application").unwrap();
        assert_eq!(key.app_id, "app1");
        assert_eq!(key.cluster, "default");
        assert_eq!(key.namespace, "application");
    }

    #[test]
    fn test_watched_key_to_string() {
        let key = WatchedKey::new(
            "app1".to_string(),
            "default".to_string(),
            "application".to_string(),
        );
        assert_eq!(key.to_key_string(), "app1+default+application");
    }

    #[test]
    fn test_watched_key_invalid_parse() {
        assert!(WatchedKey::parse("invalid").is_none());
        assert!(WatchedKey::parse("only+two").is_none());
    }
}
