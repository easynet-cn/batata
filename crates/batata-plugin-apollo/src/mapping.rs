//! Apollo to Nacos concept mapping
//!
//! This module provides mapping between Apollo Config concepts and Nacos concepts.
//!
//! ## Mapping Strategy
//!
//! | Apollo | Nacos | Mapping |
//! |--------|-------|---------|
//! | `env` | `namespace` | Direct mapping |
//! | `appId` | Part of `dataId` | `{appId}+{namespace}` → `dataId` |
//! | `cluster` | `group` | Direct mapping |
//! | `namespace` | Part of `dataId` | `{appId}+{namespace}` → `dataId` |
//! | `releaseKey` | `md5` | Generate Apollo-style releaseKey |

use std::collections::HashMap;

use chrono::Utc;

/// Default cluster name in Apollo
pub const DEFAULT_CLUSTER: &str = "default";

/// Default namespace in Apollo
pub const DEFAULT_NAMESPACE: &str = "application";

/// Separator used in dataId mapping
pub const DATA_ID_SEPARATOR: &str = "+";

/// Convert Apollo identifiers to Nacos dataId
///
/// Format: `{appId}+{namespace}`
///
/// # Examples
///
/// ```
/// use batata_plugin_apollo::mapping::to_nacos_data_id;
/// let data_id = to_nacos_data_id("myapp", "application");
/// assert_eq!(data_id, "myapp+application");
/// ```
pub fn to_nacos_data_id(app_id: &str, namespace: &str) -> String {
    format!(
        "{}{}{}",
        app_id,
        DATA_ID_SEPARATOR,
        normalize_namespace(namespace)
    )
}

/// Convert Apollo cluster to Nacos group
///
/// Apollo's cluster maps directly to Nacos group.
/// If cluster is empty, use "default".
pub fn to_nacos_group(cluster: &str) -> String {
    if cluster.is_empty() {
        DEFAULT_CLUSTER.to_string()
    } else {
        cluster.to_string()
    }
}

/// Convert Apollo env to Nacos namespace (tenant)
///
/// Apollo's env maps directly to Nacos namespace.
/// If env is empty or "default", use empty string (public namespace).
pub fn to_nacos_namespace(env: &str) -> String {
    if env.is_empty() || env == "default" || env == "DEV" {
        String::new() // Public namespace
    } else {
        env.to_lowercase()
    }
}

/// Normalize Apollo namespace name
///
/// Removes `.properties` suffix if present (Apollo convention).
pub fn normalize_namespace(namespace: &str) -> &str {
    namespace.strip_suffix(".properties").unwrap_or(namespace)
}

/// Parse Nacos dataId back to Apollo identifiers
///
/// Returns (appId, namespace) tuple.
pub fn from_nacos_data_id(data_id: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = data_id.splitn(2, DATA_ID_SEPARATOR).collect();
    if parts.len() == 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        // If no separator, treat entire dataId as namespace with empty appId
        None
    }
}

/// Generate Apollo-style release key from MD5 and timestamp
///
/// Format: `{timestamp}-{md5_prefix}`
pub fn generate_release_key(md5: &str) -> String {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let md5_prefix = if md5.len() >= 8 { &md5[..8] } else { md5 };
    format!("{}-{}", timestamp, md5_prefix)
}

/// Generate release key from content MD5 with a specific timestamp
pub fn generate_release_key_with_time(md5: &str, timestamp: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or_else(|| Utc::now());
    let ts_str = dt.format("%Y%m%d%H%M%S").to_string();
    let md5_prefix = if md5.len() >= 8 { &md5[..8] } else { md5 };
    format!("{}-{}", ts_str, md5_prefix)
}

/// Parse properties format content into key-value pairs
///
/// Supports:
/// - `key=value`
/// - `key: value`
/// - Comments starting with `#` or `!`
/// - Continuation lines ending with `\`
pub fn parse_properties(content: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();
    let mut continuation = false;

    for line in content.lines() {
        let line = line.trim_start();

        // Handle continuation
        if continuation {
            let value_part = line.trim();
            if value_part.ends_with('\\') {
                current_value.push_str(&value_part[..value_part.len() - 1]);
            } else {
                current_value.push_str(value_part);
                if let Some(key) = current_key.take() {
                    result.insert(key, current_value.clone());
                    current_value.clear();
                }
                continuation = false;
            }
            continue;
        }

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }

        // Find separator (= or :)
        let sep_pos = line.find(|c| c == '=' || c == ':');
        if let Some(pos) = sep_pos {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim();

            if value.ends_with('\\') {
                current_key = Some(key);
                current_value = value[..value.len() - 1].to_string();
                continuation = true;
            } else {
                result.insert(key, value.to_string());
            }
        }
    }

    result
}

