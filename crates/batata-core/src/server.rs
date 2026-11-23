//! Server lifecycle abstraction
//!
//! Provides a unified `ManagedServer` trait for all server types (HTTP, gRPC, Raft, etc.)
//! and a `ServerOrchestrator` that manages startup ordering, shutdown sequencing,
//! and per-server health status aggregation.
//!
//! # Design Principles
//!
//! - **Incremental adoption**: Existing servers can implement `ManagedServer` one at a time
//!   without rewriting main.rs all at once.
//! - **Lock-free hot path**: `ServerState` uses `AtomicU8` for zero-contention reads.
//! - **Dependency ordering**: Servers declare a `start_priority()` — lower values start first.
//!   Shutdown runs in reverse order.
//! - **Health aggregation**: The orchestrator exposes per-server state for diagnostics.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Lifecycle state of a single managed server.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// Server is created but not yet started.
    Created = 0,
    /// Server is in the process of starting (binding ports, loading data, etc.).
    Starting = 1,
    /// Server is fully operational and accepting traffic.
    Running = 2,
    /// Server is rejecting new connections but finishing in-flight work.
    Draining = 3,
    /// Server has stopped gracefully.
    Stopped = 4,
    /// Server encountered an unrecoverable error.
    Failed = 5,
}

impl ServerState {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Starting,
            2 => Self::Running,
            3 => Self::Draining,
            4 => Self::Stopped,
            5 => Self::Failed,
            _ => Self::Created,
        }
    }

    /// Returns true if the server is ready to handle requests.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}

impl fmt::Display for ServerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "CREATED"),
            Self::Starting => write!(f, "STARTING"),
            Self::Running => write!(f, "RUNNING"),
            Self::Draining => write!(f, "DRAINING"),
            Self::Stopped => write!(f, "STOPPED"),
            Self::Failed => write!(f, "FAILED"),
        }
    }
}

/// Thread-safe atomic state tracker for a single server.
///
/// Uses `AtomicU8` so that state reads on the hot path (health checks,
/// traffic filtering) are a single atomic load with zero contention.
#[derive(Clone)]
pub struct ServerStateTracker {
    state: Arc<AtomicU8>,
    error: Arc<tokio::sync::RwLock<Option<String>>>,
}

impl ServerStateTracker {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(ServerState::Created as u8)),
            error: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Current state (single atomic load).
    pub fn state(&self) -> ServerState {
        ServerState::from_u8(self.state.load(Ordering::Relaxed))
    }

    /// Returns `true` if running.
    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::Relaxed) == ServerState::Running as u8
    }

    /// Sets the starting.
    pub fn set_starting(&self) {
        self.state
            .store(ServerState::Starting as u8, Ordering::Relaxed);
    }

    /// Sets the running.
    pub fn set_running(&self) {
        self.state
            .store(ServerState::Running as u8, Ordering::Relaxed);
    }

    /// Sets the draining.
    pub fn set_draining(&self) {
        self.state
            .store(ServerState::Draining as u8, Ordering::Relaxed);
    }

    /// Sets the stopped.
    pub fn set_stopped(&self) {
        self.state
            .store(ServerState::Stopped as u8, Ordering::Relaxed);
    }

    /// Sets the failed.
    pub fn set_failed(&self) {
        self.state
            .store(ServerState::Failed as u8, Ordering::Relaxed);
    }

    /// Set a human-readable error message (cold path).
    pub async fn set_error(&self, msg: String) {
        *self.error.write().await = Some(msg);
    }

    /// Get the error message if any (cold path).
    pub async fn error(&self) -> Option<String> {
        self.error.read().await.clone()
    }
}

impl Default for ServerStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ServerStateTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerStateTracker")
            .field("state", &self.state())
            .finish()
    }
}

/// Classification of server types for diagnostics and ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerType {
    /// HTTP API server (main, console, plugin)
    Http,
    /// gRPC server (SDK, cluster)
    Grpc,
    /// Raft consensus server
    Raft,
    /// Background service (health checker, warmup, etc.)
    Background,
}

