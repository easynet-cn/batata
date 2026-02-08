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
        let mut success_count = 0;
        for connection_id in connection_ids {
            if self.push_message(connection_id, payload.clone()).await {
                success_count += 1;
            }
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
}
