//! Client labels collector
//!
//! Collects and manages client-side labels for gray rule matching
//! and server-side identification. Compatible with Nacos DefaultLabelsCollectorManager.

use std::collections::HashMap;

/// Collects labels about the client for server-side identification.
///
/// Labels are sent during connection setup and used by the server
/// for gray/beta config matching (e.g., matching by client IP).
pub struct ClientLabelsCollector {
    /// Custom labels provided by the user
    custom_labels: HashMap<String, String>,
    /// Application name
    app_name: String,
}

impl ClientLabelsCollector {
    pub fn new() -> Self {
        Self {
            custom_labels: HashMap::new(),
            app_name: String::new(),
        }
    }

    pub fn with_app_name(mut self, name: &str) -> Self {
        self.app_name = name.to_string();
        self
    }

    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.custom_labels
            .insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.custom_labels.extend(labels);
        self
    }

    /// Collect all labels (system + custom)
    pub fn collect(&self) -> HashMap<String, String> {
        let mut labels = HashMap::new();

        // System labels
        labels.insert("AppName".to_string(), self.app_name.clone());
        labels.insert(
            "ClientVersion".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        labels.insert("ClientLanguage".to_string(), "Rust".to_string());

        // Local IP
        let local_ip = batata_common::local_ip();
        labels.insert("ClientIp".to_string(), local_ip);

        // OS info
        labels.insert("os.name".to_string(), std::env::consts::OS.to_string());
        labels.insert("os.arch".to_string(), std::env::consts::ARCH.to_string());

        // Custom labels override system labels
        labels.extend(self.custom_labels.clone());

        labels
    }

    /// Get the app name
    pub fn app_name(&self) -> &str {
        &self.app_name
    }
}

impl Default for ClientLabelsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_labels() {
        let collector = ClientLabelsCollector::new();
        let labels = collector.collect();
        assert!(labels.contains_key("ClientVersion"));
        assert!(labels.contains_key("ClientLanguage"));
        assert_eq!(labels.get("ClientLanguage").unwrap(), "Rust");
        assert!(labels.contains_key("ClientIp"));
    }

    #[test]
    fn test_custom_labels() {
        let collector = ClientLabelsCollector::new()
            .with_app_name("my-app")
            .with_label("env", "production")
            .with_label("region", "us-east-1");
        let labels = collector.collect();
        assert_eq!(labels.get("AppName").unwrap(), "my-app");
        assert_eq!(labels.get("env").unwrap(), "production");
        assert_eq!(labels.get("region").unwrap(), "us-east-1");
    }

    #[test]
    fn test_custom_labels_override_system() {
        let collector = ClientLabelsCollector::new().with_label("ClientLanguage", "CustomRust");
        let labels = collector.collect();
        assert_eq!(labels.get("ClientLanguage").unwrap(), "CustomRust");
    }

    #[test]
    fn test_with_labels_map() {
        let mut custom = HashMap::new();
        custom.insert("key1".to_string(), "val1".to_string());
        custom.insert("key2".to_string(), "val2".to_string());

        let collector = ClientLabelsCollector::new().with_labels(custom);
        let labels = collector.collect();
        assert_eq!(labels.get("key1").unwrap(), "val1");
        assert_eq!(labels.get("key2").unwrap(), "val2");
    }
}
