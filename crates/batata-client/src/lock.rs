//! Distributed lock service
//!
//! Provides distributed lock operations via gRPC,
//! supporting acquire and release with timeout.

use std::sync::Arc;

use tracing::{debug, info};

use batata_api::remote::model::{LockInstance, LockOperationRequest, LockOperationResponse};

use crate::error::Result;
use crate::grpc::GrpcClient;

const LOCK_OP_ACQUIRE: &str = "ACQUIRE";
const LOCK_OP_RELEASE: &str = "RELEASE";

/// Distributed lock service backed by gRPC
pub struct BatataLockService {
    grpc_client: Arc<GrpcClient>,
}

impl BatataLockService {
    /// Create a new lock service with the given gRPC client.
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self { grpc_client }
    }

    /// Acquire a distributed lock.
    ///
    /// # Arguments
    /// * `key` - Lock key to acquire
    /// * `expired_time` - Lock expiration time in milliseconds
    ///
    /// # Returns
    /// * `true` if the lock was acquired, `false` if it's already held
    pub async fn lock(&self, key: &str, expired_time: i64) -> Result<bool> {
        self.lock_with_params(key, expired_time, "mutex", Default::default())
            .await
    }

    /// Acquire a distributed lock with custom parameters.
    pub async fn lock_with_params(
        &self,
        key: &str,
        expired_time: i64,
        lock_type: &str,
        params: std::collections::HashMap<String, String>,
    ) -> Result<bool> {
        let mut req = LockOperationRequest::default();
        req.request.request_id = uuid::Uuid::new_v4().to_string();
        req.module = "lock".to_string();
        req.lock_operation = LOCK_OP_ACQUIRE.to_string();
        req.lock_instance = Some(LockInstance {
            key: key.to_string(),
            expired_time,
            lock_type: lock_type.to_string(),
            params,
        });

        let resp: LockOperationResponse = self.grpc_client.request_typed(&req).await?;

        debug!("Lock acquire key={}, result={}", key, resp.result);

        Ok(resp.result)
    }

    /// Release a distributed lock.
    ///
    /// # Arguments
    /// * `key` - Lock key to release
    ///
    /// # Returns
    /// * `true` if the lock was released, `false` if it wasn't held
    pub async fn unlock(&self, key: &str) -> Result<bool> {
        let mut req = LockOperationRequest::default();
        req.request.request_id = uuid::Uuid::new_v4().to_string();
        req.module = "lock".to_string();
        req.lock_operation = LOCK_OP_RELEASE.to_string();
        req.lock_instance = Some(LockInstance {
            key: key.to_string(),
            expired_time: 0,
            lock_type: String::new(),
            params: Default::default(),
        });

        let resp: LockOperationResponse = self.grpc_client.request_typed(&req).await?;

        debug!("Lock release key={}, result={}", key, resp.result);

        Ok(resp.result)
    }

    /// Try to acquire a lock, returning immediately if not available.
    /// Same as `lock()` but semantically indicates a non-blocking try.
    pub async fn try_lock(&self, key: &str, expired_time: i64) -> Result<bool> {
        self.lock(key, expired_time).await
    }

    /// Acquire a lock and return a guard that auto-releases on drop.
    pub async fn lock_guard(
        self: &Arc<Self>,
        key: &str,
        expired_time: i64,
    ) -> Result<Option<LockGuard>> {
        let acquired = self.lock(key, expired_time).await?;
        if acquired {
            Ok(Some(LockGuard::new(key.to_string(), self.clone())))
        } else {
            Ok(None)
        }
    }
}

/// RAII lock guard that automatically releases the lock when dropped
pub struct LockGuard {
    key: String,
    lock_service: Arc<BatataLockService>,
    released: bool,
}

impl LockGuard {
    fn new(key: String, lock_service: Arc<BatataLockService>) -> Self {
        Self {
            key,
            lock_service,
            released: false,
        }
    }

    /// Explicitly release the lock
    pub async fn release(&mut self) -> Result<bool> {
        if !self.released {
            self.released = true;
            self.lock_service.unlock(&self.key).await
        } else {
            Ok(true)
        }
    }

    /// Get the lock key
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if !self.released {
            let key = self.key.clone();
            let service = self.lock_service.clone();
            tokio::spawn(async move {
                if let Err(e) = service.unlock(&key).await {
                    tracing::error!("Failed to release lock {} on drop: {}", key, e);
                } else {
                    info!("Auto-released lock {} on drop", key);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_constants() {
        assert_eq!(LOCK_OP_ACQUIRE, "ACQUIRE");
        assert_eq!(LOCK_OP_RELEASE, "RELEASE");
    }

    #[tokio::test]
    async fn test_lock_guard_key() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(GrpcClient::new(config).unwrap());
        let lock_service = Arc::new(BatataLockService::new(client));
        let guard = LockGuard::new("test-key".to_string(), lock_service);

        assert_eq!(guard.key(), "test-key");
    }

    #[tokio::test]
    async fn test_lock_guard_released_flag() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(GrpcClient::new(config).unwrap());
        let lock_service = Arc::new(BatataLockService::new(client));
        let guard = LockGuard::new("test-key".to_string(), lock_service);

        // Initially not released
        assert!(!guard.released);
    }

    #[test]
    fn test_lock_operation_request_acquire() {
        let req = LockOperationRequest {
            module: "lock".to_string(),
            lock_operation: LOCK_OP_ACQUIRE.to_string(),
            lock_instance: Some(LockInstance {
                key: "my-lock".to_string(),
                expired_time: 30000,
                lock_type: "mutex".to_string(),
                params: Default::default(),
            }),
            ..Default::default()
        };

        assert_eq!(req.lock_operation, "ACQUIRE");
        assert_eq!(req.module, "lock");
        let instance = req.lock_instance.unwrap();
        assert_eq!(instance.key, "my-lock");
        assert_eq!(instance.expired_time, 30000);
        assert_eq!(instance.lock_type, "mutex");
    }

    #[test]
    fn test_lock_operation_request_release() {
        let req = LockOperationRequest {
            module: "lock".to_string(),
            lock_operation: LOCK_OP_RELEASE.to_string(),
            lock_instance: Some(LockInstance {
                key: "my-lock".to_string(),
                expired_time: 0,
                lock_type: String::new(),
                params: Default::default(),
            }),
            ..Default::default()
        };

        assert_eq!(req.lock_operation, "RELEASE");
        let instance = req.lock_instance.unwrap();
        assert_eq!(instance.key, "my-lock");
        assert_eq!(instance.expired_time, 0);
    }

    #[test]
    fn test_lock_with_custom_params() {
        let mut params = std::collections::HashMap::new();
        params.insert("owner".to_string(), "client-1".to_string());
        params.insert("priority".to_string(), "high".to_string());

        let instance = LockInstance {
            key: "resource-lock".to_string(),
            expired_time: 60000,
            lock_type: "reentrant".to_string(),
            params: params.clone(),
        };

        assert_eq!(instance.params.get("owner").unwrap(), "client-1");
        assert_eq!(instance.params.get("priority").unwrap(), "high");
        assert_eq!(instance.lock_type, "reentrant");
    }
}
