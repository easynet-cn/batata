//! Transport abstraction layer
//!
//! Defines the `Transport` trait that decouples service implementations
//! (Config, Naming) from the concrete gRPC transport. This enables:
//! - Unit testing with mock transports
//! - Future HTTP fallback transport
//! - Connection state queries without casting

use std::sync::Arc;

use batata_api::grpc::Payload;
use batata_api::remote::model::{RequestTrait, ResponseTrait};

use crate::error::Result;
use crate::grpc::{ConnectionEventListener, ConnectionState, ServerPushHandler};

/// Transport abstraction for SDK communication with the server.
///
/// Implemented by `GrpcClient` for production use. Can be mocked for testing.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send a raw payload request and receive a response.
    async fn request_raw(&self, payload: Payload) -> Result<Payload>;

    /// Send a typed request with automatic serialization/deserialization.
    ///
    /// Includes auth token injection, timeout, and 403 auto-retry.
    async fn request_typed<Req, Resp>(&self, req: &Req) -> Result<Resp>
    where
        Req: RequestTrait + serde::Serialize + Send + Sync,
        Resp: for<'de> serde::Deserialize<'de> + Default + ResponseTrait + Send;

    /// Send a payload via the bi-directional stream.
    async fn send_bi_stream(&self, payload: Payload) -> Result<()>;

    /// Get current connection state.
    fn connection_state(&self) -> ConnectionState;

    /// Check if the connection is active.
    async fn is_connected(&self) -> bool;

    /// Register a handler for server push messages.
    fn register_push_handler(&self, type_name: &str, handler: impl ServerPushHandler);

    /// Register a connection event listener.
    fn add_connection_listener(&self, listener: Arc<dyn ConnectionEventListener>);
}

/// Implement Transport for GrpcClient
#[async_trait::async_trait]
impl Transport for crate::grpc::GrpcClient {
    async fn request_raw(&self, payload: Payload) -> Result<Payload> {
        let guard = self.connection_ref().read().await;
        let conn = guard.as_ref().ok_or(crate::error::ClientError::NotConnected)?;
        conn.request(payload).await
    }

    async fn request_typed<Req, Resp>(&self, req: &Req) -> Result<Resp>
    where
        Req: RequestTrait + serde::Serialize + Send + Sync,
        Resp: for<'de> serde::Deserialize<'de> + Default + ResponseTrait + Send,
    {
        crate::grpc::GrpcClient::request_typed(self, req).await
    }

    async fn send_bi_stream(&self, payload: Payload) -> Result<()> {
        crate::grpc::GrpcClient::send_bi_stream(self, payload).await
    }

    fn connection_state(&self) -> ConnectionState {
        crate::grpc::GrpcClient::connection_state(self)
    }

    async fn is_connected(&self) -> bool {
        crate::grpc::GrpcClient::is_connected(self).await
    }

    fn register_push_handler(&self, type_name: &str, handler: impl ServerPushHandler) {
        crate::grpc::GrpcClient::register_push_handler(self, type_name, handler)
    }

    fn add_connection_listener(&self, listener: Arc<dyn ConnectionEventListener>) {
        crate::grpc::GrpcClient::add_connection_listener(self, listener)
    }
}
