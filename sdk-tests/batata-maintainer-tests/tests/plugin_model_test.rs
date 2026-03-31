//! Plugin model type tests (no server required).

use batata_maintainer_client::model::plugin::{
    ConfigItemDefinition, PluginAvailability, PluginDetail, PluginInfo,
};

#[test]
fn test_plugin_info_deserialization() {
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
    assert_eq!(info.plugin_name, "nacos-auth");
    assert!(info.enabled);
    assert!(info.critical);
    assert!(!info.configurable);
    assert!(info.exclusive);
    assert_eq!(info.available_node_count, Some(3));
    assert_eq!(info.total_node_count, Some(3));
}

#[test]
fn test_plugin_info_deserialization_minimal() {
    let json = r#"{
        "pluginId": "test:test-plugin",
        "pluginType": "test",
        "pluginName": "test-plugin",
        "enabled": false,
        "critical": false,
        "configurable": false,
        "exclusive": false
    }"#;

    let info: PluginInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.plugin_name, "test-plugin");
    assert!(!info.enabled);
    assert!(info.available_node_count.is_none());
    assert!(info.total_node_count.is_none());
}

#[test]
fn test_plugin_detail_with_config() {
    let json = r#"{
        "pluginId": "config-encryption:aes",
        "pluginType": "config-encryption",
        "pluginName": "aes",
        "enabled": true,
        "critical": false,
        "configurable": true,
        "config": {
            "keySize": "256",
            "mode": "CBC"
        },
        "configDefinitions": [
            {
                "key": "keySize",
                "name": "Key Size",
                "description": "AES key size in bits",
                "defaultValue": "128",
                "type": "ENUM",
                "required": true,
                "enumValues": ["128", "192", "256"]
            },
            {
                "key": "mode",
                "name": "Cipher Mode",
                "type": "STRING",
                "required": false
            }
        ]
    }"#;

    let detail: PluginDetail = serde_json::from_str(json).unwrap();
    assert_eq!(detail.plugin_name, "aes");
    assert!(detail.configurable);

    let config = detail.config.as_ref().unwrap();
    assert_eq!(config.len(), 2);
    assert_eq!(config.get("keySize").unwrap(), "256");
    assert_eq!(config.get("mode").unwrap(), "CBC");

    let defs = detail.config_definitions.as_ref().unwrap();
    assert_eq!(defs.len(), 2);

    // First definition: ENUM type with enum values
    assert_eq!(defs[0].key, "keySize");
    assert_eq!(defs[0].name, "Key Size");
    assert_eq!(defs[0].item_type, "ENUM");
    assert!(defs[0].required);
    assert_eq!(defs[0].default_value.as_deref(), Some("128"));
    let enum_vals = defs[0].enum_values.as_ref().unwrap();
    assert_eq!(enum_vals, &["128", "192", "256"]);

    // Second definition: STRING type, optional
    assert_eq!(defs[1].key, "mode");
    assert!(!defs[1].required);
}

#[test]
fn test_plugin_detail_without_config() {
    let json = r#"{
        "pluginId": "auth:ldap",
        "pluginType": "auth",
        "pluginName": "ldap",
        "enabled": false,
        "critical": false,
        "configurable": false
    }"#;

    let detail: PluginDetail = serde_json::from_str(json).unwrap();
    assert!(!detail.configurable);
    assert!(detail.config.is_none());
    assert!(detail.config_definitions.is_none());
}

#[test]
fn test_plugin_availability() {
    let json = r#"{
        "pluginId": "auth:nacos-auth",
        "available": true,
        "availableCount": 3,
        "totalCount": 3
    }"#;

    let avail: PluginAvailability = serde_json::from_str(json).unwrap();
    assert_eq!(avail.plugin_id, "auth:nacos-auth");
    assert!(avail.available);
    assert_eq!(avail.available_count, Some(3));
    assert_eq!(avail.total_count, Some(3));
}

#[test]
fn test_config_item_definition_types() {
    // STRING type
    let def = ConfigItemDefinition {
        key: "host".to_string(),
        name: "Hostname".to_string(),
        description: Some("LDAP server hostname".to_string()),
        default_value: Some("localhost".to_string()),
        item_type: "STRING".to_string(),
        required: true,
        enum_values: None,
    };
    assert_eq!(def.item_type, "STRING");
    assert!(def.required);

    // NUMBER type
    let def = ConfigItemDefinition {
        key: "port".to_string(),
        name: "Port".to_string(),
        item_type: "NUMBER".to_string(),
        default_value: Some("389".to_string()),
        required: true,
        ..Default::default()
    };
    assert_eq!(def.item_type, "NUMBER");

    // BOOLEAN type
    let def = ConfigItemDefinition {
        key: "useSsl".to_string(),
        name: "Use SSL".to_string(),
        item_type: "BOOLEAN".to_string(),
        default_value: Some("false".to_string()),
        ..Default::default()
    };
    assert_eq!(def.item_type, "BOOLEAN");
}

#[test]
fn test_plugin_info_serialization_roundtrip() {
    let info = PluginInfo {
        plugin_id: "test:my-plugin".to_string(),
        plugin_type: "test".to_string(),
        plugin_name: "my-plugin".to_string(),
        enabled: true,
        critical: false,
        configurable: true,
        exclusive: false,
        available_node_count: Some(1),
        total_node_count: Some(1),
    };

    let json = serde_json::to_string(&info).unwrap();
    let roundtrip: PluginInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip.plugin_id, info.plugin_id);
    assert_eq!(roundtrip.plugin_name, info.plugin_name);
    assert_eq!(roundtrip.enabled, info.enabled);
    assert_eq!(roundtrip.available_node_count, info.available_node_count);
}
