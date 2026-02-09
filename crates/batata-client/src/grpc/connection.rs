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
    /// The underlying tonic channel (shared for both services)
    channel: Channel,
    /// Unary request client
    request_client: RequestClient<Channel>,
    /// Sender side of the bi-directional stream (client -> server)
    bi_stream_tx: mpsc::Sender<Payload>,
    /// Receiver for server push messages (server -> client)
    server_push_rx: mpsc::Receiver<Payload>,
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
    pub async fn connect(
        server_addr: &str,
        module: &str,
        labels: HashMap<String, String>,
        tenant: &str,
        access_token: Option<&str>,
    ) -> Result<Self> {
        let grpc_addr = compute_grpc_addr(server_addr)?;

        info!("Connecting to gRPC server at {}", grpc_addr);

        let channel = Channel::from_shared(grpc_addr.clone())
            .map_err(|e| ClientError::Other(anyhow::anyhow!("Invalid gRPC address: {}", e)))?
            .connect()
            .await?;

        let mut request_client = RequestClient::new(channel.clone());

        // Step 1: Server check — get connection_id
        let connection_id = Self::server_check(&mut request_client, access_token).await?;
        debug!("Server check OK, connection_id: {}", connection_id);

        // Step 2: Open bi-directional stream
        let (bi_stream_tx, client_rx) = mpsc::channel::<Payload>(256);
        let (push_tx, server_push_rx) = mpsc::channel::<Payload>(256);

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

        // Brief delay for server to process setup
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        Ok(Self {
            channel,
            request_client,
            bi_stream_tx,
            server_push_rx,
            connection_id,
        })
    }

    /// Send a unary request and receive a response.
    pub async fn request(&mut self, payload: Payload) -> Result<Payload> {
        let response = self.request_client.request(payload).await?;
        Ok(response.into_inner())
    }

    /// Send a payload via the bi-directional stream.
    pub async fn send_bi_stream(&self, payload: Payload) -> Result<()> {
        self.bi_stream_tx
            .send(payload)
            .await
            .map_err(|_| ClientError::NotConnected)
    }

    /// Try to receive the next server push message (non-blocking).
    pub fn try_recv_push(&mut self) -> Option<Payload> {
        self.server_push_rx.try_recv().ok()
    }

    /// Receive the next server push message (blocking).
    pub async fn recv_push(&mut self) -> Option<Payload> {
        self.server_push_rx.recv().await
    }

    /// Get the connection ID.
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Get a clone of the channel (for creating additional clients if needed).
    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    /// Perform server check to verify server and get connection_id.
    async fn server_check(
        request_client: &mut RequestClient<Channel>,
        access_token: Option<&str>,
    ) -> Result<String> {
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

        let response = request_client.request(payload).await?;
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
    fn test_compute_grpc_addr_invalid() {
        assert!(compute_grpc_addr("no-port").is_err());
    }
}
