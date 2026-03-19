use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use tonic::{Request, Status};

use crate::model::{Connection, ConnectionMeta, GrpcClient};

/// Listener for connection lifecycle events.
///
/// Implementors are notified when connections are established or closed,
/// enabling custom logic such as logging, metrics collection, or resource cleanup.
#[async_trait::async_trait]
pub trait ConnectionEventListener: Send + Sync {
    /// Called when a new connection is established.
    async fn on_connected(&self, connection_id: &str, meta: &ConnectionMeta);
    /// Called when a connection is closed.
    async fn on_disconnected(&self, connection_id: &str, meta: &ConnectionMeta);
}

pub fn context_interceptor<T>(mut request: Request<T>) -> Result<Request<T>, Status> {
    let mut connection = Connection::default();

    let (remote_ip, remote_port) = match request.remote_addr() {
        Some(addr) => (addr.ip().to_string(), addr.port()),
        None => ("unknown".to_string(), 0),
    };

    connection.meta_info.remote_ip = remote_ip.clone();
    connection.meta_info.remote_port = remote_port;
    // Use deterministic connection_id based on remote address only.
    // In gRPC over HTTP/2, both unary RPCs and bidirectional streams share
    // the same TCP connection (same remote IP:port), so this ensures
    // consistent connection_id across all request types from the same client.
    connection.meta_info.connection_id = format!("{}_{}", remote_ip, remote_port);

    let local_port = request.local_addr().map(|a| a.port()).unwrap_or(0);

    connection.meta_info.local_port = local_port;

    request.extensions_mut().insert(connection);

    Ok(request)
}

/// Maximum idle time before a connection is considered stale (milliseconds).
const STALE_CONNECTION_THRESHOLD_MS: u64 = 60_000;

/// Connection limit checker trait for pluggable connection control.
#[async_trait::async_trait]
pub trait ConnectionLimitChecker: Send + Sync {
    /// Check if a new connection should be allowed.
    /// Returns true if allowed, false if the connection should be rejected.
    async fn check_connection(&self, client_ip: &str, client_id: &str) -> bool;

    /// Release a connection slot when a client disconnects.
    async fn release_connection(&self, client_ip: &str, client_id: &str);
}

