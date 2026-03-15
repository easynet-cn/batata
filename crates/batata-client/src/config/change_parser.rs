//! Config change parser for detecting config content differences
//!
//! Parses config content changes and produces diff items showing
//! what was added, modified, or deleted between old and new values.

use std::collections::HashMap;

/// Type of property change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyChangeType {
    Added,
    Modified,
    Deleted,
}

impl std::fmt::Display for PropertyChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyChangeType::Added => write!(f, "ADDED"),
            PropertyChangeType::Modified => write!(f, "MODIFIED"),
            PropertyChangeType::Deleted => write!(f, "DELETED"),
        }
    }
}

/// A single config change item
#[derive(Debug, Clone)]
pub struct ConfigChangeItem {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub change_type: PropertyChangeType,
}

/// Event carrying parsed config changes
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub items: HashMap<String, ConfigChangeItem>,
}

/// Trait for parsing config content into key-value pairs
pub trait ConfigChangeParser: Send + Sync + 'static {
    /// Check if this parser supports the given config type
    fn is_responsible_for(&self, config_type: &str) -> bool;

    /// Compare old and new content and produce change items
    fn compare_content(
        &self,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem>;
}

/// Properties format parser (key=value)
pub struct PropertiesChangeParser;

impl PropertiesChangeParser {
    fn parse_properties(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                map.insert(key, value);
            } else if let Some(pos) = line.find(':') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                map.insert(key, value);
            }
        }
        map
    }
}

impl ConfigChangeParser for PropertiesChangeParser {
    fn is_responsible_for(&self, config_type: &str) -> bool {
        config_type.eq_ignore_ascii_case("properties") || config_type.eq_ignore_ascii_case("text")
    }

    fn compare_content(
        &self,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem> {
        let old_map = Self::parse_properties(old_content);
        let new_map = Self::parse_properties(new_content);
        compare_maps(&old_map, &new_map)
    }
}

/// YAML format parser
pub struct YamlChangeParser;

impl YamlChangeParser {
    fn flatten_yaml(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        // Simple YAML flattening: handles basic key: value pairs
        // and nested indentation-based structures
        let mut path_stack: Vec<(usize, String)> = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = line.len() - line.trim_start().len();

            // Remove deeper items from stack
            while let Some(&(level, _)) = path_stack.last() {
                if level >= indent {
                    path_stack.pop();
                } else {
                    break;
                }
            }

            if let Some(pos) = trimmed.find(':') {
                let key = trimmed[..pos].trim().to_string();
                let value = trimmed[pos + 1..].trim().to_string();

                if value.is_empty() {
                    // This is a parent key
                    path_stack.push((indent, key));
                } else {
                    // This is a leaf key=value
                    let full_key = if path_stack.is_empty() {
                        key
                    } else {
                        let prefix: Vec<&str> =
                            path_stack.iter().map(|(_, k)| k.as_str()).collect();
                        format!("{}.{}", prefix.join("."), key)
                    };
                    map.insert(full_key, value);
                }
            }
        }
        map
    }
}

impl ConfigChangeParser for YamlChangeParser {
    fn is_responsible_for(&self, config_type: &str) -> bool {
        config_type.eq_ignore_ascii_case("yaml") || config_type.eq_ignore_ascii_case("yml")
    }

    fn compare_content(
        &self,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem> {
        let old_map = Self::flatten_yaml(old_content);
        let new_map = Self::flatten_yaml(new_content);
        compare_maps(&old_map, &new_map)
    }
}

/// JSON format parser
pub struct JsonChangeParser;

impl JsonChangeParser {
    fn flatten_json(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
            Self::flatten_value(&value, String::new(), &mut map);
        }
        map
    }

    fn flatten_value(value: &serde_json::Value, prefix: String, map: &mut HashMap<String, String>) {
        match value {
            serde_json::Value::Object(obj) => {
                for (key, val) in obj {
                    let new_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::flatten_value(val, new_prefix, map);
                }
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let new_prefix = format!("{}[{}]", prefix, i);
                    Self::flatten_value(val, new_prefix, map);
                }
            }
            _ => {
                map.insert(prefix, value.to_string());
            }
        }
    }
}

