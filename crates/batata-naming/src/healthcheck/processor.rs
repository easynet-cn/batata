//! Health check processors for different protocols
//!
//! This module provides health check processors that match Nacos implementation:
//! - TCP health check (NIO based, 500ms timeout)
//! - HTTP health check (GET request, 200-399 status code)
//! - None (no health check)

use crate::model::Instance;
use crate::service::ClusterConfig;
use dashmap::DashMap;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

/// Health check types
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HealthCheckType {
    None,
    Tcp,
    Http,
    /// TTL-based passive check (client calls /check/pass)
    Ttl,
    /// gRPC health protocol
    Grpc,
    /// MySQL/Database health check
    Mysql,
}

impl HealthCheckType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "TCP" => Self::Tcp,
            "HTTP" => Self::Http,
            "TTL" => Self::Ttl,
            "GRPC" => Self::Grpc,
            "MYSQL" => Self::Mysql,
            _ => Self::None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "NONE",
            Self::Tcp => "TCP",
            Self::Http => "HTTP",
            Self::Ttl => "TTL",
            Self::Grpc => "GRPC",
            Self::Mysql => "MYSQL",
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

/// MySQL/Database health check processor (matches Nacos MysqlHealthCheckProcessor)
///
/// Connects to a database using sea-orm and executes a health check query.
/// Supports connection pooling via a cached connection map keyed by URL.
pub struct MysqlHealthCheckProcessor {
    /// Connection pool cache: url -> DatabaseConnection
    connections: Arc<DashMap<String, DatabaseConnection>>,
    /// Connection timeout
    connect_timeout: Duration,
}

impl MysqlHealthCheckProcessor {
    pub fn new(connect_timeout: Duration) -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
            connect_timeout,
        }
    }

    /// Get or create a sea-orm database connection for the given URL.
    async fn get_connection(&self, url: &str) -> anyhow::Result<DatabaseConnection> {
        if let Some(conn) = self.connections.get(url) {
            return Ok(conn.clone());
        }

        let mut opts = sea_orm::ConnectOptions::new(url);
        opts.max_connections(2)
            .min_connections(1)
            .connect_timeout(self.connect_timeout)
            .acquire_timeout(self.connect_timeout);

        let db = sea_orm::Database::connect(opts).await?;
        self.connections.insert(url.to_string(), db.clone());
        Ok(db)
    }

    /// Execute a health check query against the given database URL.
    ///
    /// If `command` is provided, it is used as the SQL query; otherwise `SELECT 1` is used.
    /// This is compatible with the Nacos MysqlHealthCheckProcessor pattern, which uses
    /// `SHOW GLOBAL VARIABLES WHERE Variable_name='read_only'` for master-slave detection.
    pub async fn mysql_check(&self, url: &str, command: Option<&str>) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let db = match timeout(self.connect_timeout, self.get_connection(url)).await {
            Ok(Ok(db)) => db,
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

        let query = command.unwrap_or("SELECT 1");
        let result = timeout(
            self.connect_timeout,
            db.execute(sea_orm::Statement::from_string(
                db.get_database_backend(),
                query.to_string(),
            )),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(_)) => {
                debug!("MySQL health check passed for {} ({}ms)", url, elapsed);
                HealthCheckResult {
                    success: true,
                    message: None,
                    response_time_ms: elapsed,
                }
            }
            Ok(Err(e)) => {
                debug!("MySQL health check failed for {}: {}", url, e);
                HealthCheckResult {
                    success: false,
                    message: Some(format!("Query failed: {}", e)),
                    response_time_ms: elapsed,
                }
            }
            Err(_) => {
                debug!("MySQL health check timeout for {}", url);
                HealthCheckResult {
                    success: false,
                    message: Some("Query timeout".to_string()),
                    response_time_ms: elapsed,
                }
            }
        }
    }

    /// Close and remove a cached connection.
    pub async fn remove_connection(&self, url: &str) {
        if let Some((_, db)) = self.connections.remove(url) {
            let _ = db.close().await;
        }
    }

    /// Close all cached connections.
    pub async fn close_all(&self) {
        let keys: Vec<String> = self.connections.iter().map(|e| e.key().clone()).collect();
        for key in keys {
            self.remove_connection(&key).await;
        }
    }

    /// Get the number of cached connections.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

