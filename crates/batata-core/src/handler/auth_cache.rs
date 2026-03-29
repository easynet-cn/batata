//! Auth cache invalidation handler — receives invalidation requests from peer nodes
//!
//! When a role, permission, or token is changed on one cluster node, that node
//! broadcasts an AuthCacheInvalidateRequest to all peers. This handler processes
//! those requests and evicts the relevant cached entries locally.
//!
//! Uses the `AuthCacheInvalidator` trait to avoid depending on `batata-auth`.
//! The concrete implementation is injected at startup in `batata-server`.

use std::sync::Arc;

use tonic::Status;
use tracing::{debug, warn};

use crate::model::Connection;

use crate::{
    api::{
        grpc::Payload,
        remote::model::{
            AuthCacheInvalidateRequest, AuthCacheInvalidateResponse, RequestTrait, ResponseTrait,
        },
    },
    handler::rpc::{AuthRequirement, PayloadHandler},
};

/// Trait for invalidating auth caches without depending on `batata-auth`.
///
/// Implemented in `batata-server` where both `batata-core` and `batata-auth` are available.
pub trait AuthCacheInvalidator: Send + Sync {
    /// Invalidate caches by type and target.
    ///
    /// - `invalidate_type`: "role", "permission", "token", "user", "all"
    /// - `target`: username, role name, token id, or empty for "all"
    fn invalidate(&self, invalidate_type: &str, target: &str);
}

/// Handler for AuthCacheInvalidateRequest — evicts local auth caches
#[derive(Clone)]
pub struct AuthCacheInvalidateHandler {
    pub invalidator: Arc<dyn AuthCacheInvalidator>,
}

#[tonic::async_trait]
impl PayloadHandler for AuthCacheInvalidateHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = AuthCacheInvalidateRequest::from(payload);
        let request_id = request.request_id();

        debug!(
            invalidate_type = %request.invalidate_type,
            target = %request.target,
            from = %connection.meta_info.remote_ip,
            "Received auth cache invalidation from peer"
        );

        self.invalidator
            .invalidate(&request.invalidate_type, &request.target);

        let mut response = AuthCacheInvalidateResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "AuthCacheInvalidateRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}
