//! Test server management for integration tests
//!
//! Provides utilities to start and stop a Batata server instance for testing.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for the test server
#[derive(Clone, Debug)]
pub struct TestServerConfig {
    /// HTTP port (0 for random)
    pub http_port: u16,
    /// gRPC port (0 for random)
    pub grpc_port: u16,
    /// Console port (0 for random)
    pub console_port: u16,
    /// Database URL (optional)
    pub db_url: Option<String>,
    /// Working directory
    pub working_dir: Option<String>,
    /// Server startup timeout in seconds
    pub startup_timeout_secs: u64,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            http_port: 0, // Random port
            grpc_port: 0,
            console_port: 0,
            db_url: None,
            working_dir: None,
            startup_timeout_secs: 30,
        }
    }
}

impl TestServerConfig {
    /// Create config with specific ports
    pub fn with_ports(http_port: u16, grpc_port: u16, console_port: u16) -> Self {
        Self {
            http_port,
            grpc_port,
            console_port,
            ..Default::default()
        }
    }

    /// Set database URL
    pub fn with_db(mut self, db_url: &str) -> Self {
        self.db_url = Some(db_url.to_string());
        self
    }

    /// Set working directory
    pub fn with_working_dir(mut self, dir: &str) -> Self {
        self.working_dir = Some(dir.to_string());
        self
    }
}

/// A test server instance that manages the lifecycle of a Batata server
pub struct TestServer {
    /// Server process handle
    process: Option<Child>,
    /// HTTP address (host:port)
    http_addr: String,
    /// gRPC address (host:port)
    grpc_addr: String,
    /// Console address (host:port)
    console_addr: String,
    /// Configuration used
    config: TestServerConfig,
}

impl TestServer {
    /// Start a new test server with default configuration
    pub async fn start() -> Result<Self, TestServerError> {
        Self::start_with_config(TestServerConfig::default()).await
    }

    /// Start a new test server with database
    pub async fn start_with_db(db_url: &str) -> Result<Self, TestServerError> {
        let config = TestServerConfig::default().with_db(db_url);
        Self::start_with_config(config).await
    }

    /// Start a new test server with custom configuration
    pub async fn start_with_config(config: TestServerConfig) -> Result<Self, TestServerError> {
        // Find available ports if not specified
        let http_port = if config.http_port == 0 {
            find_available_port()?
        } else {
            config.http_port
        };

        let grpc_port = if config.grpc_port == 0 {
            find_available_port()?
        } else {
            config.grpc_port
        };

        let console_port = if config.console_port == 0 {
            find_available_port()?
        } else {
            config.console_port
        };

        let http_addr = format!("127.0.0.1:{}", http_port);
        let grpc_addr = format!("127.0.0.1:{}", grpc_port);
        let console_addr = format!("127.0.0.1:{}", console_port);

        // Build command
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "-p", "batata-server", "--"])
            .arg("--main-port")
            .arg(http_port.to_string())
            .arg("--grpc-port")
            .arg(grpc_port.to_string())
            .arg("--console-port")
            .arg(console_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set database URL if provided
        if let Some(db_url) = &config.db_url {
            cmd.arg("--db-url").arg(db_url);
        }

        // Set working directory if provided
        if let Some(dir) = &config.working_dir {
            cmd.current_dir(dir);
        }

        // Start the server process
        let process = cmd.spawn().map_err(|e| TestServerError::SpawnFailed(e.to_string()))?;

        let mut server = Self {
            process: Some(process),
            http_addr: http_addr.clone(),
            grpc_addr,
            console_addr,
            config: config.clone(),
        };

        // Wait for server to be ready
        let timeout = Duration::from_secs(config.startup_timeout_secs);
        server.wait_for_ready(timeout).await?;

        Ok(server)
    }

    /// Get the HTTP base URL
    pub fn http_url(&self) -> String {
        format!("http://{}", self.http_addr)
    }

    /// Get the HTTP address (host:port)
    pub fn http_addr(&self) -> &str {
        &self.http_addr
    }

    /// Get the gRPC address (host:port)
    pub fn grpc_addr(&self) -> &str {
        &self.grpc_addr
    }

    /// Get the console address (host:port)
    pub fn console_addr(&self) -> &str {
        &self.console_addr
    }

    /// Check if the server is healthy
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/nacos/v2/core/cluster/node/self/health", self.http_url());

        match reqwest::get(&url).await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Wait for the server to be ready
    async fn wait_for_ready(&mut self, timeout: Duration) -> Result<(), TestServerError> {
        let start = std::time::Instant::now();
        let check_interval = Duration::from_millis(500);

        while start.elapsed() < timeout {
            // Check if process is still running
            if let Some(ref mut process) = self.process {
                match process.try_wait() {
                    Ok(Some(status)) => {
                        return Err(TestServerError::ProcessExited(status.code()));
                    }
                    Ok(None) => {
                        // Process is still running, check health
                        if self.health_check().await {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        return Err(TestServerError::ProcessCheckFailed(e.to_string()));
                    }
                }
            }

            sleep(check_interval).await;
        }

        Err(TestServerError::Timeout)
    }

    /// Stop the server gracefully
    pub async fn stop(&mut self) -> Result<(), TestServerError> {
        if let Some(mut process) = self.process.take() {
            // Kill the process (cross-platform)
            let _ = process.kill();

            // Wait for process to exit
            let _ = process.wait();
        }

        Ok(())
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            // Force kill on drop
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

/// Errors that can occur when managing the test server
#[derive(Debug)]
pub enum TestServerError {
    /// Failed to spawn server process
    SpawnFailed(String),
    /// Server process exited unexpectedly
    ProcessExited(Option<i32>),
    /// Failed to check process status
    ProcessCheckFailed(String),
    /// Server startup timeout
    Timeout,
    /// No available port found
    NoAvailablePort,
    /// Health check failed
    HealthCheckFailed,
}

impl std::fmt::Display for TestServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnFailed(e) => write!(f, "Failed to spawn server: {}", e),
            Self::ProcessExited(code) => write!(f, "Server process exited with code: {:?}", code),
            Self::ProcessCheckFailed(e) => write!(f, "Failed to check process: {}", e),
            Self::Timeout => write!(f, "Server startup timeout"),
            Self::NoAvailablePort => write!(f, "No available port found"),
            Self::HealthCheckFailed => write!(f, "Health check failed"),
        }
    }
}