impl fmt::Display for ServerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http => write!(f, "HTTP"),
            Self::Grpc => write!(f, "gRPC"),
            Self::Raft => write!(f, "Raft"),
            Self::Background => write!(f, "Background"),
        }
    }
}

/// A server managed by the orchestrator.
///
/// Implementors represent a single server component (e.g., "SDK gRPC Server",
/// "Main HTTP Server", "Raft Server"). The orchestrator calls `start()` and
/// `shutdown()` in priority order.
///
/// # Priority Convention
///
/// Lower values start first, shutdown last:
///
/// | Priority | Typical Server |
/// |----------|----------------|
/// | 10       | Raft server (must be ready before cluster forms) |
/// | 20       | Cluster gRPC server (inter-node communication) |
/// | 30       | SDK gRPC server (client-facing) |
/// | 40       | Main HTTP server (API endpoints) |
/// | 50       | Console HTTP server |
/// | 60       | Plugin HTTP servers (Consul, MCP, etc.) |
/// | 100      | Background tasks (health checker, warmup) |
#[async_trait::async_trait]
pub trait ManagedServer: Send + Sync {
    /// Human-readable name for logging and diagnostics (e.g., "SDK gRPC Server").
    fn name(&self) -> &str;

    /// Server type classification.
    fn server_type(&self) -> ServerType;

    /// Startup priority. Lower values start first, shutdown last.
    fn start_priority(&self) -> u32;

    /// Reference to this server's state tracker.
    fn state(&self) -> &ServerStateTracker;

    /// Start the server. Must transition state: Created → Starting → Running.
    ///
    /// The implementation should:
    /// 1. Call `state().set_starting()`
    /// 2. Bind ports, spawn tasks, etc.
    /// 3. Call `state().set_running()` when ready
    /// 4. Return Ok(()) — the server continues running in the background
    ///
    /// If startup fails, call `state().set_failed()` and return Err.
    async fn start(&self) -> Result<(), ServerError>;

    /// Gracefully shutdown the server. Must transition: Running → Draining → Stopped.
    ///
    /// The implementation should:
    /// 1. Call `state().set_draining()`
    /// 2. Stop accepting new connections
    /// 3. Wait for in-flight requests to complete (with timeout)
    /// 4. Call `state().set_stopped()`
    async fn shutdown(&self) -> Result<(), ServerError>;

    /// Optional health check. Returns true if the server is healthy.
    /// Default implementation checks if state is Running.
    async fn health_check(&self) -> bool {
        self.state().is_running()
    }
}

/// Error type for server lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Server '{name}' failed to start: {reason}")]
    /// Server failed to start.
    StartupFailed {
        /// The name of the server that failed to start.
        name: String,
        /// The reason for the startup failure.
        reason: String,
    },

    #[error("Server '{name}' failed to shutdown: {reason}")]
    /// Server failed to shut down.
    ShutdownFailed {
        /// The name of the server that failed to shut down.
        name: String,
        /// The reason for the shutdown failure.
        reason: String,
    },

    #[error("Server '{name}' bind error on {addr}: {reason}")]
    /// Server failed to bind to its address.
    BindError {
        /// The name of the server that failed to bind.
        name: String,
        /// The address the server attempted to bind to.
        addr: String,
        /// The reason for the bind failure.
        reason: String,
    },

    #[error("{0}")]
    /// The `Other` variant.
    Other(#[from] anyhow::Error),
}

/// Health status of a single server, returned by the orchestrator.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerHealthInfo {
    /// The `name` field.
    pub name: String,
    /// The `server_type` field.
    pub server_type: String,
    /// The `state` field.
    pub state: String,
    /// The `healthy` field.
    pub healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The `error` field.
    pub error: Option<String>,
}

