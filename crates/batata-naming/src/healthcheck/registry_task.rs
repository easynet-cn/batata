//! Registry-driven health check task
//!
//! A check task that reads its configuration from InstanceCheckRegistry,
//! executes the appropriate check protocol, and updates the registry result.

use std::sync::Arc;
use std::time::Duration;

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use tracing::{debug, warn};

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
                let addr = config.grpc_addr.as_deref().unwrap_or(&default_tcp_addr);
                execute_grpc_check(addr, config.timeout).await
            }
            CheckType::Mysql => {
                if let Some(ref db_url) = config.db_url {
                    execute_db_check(db_url, config.timeout).await
                } else {
                    // Fallback: TCP connection check on the default address
                    let addr = config.tcp_addr.as_deref().unwrap_or(&default_tcp_addr);
                    execute_tcp_check(addr, config.timeout).await
                }
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
    // Use reqwest for proper HTTP/HTTPS support, redirects, and connection handling.
    // Consul interprets status codes: 2xx = passing, 429 = warning, else = critical.
    // For the check result we map: 2xx/3xx = healthy, else = unhealthy.
    let client = match reqwest::Client::builder()
        .timeout(timeout_duration)
        .connect_timeout(timeout_duration)
        .danger_accept_invalid_certs(true) // Match Consul's TLSSkipVerify default
        .build()
    {
        Ok(c) => c,
        Err(e) => return (false, format!("HTTP client build failed: {} - {}", url, e)),
    };

    match client.get(url).send().await {
        Ok(response) => {
            let code = response.status().as_u16();
            if (200..400).contains(&code) {
                (true, format!("HTTP check passed: {} ({})", url, code))
            } else {
                (
                    false,
                    format!("HTTP check failed: {} returned {}", url, code),
                )
            }
        }
        Err(e) => {
            if e.is_timeout() {
                (false, format!("HTTP check timeout: {}", url))
            } else if e.is_connect() {
                (false, format!("HTTP connection failed: {} - {}", url, e))
            } else {
                (false, format!("HTTP check failed: {} - {}", url, e))
            }
        }
    }
}

/// Execute a gRPC health check.
///
/// Connects to the gRPC endpoint and verifies the server responds with HTTP/2.
/// The address format is `host:port[/service]`.
///
/// Uses a TCP connection + HTTP/2 connection preface to validate the gRPC server
/// is alive and accepting connections.
async fn execute_grpc_check(addr: &str, timeout_duration: Duration) -> (bool, String) {
    // Parse address: strip optional "/service" suffix (Consul format: "host:port/service")
    let (endpoint, service_name) = match addr.find('/') {
        Some(idx) => (&addr[..idx], &addr[idx + 1..]),
        None => (addr, ""),
    };

    let endpoint_url = format!("http://{}", endpoint);
    match tokio::time::timeout(timeout_duration, async {
        let channel = tonic::transport::Endpoint::from_shared(endpoint_url)
            .map_err(|e| format!("Invalid endpoint: {}", e))?
            .connect_timeout(timeout_duration)
            .connect()
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;

        // Call grpc.health.v1.Health/Check using tonic raw codec
        let mut client = tonic::client::Grpc::new(channel);
        client
            .ready()
            .await
            .map_err(|e| format!("Service not ready: {}", e))?;

        let path = http::uri::PathAndQuery::from_static("/grpc.health.v1.Health/Check");
        let codec =
            tonic_prost::ProstCodec::<GrpcHealthCheckRequest, GrpcHealthCheckResponse>::default();

        let request = tonic::Request::new(GrpcHealthCheckRequest {
            service: service_name.to_string(),
        });

        let response = client
            .unary(request, path, codec)
            .await
            .map_err(|e| format!("Health check RPC failed: {}", e))?;

        let status = response.into_inner().status;
        // ServingStatus: UNKNOWN=0, SERVING=1, NOT_SERVING=2, SERVICE_UNKNOWN=3
        if status == 1 {
            Ok(())
        } else {
            let status_name = match status {
                0 => "UNKNOWN",
                2 => "NOT_SERVING",
                3 => "SERVICE_UNKNOWN",
                _ => "INVALID",
            };
            Err(format!("gRPC {} serving status: {}", addr, status_name))
        }
    })
    .await
    {
        Ok(Ok(())) => {
            debug!("gRPC health check passed: {}", addr);
            (true, format!("gRPC health check passed: {}", addr))
        }
        Ok(Err(e)) => {
            debug!("gRPC health check failed: {} - {}", addr, e);
            (false, format!("gRPC health check failed: {} - {}", addr, e))
        }
        Err(_) => {
            debug!("gRPC health check timeout: {}", addr);
            (false, format!("gRPC health check timeout: {}", addr))
        }
    }
}

