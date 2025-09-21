use std::sync::Arc;

use dashmap::DashMap;
use tonic::{Request, Status};

use crate::core::model::{Connection, GrpcClient};

pub fn context_interceptor<T>(mut request: Request<T>) -> Result<Request<T>, Status> {
    let mut connection = Connection::default();

    let remote_addr = request.remote_addr().unwrap();
    let remote_ip = remote_addr.ip().to_string();
    let remote_port = remote_addr.port();

    connection.meta_info.remote_ip = remote_ip.clone();
    connection.meta_info.remote_port = remote_port;
    connection.meta_info.connection_id = format!(
        "{}_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        remote_ip,
        remote_port
    );

    let local_addr = request.local_addr().unwrap();
    let local_port = local_addr.port();

    connection.meta_info.local_port = local_port;

    request.extensions_mut().insert(connection);

    Ok(request)
}

pub struct ConnectionManager {
    clients: Arc<DashMap<String, GrpcClient>>,
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
        let count = self.clients.len();

        self.clients.remove(connection_id);
    }
}