/// Lightweight server state registry for incremental adoption.
///
/// Unlike `ServerOrchestrator`, this does NOT manage startup/shutdown.
/// It only holds `ServerStateTracker` instances for health aggregation
/// and diagnostics. Use this while migrating to `ServerOrchestrator`.
///
/// Thread-safe: can be shared across handlers via `Arc<ServerRegistry>`.
#[derive(Default)]
pub struct ServerRegistry {
    entries: std::sync::RwLock<Vec<(String, ServerType, ServerStateTracker)>>,
}

impl ServerRegistry {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a server's state tracker for monitoring.
    pub fn register(&self, name: &str, server_type: ServerType, tracker: ServerStateTracker) {
        if let Ok(mut entries) = self.entries.write() {
            entries.push((name.to_string(), server_type, tracker));
        }
    }

    /// Get health info for all registered servers.
    pub fn health(&self) -> Vec<ServerHealthInfo> {
        let entries = match self.entries.read() {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        entries
            .iter()
            .map(|(name, server_type, tracker)| ServerHealthInfo {
                name: name.clone(),
                server_type: server_type.to_string(),
                state: tracker.state().to_string(),
                healthy: tracker.is_running(),
                error: None, // async error not available in sync context
            })
            .collect()
    }

    /// Returns true if all registered servers are Running.
    pub fn all_running(&self) -> bool {
        match self.entries.read() {
            Ok(entries) => entries.iter().all(|(_, _, t)| t.is_running()),
            Err(_) => false,
        }
    }

    /// Get number of registered servers.
    pub fn server_count(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }
}

/// Manages the lifecycle of multiple `ManagedServer` instances.
///
/// Provides:
/// - **Ordered startup**: Servers start in `start_priority()` order (ascending)
/// - **Ordered shutdown**: Reverse priority order (highest priority shuts down first)
/// - **Health aggregation**: Query all servers' states for diagnostics
///
/// # Usage
///
/// ```ignore
/// let mut orchestrator = ServerOrchestrator::new();
/// orchestrator.register(Arc::new(RaftServer::new(..)));
/// orchestrator.register(Arc::new(SdkGrpcServer::new(..)));
/// orchestrator.register(Arc::new(MainHttpServer::new(..)));
///
/// orchestrator.start_all().await?;
/// // ... wait for shutdown signal ...
/// orchestrator.shutdown_all().await?;
/// ```
pub struct ServerOrchestrator {
    /// Servers keyed by (priority, name) for deterministic ordering.
    servers: BTreeMap<(u32, String), Arc<dyn ManagedServer>>,
}

impl ServerOrchestrator {
    /// Creates a new instance.
    pub fn new() -> Self {
        Self {
            servers: BTreeMap::new(),
        }
    }

    /// Register a server. Duplicate names will overwrite.
    pub fn register(&mut self, server: Arc<dyn ManagedServer>) {
        let key = (server.start_priority(), server.name().to_string());
        self.servers.insert(key, server);
    }

    /// Start all servers in priority order (lowest first).
    ///
    /// If any server fails to start, already-started servers are shut down
    /// in reverse order before returning the error.
    pub async fn start_all(&self) -> Result<(), ServerError> {
        let mut started: Vec<&Arc<dyn ManagedServer>> = Vec::new();

        for server in self.servers.values() {
            tracing::info!(
                name = server.name(),
                server_type = %server.server_type(),
                priority = server.start_priority(),
                "Starting server"
            );

            if let Err(e) = server.start().await {
                tracing::error!(
                    name = server.name(),
                    error = %e,
                    "Server failed to start, rolling back"
                );

                // Rollback: shutdown already-started servers in reverse order
                for s in started.iter().rev() {
                    let _ = s.shutdown().await;
                }
                return Err(e);
            }

            tracing::info!(
                name = server.name(),
                state = %server.state().state(),
                "Server started"
            );
            started.push(server);
        }

        Ok(())
    }

