//! Health checker implementations using strategy pattern
//!
//! This module provides health checker implementations that support
//! flexible configuration without hardcoding.

use super::configurer::{HealthCheckerConfig, HttpHealthCheckerConfig, TcpHealthCheckerConfig};
use crate::model::Instance;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub success: bool,
    pub message: Option<String>,
    pub response_time_ms: u64,
}

/// Health checker trait for extensibility
#[async_trait::async_trait]
pub trait HealthChecker: Send + Sync {
    /// Get checker type name
    fn get_type(&self) -> &str;

    /// Get checker configuration (for serialization)
    fn get_config(&self) -> Option<serde_json::Value> {
        None
    }

    /// Execute health check
    async fn check(&self, instance: &Instance, config: &HealthCheckerConfig) -> HealthCheckResult;
}

/// TCP health checker
pub struct TcpHealthChecker;

#[async_trait::async_trait]
impl HealthChecker for TcpHealthChecker {
    fn get_type(&self) -> &str {
        "TCP"
    }

    async fn check(&self, instance: &Instance, config: &HealthCheckerConfig) -> HealthCheckResult {
        let checker_config = TcpHealthCheckerConfig::from_checker_config(config);

        // Determine check port
        let check_port = if checker_config.use_instance_port || checker_config.check_port <= 0 {
            instance.port
        } else {
            checker_config.check_port
        };

        self.tcp_check(&instance.ip, check_port).await
    }
}

