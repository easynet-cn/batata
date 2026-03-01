//! Registry-driven health check task
//!
//! A check task that reads its configuration from InstanceCheckRegistry,
//! executes the appropriate check protocol, and updates the registry result.

use std::sync::Arc;
use std::time::Duration;

use tracing::debug;

use super::registry::{CheckType, InstanceCheckRegistry};

/// A check task driven by InstanceCheckRegistry
pub struct RegistryCheckTask {
    check_key: String,
    registry: Arc<InstanceCheckRegistry>,
}

impl RegistryCheckTask {
    /// Create a new registry check task
    pub fn new(check_key: String, registry: Arc<InstanceCheckRegistry>) -> Self {
        Self {
            check_key,
            registry,
        }
    }

    /// Execute one check cycle:
    /// 1. Read config from registry
    /// 2. Execute TCP/HTTP/gRPC check (skip TTL/None)
    /// 3. Call registry.update_check_result() (handles thresholds + sync)
    pub async fn execute(&self) {
        let config = match self.registry.get_check_config(&self.check_key) {
            Some(c) => c,
            None => {
                debug!("Check {} no longer in registry, skipping", self.check_key);
                return;
            }
        };

        // Only execute active checks
        if !config.check_type.is_active() {
            return;
        }

        let start = std::time::Instant::now();

        let default_tcp_addr = format!("{}:{}", config.ip, config.port);
        let default_http_url = format!("http://{}:{}/health", config.ip, config.port);

        let (success, output) = match config.check_type {
            CheckType::Tcp => {
                let addr = config.tcp_addr.as_deref().unwrap_or(&default_tcp_addr);
                execute_tcp_check(addr, config.timeout).await
            }
            CheckType::Http => {
                let url = config.http_url.as_deref().unwrap_or(&default_http_url);
                execute_http_check(url, config.timeout).await
            }
            CheckType::Grpc => {
                // gRPC health check is a stub for now
                (false, "gRPC health check not fully implemented".to_string())
            }
            CheckType::None | CheckType::Ttl => return,
        };

        let response_time_ms = start.elapsed().as_millis() as u64;

        self.registry
            .update_check_result(&self.check_key, success, output, response_time_ms);
    }

    /// Get the check interval, or None if the check was removed from registry
    pub fn interval(&self) -> Option<Duration> {
        self.registry
            .get_check_config(&self.check_key)
            .map(|c| c.interval)
    }

    /// Get the check key
    pub fn check_key(&self) -> &str {
        &self.check_key
    }
}

/// Execute a TCP health check
async fn execute_tcp_check(addr: &str, timeout_duration: Duration) -> (bool, String) {
    match tokio::time::timeout(timeout_duration, tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => {
            debug!("TCP check passed: {}", addr);
            (true, format!("TCP check passed: {}", addr))
        }
        Ok(Err(e)) => {
            debug!("TCP check failed: {} - {}", addr, e);
            (false, format!("TCP check failed: {} - {}", addr, e))
        }
        Err(_) => {
            debug!("TCP check timeout: {}", addr);
            (false, format!("TCP check timeout: {}", addr))
        }
    }
}

/// Execute an HTTP health check
async fn execute_http_check(url: &str, timeout_duration: Duration) -> (bool, String) {
    // Parse URL to extract host, port, and path for raw HTTP
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host_port, path) = match stripped.find('/') {
        Some(idx) => (&stripped[..idx], &stripped[idx..]),
        None => (stripped, "/"),
    };

    let (host, port) = match host_port.rfind(':') {
        Some(idx) => {
            let port_str = &host_port[idx + 1..];
            let port: u16 = port_str.parse().unwrap_or(80);
            (&host_port[..idx], port)
        }
        None => (host_port, 80u16),
    };

    let addr = format!("{}:{}", host, port);

    // Connect with timeout
    let stream =
        match tokio::time::timeout(timeout_duration, tokio::net::TcpStream::connect(&addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return (false, format!("HTTP connection failed: {} - {}", url, e)),
            Err(_) => return (false, format!("HTTP connection timeout: {}", url)),
        };

    // Send HTTP GET request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        path, host, port
    );

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut reader, mut writer) = stream.into_split();

    if let Err(e) = writer.write_all(request.as_bytes()).await {
        return (false, format!("HTTP send failed: {} - {}", url, e));
    }

    // Read response
    let mut response = vec![0u8; 1024];
    let remaining = timeout_duration.saturating_sub(std::time::Duration::from_millis(100));

    match tokio::time::timeout(remaining, reader.read(&mut response)).await {
        Ok(Ok(n)) if n > 0 => {
            let response_str = String::from_utf8_lossy(&response[..n]);
            if let Some(status_line) = response_str.lines().next()
                && let Some(status_code) = status_line.split_whitespace().nth(1)
                && let Ok(code) = status_code.parse::<u16>()
            {
                if (200..400).contains(&code) {
                    (true, format!("HTTP check passed: {} ({})", url, code))
                } else {
                    (
                        false,
                        format!("HTTP check failed: {} returned {}", url, code),
                    )
                }
            } else {
                (false, format!("HTTP invalid response: {}", url))
            }
        }
        Ok(Ok(_)) => (false, format!("HTTP empty response: {}", url)),
        Ok(Err(e)) => (false, format!("HTTP read failed: {} - {}", url, e)),
        Err(_) => (false, format!("HTTP response timeout: {}", url)),
    }
}
