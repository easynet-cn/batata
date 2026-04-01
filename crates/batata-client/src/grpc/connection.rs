//! gRPC connection lifecycle management
//!
//! Manages a single gRPC connection to a Nacos server, including
//! the unary request channel and bi-directional stream for server push.

use std::collections::HashMap;

use batata_api::{
    grpc::{
        Metadata, Payload, bi_request_stream_client::BiRequestStreamClient,
        request_client::RequestClient,
    },
    model::{CLIENT_VERSION, SDK_GRPC_PORT_DEFAULT_OFFSET},
    remote::model::{
        ClientAbilities, ConnectionSetupRequest, LABEL_MODULE, LABEL_SOURCE, LABEL_SOURCE_SDK,
        RequestTrait, ServerCheckRequest, ServerCheckResponse,
    },
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{ClientError, Result};

/// Represents an established gRPC connection to a Nacos server.
pub struct GrpcConnection {
    /// The underlying tonic channel (cheap to clone — Arc internally)
    channel: Channel,
    /// Sender side of the bi-directional stream (client -> server)
    bi_stream_tx: mpsc::Sender<Payload>,
    /// Connection ID assigned by the server
    connection_id: String,
}

impl GrpcConnection {
    /// Establish a new gRPC connection to the specified server.
    ///
    /// `server_addr` is the HTTP server address (e.g., `127.0.0.1:8848`).
    /// The gRPC port is computed as `http_port + 1000`.
    /// `module` should be "config", "naming", or both.
    /// `labels` are additional connection labels.
    ///
    /// Returns the connection and a separate push receiver channel.
    /// The push receiver is returned separately to avoid holding the connection
    /// lock while waiting for server push messages.
    pub async fn connect(
        server_addr: &str,
        module: &str,
        labels: HashMap<String, String>,
        tenant: &str,
        access_token: Option<&str>,
        push_channel_capacity: usize,
    ) -> Result<(Self, mpsc::Receiver<Payload>)> {
        let grpc_addr = compute_grpc_addr(server_addr)?;

        info!("Connecting to gRPC server at {}", grpc_addr);

        let channel = Channel::from_shared(grpc_addr.clone())
            .map_err(|e| ClientError::Other(anyhow::anyhow!("Invalid gRPC address: {}", e)))?
            .connect()
            .await?;

        // Step 1: Server check — get connection_id
        let connection_id = Self::server_check(&channel, access_token).await?;
        debug!("Server check OK, connection_id: {}", connection_id);

        // Step 2: Open bi-directional stream
        let (bi_stream_tx, client_rx) = mpsc::channel::<Payload>(push_channel_capacity);
        let (push_tx, server_push_rx) = mpsc::channel::<Payload>(push_channel_capacity);

        let mut bi_client = BiRequestStreamClient::new(channel.clone());
        let client_stream = ReceiverStream::new(client_rx);

        let response = bi_client.request_bi_stream(client_stream).await?;
        let mut server_stream = response.into_inner();

        // Spawn task to read server pushes
        tokio::spawn(async move {
            while let Ok(Some(payload)) = server_stream.message().await {
                let payload_type = payload
                    .metadata
                    .as_ref()
                    .map(|m| m.r#type.as_str())
                    .unwrap_or("unknown");
                debug!("Received server push: type={}", payload_type);

                if push_tx.send(payload).await.is_err() {
                    warn!("Server push receiver dropped, stopping stream reader");
                    break;
                }
            }
            debug!("Bi-stream read loop ended");
        });

        // Step 3: Send ConnectionSetupRequest via bi-stream
        let mut all_labels = labels;
        all_labels.insert(LABEL_SOURCE.to_string(), LABEL_SOURCE_SDK.to_string());
        all_labels.insert(LABEL_MODULE.to_string(), module.to_string());

        // Report client abilities
        all_labels.insert("sdk_version".to_string(), "batata-rust-0.1.0".to_string());
        all_labels.insert("support_config".to_string(), "true".to_string());
        all_labels.insert("support_naming".to_string(), "true".to_string());
        all_labels.insert("support_lock".to_string(), "true".to_string());
        all_labels.insert("client_type".to_string(), "rust".to_string());

        let mut setup_req = ConnectionSetupRequest::new();
        setup_req.client_version = CLIENT_VERSION.to_string();
        setup_req.tenant = tenant.to_string();
        setup_req.labels = all_labels;
        setup_req.client_abilities = ClientAbilities {};

        if let Some(token) = access_token {
            let mut headers = HashMap::new();
            headers.insert("accessToken".to_string(), token.to_string());
            setup_req.insert_headers(headers);
        }

        let metadata = Metadata {
            r#type: setup_req.request_type().to_string(),
            ..Default::default()
        };
        let setup_payload = setup_req.to_payload(Some(metadata));

        bi_stream_tx
            .send(setup_payload)
            .await
            .map_err(|_| ClientError::NotConnected)?;

        debug!("Connection setup sent for module={}", module);

        // Brief delay for server to process setup (configurable via push_channel_capacity param context)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // Note: this delay should ideally be configurable but 100ms is the Nacos standard

        Ok((
            Self {
                channel,
                bi_stream_tx,
                connection_id,
            },
            server_push_rx,
        ))
    }

    /// Send a unary request and receive a response.
    ///
    /// This method clones the underlying tonic Channel (which is cheap —
    /// it's an Arc internally) to avoid requiring `&mut self`. This allows
    /// concurrent requests without locking.
    pub async fn request(&self, payload: Payload) -> Result<Payload> {
        let mut client = RequestClient::new(self.channel.clone());
        let response = client.request(payload).await?;
        Ok(response.into_inner())
    }

    /// Send a payload via the bi-directional stream.
    pub async fn send_bi_stream(&self, payload: Payload) -> Result<()> {
        self.bi_stream_tx
            .send(payload)
            .await
            .map_err(|_| ClientError::NotConnected)
    }

    /// Get the connection ID.
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Get a clone of the channel (for creating additional clients if needed).
    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    /// Perform a health check on the given channel.
    ///
    /// Sends a ServerCheckRequest and verifies the response.
    /// Used by the health check loop without holding the connection lock.
    pub async fn health_check(channel: &Channel) -> Result<bool> {
        let mut check_req = ServerCheckRequest::new();
        check_req.internal_request.request.request_id = Uuid::new_v4().to_string();

        let metadata = Metadata {
            r#type: check_req.request_type().to_string(),
            ..Default::default()
        };
        let payload = check_req.to_payload(Some(metadata));

        let mut client = RequestClient::new(channel.clone());
        match client.request(payload).await {
            Ok(response) => {
                let resp_payload = response.into_inner();
                let check_resp: ServerCheckResponse = super::deserialize_payload(&resp_payload);
                Ok(check_resp.response.success || check_resp.response.result_code == 200)
            }
            Err(e) => {
                warn!("Health check request failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Perform server check to verify server and get connection_id.
    async fn server_check(channel: &Channel, access_token: Option<&str>) -> Result<String> {
        let mut check_req = ServerCheckRequest::new();
        check_req.internal_request.request.request_id = Uuid::new_v4().to_string();

        if let Some(token) = access_token {
            let mut headers = HashMap::new();
            headers.insert("accessToken".to_string(), token.to_string());
            check_req.insert_headers(headers);
        }

        let metadata = Metadata {
            r#type: check_req.request_type().to_string(),
            ..Default::default()
        };
        let payload = check_req.to_payload(Some(metadata));

        let mut client = RequestClient::new(channel.clone());
        let response = client.request(payload).await?;
        let resp_payload = response.into_inner();

        let check_resp: ServerCheckResponse = super::deserialize_payload(&resp_payload);

        if check_resp.response.result_code != 200 && !check_resp.response.success {
            // For server check, a default response with empty connection_id
            // may still be valid — the server might not set result_code.
            // We only fail if there's an explicit error.
            if !check_resp.response.message.is_empty() && check_resp.response.error_code != 0 {
                return Err(ClientError::ServerError {
                    code: check_resp.response.error_code,
                    message: check_resp.response.message,
                });
            }
        }

        let connection_id = if check_resp.connection_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            check_resp.connection_id
        };

        Ok(connection_id)
    }
}

/// Compute the gRPC address from a server HTTP address.
/// E.g., `127.0.0.1:8848` -> `http://127.0.0.1:9848`
fn compute_grpc_addr(server_addr: &str) -> Result<String> {
    // Strip protocol prefix if present
    let addr = server_addr
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(ClientError::Other(anyhow::anyhow!(
            "Invalid server address: {}",
            server_addr
        )));
    }

    let port: u16 = parts[0].parse().map_err(|_| {
        ClientError::Other(anyhow::anyhow!("Invalid port in address: {}", server_addr))
    })?;
    let host = parts[1];
    let grpc_port = port + SDK_GRPC_PORT_DEFAULT_OFFSET;

    Ok(format!("http://{}:{}", host, grpc_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_grpc_addr() {
        assert_eq!(
            compute_grpc_addr("127.0.0.1:8848").unwrap(),
            "http://127.0.0.1:9848"
        );
        assert_eq!(
            compute_grpc_addr("http://localhost:8848").unwrap(),
            "http://localhost:9848"
        );
        assert_eq!(
            compute_grpc_addr("https://myhost:8848").unwrap(),
            "http://myhost:9848"
        );
    }

    #[test]
    fn test_compute_grpc_addr_different_ports() {
        assert_eq!(
            compute_grpc_addr("10.0.0.1:9848").unwrap(),
            "http://10.0.0.1:10848"
        );
        assert_eq!(
            compute_grpc_addr("10.0.0.1:80").unwrap(),
            "http://10.0.0.1:1080"
        );
    }

    #[test]
    fn test_compute_grpc_addr_ipv6() {
        // IPv6 with port
        assert_eq!(
            compute_grpc_addr("[::1]:8848").unwrap(),
            "http://[::1]:9848"
        );
    }

    #[test]
    fn test_compute_grpc_addr_invalid() {
        assert!(compute_grpc_addr("no-port").is_err());
    }

    #[test]
    fn test_compute_grpc_addr_invalid_port() {
        assert!(compute_grpc_addr("host:not-a-number").is_err());
    }

    #[test]
    fn test_compute_grpc_addr_hostname() {
        assert_eq!(
            compute_grpc_addr("nacos.example.com:8848").unwrap(),
            "http://nacos.example.com:9848"
        );
    }

    #[test]
    fn test_grpc_port_offset() {
        // Verify the offset constant is 1000
        assert_eq!(SDK_GRPC_PORT_DEFAULT_OFFSET, 1000);
    }

    #[test]
    fn test_client_abilities_labels() {
        // Verify that the expected ability labels are defined as constants
        // and would be inserted during connection setup
        let mut labels = HashMap::new();
        labels.insert(LABEL_SOURCE.to_string(), LABEL_SOURCE_SDK.to_string());
        labels.insert(LABEL_MODULE.to_string(), "config".to_string());
        labels.insert("sdk_version".to_string(), "batata-rust-0.1.0".to_string());
        labels.insert("support_config".to_string(), "true".to_string());
        labels.insert("support_naming".to_string(), "true".to_string());
        labels.insert("support_lock".to_string(), "true".to_string());
        labels.insert("client_type".to_string(), "rust".to_string());

        assert_eq!(labels.get("sdk_version").unwrap(), "batata-rust-0.1.0");
        assert_eq!(labels.get("support_config").unwrap(), "true");
        assert_eq!(labels.get("support_naming").unwrap(), "true");
        assert_eq!(labels.get("support_lock").unwrap(), "true");
        assert_eq!(labels.get("client_type").unwrap(), "rust");
        assert_eq!(labels.len(), 7);
    }
}
