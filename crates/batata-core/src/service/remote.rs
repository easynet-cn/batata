use std::sync::Arc;

use dashmap::DashMap;
use tonic::{Request, Status};

use crate::model::{Connection, GrpcClient};

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

pub struct ConnectionManager {
    clients: Arc<DashMap<String, GrpcClient>>,
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
        }
    }

    pub fn from_arc(clients: Arc<DashMap<String, GrpcClient>>) -> Self {
        Self { clients }
    }

    pub fn register(&self, connection_id: &str, client: GrpcClient) -> bool {
        if self.clients.contains_key(connection_id) {
            return true;
        }

        self.clients.insert(connection_id.to_string(), client);

        true
    }

    pub fn unregister(&self, connection_id: &str) {
        self.clients.remove(connection_id);
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
                    self.unregister(connection_id);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_client() -> GrpcClient {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        GrpcClient::new(Connection::default(), tx)
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

    #[test]
    fn test_connection_manager_register_unregister() {
        let mgr = ConnectionManager::new();
        let client = make_test_client();

        mgr.register("conn-1", client);
        assert_eq!(mgr.connection_count(), 1);
        assert!(mgr.has_connection("conn-1"));

        mgr.unregister("conn-1");
        assert_eq!(mgr.connection_count(), 0);
        assert!(!mgr.has_connection("conn-1"));
    }

    #[test]
    fn test_connection_manager_duplicate_register() {
        let mgr = ConnectionManager::new();
        let client1 = make_test_client();
        let client2 = make_test_client();

        let result1 = mgr.register("conn-1", client1);
        assert!(result1);

        // Second register with same ID returns true (already exists)
        let result2 = mgr.register("conn-1", client2);
        assert!(result2);

        // Only one connection exists
        assert_eq!(mgr.connection_count(), 1);
    }

    #[test]
    fn test_connection_manager_unregister_nonexistent() {
        let mgr = ConnectionManager::new();
        // Should not panic
        mgr.unregister("nonexistent");
        assert_eq!(mgr.connection_count(), 0);
    }

    #[test]
    fn test_connection_manager_get_all_connection_ids() {
        let mgr = ConnectionManager::new();
        mgr.register("conn-a", make_test_client());
        mgr.register("conn-b", make_test_client());
        mgr.register("conn-c", make_test_client());

        let mut ids = mgr.get_all_connection_ids();
        ids.sort();
        assert_eq!(ids, vec!["conn-a", "conn-b", "conn-c"]);
    }

    #[test]
    fn test_connection_manager_has_connection() {
        let mgr = ConnectionManager::new();
        assert!(!mgr.has_connection("conn-1"));

        mgr.register("conn-1", make_test_client());
        assert!(mgr.has_connection("conn-1"));

        mgr.unregister("conn-1");
        assert!(!mgr.has_connection("conn-1"));
    }

    #[test]
    fn test_connection_manager_get_client() {
        let mgr = ConnectionManager::new();
        assert!(mgr.get_client("conn-1").is_none());

        mgr.register("conn-1", make_test_client());
        assert!(mgr.get_client("conn-1").is_some());
    }
}
