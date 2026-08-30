//! Namespace model types for the maintainer client.

use serde::{Deserialize, Serialize};

/// Namespace information.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    /// The unique identifier of the namespace.
    pub namespace: String,
    /// The display name of the namespace.
    pub namespace_show_name: String,
    /// The description of the namespace.
    pub namespace_desc: String,
    /// The maximum number of configs allowed in this namespace.
    pub quota: i32,
    /// The current number of configs in this namespace.
    pub config_count: i64,
    /// The namespace type (reserved for future use).
    #[serde(rename = "type")]
    pub type_: i32,
}

impl Default for Namespace {
    fn default() -> Self {
        Self {
            namespace: "public".to_string(),
            namespace_show_name: "Public".to_string(),
            namespace_desc: "Public Namespace".to_string(),
            quota: 200,
            config_count: 0,
            type_: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_serialization() {
        let ns = Namespace::default();
        let json = serde_json::to_string(&ns).unwrap();
        assert!(json.contains("\"namespace\":\"public\""));
        assert!(json.contains("\"namespaceShowName\":\"Public\""));
        assert!(json.contains("\"type\":0"));

        let deserialized: Namespace = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.namespace, "public");
        assert_eq!(deserialized.quota, 200);
    }
}