/// gRPC Health Check Request (grpc.health.v1.HealthCheckRequest)
#[derive(Clone, PartialEq, prost::Message)]
struct GrpcHealthCheckRequest {
    #[prost(string, tag = "1")]
    service: String,
}

/// gRPC Health Check Response (grpc.health.v1.HealthCheckResponse)
#[derive(Clone, PartialEq, prost::Message)]
struct GrpcHealthCheckResponse {
    /// ServingStatus: UNKNOWN=0, SERVING=1, NOT_SERVING=2, SERVICE_UNKNOWN=3
    #[prost(int32, tag = "1")]
    status: i32,
}

/// Execute a database health check via sea-orm.
///
/// Connects to the database using the provided URL and executes a simple
/// query (`SELECT 1`) to verify connectivity. Supports MySQL, PostgreSQL,
/// and SQLite via sea-orm's backend detection.
async fn execute_db_check(db_url: &str, timeout_duration: Duration) -> (bool, String) {
    match tokio::time::timeout(timeout_duration, async {
        // sea-orm auto-detects backend from URL prefix (mysql://, postgres://, sqlite://)
        let db = Database::connect(db_url)
            .await
            .map_err(|e| format!("Database connection failed: {}", e))?;

        // Determine the SQL dialect for a simple health query
        let sql = match db.get_database_backend() {
            DbBackend::MySql | DbBackend::Postgres => "SELECT 1",
            DbBackend::Sqlite => "SELECT 1",
        };

        db.execute(Statement::from_string(db.get_database_backend(), sql))
            .await
            .map_err(|e| format!("Database query failed: {}", e))?;

        // Close connection
        db.close().await.ok();
        Ok::<_, String>(())
    })
    .await
    {
        Ok(Ok(())) => {
            debug!("Database check passed: {}", db_url);
            (
                true,
                format!("Database check passed: {}", sanitize_db_url(db_url)),
            )
        }
        Ok(Err(e)) => {
            warn!("Database check failed: {} - {}", sanitize_db_url(db_url), e);
            (
                false,
                format!("Database check failed: {} - {}", sanitize_db_url(db_url), e),
            )
        }
        Err(_) => {
            warn!("Database check timeout: {}", sanitize_db_url(db_url));
            (
                false,
                format!("Database check timeout: {}", sanitize_db_url(db_url)),
            )
        }
    }
}