    /// Shutdown all servers in reverse priority order (highest first).
    ///
    /// Continues shutting down remaining servers even if one fails,
    /// and returns the first error encountered.
    pub async fn shutdown_all(&self) -> Result<(), ServerError> {
        let mut first_error: Option<ServerError> = None;

        for server in self.servers.values().rev() {
            tracing::info!(
                name = server.name(),
                state = %server.state().state(),
                "Shutting down server"
            );

            if let Err(e) = server.shutdown().await {
                tracing::error!(
                    name = server.name(),
                    error = %e,
                    "Server shutdown error"
                );
                if first_error.is_none() {
                    first_error = Some(e);
                }
            } else {
                tracing::info!(name = server.name(), "Server stopped");
            }
        }

        match first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Get health info for all registered servers.
    pub async fn health(&self) -> Vec<ServerHealthInfo> {
        let mut infos = Vec::with_capacity(self.servers.len());
        for server in self.servers.values() {
            let tracker = server.state();
            let healthy = server.health_check().await;
            let error = tracker.error().await;
            infos.push(ServerHealthInfo {
                name: server.name().to_string(),
                server_type: server.server_type().to_string(),
                state: tracker.state().to_string(),
                healthy,
                error,
            });
        }
        infos
    }

    /// Returns true if all registered servers are in Running state.
    pub fn all_running(&self) -> bool {
        self.servers.values().all(|s| s.state().is_running())
    }

    /// Get a snapshot of all server states (name -> state string).
    pub fn state_snapshot(&self) -> Vec<(String, ServerState)> {
        self.servers
            .values()
            .map(|s| (s.name().to_string(), s.state().state()))
            .collect()
    }

    /// Get the number of registered servers.
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }
}

impl Default for ServerOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Test server for unit testing
    struct TestServer {
        name: String,
        server_type: ServerType,
        priority: u32,
        state: ServerStateTracker,
        fail_start: bool,
    }

    impl TestServer {
        fn new(name: &str, server_type: ServerType, priority: u32) -> Self {
            Self {
                name: name.to_string(),
                server_type,
                priority,
                state: ServerStateTracker::new(),
                fail_start: false,
            }
        }

        fn with_fail_start(mut self) -> Self {
            self.fail_start = true;
            self
        }
    }

    #[async_trait::async_trait]
    impl ManagedServer for TestServer {
        fn name(&self) -> &str {
            &self.name
        }

        fn server_type(&self) -> ServerType {
            self.server_type
        }

        fn start_priority(&self) -> u32 {
            self.priority
        }

        fn state(&self) -> &ServerStateTracker {
            &self.state
        }

        async fn start(&self) -> Result<(), ServerError> {
            self.state.set_starting();
            if self.fail_start {
                self.state.set_failed();
                self.state
                    .set_error("intentional test failure".to_string())
                    .await;
                return Err(ServerError::StartupFailed {
                    name: self.name.clone(),
                    reason: "intentional test failure".to_string(),
                });
            }
            self.state.set_running();
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), ServerError> {
            self.state.set_draining();
            self.state.set_stopped();
            Ok(())
        }
    }

    struct OrderServer {
        name: String,
        server_type: ServerType,
        priority: u32,
        state: ServerStateTracker,
        starts: Arc<Mutex<Vec<String>>>,
        shutdowns: Arc<Mutex<Vec<String>>>,
    }

    impl OrderServer {
        fn new(
            name: &str,
            server_type: ServerType,
            priority: u32,
            starts: Arc<Mutex<Vec<String>>>,
            shutdowns: Arc<Mutex<Vec<String>>>,
        ) -> Self {
            Self {
                name: name.to_string(),
                server_type,
                priority,
                state: ServerStateTracker::new(),
                starts,
                shutdowns,
            }
        }
    }

    #[async_trait::async_trait]
    impl ManagedServer for OrderServer {
        fn name(&self) -> &str {
            &self.name
        }

        fn server_type(&self) -> ServerType {
            self.server_type
        }

        fn start_priority(&self) -> u32 {
            self.priority
        }

