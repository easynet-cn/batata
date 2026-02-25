//! Health checker configuration structures
//!
//! This module provides configuration structures for health checkers
//! that support flexible configuration without hardcoding.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Health checker base configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckerConfig {
    /// Checker type (e.g., "TCP", "HTTP", "NONE")
    pub r#type: String,

    /// Extended configuration data (JSON-compatible map)
    pub extend_data: Option<HashMap<String, String>>,
}

impl HealthCheckerConfig {
    /// Create a new health checker configuration
    pub fn new(r#type: impl Into<String>) -> Self {
        Self {
            r#type: r#type.into(),
            extend_data: None,
        }
    }

    /// Create with extend data
    pub fn with_extend_data(mut self, data: HashMap<String, String>) -> Self {
        self.extend_data = Some(data);
        self
    }

    /// Get a string value from extend_data
    pub fn get_string(&self, key: &str) -> Option<&String> {
        self.extend_data.as_ref()?.get(key)
    }

    /// Get an i32 value from extend_data
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.get_string(key)?.parse().ok()
    }

    /// Get a bool value from extend_data
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get_string(key)?.as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        }
    }
}

/// TCP health checker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpHealthCheckerConfig {
    /// Check port (0 means use instance port)
    pub check_port: i32,

    /// Use instance port for health check
    pub use_instance_port: bool,
}

impl Default for TcpHealthCheckerConfig {
    fn default() -> Self {
        Self {
            check_port: 0,
            use_instance_port: true,
        }
    }
}

impl TcpHealthCheckerConfig {
    /// Parse from HealthCheckerConfig extend_data
    pub fn from_checker_config(config: &HealthCheckerConfig) -> Self {
        Self {
            check_port: config.get_i32("checkPort").unwrap_or_default(),
            use_instance_port: config.get_bool("useInstancePort4Check").unwrap_or(true),
        }
    }
}

/// HTTP health checker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHealthCheckerConfig {
    /// Check path (e.g., "/health")
    pub path: String,

    /// Check port (0 means use instance port)
    pub check_port: i32,

    /// Use instance port for health check
    pub use_instance_port: bool,

    /// Request headers (JSON string, e.g., {"Authorization": "Bearer xxx"})
    pub headers: Option<String>,

    /// Expected HTTP status codes
    /// Supports: "200-399" (range), "200,201,204" (list), "200" (single)
    pub expected_code: Option<String>,
}

impl Default for HttpHealthCheckerConfig {
    fn default() -> Self {
        Self {
            path: "/health".to_string(),
            check_port: 0,
            use_instance_port: true,
            headers: None,
            expected_code: Some("200-399".to_string()),
        }
    }
}

impl HttpHealthCheckerConfig {
    /// Parse from HealthCheckerConfig extend_data
    pub fn from_checker_config(config: &HealthCheckerConfig) -> Self {
        Self {
            path: config
                .get_string("path")
                .cloned()
                .unwrap_or_else(|| "/health".to_string()),
            check_port: config.get_i32("checkPort").unwrap_or_default(),
            use_instance_port: config.get_bool("useInstancePort4Check").unwrap_or(true),
            headers: config.get_string("headers").cloned(),
            expected_code: config.get_string("expectedCode").cloned(),
        }
    }

    /// Parse headers from JSON string
    pub fn parse_headers(&self) -> Option<HashMap<String, String>> {
        self.headers
            .as_ref()
            .and_then(|h| serde_json::from_str(h).ok())
    }

    /// Parse expected status codes into a list
    pub fn parse_expected_codes(&self) -> Vec<u16> {
        let default = "200-399".to_string();
        let code_str = self.expected_code.as_ref().unwrap_or(&default);
        parse_expected_code(code_str)
    }
}

/// Parse expected status codes from string
/// Supports: "200-399" (range), "200,201,204" (list), "200" (single)
pub fn parse_expected_code(s: &str) -> Vec<u16> {
    let mut codes = Vec::new();

    // Check for range: "200-399"
    if let Some((start, end)) = s.split_once('-') {
        if let (Ok(start), Ok(end)) = (start.trim().parse::<u16>(), end.trim().parse::<u16>()) {
            for code in start..=end {
                codes.push(code);
            }
        }
    }
    // Check for list: "200,201,204"
    else if s.contains(',') {
        for part in s.split(',') {
            if let Ok(code) = part.trim().parse::<u16>() {
                codes.push(code);
            }
        }
    }
    // Single code: "200"
    else if let Ok(code) = s.trim().parse::<u16>() {
        codes.push(code);
    }

    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker_config() {
        let mut data = HashMap::new();
        data.insert("checkPort".to_string(), "8080".to_string());
        data.insert("useInstancePort4Check".to_string(), "false".to_string());

        let config = HealthCheckerConfig::new("TCP").with_extend_data(data);
        assert_eq!(config.r#type, "TCP");
        assert_eq!(config.get_i32("checkPort"), Some(8080));
        assert_eq!(config.get_bool("useInstancePort4Check"), Some(false));
    }

    #[test]
    fn test_tcp_config_from_checker() {
        let mut data = HashMap::new();
        data.insert("checkPort".to_string(), "9090".to_string());
        data.insert("useInstancePort4Check".to_string(), "true".to_string());

        let checker_config = HealthCheckerConfig::new("TCP").with_extend_data(data);
        let tcp_config = TcpHealthCheckerConfig::from_checker_config(&checker_config);

        assert_eq!(tcp_config.check_port, 9090);
        assert!(tcp_config.use_instance_port);
    }

    #[test]
    fn test_http_config_from_checker() {
        let mut data = HashMap::new();
        data.insert("path".to_string(), "/status".to_string());
        data.insert("expectedCode".to_string(), "200,201,204".to_string());
        data.insert(
            "headers".to_string(),
            r#"{"Authorization": "Bearer xxx"}"#.to_string(),
        );

        let checker_config = HealthCheckerConfig::new("HTTP").with_extend_data(data);
        let http_config = HttpHealthCheckerConfig::from_checker_config(&checker_config);

        assert_eq!(http_config.path, "/status");
        assert_eq!(http_config.expected_code, Some("200,201,204".to_string()));

        let codes = http_config.parse_expected_codes();
        assert_eq!(codes, vec![200, 201, 204]);
    }

    #[test]
    fn test_parse_expected_code_range() {
        let codes = parse_expected_code("200-399");
        assert_eq!(codes.len(), 200);
        assert_eq!(codes[0], 200);
        assert_eq!(codes[199], 399);
    }

    #[test]
    fn test_parse_expected_code_list() {
        let codes = parse_expected_code("200,201,204,404");
        assert_eq!(codes, vec![200, 201, 204, 404]);
    }

    #[test]
    fn test_parse_expected_code_single() {
        let codes = parse_expected_code("200");
        assert_eq!(codes, vec![200]);
    }

    #[test]
    fn test_parse_expected_code_invalid() {
        let codes = parse_expected_code("invalid");
        assert!(codes.is_empty());
    }

    #[test]
    fn test_http_parse_headers() {
        let mut data = HashMap::new();
        data.insert("path".to_string(), "/health".to_string());
        data.insert(
            "headers".to_string(),
            r#"{"Authorization": "Bearer xyz","User-Agent": "Test"}"#.to_string(),
        );

        let checker_config = HealthCheckerConfig::new("HTTP").with_extend_data(data);
        let http_config = HttpHealthCheckerConfig::from_checker_config(&checker_config);

        let headers = http_config.parse_headers();
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer xyz".to_string())
        );
        assert_eq!(headers.get("User-Agent"), Some(&"Test".to_string()));
    }
}