impl std::error::Error for TestServerError {}

/// Find an available TCP port
fn find_available_port() -> Result<u16, TestServerError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| TestServerError::NoAvailablePort)?;
    let port = listener
        .local_addr()
        .map_err(|_| TestServerError::NoAvailablePort)?
        .port();
    Ok(port)
}

/// Test server builder for more complex configurations
pub struct TestServerBuilder {
    config: TestServerConfig,
}

impl TestServerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: TestServerConfig::default(),
        }
    }

    /// Set HTTP port
    pub fn http_port(mut self, port: u16) -> Self {
        self.config.http_port = port;
        self
    }

    /// Set gRPC port
    pub fn grpc_port(mut self, port: u16) -> Self {
        self.config.grpc_port = port;
        self
    }

    /// Set console port
    pub fn console_port(mut self, port: u16) -> Self {
        self.config.console_port = port;
        self
    }

    /// Set database URL
    pub fn db_url(mut self, url: &str) -> Self {
        self.config.db_url = Some(url.to_string());
        self
    }

    /// Set working directory
    pub fn working_dir(mut self, dir: &str) -> Self {
        self.config.working_dir = Some(dir.to_string());
        self
    }

    /// Set startup timeout
    pub fn startup_timeout(mut self, secs: u64) -> Self {
        self.config.startup_timeout_secs = secs;
        self
    }

    /// Build and start the server
    pub async fn start(self) -> Result<TestServer, TestServerError> {
        TestServer::start_with_config(self.config).await
    }
}

impl Default for TestServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_available_port() {
        let port = find_available_port().unwrap();
        assert!(port > 0);
    }

    #[test]
    fn test_config_default() {
        let config = TestServerConfig::default();
        assert_eq!(config.http_port, 0);
        assert_eq!(config.startup_timeout_secs, 30);
    }

    #[test]
    fn test_config_builder() {
        let config = TestServerConfig::with_ports(8848, 9848, 8081)
            .with_db("mysql://localhost/test");

        assert_eq!(config.http_port, 8848);
        assert_eq!(config.grpc_port, 9848);
        assert_eq!(config.console_port, 8081);
        assert_eq!(config.db_url, Some("mysql://localhost/test".to_string()));
    }
}
