// RPC service implementations for gRPC communication
// This file defines the core RPC services for handling client connections and message processing

use std::{collections::HashMap, pin::Pin, sync::Arc};

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tonic::{Request, Response, Status, Streaming};

use crate::{
    api::{
        grpc::{Payload, bi_request_stream_server::BiRequestStream},
        model::APPNAME,
        remote::model::ConnectionSetupRequest,
    },
    core::{
        model::{Connection, GrpcClient},
        service::remote::ConnectionManager,
    },
};

// Trait for handling gRPC payload messages
#[tonic::async_trait]
pub trait PayloadHandler: Send + Sync {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let message_type = payload.metadata.clone().unwrap_or_default().r#type;

        Err(Status::unimplemented(format!(
            "Unknown message type '{}'",
            message_type
        )))
    }

    fn can_handle(&self) -> &'static str {
        ""
    }
}

// Default handler for unregistered message types
#[derive(Clone)]
pub struct DefaultHandler;

#[tonic::async_trait]
impl PayloadHandler for DefaultHandler {}

// Registry for managing payload handlers by message type
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn PayloadHandler>>,
    default_handler: Arc<dyn PayloadHandler>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            default_handler: Arc::new(DefaultHandler {}),
        }
    }

    pub fn get_handler(&self, message_type: &str) -> Arc<dyn PayloadHandler> {
        self.handlers
            .get(message_type)
            .unwrap_or(&self.default_handler)
            .clone()
    }

    pub fn register_handler(&mut self, handler: Arc<dyn PayloadHandler>) {
        self.handlers
            .insert(handler.can_handle().to_string(), handler);
    }
}

#[derive(Clone)]
pub struct GrpcRequestService {
    handler_registry: Arc<HandlerRegistry>,
}

impl GrpcRequestService {
    pub fn new(handler_registry: HandlerRegistry) -> Self {
        Self {
            handler_registry: Arc::new(handler_registry),
        }
    }

    pub fn from_arc(handler_registry: Arc<HandlerRegistry>) -> Self {
        Self { handler_registry }
    }
}

#[tonic::async_trait]
impl crate::api::grpc::request_server::Request for GrpcRequestService {
    async fn request(
        &self,
        request: tonic::Request<Payload>,
    ) -> std::result::Result<tonic::Response<Payload>, tonic::Status> {
        if let Some(metadata) = &request.get_ref().metadata {
            let connection = request
                .extensions()
                .get::<Connection>()
                .cloned()
                .unwrap_or_default();

            let message_type = &metadata.r#type;
            let payload = request.get_ref();

            let handler = self.handler_registry.get_handler(message_type);

            return match handler.handle(&connection, &payload).await {
                Ok(reponse_payload) => Ok(Response::new(reponse_payload)),
                Err(err) => Err(err),
            };
        }

        Err(tonic::Status::not_found("unknow message type"))
    }
}

#[derive(Clone)]
pub struct GrpcBiRequestStreamService {
    handler_registry: Arc<HandlerRegistry>,
    connection_manager: Arc<ConnectionManager>,
}

impl GrpcBiRequestStreamService {
    pub fn new(handler_registry: HandlerRegistry, connection_manager: ConnectionManager) -> Self {
        Self {
            handler_registry: Arc::new(handler_registry),
            connection_manager: Arc::new(connection_manager),
        }
    }
    pub fn from_arc(
        handler_registry: Arc<HandlerRegistry>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            handler_registry,
            connection_manager,
        }
    }
}

#[tonic::async_trait]
impl BiRequestStream for GrpcBiRequestStreamService {
    type requestBiStreamStream =
        Pin<Box<dyn Stream<Item = Result<Payload, Status>> + Send + 'static>>;

    async fn request_bi_stream(
        &self,
        request: Request<Streaming<Payload>>,
    ) -> Result<Response<Self::requestBiStreamStream>, Status> {
        let connection = request
            .extensions()
            .get::<Connection>()
            .cloned()
            .unwrap_or_default();
        let connection_id = connection.meta_info.connection_id.clone();
        let mut inbound_stream = request.into_inner();

        let (tx, rx) = mpsc::channel(128);

        let handler_registry = self.handler_registry.clone();
        let connection_manager = self.connection_manager.clone();

        tokio::spawn(async move {
            while let Some(message) = inbound_stream.next().await {
                match message {
                    Ok(payload) => {
                        if let Some(metadata) = &payload.metadata {
                            let message_type = &metadata.r#type;

                            if message_type == "ConnectionSetupRequest" {
                                let request = ConnectionSetupRequest::from(&payload);

                                let mut con = connection.clone();

                                con.meta_info.client_ip = metadata.client_ip.clone();
                                con.meta_info.version = request.client_version;
                                con.meta_info.labels = request.labels.clone();
                                con.meta_info.app_name = con
                                    .meta_info
                                    .labels
                                    .get(APPNAME)
                                    .map_or("-".to_string(), |e| e.to_string());
                                con.meta_info.namespace_id = request.tenant;

                                let connection_id = con.meta_info.connection_id.clone();

                                let client = GrpcClient::new(con.clone(), tx.clone());

                                connection_manager.register(&connection_id, client);
                            }

                            let handler = handler_registry.get_handler(message_type);

                            match handler.handle(&connection, &payload).await {
                                Ok(response_payload) => {
                                    if let Err(e) = tx.send(Ok(response_payload)).await {
                                        tracing::error!("Failed to send response: {}", e);

                                        break;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Handler error: {}", e);
                                    if let Err(send_err) = tx.send(Err(e)).await {
                                        tracing::error!(
                                            "Failed to send error response: {}",
                                            send_err
                                        );

                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error receiving message: {}", e);

                        break;
                    }
                }
            }
            
            // 连接断开时清理资源
            connection_manager.unregister(&connection_id);
        });

        let output_stream = ReceiverStream::new(rx);

        Ok(Response::new(
            Box::pin(output_stream) as Self::requestBiStreamStream
        ))
    }
}
