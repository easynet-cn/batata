// Lock module gRPC handler: LockOperationHandler
// Handles distributed lock acquire/release operations

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use batata_consistency::RaftNode;
use batata_consistency::raft::request::RaftRequest;
use tonic::Status;
use tracing::{debug, warn};

use crate::{GrpcAuthService, GrpcResource, PermissionAction, model::Connection};

use crate::{
    api::{
        grpc::Payload,
        remote::model::{LockOperationRequest, LockOperationResponse, RequestTrait, ResponseTrait},
    },
    handler::rpc::{AuthRequirement, PayloadHandler},
    service::lock::LockService,
};

const LOCK_OP_ACQUIRE: &str = "ACQUIRE";
const LOCK_OP_RELEASE: &str = "RELEASE";
/// Default namespace used when a Nacos `LockOperationRequest` arrives without
/// an explicit namespace field (the Nacos wire format only carries a flat `key`).
const DEFAULT_LOCK_NAMESPACE: &str = "";

/// Metric name aligned with Nacos's `LockMetricsMonitor` (grpcLockTotal /
/// grpcUnLockTotal collapsed onto one counter with an `op` label so the
/// series count stays small).
const METRIC_LOCK_GRPC_TOTAL: &str = "nacos_monitor_lock_grpc_total";
/// Counter of successful lock operations (success==true in the response).
const METRIC_LOCK_GRPC_SUCCESS: &str = "nacos_monitor_lock_grpc_success";
/// Handler latency histogram (matches Nacos `lockHandlerRt` timer).
const METRIC_LOCK_HANDLER_RT: &str = "nacos_monitor_lock_handler_rt_seconds";

/// Metric label value for acquire op.
const METRIC_OP_LOCK: &str = "lock";
/// Metric label value for release op.
const METRIC_OP_UNLOCK: &str = "unlock";

/// Handler for LockOperationRequest - processes distributed lock operations.
///
/// When `raft_node` is present, writes are routed through Raft consensus so
/// lock state is replicated across the cluster and survives restart — the
/// semantics required by Nacos SDK clients. When `raft_node` is `None`
/// (standalone mode), falls back to the in-memory `LockService`.
#[derive(Clone)]
pub struct LockOperationHandler {
    pub lock_service: Arc<LockService>,
    pub auth_service: Arc<GrpcAuthService>,
    /// Optional Raft node. `Some` in cluster mode (CP), `None` in standalone.
    pub raft_node: Option<Arc<RaftNode>>,
    /// Monotonic counter used to allocate fence tokens for Raft acquire writes.
    /// Not globally ordered across leader failover — Nacos SDK does not use
    /// fence tokens on the wire, so this is only consumed internally.
    pub fence_counter: Arc<AtomicU64>,
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

        let owner = connection.meta_info.connection_id.clone();

        debug!(
            operation = %operation,
            key = %lock_instance.key,
            owner = %owner,
            raft_mode = %self.raft_node.is_some(),
            "Processing lock operation"
        );

        let start = std::time::Instant::now();
        let metric_op = match operation.as_str() {
            LOCK_OP_ACQUIRE => METRIC_OP_LOCK,
            LOCK_OP_RELEASE => METRIC_OP_UNLOCK,
            _ => "unknown",
        };

        let result = match operation.as_str() {
            LOCK_OP_ACQUIRE => {
                let acquired = if let Some(ref raft) = self.raft_node {
                    self.raft_acquire(
                        raft.as_ref(),
                        &lock_instance.key,
                        &owner,
                        lock_instance.expired_time,
                    )
                    .await
                } else {
                    self.lock_service
                        .acquire(&lock_instance.key, &owner, lock_instance.expired_time)
                        .is_some()
                };

                let mut response = LockOperationResponse::new();
                response.response.request_id = request_id;
                response.result = acquired;

                Ok((acquired, response.build_payload()))
            }
            LOCK_OP_RELEASE => {
                let released = if let Some(ref raft) = self.raft_node {
                    self.raft_release(raft.as_ref(), &lock_instance.key, &owner)
                        .await
                } else {
                    self.lock_service.release(&lock_instance.key, &owner)
                };

                let mut response = LockOperationResponse::new();
                response.response.request_id = request_id;
                response.result = released;

                Ok((released, response.build_payload()))
            }
            _ => {
                let response = crate::error_response!(
                    LockOperationResponse,
                    request_id,
                    format!("Unsupported lock operation: {}", operation)
                );
                // Unknown ops still emit the total counter with op="unknown"
                // so alerting can catch clients sending garbage.
                Err(response.build_payload())
            }
        };

        // Record metrics once, at the end, regardless of which branch ran.
        metrics::counter!(METRIC_LOCK_GRPC_TOTAL, "op" => metric_op).increment(1);
        let elapsed = start.elapsed().as_secs_f64();
        metrics::histogram!(METRIC_LOCK_HANDLER_RT, "op" => metric_op).record(elapsed);

        match result {
            Ok((success, payload)) => {
                if success {
                    metrics::counter!(METRIC_LOCK_GRPC_SUCCESS, "op" => metric_op).increment(1);
                }
                Ok(payload)
            }
            Err(payload) => Ok(payload),
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

    fn resource_from_payload(&self, payload: &Payload) -> Option<(GrpcResource, PermissionAction)> {
        let request = LockOperationRequest::from(payload);
        let key = request
            .lock_instance
            .as_ref()
            .map(|li| li.key.as_str())
            .unwrap_or("*");
        Some((GrpcResource::lock(key), PermissionAction::Write))
    }

    fn resource_type(&self) -> crate::ResourceType {
        crate::ResourceType::Lock
    }
}

impl LockOperationHandler {
    /// Issue a Raft-replicated lock acquire. Returns `true` on success.
    ///
    /// Negative or zero TTLs are clamped to zero (the state machine treats
    /// zero as "immediately expired"); Nacos SDK sends a positive
    /// `expiredTime` field in milliseconds, but we are defensive here.
    async fn raft_acquire(
        &self,
        raft: &RaftNode,
        key: &str,
        owner: &str,
        expired_time_ms: i64,
    ) -> bool {
        let ttl_ms = expired_time_ms.max(0) as u64;
        let fence_token = self.fence_counter.fetch_add(1, Ordering::Relaxed);
        let req = RaftRequest::LockAcquire {
            namespace: DEFAULT_LOCK_NAMESPACE.to_string(),
            name: key.to_string(),
            owner: owner.to_string(),
            ttl_ms,
            fence_token,
            owner_metadata: None,
        };
        match raft.write(req).await {
            Ok(resp) => resp.success,
            Err(e) => {
                warn!(key = %key, owner = %owner, error = %e, "Raft lock acquire failed");
                false
            }
        }
    }

    /// Issue a Raft-replicated lock release. Returns `true` on success.
    async fn raft_release(&self, raft: &RaftNode, key: &str, owner: &str) -> bool {
        let req = RaftRequest::LockRelease {
            namespace: DEFAULT_LOCK_NAMESPACE.to_string(),
            name: key.to_string(),
            owner: owner.to_string(),
            fence_token: None,
        };
        match raft.write(req).await {
            Ok(resp) => resp.success,
            Err(e) => {
                warn!(key = %key, owner = %owner, error = %e, "Raft lock release failed");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lock_service() -> Arc<LockService> {
        Arc::new(LockService {
            locks: Arc::new(dashmap::DashMap::new()),
            fencing_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
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
            raft_node: None,
            fence_counter: Arc::new(AtomicU64::new(1)),
        };
        assert_eq!(handler.can_handle(), "LockOperationRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }
}