impl ConfigChangeParser for JsonChangeParser {
    fn is_responsible_for(&self, config_type: &str) -> bool {
        config_type.eq_ignore_ascii_case("json")
    }

    fn compare_content(
        &self,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem> {
        let old_map = Self::flatten_json(old_content);
        let new_map = Self::flatten_json(new_content);
        compare_maps(&old_map, &new_map)
    }
}

/// TOML format parser
pub struct TomlChangeParser;

impl TomlChangeParser {
    fn flatten_toml(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(value) = content.parse::<toml::Value>() {
            Self::flatten_value(&value, String::new(), &mut map);
        }
        map
    }

    fn flatten_value(value: &toml::Value, prefix: String, map: &mut HashMap<String, String>) {
        match value {
            toml::Value::Table(table) => {
                for (key, val) in table {
                    let new_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::flatten_value(val, new_prefix, map);
                }
            }
            toml::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let new_prefix = format!("{}[{}]", prefix, i);
                    Self::flatten_value(val, new_prefix, map);
                }
            }
            _ => {
                map.insert(prefix, value.to_string());
            }
        }
    }
}

impl ConfigChangeParser for TomlChangeParser {
    fn is_responsible_for(&self, config_type: &str) -> bool {
        config_type.eq_ignore_ascii_case("toml")
    }

    fn compare_content(
        &self,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem> {
        let old_map = Self::flatten_toml(old_content);
        let new_map = Self::flatten_toml(new_content);
        compare_maps(&old_map, &new_map)
    }
}

/// XML format parser — uses line-based comparison
///
/// Since XML has complex structure, we use a simplified approach:
/// line-by-line comparison of non-empty trimmed lines.
pub struct XmlChangeParser;

impl ConfigChangeParser for XmlChangeParser {
    fn is_responsible_for(&self, config_type: &str) -> bool {
        config_type.eq_ignore_ascii_case("xml") || config_type.eq_ignore_ascii_case("html")
    }

    fn compare_content(
        &self,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem> {
        let old_lines: HashMap<String, String> = old_content
            .lines()
            .enumerate()
            .map(|(i, line)| (format!("line_{}", i + 1), line.trim().to_string()))
            .filter(|(_, v)| !v.is_empty())
            .collect();

        let new_lines: HashMap<String, String> = new_content
            .lines()
            .enumerate()
            .map(|(i, line)| (format!("line_{}", i + 1), line.trim().to_string()))
            .filter(|(_, v)| !v.is_empty())
            .collect();

        compare_maps(&old_lines, &new_lines)
    }
}

/// Compare two key-value maps and produce change items
fn compare_maps(
    old_map: &HashMap<String, String>,
    new_map: &HashMap<String, String>,
) -> HashMap<String, ConfigChangeItem> {
    let mut changes = HashMap::new();

    // Find modified and deleted
    for (key, old_value) in old_map {
        match new_map.get(key) {
            Some(new_value) if new_value != old_value => {
                changes.insert(
                    key.clone(),
                    ConfigChangeItem {
                        key: key.clone(),
                        old_value: Some(old_value.clone()),
                        new_value: Some(new_value.clone()),
                        change_type: PropertyChangeType::Modified,
                    },
                );
            }
            None => {
                changes.insert(
                    key.clone(),
                    ConfigChangeItem {
                        key: key.clone(),
                        old_value: Some(old_value.clone()),
                        new_value: None,
                        change_type: PropertyChangeType::Deleted,
                    },
                );
            }
            _ => {}
        }
    }

    // Find added
    for (key, new_value) in new_map {
        if !old_map.contains_key(key) {
            changes.insert(
                key.clone(),
                ConfigChangeItem {
                    key: key.clone(),
                    old_value: None,
                    new_value: Some(new_value.clone()),
                    change_type: PropertyChangeType::Added,
                },
            );
        }
    }

    changes
}

/// Manager that routes to the right parser based on config type
pub struct ConfigChangeHandler {
    parsers: Vec<Box<dyn ConfigChangeParser>>,
}

impl ConfigChangeHandler {
    pub fn new() -> Self {
        Self {
            parsers: vec![
                Box::new(PropertiesChangeParser),
                Box::new(YamlChangeParser),
                Box::new(JsonChangeParser),
                Box::new(TomlChangeParser),
                Box::new(XmlChangeParser),
            ],
        }
    }

