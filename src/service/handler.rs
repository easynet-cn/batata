use tonic::Status;

use crate::{api::rpc::model::HealthCheckResponse, grpc::Payload, service::rpc::PayloadHandler};

#[derive(Clone)]
pub struct HealthCheckHandler {}

#[tonic::async_trait]
impl PayloadHandler for HealthCheckHandler {
    async fn handle(&self, payload: &Payload) -> Result<Payload, Status> {
        Ok(payload.clone())
    }

    fn can_handle(&self) -> String {
        "HealthCheckRequest".to_string()
    }
}
