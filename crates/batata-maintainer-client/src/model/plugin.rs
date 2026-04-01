// Plugin management model types matching Nacos PluginInfoVO / PluginDetailVO

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin summary information (list view).
///
/// Matches Nacos `PluginInfoVO`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginInfo {
    /// Plugin unique ID (format: "{type}:{name}")
    pub plugin_id: String,
    /// Plugin type (e.g., "auth", "config-encryption", "datasource-dialect")
    pub plugin_type: String,
    /// Plugin name
    pub plugin_name: String,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Whether the plugin is critical (cannot be disabled)
    pub critical: bool,
    /// Whether the plugin supports configuration
    pub configurable: bool,
    /// Whether only one plugin of this type can be active (exclusive group)
    pub exclusive: bool,
    /// Number of cluster nodes where this plugin is available
    pub available_node_count: Option<i32>,
    /// Total number of cluster nodes
    pub total_node_count: Option<i32>,
}

/// Plugin detail information (single view).
///
/// Matches Nacos `PluginDetailVO`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginDetail {
    /// Plugin unique ID
    pub plugin_id: String,
    /// Plugin type
    pub plugin_type: String,
    /// Plugin name
    pub plugin_name: String,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Whether the plugin is critical
    pub critical: bool,
    /// Whether the plugin supports configuration
    pub configurable: bool,
    /// Current configuration key-value pairs
    pub config: Option<HashMap<String, String>>,
    /// Configuration parameter definitions
    pub config_definitions: Option<Vec<ConfigItemDefinition>>,
}

/// Definition of a plugin configuration parameter.
///
/// Matches Nacos `ConfigItemDefinition`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigItemDefinition {
    /// Configuration key
    pub key: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Default value
    pub default_value: Option<String>,
    /// Value type
    #[serde(rename = "type")]
    pub item_type: String,
    /// Whether this parameter is required
    pub required: bool,
    /// Allowed values (when type is "ENUM")
    pub enum_values: Option<Vec<String>>,
}

/// Plugin availability status across cluster nodes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PluginAvailability {
    /// Plugin ID
    pub plugin_id: String,
    /// Whether the plugin is available on this node
    pub available: bool,
    /// Number of nodes where available
    pub available_count: Option<i32>,
    /// Total nodes in cluster
    pub total_count: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info_deserialize() {
        let json = r#"{
            "pluginId": "auth:nacos-auth",
            "pluginType": "auth",
            "pluginName": "nacos-auth",
            "enabled": true,
            "critical": true,
            "configurable": false,
            "exclusive": true,
            "availableNodeCount": 3,
            "totalNodeCount": 3
        }"#;
        let info: PluginInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.plugin_id, "auth:nacos-auth");
        assert_eq!(info.plugin_type, "auth");
        assert!(info.enabled);
        assert!(info.critical);
        assert!(info.exclusive);
        assert_eq!(info.available_node_count, Some(3));
    }

    #[test]
    fn test_plugin_detail_deserialize() {
        let json = r#"{
            "pluginId": "config-encryption:aes",
            "pluginType": "config-encryption",
            "pluginName": "aes",
            "enabled": true,
            "critical": false,
            "configurable": true,
            "config": {"keySize": "256"},
            "configDefinitions": [{
                "key": "keySize",
                "name": "Key Size",
                "description": "AES key size in bits",
                "defaultValue": "128",
                "type": "ENUM",
                "required": true,
                "enumValues": ["128", "192", "256"]
            }]
        }"#;
        let detail: PluginDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.plugin_name, "aes");
        assert!(detail.configurable);
        assert_eq!(
            detail.config.as_ref().unwrap().get("keySize").unwrap(),
            "256"
        );
        let defs = detail.config_definitions.as_ref().unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].key, "keySize");
        assert_eq!(defs[0].item_type, "ENUM");
        assert!(defs[0].required);
        assert_eq!(defs[0].enum_values.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn test_plugin_info_default() {
        let info = PluginInfo::default();
        assert!(info.plugin_id.is_empty());
        assert!(!info.enabled);
        assert!(!info.critical);
    }
}
