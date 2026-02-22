//! Health check configuration for Nacos compatibility
//!
//! This module provides configuration structures for health checking
//! that match Nacos's SwitchDomain behavior.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Health check parameters for TCP protocol (matches Nacos TcpHealthParams)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TcpHealthParams {
    /// Maximum check interval in milliseconds (default: 5000ms)
    #[serde(default = "default_max_interval")]
    pub max: u64,

    /// Minimum check interval in milliseconds (default: 2000ms)
    #[serde(default = "default_min_interval")]
    pub min: u64,

    /// Adaptive factor for interval adjustment (default: 0.75)
    #[serde(default = "default_factor")]
    pub factor: f64,
}

impl Default for TcpHealthParams {
    fn default() -> Self {
        Self {
            max: 5000,
            min: 2000,
            factor: 0.75,
        }
    }
}

/// Health check parameters for HTTP protocol (matches Nacos HttpHealthParams)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpHealthParams {
    /// Maximum check interval in milliseconds (default: 5000ms)
    #[serde(default = "default_max_interval")]
    pub max: u64,

    /// Minimum check interval in milliseconds (default: 500ms)
    #[serde(default = "default_http_min_interval")]
    pub min: u64,

    /// Adaptive factor for interval adjustment (default: 0.85)
    #[serde(default = "default_http_factor")]
    pub factor: f64,
}

impl Default for HttpHealthParams {
    fn default() -> Self {
        Self {
            max: 5000,
            min: 500,
            factor: 0.85,
        }
    }
}



/// Main health check configuration (matches Nacos SwitchDomain)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Enable health check (default: true)
    #[serde(default = "default_health_check_enabled")]
    pub health_check_enabled: bool,

    /// Enable auto change health check (default: true)
    #[serde(default = "default_auto_change_health_check_enabled")]
    pub auto_change_health_check_enabled: bool,

    /// Number of consecutive failures before marking unhealthy (default: 3)
    #[serde(default = "default_check_times")]
    pub check_times: u32,

    /// TCP health check parameters
    #[serde(default)]
    pub tcp_health_params: TcpHealthParams,

    /// HTTP health check parameters
    #[serde(default)]
    pub http_health_params: HttpHealthParams,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            health_check_enabled: true,
            auto_change_health_check_enabled: true,
            check_times: 3,
            tcp_health_params: TcpHealthParams::default(),
            http_health_params: HttpHealthParams::default(),
        }
    }
}

impl HealthCheckConfig {
    /// Create a new health check config from application.yml
    pub fn from_yaml_config(yaml_config: &serde_yaml::Value) -> Self {
        // Parse from yaml if available, otherwise use defaults
        // This allows overriding from application.yml
        let mut config = Self::default();

        if let Some(health_check) = yaml_config.get("nacos")
            .and_then(|n| n.get("naming"))
            .and_then(|n| n.get("health"))
        {
            if let Some(enabled) = health_check.get("enabled").and_then(|v| v.as_bool()) {
                config.health_check_enabled = enabled;
            }
            if let Some(auto_change) = health_check.get("auto_change_enabled").and_then(|v| v.as_bool()) {
                config.auto_change_health_check_enabled = auto_change;
            }
            if let Some(check_times) = health_check.get("check_times").and_then(|v| v.as_i64()) {
                config.check_times = check_times as u32;
            }
        }

        config
    }

    /// Check if health check is enabled
    pub fn is_enabled(&self) -> bool {
        self.health_check_enabled
    }

    /// Check if auto change health check is enabled
    pub fn is_auto_change_enabled(&self) -> bool {
        self.auto_change_health_check_enabled && self.health_check_enabled
    }

    /// Get the failure threshold (number of consecutive failures)
    pub fn get_failure_threshold(&self) -> u32 {
        self.check_times
    }

    /// Get the initial check interval for TCP
    pub fn get_tcp_initial_interval(&self) -> Duration {
        let min = self.tcp_health_params.min;
        let max = self.tcp_health_params.max;
        Duration::from_millis((min + max) / 2)
    }

    /// Get the initial check interval for HTTP
    pub fn get_http_initial_interval(&self) -> Duration {
        let min = self.http_health_params.min;
        let max = self.http_health_params.max;
        Duration::from_millis((min + max) / 2)
    }

    /// Get the TCP adaptive factor
    pub fn get_tcp_factor(&self) -> f64 {
        self.tcp_health_params.factor
    }

    /// Get the HTTP adaptive factor
    pub fn get_http_factor(&self) -> f64 {
        self.http_health_params.factor
    }
}

// Default functions for serde
fn default_health_check_enabled() -> bool {
    true
}

fn default_auto_change_health_check_enabled() -> bool {
    true
}

fn default_check_times() -> u32 {
    3
}

fn default_max_interval() -> u64 {
    5000
}

fn default_min_interval() -> u64 {
    2000
}

fn default_http_min_interval() -> u64 {
    500
}

fn default_factor() -> f64 {
    0.75
}

fn default_http_factor() -> f64 {
    0.85
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_params_default() {
        let params = TcpHealthParams::default();
        assert_eq!(params.max, 5000);
        assert_eq!(params.min, 2000);
        assert_eq!(params.factor, 0.75);
    }

    #[test]
    fn test_http_params_default() {
        let params = HttpHealthParams::default();
        assert_eq!(params.max, 5000);
        assert_eq!(params.min, 500);
        assert_eq!(params.factor, 0.85);
    }

    #[test]
    fn test_config_default() {
        let config = HealthCheckConfig::default();
        assert!(config.is_enabled());
        assert!(config.is_auto_change_enabled());
        assert_eq!(config.get_failure_threshold(), 3);
    }

    #[test]
    fn test_config_initial_intervals() {
        let config = HealthCheckConfig::default();
        let tcp_interval = config.get_tcp_initial_interval();
        let http_interval = config.get_http_initial_interval();
        assert_eq!(tcp_interval.as_millis(), 3500); // (2000 + 5000) / 2
        assert_eq!(http_interval.as_millis(), 2750); // (500 + 5000) / 2
    }

    #[test]
    fn test_config_from_yaml() {
        let yaml_value = serde_yaml::from_str(
            r#"
            nacos:
              naming:
                health:
                  enabled: false
                  check_times: 5
            "#,
        )
        .unwrap();
        let config = HealthCheckConfig::from_yaml_config(&yaml_value);
        assert!(!config.is_enabled());
        assert_eq!(config.get_failure_threshold(), 5);
    }
}
