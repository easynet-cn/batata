// Lock module gRPC handler: LockOperationHandler
// Handles distributed lock acquire/release operations

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use batata_consistency::RaftNode;
use batata_consistency::raft::request::RaftRequest;
use tonic::Status;
use tracing::{debug, warn};

use crate::{GrpcAuthService, GrpcResource, PermissionAction, model::Connection};

use crate::handler::rpc::{AuthRequirement, PayloadHandler};
use crate::service::lock::{LockService, MAX_LOCK_EXPIRE_MS};
use batata_api::grpc::Payload;
use batata_api::remote::model::{
    LockInstance, LockOperationRequest, LockOperationResponse, RequestTrait, ResponseTrait,
};

const LOCK_OP_ACQUIRE: &str = "ACQUIRE";
const LOCK_OP_RELEASE: &str = "RELEASE";
const LOCK_OP_RENEW: &str = "RENEW";
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
/// Metric label value for renew op.
const METRIC_OP_RENEW: &str = "renew";

/// Resolve the lock owner for *every* lock operation from a single source.
///
/// A 3.3.0 client sends `LockInstance.owner` (`clientId:threadId`); when it is
/// absent (older payloads) we fall back to the connection id. ACQUIRE,
/// RELEASE and RENEW MUST all use this same value, otherwise a RENEW issued
/// by the owner that performed the ACQUIRE would fail the owner check.
fn resolve_owner(lock_instance: &LockInstance, connection: &Connection) -> String {
    match &lock_instance.owner {
        Some(owner) if !owner.is_empty() => owner.clone(),
        _ => connection.meta_info.connection_id.clone(),
    }
}

/// Translate a RENEW `expiredTime` into the `ttl_ms` carried by
/// [`RaftRequest::LockRenew`].
///
/// Nacos treats `expiredTime` as a *duration*: positive values are clamped to
/// `maxExpireTime`, non-positive values mean "keep the existing TTL" (`None`,
/// which the state machine resolves to the stored `ttl_ms`). `expiredTime == 0`
/// is rejected earlier by the handler as a fail-closed error.
fn lock_renew_ttl_ms(expired_time_ms: i64) -> Option<u64> {
    if expired_time_ms > 0 {
        Some(expired_time_ms.min(MAX_LOCK_EXPIRE_MS) as u64)
    } else {
        None
    }
}

/// Build the Raft `LockRenew` request (pure, so it can be unit-tested without
/// a live `RaftNode`). `now_ms` is supplied by the caller as the leader clock.
fn lock_renew_request(key: &str, owner: &str, expired_time_ms: i64, now_ms: i64) -> RaftRequest {
    RaftRequest::LockRenew {
        namespace: DEFAULT_LOCK_NAMESPACE.to_string(),
        name: key.to_string(),
        owner: owner.to_owned(),
        ttl_ms: lock_renew_ttl_ms(expired_time_ms),
        now_ms,
    }
}

/// Handler for LockOperationRequest - processes distributed lock operations.
///
/// When `raft_node` is present, writes are routed through Raft consensus so
/// lock state is replicated across the cluster and survives restart — the
/// semantics required by Nacos SDK clients. When `raft_node` is `None`
/// (standalone mode), falls back to the in-memory `LockService`.
#[derive(Clone)]
pub struct LockOperationHandler {
    /// The `lock_service` field.
    pub lock_service: Arc<LockService>,
    /// The `auth_service` field.
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