        fn state(&self) -> &ServerStateTracker {
            &self.state
        }

        async fn start(&self) -> Result<(), ServerError> {
            self.starts
                .lock()
                .expect("start-order mutex poisoned")
                .push(self.name.clone());
            self.state.set_running();
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), ServerError> {
            self.shutdowns
                .lock()
                .expect("shutdown-order mutex poisoned")
                .push(self.name.clone());
            self.state.set_stopped();
            Ok(())
        }
    }

    #[test]
    fn test_server_state_lifecycle() {
        let tracker = ServerStateTracker::new();
        assert_eq!(tracker.state(), ServerState::Created);
        assert!(!tracker.is_running());

        tracker.set_starting();
        assert_eq!(tracker.state(), ServerState::Starting);

        tracker.set_running();
        assert!(tracker.is_running());
        assert_eq!(tracker.state(), ServerState::Running);

        tracker.set_draining();
        assert!(!tracker.is_running());
        assert_eq!(tracker.state(), ServerState::Draining);

        tracker.set_stopped();
        assert_eq!(tracker.state(), ServerState::Stopped);
    }

    #[test]
    fn test_server_state_display() {
        assert_eq!(ServerState::Created.to_string(), "CREATED");
        assert_eq!(ServerState::Starting.to_string(), "STARTING");
        assert_eq!(ServerState::Running.to_string(), "RUNNING");
        assert_eq!(ServerState::Draining.to_string(), "DRAINING");
        assert_eq!(ServerState::Stopped.to_string(), "STOPPED");
        assert_eq!(ServerState::Failed.to_string(), "FAILED");
    }

    #[test]
    fn test_server_state_tracker_clone_shares_state() {
        let tracker = ServerStateTracker::new();
        let clone = tracker.clone();
        tracker.set_running();
        assert!(clone.is_running());
    }

    #[tokio::test]
    async fn test_server_state_tracker_error() {
        let tracker = ServerStateTracker::new();
        assert!(tracker.error().await.is_none());

        tracker.set_error("bind failed".to_string()).await;
        assert_eq!(tracker.error().await, Some("bind failed".to_string()));
    }

    #[tokio::test]
    async fn test_orchestrator_start_all_in_priority_order() {
        let mut orchestrator = ServerOrchestrator::new();

        let raft = Arc::new(TestServer::new("Raft", ServerType::Raft, 10));
        let grpc = Arc::new(TestServer::new("SDK gRPC", ServerType::Grpc, 30));
        let http = Arc::new(TestServer::new("Main HTTP", ServerType::Http, 40));

        // Register in arbitrary order
        orchestrator.register(http.clone());
        orchestrator.register(raft.clone());
        orchestrator.register(grpc.clone());

        orchestrator.start_all().await.unwrap();

        // All should be running
        assert!(raft.state().is_running());
        assert!(grpc.state().is_running());
        assert!(http.state().is_running());
        assert!(orchestrator.all_running());
    }

    #[tokio::test]
    async fn test_orchestrator_shutdown_all() {
        let mut orchestrator = ServerOrchestrator::new();

        let s1 = Arc::new(TestServer::new("S1", ServerType::Http, 10));
        let s2 = Arc::new(TestServer::new("S2", ServerType::Grpc, 20));

        orchestrator.register(s1.clone());
        orchestrator.register(s2.clone());

        orchestrator.start_all().await.unwrap();
        assert!(orchestrator.all_running());

        orchestrator.shutdown_all().await.unwrap();

        assert_eq!(s1.state().state(), ServerState::Stopped);
        assert_eq!(s2.state().state(), ServerState::Stopped);
        assert!(!orchestrator.all_running());
    }

