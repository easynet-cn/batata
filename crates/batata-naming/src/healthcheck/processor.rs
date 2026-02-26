//! Health check processors for different protocols
//!
//! This module provides health check processors that match Nacos implementation:
//! - TCP health check (NIO based, 500ms timeout)
//! - HTTP health check (GET request, 200-399 status code)
//! - None (no health check)

use crate::model::Instance;
use crate::service::ClusterConfig;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

/// Health check types (matches Nacos HealthCheckType)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthCheckType {
    None,
    Tcp,
    Http,
}

impl HealthCheckType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "TCP" => Self::Tcp,
            "HTTP" => Self::Http,
            _ => Self::None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "NONE",
            Self::Tcp => "TCP",
            Self::Http => "HTTP",
        }
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub success: bool,
    pub message: Option<String>,
    pub response_time_ms: u64,
}

/// Health check processor trait
#[allow(async_fn_in_trait)]
pub trait HealthCheckProcessor: Send + Sync {
    /// Check the health of an instance
    async fn check(&self, instance: &Instance, cluster_config: &ClusterConfig)
    -> HealthCheckResult;

    /// Get the processor type
    fn get_type(&self) -> HealthCheckType;
}

/// TCP health check processor (matches Nacos TcpHealthCheckProcessor)
pub struct TcpHealthCheckProcessor;

impl TcpHealthCheckProcessor {
    pub fn new() -> Self {
        Self
    }

    async fn tcp_check(
        &self,
        ip: &str,
        port: i32,
        timeout_duration: Duration,
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

        // Use 500ms timeout as per Nacos implementation
        let timeout_ms = timeout_duration.as_millis().min(500);

        match timeout(
            Duration::from_millis(timeout_ms as u64),
            TcpStream::connect(&addr),
        )
        .await
        {
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

impl Default for TcpHealthCheckProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckProcessor for TcpHealthCheckProcessor {
    async fn check(
        &self,
        instance: &Instance,
        cluster_config: &ClusterConfig,
    ) -> HealthCheckResult {
        // Determine check port
        let check_port = if cluster_config.use_instance_port || cluster_config.check_port <= 0 {
            instance.port
        } else {
            cluster_config.check_port
        };

        // Use 500ms timeout as per Nacos
        self.tcp_check(&instance.ip, check_port, Duration::from_millis(500))
            .await
    }

    fn get_type(&self) -> HealthCheckType {
        HealthCheckType::Tcp
    }
}

/// HTTP health check processor (matches Nacos HttpHealthCheckProcessor)
pub struct HttpHealthCheckProcessor;

impl HttpHealthCheckProcessor {
    pub fn new() -> Self {
        Self
    }

    async fn http_check(
        &self,
        ip: &str,
        port: i32,
        path: &str,
        timeout_duration: Duration,
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
        let stream = match timeout(timeout_duration, TcpStream::connect(&addr)).await {
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

        // Send HTTP GET request
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
            path, ip, port
        );

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
        let remaining = timeout_duration.saturating_sub(start.elapsed());

        match timeout(remaining, reader.read(&mut response)).await {
            Ok(Ok(n)) if n > 0 => {
                let response_str = String::from_utf8_lossy(&response[..n]);

                // Check HTTP status code (2xx or 3xx is considered healthy)
                if let Some(status_line) = response_str.lines().next()
                    && let Some(status_code) = status_line.split_whitespace().nth(1)
                    && let Ok(code) = status_code.parse::<u16>()
                {
                    if (200..400).contains(&code) {
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

impl Default for HttpHealthCheckProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckProcessor for HttpHealthCheckProcessor {
    async fn check(
        &self,
        instance: &Instance,
        cluster_config: &ClusterConfig,
    ) -> HealthCheckResult {
        // Determine check port
        let check_port = if cluster_config.use_instance_port || cluster_config.check_port <= 0 {
            instance.port
        } else {
            cluster_config.check_port
        };

        // Get health check path from metadata
        let path = cluster_config
            .metadata
            .get("health_check_path")
            .map(|s| s.as_str())
            .unwrap_or("/health");

        // Use 500ms timeout as per Nacos
        self.http_check(&instance.ip, check_port, path, Duration::from_millis(500))
            .await
    }

    fn get_type(&self) -> HealthCheckType {
        HealthCheckType::Http
    }
}

/// None health check processor (no health check)
pub struct NoneHealthCheckProcessor;

impl NoneHealthCheckProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoneHealthCheckProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckProcessor for NoneHealthCheckProcessor {
    async fn check(
        &self,
        _instance: &Instance,
        _cluster_config: &ClusterConfig,
    ) -> HealthCheckResult {
        // No health check, always return success
        HealthCheckResult {
            success: true,
            message: None,
            response_time_ms: 0,
        }
    }

    fn get_type(&self) -> HealthCheckType {
        HealthCheckType::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_type_from_str() {
        assert_eq!(HealthCheckType::from_str("TCP"), HealthCheckType::Tcp);
        assert_eq!(HealthCheckType::from_str("tcp"), HealthCheckType::Tcp);
        assert_eq!(HealthCheckType::from_str("HTTP"), HealthCheckType::Http);
        assert_eq!(HealthCheckType::from_str("NONE"), HealthCheckType::None);
        assert_eq!(HealthCheckType::from_str("UNKNOWN"), HealthCheckType::None);
    }

    #[test]
    fn test_health_check_type_as_str() {
        assert_eq!(HealthCheckType::Tcp.as_str(), "TCP");
        assert_eq!(HealthCheckType::Http.as_str(), "HTTP");
        assert_eq!(HealthCheckType::None.as_str(), "NONE");
    }

    #[tokio::test]
    async fn test_tcp_health_check_invalid_address() {
        let processor = TcpHealthCheckProcessor::new();
        let instance = Instance {
            ip: "invalid".to_string(),
            port: 8080,
            ..Default::default()
        };
        let cluster_config = ClusterConfig::default();
        let result = processor.check(&instance, &cluster_config).await;
        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_http_health_check_invalid_address() {
        let processor = HttpHealthCheckProcessor::new();
        let instance = Instance {
            ip: "invalid".to_string(),
            port: 8080,
            ..Default::default()
        };
        let cluster_config = ClusterConfig::default();
        let result = processor.check(&instance, &cluster_config).await;
        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_none_health_check() {
        let processor = NoneHealthCheckProcessor::new();
        let instance = Instance::default();
        let cluster_config = ClusterConfig::default();
        let result = processor.check(&instance, &cluster_config).await;
        assert!(result.success);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_health_check_type_case_insensitive() {
        assert_eq!(HealthCheckType::from_str("TCP"), HealthCheckType::Tcp);
        assert_eq!(HealthCheckType::from_str("Tcp"), HealthCheckType::Tcp);
        assert_eq!(HealthCheckType::from_str("tcp"), HealthCheckType::Tcp);
        assert_eq!(HealthCheckType::from_str("http"), HealthCheckType::Http);
        assert_eq!(HealthCheckType::from_str("NONE"), HealthCheckType::None);
        assert_eq!(HealthCheckType::from_str("none"), HealthCheckType::None);
    }
}