pub struct ConnectionManager {
    clients: Arc<DashMap<String, GrpcClient>>,
    listeners: Arc<RwLock<Vec<Arc<dyn ConnectionEventListener>>>>,
    connection_limit_checker: std::sync::Mutex<Option<Arc<dyn ConnectionLimitChecker>>>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            listeners: Arc::new(RwLock::new(Vec::new())),
            connection_limit_checker: std::sync::Mutex::new(None),
        }
    }

    pub fn from_arc(clients: Arc<DashMap<String, GrpcClient>>) -> Self {
        Self {
            clients,
            listeners: Arc::new(RwLock::new(Vec::new())),
            connection_limit_checker: std::sync::Mutex::new(None),
        }
    }

    /// Set the connection limit checker for controlling max connections.
    pub fn set_connection_limit_checker(&self, checker: Arc<dyn ConnectionLimitChecker>) {
        if let Ok(mut guard) = self.connection_limit_checker.lock() {
            *guard = Some(checker);
        }
    }

    /// Get the connection limit checker (clone of Arc, lock-free after extraction).
    fn get_limit_checker(&self) -> Option<Arc<dyn ConnectionLimitChecker>> {
        self.connection_limit_checker
            .lock()
            .ok()
            .and_then(|g| g.clone())
    }

    /// Register a connection event listener.
    pub fn add_listener(&self, listener: Arc<dyn ConnectionEventListener>) {
        if let Ok(mut listeners) = self.listeners.write() {
            listeners.push(listener);
        }
    }

    /// Notify all listeners of a new connection.
    async fn fire_connected(&self, connection_id: &str, meta: &ConnectionMeta) {
        let listeners = self.listeners.read().map(|l| l.clone()).unwrap_or_default();
        for listener in &listeners {
            listener.on_connected(connection_id, meta).await;
        }
    }

    /// Notify all listeners of a disconnection.
    async fn fire_disconnected(&self, connection_id: &str, meta: &ConnectionMeta) {
        let listeners = self.listeners.read().map(|l| l.clone()).unwrap_or_default();
        for listener in &listeners {
            listener.on_disconnected(connection_id, meta).await;
        }
    }

    pub async fn register(&self, connection_id: &str, client: GrpcClient) -> bool {
        if self.clients.contains_key(connection_id) {
            tracing::debug!(connection_id, "Connection already registered, skipping");
            return true;
        }

        // Check connection limit before registering
        let meta = client.connection.meta_info.clone();
        if let Some(checker) = self.get_limit_checker()
            && !checker
                .check_connection(&meta.client_ip, connection_id)
                .await
        {
            tracing::warn!(
                connection_id,
                client_ip = %meta.client_ip,
                "Connection rejected: server is over connection limit"
            );
            return false;
        }

        tracing::info!(
            connection_id,
            client_ip = %meta.client_ip,
            app_name = %meta.app_name,
            "Registered new gRPC client connection"
        );
        self.clients.insert(connection_id.to_string(), client);
        self.fire_connected(connection_id, &meta).await;

        true
    }

    pub async fn unregister(&self, connection_id: &str) {
        if let Some((_, client)) = self.clients.remove(connection_id) {
            // Release connection slot in the limiter
            if let Some(checker) = self.get_limit_checker() {
                checker
                    .release_connection(&client.connection.meta_info.client_ip, connection_id)
                    .await;
            }
            self.fire_disconnected(connection_id, &client.connection.meta_info)
                .await;
        }
    }

    /// Get the current number of connected clients
    pub fn connection_count(&self) -> usize {
        self.clients.len()
    }

    /// Push a message to a specific connection
    ///
    /// Returns true if the message was sent successfully, false if the connection doesn't exist
    /// or the send failed
    pub async fn push_message(
        &self,
        connection_id: &str,
        payload: batata_api::grpc::Payload,
    ) -> bool {
        if let Some(client) = self.clients.get(connection_id) {
            match client.tx.send(Ok(payload)).await {
                Ok(_) => {
                    tracing::debug!(connection_id, "Message pushed successfully");
                    true
                }
                Err(e) => {
                    tracing::warn!(connection_id, error = %e, "Failed to push message");
                    false
                }
            }
        } else {
            tracing::debug!(connection_id, "Connection not found for push");
            false
        }
    }

    /// Push a message to multiple connections
    ///
    /// Returns the number of successful sends
    pub async fn push_message_to_many(
        &self,
        connection_ids: &[String],
        payload: batata_api::grpc::Payload,
    ) -> usize {
        if connection_ids.is_empty() {
            return 0;
        }
        let mut success_count = 0;
        let (last, rest) = connection_ids.split_last().unwrap();
        for connection_id in rest {
            if self.push_message(connection_id, payload.clone()).await {
                success_count += 1;
            }
        }
        if self.push_message(last, payload).await {
            success_count += 1;
        }
        success_count
    }

    /// Get a client by connection ID
    pub fn get_client(&self, connection_id: &str) -> Option<GrpcClient> {
        self.clients.get(connection_id).map(|r| (*r).clone())
    }

    /// Check if a connection exists
    pub fn has_connection(&self, connection_id: &str) -> bool {
        self.clients.contains_key(connection_id)
    }

    /// Get all connection IDs
    pub fn get_all_connection_ids(&self) -> Vec<String> {
        self.clients
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all clients as a list
    pub fn get_all_clients(&self) -> Vec<GrpcClient> {
        self.clients
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Send a ConnectResetRequest to a specific client, instructing it to reconnect
    /// (optionally to a different server address).
    ///
    /// Returns true if the reset request was sent successfully.
    pub async fn load_single(&self, connection_id: &str, redirect_address: Option<&str>) -> bool {
        use batata_api::remote::model::{ConnectResetRequest, RequestTrait};

        if let Some(client) = self.clients.get(connection_id) {
            let mut reset_request = ConnectResetRequest::new();
            if let Some(addr) = redirect_address
                && let Some((ip, port)) = addr.split_once(':')
            {
                reset_request.server_ip = ip.to_string();
                reset_request.server_port = port.to_string();
            }

            let payload = reset_request.build_server_push_payload();
            match client.tx.send(Ok(payload)).await {
                Ok(_) => {
                    tracing::info!(
                        connection_id,
                        redirect = ?redirect_address,
                        "ConnectResetRequest sent to client"
                    );
                    true
                }
                Err(e) => {
                    tracing::warn!(
                        connection_id,
                        error = %e,
                        "Failed to send ConnectResetRequest"
                    );
                    // Channel closed, remove the stale connection
                    drop(client);
                    self.unregister(connection_id).await;
                    false
                }
            }
        } else {
            tracing::debug!(connection_id, "Connection not found for load_single");
            false
        }
    }

    /// Eject excess connections by sending ConnectResetRequest to clients until
    /// the connection count drops to `target_count`.
    ///
    /// Returns the number of clients that were sent reset requests.
    pub async fn load_count(&self, target_count: usize, redirect_address: Option<&str>) -> usize {
        let current = self.connection_count();
        if current <= target_count {
            tracing::info!(
                current,
                target_count,
                "Connection count already within limit, no ejection needed"
            );
            return 0;
        }

        let ejecting_count = current - target_count;
        let ids = self.get_all_connection_ids();
        let mut ejected = 0;

        for id in ids {
            if ejected >= ejecting_count {
                break;
            }
            if self.load_single(&id, redirect_address).await {
                ejected += 1;
            }
        }

        tracing::info!(
            ejected,
            target_count,
            current,
            "Connection ejection completed"
        );
        ejected
    }

    /// Count the number of active connections from a given IP address.
    pub fn connections_for_ip(&self, ip: &str) -> usize {
        self.clients
            .iter()
            .filter(|entry| entry.value().connection.meta_info.client_ip == ip)
            .count()
    }

    /// Update the last active timestamp for a specific connection.
    pub fn touch_connection(&self, connection_id: &str) {
        if let Some(client) = self.clients.get(connection_id) {
            client.connection.last_active.touch();
        }
    }

    /// Start a background task that periodically checks for stale connections
    /// and ejects them using the default stale threshold.
    pub fn start_default_health_checker(self: &Arc<Self>) {
        self.start_health_checker(STALE_CONNECTION_THRESHOLD_MS);
    }

    /// Start a background task that periodically checks for stale connections
    /// and ejects them. A connection is considered stale if it has been idle
    /// for longer than the specified threshold.
    pub fn start_health_checker(self: &Arc<Self>, stale_threshold_ms: u64) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(20));
            loop {
                interval.tick().await;
                manager.check_connections(stale_threshold_ms).await;
            }
        });
    }

    /// Check all connections for staleness and eject any that have been
    /// idle longer than the threshold.
    async fn check_connections(&self, stale_threshold_ms: u64) {
        let mut stale_ids = Vec::new();

        for entry in self.clients.iter() {
            let connection_id = entry.key().clone();
            let client = entry.value();
            if client.connection.idle_millis() > stale_threshold_ms {
                stale_ids.push(connection_id);
            }
        }

        for id in stale_ids {
            tracing::warn!("Ejecting stale connection: {}", id);
            self.unregister(&id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AtomicLastActive;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    fn make_test_client() -> GrpcClient {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        GrpcClient::new(Connection::default(), tx)
    }

    fn make_test_client_with_ip(ip: &str) -> GrpcClient {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let mut conn = Connection::default();
        conn.meta_info.client_ip = ip.to_string();
        conn.last_active = AtomicLastActive::now();
        GrpcClient::new(conn, tx)
    }

    #[test]
    fn test_connection_manager_new() {
        let mgr = ConnectionManager::new();
        assert_eq!(mgr.connection_count(), 0);
        assert!(mgr.get_all_connection_ids().is_empty());
    }

    #[test]
    fn test_connection_manager_default() {
        let mgr = ConnectionManager::default();
        assert_eq!(mgr.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_connection_manager_register_unregister() {
        let mgr = ConnectionManager::new();
        let client = make_test_client();

        mgr.register("conn-1", client).await;
        assert_eq!(mgr.connection_count(), 1);
        assert!(mgr.has_connection("conn-1"));

        mgr.unregister("conn-1").await;
        assert_eq!(mgr.connection_count(), 0);
        assert!(!mgr.has_connection("conn-1"));
    }

    #[tokio::test]
    async fn test_connection_manager_duplicate_register() {
        let mgr = ConnectionManager::new();
        let client1 = make_test_client();
        let client2 = make_test_client();

        let result1 = mgr.register("conn-1", client1).await;
        assert!(result1);

        // Second register with same ID returns true (already exists)
        let result2 = mgr.register("conn-1", client2).await;
        assert!(result2);

        // Only one connection exists
        assert_eq!(mgr.connection_count(), 1);
    }

    #[tokio::test]
    async fn test_connection_manager_unregister_nonexistent() {
        let mgr = ConnectionManager::new();
        // Should not panic
        mgr.unregister("nonexistent").await;
        assert_eq!(mgr.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_connection_manager_get_all_connection_ids() {
        let mgr = ConnectionManager::new();
        mgr.register("conn-a", make_test_client()).await;
        mgr.register("conn-b", make_test_client()).await;
        mgr.register("conn-c", make_test_client()).await;

        let mut ids = mgr.get_all_connection_ids();
        ids.sort();
        assert_eq!(ids, vec!["conn-a", "conn-b", "conn-c"]);
    }

    #[tokio::test]
    async fn test_connection_manager_has_connection() {
        let mgr = ConnectionManager::new();
        assert!(!mgr.has_connection("conn-1"));

        mgr.register("conn-1", make_test_client()).await;
        assert!(mgr.has_connection("conn-1"));

        mgr.unregister("conn-1").await;
        assert!(!mgr.has_connection("conn-1"));
    }

    #[tokio::test]
    async fn test_connection_manager_get_client() {
        let mgr = ConnectionManager::new();
        assert!(mgr.get_client("conn-1").is_none());

        mgr.register("conn-1", make_test_client()).await;
        assert!(mgr.get_client("conn-1").is_some());
    }

    // --- New tests for enhanced functionality ---

    #[test]
    fn test_connection_meta_timestamps() {
        let mut meta = ConnectionMeta::default();
        assert_eq!(meta.create_time, 0);
        assert_eq!(meta.last_active_time, 0);

        meta.initialize_timestamps();
        assert!(meta.create_time > 0);
        assert!(meta.last_active_time > 0);
        assert_eq!(meta.create_time, meta.last_active_time);

        let old_active = meta.last_active_time;
        meta.update_last_active();
        assert!(meta.get_last_active() >= old_active);
    }

    #[test]
    fn test_connection_meta_idle_millis() {
        let mut meta = ConnectionMeta::default();
        meta.initialize_timestamps();
        // Just initialized, idle time should be very small
        let idle = meta.idle_millis();
        assert!(
            idle < 1000,
            "idle_millis should be less than 1s, got {idle}"
        );
    }

    #[test]
    fn test_atomic_last_active() {
        let tracker = AtomicLastActive::now();
        assert!(tracker.get() > 0);
        let idle = tracker.idle_millis();
        assert!(idle < 1000);

        tracker.touch();
        assert!(tracker.idle_millis() < 1000);
    }

    #[test]
    fn test_connection_idle_millis() {
        let mut conn = Connection::default();
        conn.last_active = AtomicLastActive::now();
        assert!(conn.idle_millis() < 1000);
    }

    #[tokio::test]
    async fn test_connections_for_ip() {
        let mgr = ConnectionManager::new();
        mgr.register("conn-1", make_test_client_with_ip("10.0.0.1"))
            .await;
        mgr.register("conn-2", make_test_client_with_ip("10.0.0.1"))
            .await;
        mgr.register("conn-3", make_test_client_with_ip("10.0.0.2"))
            .await;

        assert_eq!(mgr.connections_for_ip("10.0.0.1"), 2);
        assert_eq!(mgr.connections_for_ip("10.0.0.2"), 1);
        assert_eq!(mgr.connections_for_ip("10.0.0.3"), 0);
    }

    #[tokio::test]
    async fn test_touch_connection() {
        let mgr = ConnectionManager::new();
        let client = make_test_client_with_ip("10.0.0.1");
        mgr.register("conn-1", client).await;

        mgr.touch_connection("conn-1");

        let client = mgr.get_client("conn-1").unwrap();
        assert!(client.connection.idle_millis() < 1000);
    }

    #[tokio::test]
    async fn test_check_connections_ejects_stale() {
        let mgr = ConnectionManager::new();

        // Create a client with last_active set to 0 (epoch), making it very stale
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let conn = Connection::default(); // last_active defaults to 0
        let client = GrpcClient::new(conn, tx);

        mgr.register("stale-conn", client).await;
        assert_eq!(mgr.connection_count(), 1);

        // Check with a 1ms threshold; the connection with epoch-0 last_active is very stale
        mgr.check_connections(1).await;
        assert_eq!(mgr.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_check_connections_keeps_active() {
        let mgr = ConnectionManager::new();
        let client = make_test_client_with_ip("10.0.0.1");
        mgr.register("active-conn", client).await;
        mgr.touch_connection("active-conn");

        // Check with a large threshold; active connection should remain
        mgr.check_connections(60_000).await;
        assert_eq!(mgr.connection_count(), 1);
    }

    /// Test listener that records events
    struct TestListener {
        connected_count: AtomicUsize,
        disconnected_count: AtomicUsize,
    }

    impl TestListener {
        fn new() -> Self {
            Self {
                connected_count: AtomicUsize::new(0),
                disconnected_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl ConnectionEventListener for TestListener {
        async fn on_connected(&self, _connection_id: &str, _meta: &ConnectionMeta) {
            self.connected_count.fetch_add(1, AtomicOrdering::SeqCst);
        }

        async fn on_disconnected(&self, _connection_id: &str, _meta: &ConnectionMeta) {
            self.disconnected_count.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }

    #[tokio::test]
    async fn test_event_listener_fires_on_register_and_unregister() {
        let mgr = ConnectionManager::new();
        let listener = Arc::new(TestListener::new());
        mgr.add_listener(listener.clone());

        mgr.register("conn-1", make_test_client()).await;
        assert_eq!(listener.connected_count.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(listener.disconnected_count.load(AtomicOrdering::SeqCst), 0);

        mgr.unregister("conn-1").await;
        assert_eq!(listener.connected_count.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(listener.disconnected_count.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_event_listener_not_fired_on_duplicate_register() {
        let mgr = ConnectionManager::new();
        let listener = Arc::new(TestListener::new());
        mgr.add_listener(listener.clone());

        mgr.register("conn-1", make_test_client()).await;
        mgr.register("conn-1", make_test_client()).await; // duplicate

        // Only one connected event should fire
        assert_eq!(listener.connected_count.load(AtomicOrdering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_event_listener_not_fired_on_unregister_nonexistent() {
        let mgr = ConnectionManager::new();
        let listener = Arc::new(TestListener::new());
        mgr.add_listener(listener.clone());

        mgr.unregister("nonexistent").await;
        assert_eq!(listener.disconnected_count.load(AtomicOrdering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_multiple_listeners() {
        let mgr = ConnectionManager::new();
        let listener1 = Arc::new(TestListener::new());
        let listener2 = Arc::new(TestListener::new());
        mgr.add_listener(listener1.clone());
        mgr.add_listener(listener2.clone());

        mgr.register("conn-1", make_test_client()).await;
        assert_eq!(listener1.connected_count.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(listener2.connected_count.load(AtomicOrdering::SeqCst), 1);
    }
}
