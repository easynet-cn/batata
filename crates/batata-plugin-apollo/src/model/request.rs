//! Apollo API request models
//!
//! These models define the request parameters for Apollo Config Service APIs.

use serde::{Deserialize, Serialize};

/// Query parameters for the config endpoint
///
/// GET `/configs/{appId}/{clusterName}/{namespace}`
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfigQueryParams {
    /// Data center (optional)
    #[serde(default)]
    pub data_center: Option<String>,

    /// Client's current release key for change detection
    #[serde(default = "default_release_key")]
    pub release_key: String,

    /// Client IP address
    #[serde(default)]
    pub ip: Option<String>,

    /// Client label
    #[serde(default)]
    pub label: Option<String>,

    /// Serialized notification messages (JSON)
    #[serde(default)]
    pub messages: Option<String>,
}

fn default_release_key() -> String {
    "-1".to_string()
}

/// Path parameters for the config endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigPathParams {
    /// Application ID
    pub app_id: String,

    /// Cluster name
    pub cluster_name: String,

    /// Namespace (may include format suffix like `.json`)
    pub namespace: String,
}

impl ConfigPathParams {
    /// Get the base namespace name (without format suffix)
    pub fn base_namespace(&self) -> &str {
        // Apollo convention: namespace ending with `.properties` is stripped
        self.namespace
            .strip_suffix(".properties")
            .unwrap_or(&self.namespace)
    }

    /// Check if namespace indicates a non-properties format
    pub fn has_format_suffix(&self) -> bool {
        let suffixes = [".json", ".yaml", ".yml", ".xml", ".txt"];
        suffixes.iter().any(|s| self.namespace.ends_with(s))
    }
}

/// Query parameters for the notifications endpoint
///
/// GET `/notifications/v2`
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationQueryParams {
    /// Application ID
    pub app_id: String,

    /// Cluster name
    pub cluster: String,

    /// JSON array of notification requests
    pub notifications: String,

    /// Data center (optional)
    #[serde(default)]
    pub data_center: Option<String>,

    /// Client IP address (optional)
    #[serde(default)]
    pub ip: Option<String>,
}

/// Single notification request in the notifications array
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRequest {
    /// Namespace name
    pub namespace_name: String,

    /// Client's current notification ID (-1 for initial request)
    pub notification_id: i64,
}

impl NotificationQueryParams {
    /// Parse the notifications JSON array
    pub fn parse_notifications(&self) -> Result<Vec<NotificationRequest>, serde_json::Error> {
        serde_json::from_str(&self.notifications)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_params_base_namespace() {
        let params = ConfigPathParams {
            app_id: "app1".to_string(),
            cluster_name: "default".to_string(),
            namespace: "application.properties".to_string(),
        };
        assert_eq!(params.base_namespace(), "application");

        let params2 = ConfigPathParams {
            app_id: "app1".to_string(),
            cluster_name: "default".to_string(),
            namespace: "application".to_string(),
        };
        assert_eq!(params2.base_namespace(), "application");
    }

    #[test]
    fn test_config_path_params_format_suffix() {
        let params_json = ConfigPathParams {
            app_id: "app1".to_string(),
            cluster_name: "default".to_string(),
            namespace: "config.json".to_string(),
        };
        assert!(params_json.has_format_suffix());

        let params_props = ConfigPathParams {
            app_id: "app1".to_string(),
            cluster_name: "default".to_string(),
            namespace: "application".to_string(),
        };
        assert!(!params_props.has_format_suffix());
    }

    #[test]
    fn test_notification_params_parse() {
        let params = NotificationQueryParams {
            app_id: "app1".to_string(),
            cluster: "default".to_string(),
            notifications: r#"[{"namespaceName":"application","notificationId":-1}]"#.to_string(),
            data_center: None,
            ip: None,
        };

        let notifications = params.parse_notifications().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].namespace_name, "application");
        assert_eq!(notifications[0].notification_id, -1);
    }
}