impl TcpHealthChecker {
    async fn tcp_check(&self, ip: &str, port: i32) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let addr_str = format!("{}:{}", ip, port);
        let addr: SocketAddr = match addr_str.parse() {
            Ok(a) => a,
            Err(e) => {
                return HealthCheckResult {
                    success: false,
                    message: Some(format!("Invalid address: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Use 500ms timeout as per Nacos
        match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
            Ok(Ok(_stream)) => {
                debug!("TCP health check passed for {}:{}", ip, port);
                HealthCheckResult {
                    success: true,
                    message: None,
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            Ok(Err(e)) => {
                debug!("TCP health check failed for {}:{}: {}", ip, port, e);
                HealthCheckResult {
                    success: false,
                    message: Some(format!("Connection failed: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            Err(_) => {
                debug!("TCP health check timeout for {}:{}", ip, port);
                HealthCheckResult {
                    success: false,
                    message: Some("Connection timeout".to_string()),
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
        }
    }
}

/// HTTP health checker
pub struct HttpHealthChecker;

#[async_trait::async_trait]
impl HealthChecker for HttpHealthChecker {
    fn get_type(&self) -> &str {
        "HTTP"
    }

    async fn check(&self, instance: &Instance, config: &HealthCheckerConfig) -> HealthCheckResult {
        let checker_config = HttpHealthCheckerConfig::from_checker_config(config);

        // Determine check port
        let check_port = if checker_config.use_instance_port || checker_config.check_port <= 0 {
            instance.port
        } else {
            checker_config.check_port
        };

        // Parse headers
        let headers = checker_config.parse_headers().unwrap_or_default();

        // Parse expected codes
        let expected_codes = checker_config.parse_expected_codes();

        self.http_check(
            &instance.ip,
            check_port,
            &checker_config.path,
            &headers,
            &expected_codes,
        )
        .await
    }
}

impl HttpHealthChecker {
    async fn http_check(
        &self,
        ip: &str,
        port: i32,
        path: &str,
        headers: &std::collections::HashMap<String, String>,
        expected_codes: &[u16],
    ) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let addr_str = format!("{}:{}", ip, port);
        let addr: SocketAddr = match addr_str.parse() {
            Ok(a) => a,
            Err(e) => {
                return HealthCheckResult {
                    success: false,
                    message: Some(format!("Invalid address: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Connect with timeout
        let stream = match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                return HealthCheckResult {
                    success: false,
                    message: Some(format!("Connection failed: {}", e)),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
            Err(_) => {
                return HealthCheckResult {
                    success: false,
                    message: Some("Connection timeout".to_string()),
                    response_time_ms: start.elapsed().as_millis() as u64,
                };
            }
        };

        // Build HTTP request with custom headers
        let mut request = format!("GET {} HTTP/1.1\r\nHost: {}:{}\r\n", path, ip, port);

        // Add custom headers
        for (key, value) in headers {
            request.push_str(&format!("{}: {}\r\n", key, value));
        }

        request.push_str("Connection: close\r\n\r\n");

        let (mut reader, mut writer) = stream.into_split();

        if let Err(e) = writer.write_all(request.as_bytes()).await {
            return HealthCheckResult {
                success: false,
                message: Some(format!("Failed to send request: {}", e)),
                response_time_ms: start.elapsed().as_millis() as u64,
            };
        }

        // Read response with timeout
        let mut response = vec![0u8; 1024];
        let remaining = Duration::from_millis(500).saturating_sub(start.elapsed());

        match timeout(remaining, reader.read(&mut response)).await {
            Ok(Ok(n)) if n > 0 => {
                let response_str = String::from_utf8_lossy(&response[..n]);

                // Check HTTP status code
                if let Some(status_line) = response_str.lines().next()
                    && let Some(status_code) = status_line.split_whitespace().nth(1)
                    && let Ok(code) = status_code.parse::<u16>()
                {
                    if expected_codes.contains(&code) {
                        debug!(
                            "HTTP health check passed for {}:{}{} with status {}",
                            ip, port, path, code
                        );
                        return HealthCheckResult {
                            success: true,
                            message: None,
                            response_time_ms: start.elapsed().as_millis() as u64,
                        };
                    } else {
                        return HealthCheckResult {
                            success: false,
                            message: Some(format!("HTTP status code: {}", code)),
                            response_time_ms: start.elapsed().as_millis() as u64,
                        };
                    }
                }

                HealthCheckResult {
                    success: false,
                    message: Some("Invalid HTTP response".to_string()),
                    response_time_ms: start.elapsed().as_millis() as u64,
                }
            }
            Ok(Ok(_)) => HealthCheckResult {
                success: false,
                message: Some("Empty response".to_string()),
                response_time_ms: start.elapsed().as_millis() as u64,
            },
            Ok(Err(e)) => HealthCheckResult {
                success: false,
                message: Some(format!("Failed to read response: {}", e)),
                response_time_ms: start.elapsed().as_millis() as u64,
            },
            Err(_) => HealthCheckResult {
                success: false,
                message: Some("Response timeout".to_string()),
                response_time_ms: start.elapsed().as_millis() as u64,
            },
        }
    }
}

/// NONE health checker (no health check)
pub struct NoneHealthChecker;

#[async_trait::async_trait]
impl HealthChecker for NoneHealthChecker {
    fn get_type(&self) -> &str {
        "NONE"
    }

    async fn check(
        &self,
        _instance: &Instance,
        _config: &HealthCheckerConfig,
    ) -> HealthCheckResult {
        HealthCheckResult {
            success: true,
            message: None,
            response_time_ms: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tcp_checker_invalid_address() {
        let checker = TcpHealthChecker;
        let instance = Instance {
            ip: "invalid".to_string(),
            port: 8080,
            ..Default::default()
        };

        let config = HealthCheckerConfig::new("TCP");
        let result = checker.check(&instance, &config).await;

        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_http_checker_invalid_address() {
        let checker = HttpHealthChecker;
        let instance = Instance {
            ip: "invalid".to_string(),
            port: 8080,
            ..Default::default()
        };

        let config = HealthCheckerConfig::new("HTTP");
        let result = checker.check(&instance, &config).await;

        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_none_checker() {
        let checker = NoneHealthChecker;
        let instance = Instance::default();
        let config = HealthCheckerConfig::new("NONE");
        let result = checker.check(&instance, &config).await;

        assert!(result.success);
        assert!(result.message.is_none());
    }

    #[tokio::test]
    async fn test_tcp_checker_port_override() {
        let checker = TcpHealthChecker;

        let instance = Instance {
            ip: "127.0.0.1".to_string(),
            port: 8080,
            ..Default::default()
        };

        // Test with custom port (use_instance_port = false)
        let mut data = std::collections::HashMap::new();
        data.insert("checkPort".to_string(), "9090".to_string());
        data.insert("useInstancePort4Check".to_string(), "false".to_string());
        let config = HealthCheckerConfig::new("TCP").with_extend_data(data);

        // This will fail because port 9090 is not open, but it should try 9090
        let result = checker.check(&instance, &config).await;
        assert!(!result.success); // Expected to fail

        // Test with use_instance_port = true
        let data2 = std::collections::HashMap::new();
        let config2 = HealthCheckerConfig::new("TCP").with_extend_data(data2);
        let result2 = checker.check(&instance, &config2).await;
        // This will also fail because 8080 is not open
        assert!(!result2.success);
    }
}
