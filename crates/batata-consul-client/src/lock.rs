//! Lock and Semaphore API for distributed coordination

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{WriteMeta, WriteOptions};

/// Lock and Semaphore operations
impl ConsulClient {
    /// Acquire a distributed lock via KV + Session
    ///
    /// This implements the Consul lock pattern:
    /// 1. Create a session
    /// 2. Acquire the KV key with that session
    pub async fn lock_acquire(
        &self,
        key: &str,
        session: &str,
        value: Option<&[u8]>,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let extra = vec![("acquire".to_string(), session.to_string())];
        let val = value.unwrap_or(b"").to_vec();
        self.put_bytes(&format!("/v1/kv/{}", key), val, opts, &extra)
            .await
    }

    /// Release a distributed lock
    pub async fn lock_release(
        &self,
        key: &str,
        session: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let extra = vec![("release".to_string(), session.to_string())];
        self.put_bytes(&format!("/v1/kv/{}", key), vec![], opts, &extra)
            .await
    }

    /// Acquire a semaphore slot
    pub async fn semaphore_acquire(
        &self,
        prefix: &str,
        session: &str,
        limit: u32,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let contender_key = format!("{}/{}", prefix, session);
        let body = serde_json::json!({"Limit": limit, "Session": session}).to_string();
        let extra = vec![("acquire".to_string(), session.to_string())];
        self.put_bytes(
            &format!("/v1/kv/{}", contender_key),
            body.into_bytes(),
            opts,
            &extra,
        )
        .await
    }

    /// Release a semaphore slot
    pub async fn semaphore_release(
        &self,
        prefix: &str,
        session: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let contender_key = format!("{}/{}", prefix, session);
        let extra = vec![("release".to_string(), session.to_string())];
        self.put_bytes(&format!("/v1/kv/{}", contender_key), vec![], opts, &extra)
            .await
    }
}
