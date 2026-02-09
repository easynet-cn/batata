// Namespace model types

use serde::{Deserialize, Serialize};

/// Namespace information
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub namespace: String,
    pub namespace_show_name: String,
    pub namespace_desc: String,
    pub quota: i32,
    pub config_count: i32,
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