/// Convert key-value pairs to properties format string
pub fn to_properties_string(configs: &HashMap<String, String>) -> String {
    let mut lines: Vec<String> = configs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    lines.sort(); // Consistent ordering
    lines.join("\n")
}

/// Apollo mapping context for a single request
#[derive(Debug, Clone)]
pub struct ApolloMappingContext {
    /// Original Apollo appId
    pub app_id: String,
    /// Original Apollo cluster
    pub cluster: String,
    /// Original Apollo namespace
    pub namespace: String,
    /// Original Apollo env (from path or header)
    pub env: Option<String>,
    /// Mapped Nacos dataId
    pub nacos_data_id: String,
    /// Mapped Nacos group
    pub nacos_group: String,
    /// Mapped Nacos namespace (tenant)
    pub nacos_namespace: String,
}

impl ApolloMappingContext {
    /// Create a new mapping context
    pub fn new(app_id: &str, cluster: &str, namespace: &str, env: Option<&str>) -> Self {
        let normalized_namespace = normalize_namespace(namespace);
        Self {
            app_id: app_id.to_string(),
            cluster: cluster.to_string(),
            namespace: normalized_namespace.to_string(),
            env: env.map(|s| s.to_string()),
            nacos_data_id: to_nacos_data_id(app_id, normalized_namespace),
            nacos_group: to_nacos_group(cluster),
            nacos_namespace: env.map(|e| to_nacos_namespace(e)).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_nacos_data_id() {
        assert_eq!(
            to_nacos_data_id("myapp", "application"),
            "myapp+application"
        );
        assert_eq!(to_nacos_data_id("app1", "config.json"), "app1+config.json");
    }

    #[test]
    fn test_to_nacos_group() {
        assert_eq!(to_nacos_group("default"), "default");
        assert_eq!(to_nacos_group("cluster1"), "cluster1");
        assert_eq!(to_nacos_group(""), "default");
    }

    #[test]
    fn test_to_nacos_namespace() {
        assert_eq!(to_nacos_namespace(""), "");
        assert_eq!(to_nacos_namespace("default"), "");
        assert_eq!(to_nacos_namespace("DEV"), "");
        assert_eq!(to_nacos_namespace("PRO"), "pro");
        assert_eq!(to_nacos_namespace("FAT"), "fat");
    }

    #[test]
    fn test_normalize_namespace() {
        assert_eq!(normalize_namespace("application.properties"), "application");
        assert_eq!(normalize_namespace("application"), "application");
        assert_eq!(normalize_namespace("config.json"), "config.json");
    }

    #[test]
    fn test_from_nacos_data_id() {
        let (app_id, ns) = from_nacos_data_id("myapp+application").unwrap();
        assert_eq!(app_id, "myapp");
        assert_eq!(ns, "application");

        assert!(from_nacos_data_id("no-separator").is_none());
    }

    #[test]
    fn test_parse_properties() {
        let content = r#"
# Comment
key1=value1
key2: value2
key3 = value3

! Another comment
key4=value with spaces
"#;
        let props = parse_properties(content);
        assert_eq!(props.get("key1"), Some(&"value1".to_string()));
        assert_eq!(props.get("key2"), Some(&"value2".to_string()));
        assert_eq!(props.get("key3"), Some(&"value3".to_string()));
        assert_eq!(props.get("key4"), Some(&"value with spaces".to_string()));
    }

    #[test]
    fn test_to_properties_string() {
        let mut configs = HashMap::new();
        configs.insert("key1".to_string(), "value1".to_string());
        configs.insert("key2".to_string(), "value2".to_string());

        let result = to_properties_string(&configs);
        assert!(result.contains("key1=value1"));
        assert!(result.contains("key2=value2"));
    }

    #[test]
    fn test_mapping_context() {
        let ctx =
            ApolloMappingContext::new("myapp", "default", "application.properties", Some("PRO"));
        assert_eq!(ctx.app_id, "myapp");
        assert_eq!(ctx.cluster, "default");
        assert_eq!(ctx.namespace, "application");
        assert_eq!(ctx.nacos_data_id, "myapp+application");
        assert_eq!(ctx.nacos_group, "default");
        assert_eq!(ctx.nacos_namespace, "pro");
    }

    #[test]
    fn test_generate_release_key() {
        let release_key = generate_release_key("abc123def456");
        assert!(release_key.contains("-abc123de"));
        assert_eq!(release_key.len(), 15 + 8); // timestamp + separator + md5 prefix
    }
}
