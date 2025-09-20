use prost_types::Any;
use tonic::{Request, Status};

use crate::{
    api::rpc::model::{HealthCheckResponse, Response, ResponseTrait, ServerCheckResponse},
    core::model::Connection,
    grpc::{Metadata, Payload},
    service::rpc::PayloadHandler,
};

#[derive(Clone)]
pub struct HealthCheckHandler {}

#[tonic::async_trait]
impl PayloadHandler for HealthCheckHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let response = HealthCheckResponse {
            respones: Response::new(),
        };

        let metadata = Metadata {
            r#type: response.response_type(),
            ..Default::default()
        };

        let response_payload = response.into_payload(Some(metadata));

        Ok(response_payload)
    }

    fn can_handle(&self) -> String {
        "HealthCheckRequest".to_string()
    }
}

#[derive(Clone)]
pub struct ServerCheckHanlder {}

#[tonic::async_trait]
impl PayloadHandler for ServerCheckHanlder {
    async fn handle(&self, connection: &Connection, _: &Payload) -> Result<Payload, Status> {
        let response = ServerCheckResponse {
            respones: Response::new(),
            connection_id: connection.meta_info.connection_id.clone(),
            ..Default::default()
        };

        let metadata = Metadata {
            r#type: response.response_type(),
            ..Default::default()
        };

        let response_payload = response.into_payload(Some(metadata));

        Ok(response_payload)
    }

    fn can_handle(&self) -> String {
        "ServerCheckRequest".to_string()
    }
}

#[derive(Clone)]
pub struct ConnectionSetupHandler {}

#[tonic::async_trait]
impl PayloadHandler for ConnectionSetupHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        if let Some(meta) = &payload.metadata {
            let client_ip = &meta.client_ip;
            let headers = &meta.headers;

            for (k, v) in headers.iter() {
                println!("{}:{}", k, v)
            }

            if let Some(body) = &payload.body {
                let json_str = String::from_utf8_lossy(&body.value.to_vec()).into_owned();

                println!("{}", json_str)
            }
        }

        Ok(payload.clone())
    }

    fn can_handle(&self) -> String {
        "ConnectionSetupRequest".to_string()
    }
}