impl Default for MysqlHealthCheckProcessor {
    fn default() -> Self {
        Self::new(Duration::from_secs(3))
    }
}

impl HealthCheckProcessor for MysqlHealthCheckProcessor {
    async fn check(
        &self,
        instance: &Instance,
        cluster_config: &ClusterConfig,
    ) -> HealthCheckResult {
        // Build database URL from instance metadata or cluster config.
        // The URL should be stored in the cluster metadata under "db_url" key.
        // Falls back to building a mysql:// URL from instance ip:port.
        let url = cluster_config
            .metadata
            .get("db_url")
            .cloned()
            .unwrap_or_else(|| {
                let check_port =
                    if cluster_config.use_instance_port || cluster_config.check_port <= 0 {
                        instance.port
                    } else {
                        cluster_config.check_port
                    };
                format!("mysql://{}:{}", instance.ip, check_port)
            });

        // Get custom health check command from metadata
        let command = cluster_config.metadata.get("health_check_command");

        self.mysql_check(&url, command.map(|s| s.as_str())).await
    }

    fn get_type(&self) -> HealthCheckType {
        HealthCheckType::Mysql
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

    #[test]
    fn test_health_check_type_mysql() {
        assert_eq!(HealthCheckType::from_str("MYSQL"), HealthCheckType::Mysql);
        assert_eq!(HealthCheckType::from_str("mysql"), HealthCheckType::Mysql);
        assert_eq!(HealthCheckType::from_str("Mysql"), HealthCheckType::Mysql);
        assert_eq!(HealthCheckType::Mysql.as_str(), "MYSQL");
    }

    #[test]
    fn test_mysql_processor_default() {
        let processor = MysqlHealthCheckProcessor::default();
        assert_eq!(processor.get_type(), HealthCheckType::Mysql);
        assert_eq!(processor.connection_count(), 0);
    }

    #[test]
    fn test_mysql_processor_custom_timeout() {
        let processor = MysqlHealthCheckProcessor::new(Duration::from_secs(5));
        assert_eq!(processor.get_type(), HealthCheckType::Mysql);
        assert_eq!(processor.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_mysql_health_check_invalid_url() {
        let processor = MysqlHealthCheckProcessor::new(Duration::from_secs(1));
        let result = processor
            .mysql_check("mysql://invalid-host:3306/test", None)
            .await;
        assert!(!result.success);
        assert!(result.message.is_some());
        assert!(result.response_time_ms > 0 || result.message.as_ref().unwrap().contains("failed"));
    }

    #[tokio::test]
    async fn test_mysql_health_check_via_processor_trait() {
        let processor = MysqlHealthCheckProcessor::new(Duration::from_secs(1));
        let instance = Instance {
            ip: "invalid-host".to_string(),
            port: 3306,
            ..Default::default()
        };
        let cluster_config = ClusterConfig::default();
        let result = processor.check(&instance, &cluster_config).await;
        assert!(!result.success);
        assert!(result.message.is_some());
    }

    #[tokio::test]
    async fn test_mysql_health_check_with_db_url_metadata() {
        let processor = MysqlHealthCheckProcessor::new(Duration::from_secs(1));
        let instance = Instance::default();
        let mut cluster_config = ClusterConfig::default();
        cluster_config.metadata.insert(
            "db_url".to_string(),
            "mysql://test-host:3306/testdb".to_string(),
        );
        let result = processor.check(&instance, &cluster_config).await;
        // Will fail because host is invalid, but verifies the metadata path
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_mysql_processor_close_all() {
        let processor = MysqlHealthCheckProcessor::default();
        // Should not panic even with no connections
        processor.close_all().await;
        assert_eq!(processor.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_mysql_processor_remove_nonexistent_connection() {
        let processor = MysqlHealthCheckProcessor::default();
        // Should not panic when removing a connection that does not exist
        processor
            .remove_connection("mysql://nonexistent:3306/db")
            .await;
        assert_eq!(processor.connection_count(), 0);
    }
}