/// Sanitize database URL for logging (hide password)
fn sanitize_db_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@')
        && let Some(colon_pos) = url[..at_pos].rfind(':')
    {
        let prefix = &url[..colon_pos + 1];
        let suffix = &url[at_pos..];
        return format!("{}***{}", prefix, suffix);
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::super::registry::*;
    use super::*;
    use crate::service::NamingService;

    fn create_registry() -> Arc<InstanceCheckRegistry> {
        let naming_service = Arc::new(NamingService::new());
        Arc::new(InstanceCheckRegistry::with_naming_service(naming_service))
    }

    fn create_tcp_check_config(check_id: &str) -> InstanceCheckConfig {
        InstanceCheckConfig {
            check_id: check_id.to_string(),
            name: format!("TCP check {}", check_id),
            check_type: CheckType::Tcp,
            namespace: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-svc".to_string(),
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            http_url: None,
            tcp_addr: None,
            grpc_addr: None,
            db_url: None,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(2),
            ttl: None,
            success_before_passing: 0,
            failures_before_critical: 0,
            deregister_critical_after: None,
            initial_status: CheckStatus::Passing,
            notes: String::new(),
            service_tags: vec![],
        }
    }

    #[test]
    fn test_registry_task_creation() {
        let registry = create_registry();
        let task = RegistryCheckTask::new("test-check".to_string(), registry);
        assert_eq!(task.check_key(), "test-check");
    }

    #[test]
    fn test_registry_task_interval_returns_none_when_removed() {
        let registry = create_registry();
        let task = RegistryCheckTask::new("nonexistent".to_string(), registry);
        assert!(
            task.interval().is_none(),
            "Interval should be None for nonexistent check"
        );
    }

    #[test]
    fn test_registry_task_interval_returns_configured_value() {
        let registry = create_registry();
        let config = create_tcp_check_config("interval-test");
        registry.register_check(config);

        let task = RegistryCheckTask::new("interval-test".to_string(), registry);
        assert_eq!(task.interval(), Some(Duration::from_secs(10)));
    }

    #[tokio::test]
    async fn test_registry_task_execute_skips_removed_check() {
        let registry = create_registry();
        // Don't register any check — execute should gracefully skip
        let task = RegistryCheckTask::new("removed-check".to_string(), registry);
        task.execute().await; // Should not panic
    }

    #[tokio::test]
    async fn test_registry_task_execute_skips_ttl_check() {
        let registry = create_registry();
        let mut config = create_tcp_check_config("ttl-skip");
        config.check_type = CheckType::Ttl;
        registry.register_check(config);

        let task = RegistryCheckTask::new("ttl-skip".to_string(), registry.clone());
        task.execute().await;

        // TTL check should not be executed — status unchanged
        let (_, status) = registry.get_check("ttl-skip").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "TTL check should not be executed by registry task"
        );
    }

    #[tokio::test]
    async fn test_registry_task_execute_skips_none_check() {
        let registry = create_registry();
        let mut config = create_tcp_check_config("none-skip");
        config.check_type = CheckType::None;
        registry.register_check(config);

        let task = RegistryCheckTask::new("none-skip".to_string(), registry.clone());
        task.execute().await;

        let (_, status) = registry.get_check("none-skip").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Passing,
            "None check should not be executed"
        );
    }

    #[tokio::test]
    async fn test_registry_task_tcp_check_unreachable_host() {
        let registry = create_registry();
        let mut config = create_tcp_check_config("tcp-fail");
        config.tcp_addr = Some("127.0.0.1:19".to_string()); // Port 19 (chargen) — typically not listening
        config.timeout = Duration::from_millis(500);
        config.initial_status = CheckStatus::Passing;
        registry.register_check(config);

        let task = RegistryCheckTask::new("tcp-fail".to_string(), registry.clone());
        task.execute().await;

        let (_, status) = registry.get_check("tcp-fail").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Critical,
            "TCP check to unreachable host should fail"
        );
        assert!(
            status.output.contains("TCP check"),
            "Output should describe TCP check result"
        );
    }

    #[tokio::test]
    async fn test_registry_task_http_check_unreachable_host() {
        let registry = create_registry();
        let mut config = create_tcp_check_config("http-fail");
        config.check_type = CheckType::Http;
        config.http_url = Some("http://127.0.0.1:19/health".to_string());
        config.timeout = Duration::from_millis(500);
        config.initial_status = CheckStatus::Passing;
        registry.register_check(config);

        let task = RegistryCheckTask::new("http-fail".to_string(), registry.clone());
        task.execute().await;

        let (_, status) = registry.get_check("http-fail").unwrap();
        assert_eq!(
            status.status,
            CheckStatus::Critical,
            "HTTP check to unreachable host should fail"
        );
        assert!(
            status.output.contains("HTTP"),
            "Output should describe HTTP check result"
        );
    }

    #[test]
    fn test_sanitize_db_url_hides_password() {
        assert_eq!(
            sanitize_db_url("mysql://user:secret@host:3306/db"),
            "mysql://user:***@host:3306/db"
        );
    }

    #[test]
    fn test_sanitize_db_url_no_password() {
        assert_eq!(sanitize_db_url("sqlite://data.db"), "sqlite://data.db");
    }
}