    #[tokio::test]
    async fn test_orchestrator_rollback_on_failure() {
        let mut orchestrator = ServerOrchestrator::new();

        let s1 = Arc::new(TestServer::new("S1-OK", ServerType::Raft, 10));
        let s2 = Arc::new(TestServer::new("S2-FAIL", ServerType::Grpc, 20).with_fail_start());

        orchestrator.register(s1.clone());
        orchestrator.register(s2.clone());

        let result = orchestrator.start_all().await;
        assert!(result.is_err());

        // S1 should have been rolled back (shutdown after S2 failed)
        assert_eq!(s1.state().state(), ServerState::Stopped);
        // S2 should be in Failed state
        assert_eq!(s2.state().state(), ServerState::Failed);
    }

    #[tokio::test]
    async fn test_orchestrator_health() {
        let mut orchestrator = ServerOrchestrator::new();

        let s1 = Arc::new(TestServer::new("Healthy", ServerType::Http, 10));
        let s2 = Arc::new(TestServer::new("NotStarted", ServerType::Grpc, 20));

        orchestrator.register(s1.clone());
        orchestrator.register(s2.clone());

        // Only start s1
        s1.start().await.unwrap();

        let health = orchestrator.health().await;
        assert_eq!(health.len(), 2);

        let h1 = health.iter().find(|h| h.name == "Healthy").unwrap();
        assert!(h1.healthy);
        assert_eq!(h1.state, "RUNNING");

        let h2 = health.iter().find(|h| h.name == "NotStarted").unwrap();
        assert!(!h2.healthy);
        assert_eq!(h2.state, "CREATED");
    }

