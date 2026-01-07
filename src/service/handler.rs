use std::collections::HashMap;

use tonic::Status;

use crate::{
    api::{
        grpc::{Metadata, Payload},
        remote::model::{
            ClientDetectionResponse, ConnectResetResponse, HealthCheckResponse, Response,
            ResponseTrait, ServerCheckResponse, ServerLoaderInfoResponse, ServerReloadResponse,
            SetupAckResponse,
        },
    },
    core::model::Connection,
    service::rpc::PayloadHandler,
};

#[derive(Clone)]
pub struct HealthCheckHandler {}

#[tonic::async_trait]
impl PayloadHandler for HealthCheckHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let response = HealthCheckResponse {
            response: Response::new(),
        };

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        let response_payload = response.to_payload(Some(metadata));

        Ok(response_payload)
    }

    fn can_handle(&self) -> &'static str {
        "HealthCheckRequest"
    }
}

#[derive(Clone)]
pub struct ServerCheckHandler {}

#[tonic::async_trait]
impl PayloadHandler for ServerCheckHandler {
    async fn handle(&self, connection: &Connection, _: &Payload) -> Result<Payload, Status> {
        let response = ServerCheckResponse {
            response: Response::new(),
            connection_id: connection.meta_info.connection_id.clone(),
            ..Default::default()
        };

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        let response_payload = response.to_payload(Some(metadata));

        Ok(response_payload)
    }

    fn can_handle(&self) -> &'static str {
        "ServerCheckRequest"
    }
}

#[derive(Clone)]
pub struct ConnectionSetupHandler {}

#[tonic::async_trait]
impl PayloadHandler for ConnectionSetupHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        // Connection setup is handled in the bi-stream service
        // Just return the payload as acknowledgment
        Ok(payload.clone())
    }

    fn can_handle(&self) -> &'static str {
        "ConnectionSetupRequest"
    }
}

// Handler for ClientDetectionRequest - detects client status
#[derive(Clone)]
pub struct ClientDetectionHandler {}

#[tonic::async_trait]
impl PayloadHandler for ClientDetectionHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let response = ClientDetectionResponse::new();

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.to_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ClientDetectionRequest"
    }
}

// Handler for ServerLoaderInfoRequest - returns server load information
#[derive(Clone)]
pub struct ServerLoaderInfoHandler {}

#[tonic::async_trait]
impl PayloadHandler for ServerLoaderInfoHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let mut response = ServerLoaderInfoResponse::new();

        // Add basic server metrics
        let mut metrics = HashMap::new();
        metrics.insert("sdkConCount".to_string(), "0".to_string());
        metrics.insert("cpu".to_string(), "0".to_string());
        metrics.insert("load".to_string(), "0".to_string());
        metrics.insert("mem".to_string(), "0".to_string());
        response.loader_metrics = metrics;

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.to_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ServerLoaderInfoRequest"
    }
}

// Handler for ServerReloadRequest - triggers server configuration reload
#[derive(Clone)]
pub struct ServerReloadHandler {}

#[tonic::async_trait]
impl PayloadHandler for ServerReloadHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        // In a real implementation, this would trigger configuration reload
        let response = ServerReloadResponse::new();

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.to_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ServerReloadRequest"
    }
}

// Handler for ConnectResetRequest - handles connection reset
#[derive(Clone)]
pub struct ConnectResetHandler {}

#[tonic::async_trait]
impl PayloadHandler for ConnectResetHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        // In a real implementation, this would reset the connection
        let response = ConnectResetResponse::new();

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.to_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "ConnectResetRequest"
    }
}

// Handler for SetupAckRequest - acknowledges connection setup
#[derive(Clone)]
pub struct SetupAckHandler {}

#[tonic::async_trait]
impl PayloadHandler for SetupAckHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let response = SetupAckResponse::new();

        let metadata = Metadata {
            r#type: response.response_type().to_string(),
            ..Default::default()
        };

        Ok(response.to_payload(Some(metadata)))
    }

    fn can_handle(&self) -> &'static str {
        "SetupAckRequest"
    }
}

// Handler for PushAckRequest - acknowledges server push
#[derive(Clone)]
pub struct PushAckHandler {}

#[tonic::async_trait]
impl PayloadHandler for PushAckHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        // PushAck doesn't require a response, just acknowledge
        Ok(payload.clone())
    }

    fn can_handle(&self) -> &'static str {
        "PushAckRequest"
    }
}