        // Single owner resolution shared by ACQUIRE / RELEASE / RENEW.
        let owner = resolve_owner(lock_instance, connection);

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
            LOCK_OP_RENEW => METRIC_OP_RENEW,
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
            LOCK_OP_RENEW => {
                // GA rejects expiredTime == 0 for RENEW: a zero duration would
                // make the lock expire immediately. Fail closed (error response,
                // result=false) rather than refreshing with a bogus TTL.
                if lock_instance.expired_time == 0 {
                    let response = crate::error_response!(
                        LockOperationResponse,
                        request_id,
                        "Lock expiredTime must be non-zero for RENEW"
                    );
                    Err(response.build_payload())
                } else {
                    let renewed = if let Some(ref raft) = self.raft_node {
                        self.raft_renew(
                            raft.as_ref(),
                            &lock_instance.key,
                            &owner,
                            lock_instance.expired_time,
                        )
                        .await
                    } else {
                        self.lock_service.renew(
                            &lock_instance.key,
                            &owner,
                            lock_instance.expired_time,
                        )
                    };

                    let mut response = LockOperationResponse::new();
                    response.response.request_id = request_id;
                    response.result = renewed;

                    Ok((renewed, response.build_payload()))
                }
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
            now_ms: chrono::Utc::now().timestamp_millis(),
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

    /// Issue a Raft-replicated lock renew. Returns `true` on success.
    ///
    /// The state machine already owns the `LockRenew` variant and its apply
    /// logic (owner check / expiry check / `expires_at = now + ttl`); this
    /// method only constructs the request and writes it. A write failure is
    /// logged and reported as `false` (fail-closed).
    async fn raft_renew(
        &self,
        raft: &RaftNode,
        key: &str,
        owner: &str,
        expired_time_ms: i64,
    ) -> bool {
        let req = lock_renew_request(
            key,
            owner,
            expired_time_ms,
            chrono::Utc::now().timestamp_millis(),
        );
        match raft.write(req).await {
            Ok(resp) => resp.success,
            Err(e) => {
                warn!(key = %key, owner = %owner, error = %e, "Raft lock renew failed");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_lock_service() -> Arc<LockService> {
        Arc::new(LockService {
            locks: Arc::new(dashmap::DashMap::new()),
            fencing_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        })
    }

    fn test_auth_service() -> Arc<GrpcAuthService> {
        Arc::new(GrpcAuthService::default())
    }

    fn test_handler(svc: Arc<LockService>) -> LockOperationHandler {
        LockOperationHandler {
            lock_service: svc,
            auth_service: test_auth_service(),
            raft_node: None,
            fence_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    fn test_connection(connection_id: &str) -> Connection {
        let mut conn = Connection::default();
        conn.meta_info.connection_id = connection_id.to_string();
        conn
    }

    fn lock_instance(key: &str, expired_time: i64, owner: Option<&str>) -> LockInstance {
        LockInstance {
            key: key.to_string(),
            expired_time,
            lock_type: "mutex".to_string(),
            params: HashMap::new(),
            owner: owner.map(|o| o.to_string()),
        }
    }

    fn operation_request(
        op: &str,
        key: &str,
        expired_time: i64,
        owner: Option<&str>,
    ) -> LockOperationRequest {
        let mut req = LockOperationRequest::default();
        req.request.request_id = "req-1".to_string();
        req.module = "lock".to_string();
        req.lock_operation = op.to_string();
        req.lock_instance = Some(lock_instance(key, expired_time, owner));
        req
    }

    fn request_payload(req: &LockOperationRequest) -> Payload {
        // RequestTrait::to_payload serializes the request to JSON in the body,
        // exactly what LockOperationRequest::from() expects on the server.
        req.to_payload(None)
    }

    fn decode_response(payload: &Payload) -> LockOperationResponse {
        let bytes = &payload.body.as_ref().expect("response body").value;
        serde_json::from_slice(bytes).expect("valid LockOperationResponse JSON")
    }

    #[test]
    fn test_lock_operation_handler_can_handle() {
        let handler = test_handler(test_lock_service());
        assert_eq!(handler.can_handle(), "LockOperationRequest");
        assert_eq!(handler.auth_requirement(), AuthRequirement::Write);
    }

    #[tokio::test]
    async fn test_renew_standalone_success() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-A", 5000);
        let handler = test_handler(svc.clone());
        // connection id deliberately differs from the payload owner: owner
        // resolution must use the payload owner, not the connection.
        let conn = test_connection("conn-different");
        let req = operation_request("RENEW", "k", 30000, Some("owner-A"));
        let resp = decode_response(&handler.handle(&conn, &request_payload(&req)).await.unwrap());
        assert!(resp.result, "RENEW must succeed for the lock owner");
        assert!(resp.response.success);
        assert_eq!(
            svc.get_lock_count("k"),
            1,
            "renew must not change lock_count"
        );
    }

    #[tokio::test]
    async fn test_renew_owner_falls_back_to_connection_id() {
        let svc = test_lock_service();
        svc.acquire("k", "conn-owner", 5000);
        let handler = test_handler(svc.clone());
        let conn = test_connection("conn-owner");
        // No owner in the payload -> must fall back to connection id.
        let req = operation_request("RENEW", "k", 30000, None);
        let resp = decode_response(&handler.handle(&conn, &request_payload(&req)).await.unwrap());
        assert!(
            resp.result,
            "RENEW must fall back to connection id as owner"
        );
    }

    #[tokio::test]
    async fn test_owner_resolution_shared_across_acquire_renew_release() {
        let svc = test_lock_service();
        let handler = test_handler(svc.clone());
        let conn = test_connection("conn-different");

        // ACQUIRE with payload owner (connection id differs).
        let acquire = operation_request("ACQUIRE", "k", 5000, Some("owner-A"));
        let resp = decode_response(
            &handler
                .handle(&conn, &request_payload(&acquire))
                .await
                .unwrap(),
        );
        assert!(resp.result, "ACQUIRE must succeed");
        assert_eq!(svc.get_lock_count("k"), 1);

        // RENEW must resolve the SAME owner as ACQUIRE.
        let renew = operation_request("RENEW", "k", 30000, Some("owner-A"));
        let resp = decode_response(
            &handler
                .handle(&conn, &request_payload(&renew))
                .await
                .unwrap(),
        );
        assert!(resp.result, "RENEW must use the same owner as ACQUIRE");

        // RELEASE must resolve the SAME owner as ACQUIRE. expired_time 0 here
        // mimics unlock(): it must still be treated as RELEASE, not RENEW.
        let release = operation_request("RELEASE", "k", 0, Some("owner-A"));
        let resp = decode_response(
            &handler
                .handle(&conn, &request_payload(&release))
                .await
                .unwrap(),
        );
        assert!(resp.result, "RELEASE must use the same owner as ACQUIRE");
        assert_eq!(svc.get_lock_count("k"), 0);
    }

    #[tokio::test]
    async fn test_renew_expired_time_zero_fail_closed() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-A", 5000);
        let handler = test_handler(svc.clone());
        let conn = test_connection("conn");
        let req = operation_request("RENEW", "k", 0, Some("owner-A"));
        let resp = decode_response(&handler.handle(&conn, &request_payload(&req)).await.unwrap());
        assert!(!resp.result, "expiredTime==0 RENEW must be rejected");
        assert!(!resp.response.success);
        assert_eq!(resp.response.result_code, 500);
        assert_eq!(resp.response.error_code, 500);
        assert!(resp.response.message.contains("non-zero"));
        // The renew did not run, so the lock entry is untouched.
        assert_eq!(svc.get_lock_count("k"), 1);
    }

    #[tokio::test]
    async fn test_renew_wrong_owner_returns_false() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-A", 60000);
        let handler = test_handler(svc.clone());
        let conn = test_connection("conn");
        let req = operation_request("RENEW", "k", 30000, Some("owner-B"));
        let resp = decode_response(&handler.handle(&conn, &request_payload(&req)).await.unwrap());
        assert!(!resp.result);
        assert!(resp.response.success, "a normal false result, not an error");
        assert_eq!(svc.get_lock_count("k"), 1);
    }

    #[tokio::test]
    async fn test_unknown_op_fail_closed() {
        let handler = test_handler(test_lock_service());
        let conn = test_connection("conn");
        let req = operation_request("BOGUS", "k", 30000, Some("owner-A"));
        let resp = decode_response(&handler.handle(&conn, &request_payload(&req)).await.unwrap());
        assert!(!resp.response.success);
        assert!(!resp.result);
    }

    #[test]
    fn test_resolve_owner() {
        let conn = test_connection("conn-id");
        assert_eq!(
            resolve_owner(&lock_instance("k", 1, Some("payload-owner")), &conn),
            "payload-owner"
        );
        assert_eq!(
            resolve_owner(&lock_instance("k", 1, None), &conn),
            "conn-id"
        );
        assert_eq!(
            resolve_owner(&lock_instance("k", 1, Some("")), &conn),
            "conn-id",
            "an empty owner must fall back to the connection id"
        );
    }

    #[test]
    fn test_lock_renew_ttl_ms() {
        assert_eq!(lock_renew_ttl_ms(30000), Some(30000));
        assert_eq!(lock_renew_ttl_ms(100), Some(100));
        assert_eq!(lock_renew_ttl_ms(0), None);
        assert_eq!(lock_renew_ttl_ms(-1), None);
        assert_eq!(
            lock_renew_ttl_ms(MAX_LOCK_EXPIRE_MS + 1000),
            Some(MAX_LOCK_EXPIRE_MS as u64),
            "ttl_ms must be clamped to maxExpireTime"
        );
    }

    #[test]
    fn test_lock_renew_raft_request() {
        match lock_renew_request("k", "owner-A", 30000, 12345) {
            RaftRequest::LockRenew {
                namespace,
                name,
                owner,
                ttl_ms,
                now_ms,
            } => {
                assert_eq!(namespace, "");
                assert_eq!(name, "k");
                assert_eq!(owner, "owner-A");
                assert_eq!(ttl_ms, Some(30000));
                assert_eq!(now_ms, 12345);
            }
            _ => panic!("expected RaftRequest::LockRenew"),
        }
        match lock_renew_request("k", "owner-A", -1, 0) {
            RaftRequest::LockRenew { ttl_ms, .. } => assert_eq!(ttl_ms, None),
            _ => panic!("expected RaftRequest::LockRenew"),
        }
    }
}