    #[tokio::test]
    async fn test_orchestrator_state_snapshot() {
        let mut orchestrator = ServerOrchestrator::new();

        let s1 = Arc::new(TestServer::new("A", ServerType::Http, 10));
        let s2 = Arc::new(TestServer::new("B", ServerType::Grpc, 20));

        orchestrator.register(s1.clone());
        orchestrator.register(s2.clone());

        s1.state().set_running();
        s2.state().set_starting();

        let snapshot = orchestrator.state_snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0], ("A".to_string(), ServerState::Running));
        assert_eq!(snapshot[1], ("B".to_string(), ServerState::Starting));
    }

    #[test]
    fn test_server_type_display() {
        assert_eq!(ServerType::Http.to_string(), "HTTP");
        assert_eq!(ServerType::Grpc.to_string(), "gRPC");
        assert_eq!(ServerType::Raft.to_string(), "Raft");
        assert_eq!(ServerType::Background.to_string(), "Background");
    }

    #[test]
    fn test_server_error_display() {
        let err = ServerError::StartupFailed {
            name: "SDK gRPC".to_string(),
            reason: "port in use".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Server 'SDK gRPC' failed to start: port in use"
        );

        let err = ServerError::BindError {
            name: "Main HTTP".to_string(),
            addr: "0.0.0.0:8848".to_string(),
            reason: "address already in use".to_string(),
        };
        assert!(err.to_string().contains("0.0.0.0:8848"));
    }

    #[test]
    fn server_registry_starts_empty() {
        let registry = ServerRegistry::new();
        assert_eq!(registry.server_count(), 0);
        assert!(registry.health().is_empty());
        assert!(registry.all_running());
    }

    #[test]
    fn server_registry_reports_registered_tracker_states() {
        let registry = ServerRegistry::new();
        let main = ServerStateTracker::new();
        main.set_running();
        let raft = ServerStateTracker::new();
        raft.set_stopped();

        registry.register("main", ServerType::Http, main);
        registry.register("raft", ServerType::Raft, raft);

        assert_eq!(registry.server_count(), 2);
        let health = registry.health();
        assert_eq!(health[0].name, "main");
        assert_eq!(health[0].server_type, "HTTP");
        assert_eq!(health[0].state, "RUNNING");
        assert!(health[0].healthy);
        assert_eq!(health[1].name, "raft");
        assert_eq!(health[1].server_type, "Raft");
        assert_eq!(health[1].state, "STOPPED");
        assert!(!health[1].healthy);
        assert!(!registry.all_running());
    }

    #[tokio::test]
    async fn orchestrator_starts_servers_in_declared_priority_order() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let shutdowns = Arc::new(Mutex::new(Vec::new()));
        let mut orchestrator = ServerOrchestrator::new();

        orchestrator.register(Arc::new(OrderServer::new(
            "C",
            ServerType::Http,
            30,
            starts.clone(),
            shutdowns.clone(),
        )));
        orchestrator.register(Arc::new(OrderServer::new(
            "A",
            ServerType::Raft,
            10,
            starts.clone(),
            shutdowns.clone(),
        )));
        orchestrator.register(Arc::new(OrderServer::new(
            "B",
            ServerType::Grpc,
            20,
            starts.clone(),
            shutdowns,
        )));

        orchestrator
            .start_all()
            .await
            .expect("startup must succeed");
        assert_eq!(
            *starts.lock().expect("start-order mutex poisoned"),
            vec!["A", "B", "C"]
        );
    }

    #[tokio::test]
    async fn orchestrator_shuts_down_servers_in_reverse_priority_order() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let shutdowns = Arc::new(Mutex::new(Vec::new()));
        let mut orchestrator = ServerOrchestrator::new();

        for (name, server_type, priority) in [
            ("C", ServerType::Http, 30),
            ("A", ServerType::Raft, 10),
            ("B", ServerType::Grpc, 20),
        ] {
            orchestrator.register(Arc::new(OrderServer::new(
                name,
                server_type,
                priority,
                starts.clone(),
                shutdowns.clone(),
            )));
        }

        orchestrator
            .start_all()
            .await
            .expect("startup must succeed");
        orchestrator
            .shutdown_all()
            .await
            .expect("shutdown must succeed");
        assert_eq!(
            *shutdowns.lock().expect("shutdown-order mutex poisoned"),
            vec!["C", "B", "A"]
        );
    }

    #[test]
    fn server_registry_preserves_registration_order() {
        let registry = ServerRegistry::new();
        for name in ["first", "second", "third"] {
            registry.register(name, ServerType::Http, ServerStateTracker::new());
        }
        let names = registry
            .health()
            .into_iter()
            .map(|info| info.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["first", "second", "third"]);
    }

    #[test]
    fn server_registry_health_tracks_live_state_transitions() {
        let registry = ServerRegistry::new();
        let tracker = ServerStateTracker::new();
        registry.register("main", ServerType::Http, tracker.clone());

        assert_eq!(registry.health()[0].state, "CREATED");
        tracker.set_running();
        assert_eq!(registry.health()[0].state, "RUNNING");
        assert!(registry.health()[0].healthy);
        tracker.set_draining();
        assert_eq!(registry.health()[0].state, "DRAINING");
        assert!(!registry.health()[0].healthy);
    }

    #[test]
    fn server_registry_all_running_requires_every_tracker() {
        let registry = ServerRegistry::new();
        let first = ServerStateTracker::new();
        let second = ServerStateTracker::new();
        registry.register("first", ServerType::Http, first.clone());
        registry.register("second", ServerType::Grpc, second.clone());

        assert!(!registry.all_running());
        first.set_running();
        assert!(!registry.all_running());
        second.set_running();
        assert!(registry.all_running());
    }

    #[test]
    fn server_registry_health_reports_background_server_type() {
        let registry = ServerRegistry::new();
        registry.register(
            "health-checker",
            ServerType::Background,
            ServerStateTracker::new(),
        );
        assert_eq!(registry.health()[0].server_type, "Background");
    }

    #[test]
    fn server_registry_allows_duplicate_server_names() {
        let registry = ServerRegistry::new();
        registry.register("probe", ServerType::Http, ServerStateTracker::new());
        registry.register("probe", ServerType::Grpc, ServerStateTracker::new());
        assert_eq!(registry.server_count(), 2);
        assert_eq!(registry.health().len(), 2);
    }

    #[test]
    fn server_registry_server_count_matches_health_len() {
        let registry = ServerRegistry::new();
        for name in ["a", "b", "c"] {
            registry.register(name, ServerType::Raft, ServerStateTracker::new());
        }
        assert_eq!(registry.server_count(), registry.health().len());
    }

    #[tokio::test]
    async fn server_registry_sync_health_view_has_no_async_error() {
        let registry = ServerRegistry::new();
        let tracker = ServerStateTracker::new();
        tracker.set_error("async-only failure".to_string()).await;
        registry.register("failed", ServerType::Grpc, tracker);
        assert_eq!(registry.health()[0].error, None);
    }

    #[test]
    fn server_state_failed_is_terminal_and_not_running() {
        assert_eq!(ServerState::Failed.to_string(), "FAILED");
        assert!(!ServerState::Failed.is_running());

        let tracker = ServerStateTracker::new();
        tracker.set_failed();
        assert_eq!(tracker.state(), ServerState::Failed);
        assert!(!tracker.is_running());
    }

    #[test]
    fn server_state_tracker_unknown_atomic_value_defaults_created() {
        let tracker = ServerStateTracker::new();
        tracker.state.store(200, Ordering::Relaxed);
        assert_eq!(tracker.state(), ServerState::Created);
        assert!(!tracker.is_running());
    }

    #[test]
    fn server_registry_health_reflects_failed_state() {
        let registry = ServerRegistry::new();
        let tracker = ServerStateTracker::new();
        tracker.set_failed();
        registry.register("broken", ServerType::Raft, tracker);

        let health = registry.health();
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].state, "FAILED");
        assert!(!health[0].healthy);
        assert!(!registry.all_running());
    }

    #[test]
    fn server_type_hash_set_has_four_distinct_entries() {
        let all = [
            ServerType::Http,
            ServerType::Grpc,
            ServerType::Raft,
            ServerType::Background,
        ];
        let types = std::collections::HashSet::from(all);
        assert_eq!(types.len(), 4);
        assert!(types.contains(&ServerType::Grpc));
        assert!(types.contains(&ServerType::Background));

        let labels = all.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(labels, vec!["HTTP", "gRPC", "Raft", "Background"]);
    }

    #[test]
    fn server_error_shutdown_failed_display() {
        let error = ServerError::ShutdownFailed {
            name: "Raft".to_string(),
            reason: "join timed out".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "Server 'Raft' failed to shutdown: join timed out"
        );
    }

    #[tokio::test]
    async fn managed_server_health_check_default_follows_state() {
        let server = TestServer::new("probe", ServerType::Http, 10);
        assert!(!server.health_check().await);

        server.start().await.expect("start must succeed");
        assert!(server.health_check().await);

        server.shutdown().await.expect("shutdown must succeed");
        assert!(!server.health_check().await);
    }

    #[test]
    fn server_orchestrator_empty_has_zero_count_and_snapshot() {
        let orchestrator = ServerOrchestrator::new();
        assert_eq!(orchestrator.server_count(), 0);
        assert!(orchestrator.state_snapshot().is_empty());
        assert!(orchestrator.all_running());
    }

    #[test]
    fn server_orchestrator_server_count_matches_snapshot_len() {
        let mut orchestrator = ServerOrchestrator::new();
        for name in ["a", "b", "c"] {
            orchestrator.register(Arc::new(TestServer::new(name, ServerType::Grpc, 10)));
        }
        assert_eq!(orchestrator.server_count(), 3);
        assert_eq!(orchestrator.state_snapshot().len(), 3);
    }

    #[tokio::test]
    async fn server_orchestrator_health_includes_tracker_error() {
        let mut orchestrator = ServerOrchestrator::new();
        let server = Arc::new(TestServer::new("s1", ServerType::Grpc, 10));
        server.state().set_error("bind failed".to_string()).await;
        orchestrator.register(server);

        let health = orchestrator.health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].error, Some("bind failed".to_string()));
        assert!(!health[0].healthy);
    }
}