    /// Parse changes between old and new config content
    pub fn parse_change(
        &self,
        config_type: &str,
        old_content: &str,
        new_content: &str,
    ) -> HashMap<String, ConfigChangeItem> {
        for parser in &self.parsers {
            if parser.is_responsible_for(config_type) {
                return parser.compare_content(old_content, new_content);
            }
        }
        // Fallback: treat as plain text properties
        PropertiesChangeParser.compare_content(old_content, new_content)
    }
}

impl Default for ConfigChangeHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_properties_added() {
        let old = "key1=value1\nkey2=value2";
        let new = "key1=value1\nkey2=value2\nkey3=value3";
        let parser = PropertiesChangeParser;
        let changes = parser.compare_content(old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes["key3"].change_type, PropertyChangeType::Added);
        assert_eq!(changes["key3"].new_value.as_deref(), Some("value3"));
    }

    #[test]
    fn test_properties_modified() {
        let old = "key1=value1\nkey2=value2";
        let new = "key1=value1\nkey2=new_value2";
        let parser = PropertiesChangeParser;
        let changes = parser.compare_content(old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes["key2"].change_type, PropertyChangeType::Modified);
        assert_eq!(changes["key2"].old_value.as_deref(), Some("value2"));
        assert_eq!(changes["key2"].new_value.as_deref(), Some("new_value2"));
    }

    #[test]
    fn test_properties_deleted() {
        let old = "key1=value1\nkey2=value2";
        let new = "key1=value1";
        let parser = PropertiesChangeParser;
        let changes = parser.compare_content(old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes["key2"].change_type, PropertyChangeType::Deleted);
    }

    #[test]
    fn test_properties_no_change() {
        let content = "key1=value1\nkey2=value2";
        let parser = PropertiesChangeParser;
        let changes = parser.compare_content(content, content);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_properties_with_comments() {
        let old = "# comment\nkey1=value1";
        let new = "# different comment\nkey1=value1\nkey2=value2";
        let parser = PropertiesChangeParser;
        let changes = parser.compare_content(old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes["key2"].change_type, PropertyChangeType::Added);
    }

    #[test]
    fn test_json_changes() {
        let old = r#"{"name":"old","port":8080}"#;
        let new = r#"{"name":"new","port":8080,"host":"localhost"}"#;
        let parser = JsonChangeParser;
        let changes = parser.compare_content(old, new);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes["name"].change_type, PropertyChangeType::Modified);
        assert_eq!(changes["host"].change_type, PropertyChangeType::Added);
    }

    #[test]
    fn test_yaml_changes() {
        let old = "server:\n  port: 8080\n  name: old";
        let new = "server:\n  port: 9090\n  name: old\n  host: localhost";
        let parser = YamlChangeParser;
        let changes = parser.compare_content(old, new);
        assert_eq!(changes.len(), 2);
        assert_eq!(
            changes["server.port"].change_type,
            PropertyChangeType::Modified
        );
        assert_eq!(
            changes["server.host"].change_type,
            PropertyChangeType::Added
        );
    }

    #[test]
    fn test_handler_routing() {
        let handler = ConfigChangeHandler::new();
        let old = "key=old";
        let new = "key=new";
        let changes = handler.parse_change("properties", old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes["key"].change_type, PropertyChangeType::Modified);
    }

