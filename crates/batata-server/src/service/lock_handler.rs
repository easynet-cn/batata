// Lock module gRPC handler: LockOperationHandler
// Handles distributed lock acquire/release operations

use std::sync::Arc;

use tonic::Status;
use tracing::debug;

use batata_core::{GrpcAuthService, GrpcResource, PermissionAction, model::Connection};

use crate::{
    api::{
        grpc::Payload,
        remote::model::{LockOperationRequest, LockOperationResponse, RequestTrait, ResponseTrait},
    },
    service::{
        lock::LockService,
        rpc::{
            AuthRequirement, PayloadHandler, check_authority, extract_auth_context_from_payload,
        },
    },
};

const LOCK_OP_ACQUIRE: &str = "ACQUIRE";
const LOCK_OP_RELEASE: &str = "RELEASE";

/// Handler for LockOperationRequest - processes distributed lock operations
#[derive(Clone)]
pub struct LockOperationHandler {
    pub lock_service: Arc<LockService>,
    pub auth_service: Arc<GrpcAuthService>,
}

#[tonic::async_trait]
impl PayloadHandler for LockOperationHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = LockOperationRequest::from(payload);
        let request_id = request.request_id();
        let operation = request.lock_operation.to_uppercase();

        let Some(ref lock_instance) = request.lock_instance else {
            let response = crate::error_response!(
                LockOperationResponse,
                request_id,
                "Missing lock_instance in LockOperationRequest"
            );
            return Ok(response.build_payload());
        };

        // Check permission for lock resource
        let auth_context = extract_auth_context_from_payload(&self.auth_service, payload);
        let resource = GrpcResource::lock(&lock_instance.key);
        check_authority(
            &self.auth_service,
            &auth_context,
            &resource,
            PermissionAction::Write,
            &[],
        )?;

        let owner = &connection.meta_info.connection_id;

        debug!(
            operation = %operation,
            key = %lock_instance.key,
            owner = %owner,
            "Processing lock operation"
        );

        match operation.as_str() {
            LOCK_OP_ACQUIRE => {
                let acquired = self.lock_service.acquire(
                    &lock_instance.key,
                    owner,
                    lock_instance.expired_time,
                );

                let mut response = LockOperationResponse::new();
                response.response.request_id = request_id;
                response.result = acquired;

                Ok(response.build_payload())
            }
            LOCK_OP_RELEASE => {
                let released = self.lock_service.release(&lock_instance.key, owner);

                let mut response = LockOperationResponse::new();
                response.response.request_id = request_id;
                response.result = released;

                Ok(response.build_payload())
            }
            _ => {
                let response = crate::error_response!(
                    LockOperationResponse,
                    request_id,
                    format!("Unsupported lock operation: {}", operation)
                );
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "LockOperationRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "lock"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Lock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lock_service() -> Arc<LockService> {
        Arc::new(LockService {
            locks: Arc::new(dashmap::DashMap::new()),
        })
    }

    fn test_auth_service() -> Arc<GrpcAuthService> {
        Arc::new(GrpcAuthService::default())
    }

    #[test]
    fn test_lock_operation_handler_can_handle() {
        let handler = LockOperationHandler {
            lock_service: test_lock_service(),
            auth_service: test_auth_service(),
        };
        assert_eq!(handler.can_handle(), "LockOperationRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }
}