    #[test]
    fn test_toml_parser_basic() {
        let parser = TomlChangeParser;
        assert!(parser.is_responsible_for("toml"));
        assert!(parser.is_responsible_for("TOML"));
        assert!(!parser.is_responsible_for("json"));
    }

    #[test]
    fn test_toml_parser_changes() {
        let parser = TomlChangeParser;
        let old = "[server]\nport = 8080\nname = \"old\"";
        let new = "[server]\nport = 9090\nname = \"old\"\nhost = \"localhost\"";
        let changes = parser.compare_content(old, new);

        assert!(changes.contains_key("server.port"));
        assert_eq!(
            changes["server.port"].change_type,
            PropertyChangeType::Modified
        );
        assert!(changes.contains_key("server.host"));
        assert_eq!(
            changes["server.host"].change_type,
            PropertyChangeType::Added
        );
    }

    #[test]
    fn test_toml_parser_deleted() {
        let parser = TomlChangeParser;
        let old = "[server]\nport = 8080\nname = \"old\"";
        let new = "[server]\nport = 8080";
        let changes = parser.compare_content(old, new);

        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes["server.name"].change_type,
            PropertyChangeType::Deleted
        );
    }

    #[test]
    fn test_toml_parser_no_change() {
        let parser = TomlChangeParser;
        let content = "[server]\nport = 8080";
        let changes = parser.compare_content(content, content);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_toml_parser_invalid_content() {
        let parser = TomlChangeParser;
        let changes = parser.compare_content("not valid toml {{{", "also invalid {{{");
        assert!(changes.is_empty());
    }

    #[test]
    fn test_toml_parser_nested_and_arrays() {
        let parser = TomlChangeParser;
        let old = "[database]\nports = [8000, 8001]";
        let new = "[database]\nports = [8000, 8002]";
        let changes = parser.compare_content(old, new);
        assert!(changes.contains_key("database.ports[1]"));
        assert_eq!(
            changes["database.ports[1]"].change_type,
            PropertyChangeType::Modified
        );
    }

    #[test]
    fn test_xml_parser_basic() {
        let parser = XmlChangeParser;
        assert!(parser.is_responsible_for("xml"));
        assert!(parser.is_responsible_for("XML"));
        assert!(parser.is_responsible_for("html"));
        assert!(parser.is_responsible_for("HTML"));
        assert!(!parser.is_responsible_for("json"));
    }

    #[test]
    fn test_xml_parser_changes() {
        let parser = XmlChangeParser;
        let old = "<root>\n  <name>old</name>\n  <port>8080</port>\n</root>";
        let new =
            "<root>\n  <name>new</name>\n  <port>8080</port>\n  <host>localhost</host>\n</root>";
        let changes = parser.compare_content(old, new);
        // Line-based comparison should detect changes
        assert!(!changes.is_empty());
    }

    #[test]
    fn test_xml_parser_no_change() {
        let parser = XmlChangeParser;
        let content = "<root>\n  <name>test</name>\n</root>";
        let changes = parser.compare_content(content, content);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_handler_routes_toml() {
        let handler = ConfigChangeHandler::new();
        let old = "key = \"old\"";
        let new = "key = \"new\"";
        let changes = handler.parse_change("toml", old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes["key"].change_type, PropertyChangeType::Modified);
    }

    #[test]
    fn test_handler_routes_xml() {
        let handler = ConfigChangeHandler::new();
        let old = "<root>\n  <key>old</key>\n</root>";
        let new = "<root>\n  <key>new</key>\n</root>";
        let changes = handler.parse_change("xml", old, new);
        assert!(!changes.is_empty());
    }

    #[test]
    fn test_property_change_type_display() {
        assert_eq!(PropertyChangeType::Added.to_string(), "ADDED");
        assert_eq!(PropertyChangeType::Modified.to_string(), "MODIFIED");
        assert_eq!(PropertyChangeType::Deleted.to_string(), "DELETED");
    }
}
