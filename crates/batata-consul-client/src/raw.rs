//! Raw HTTP escape hatch.
//!
//! Mirrors Consul Go SDK's `api/raw.go`. Use when you need to call a Consul
//! endpoint that the strongly-typed helpers don't wrap — for example, new
//! endpoints added by later Consul versions or custom plugin endpoints.
//!
//! Three entry points:
//! - [`ConsulClient::raw_query`]   → `GET`
//! - [`ConsulClient::raw_write`]   → `PUT`
//! - [`ConsulClient::raw_delete`]  → `DELETE`

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions, WriteMeta, WriteOptions};

impl ConsulClient {
    /// Issue a raw `GET` request against `endpoint` (e.g., `/v1/some/custom`)
    /// and deserialize the JSON response into `T`.
    ///
    /// Respects QueryOptions (datacenter, token, blocking, consistency).
    pub async fn raw_query<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        opts: &QueryOptions,
    ) -> Result<(T, QueryMeta)> {
        self.get(endpoint, opts).await
    }

    /// Issue a raw `PUT` request with `body` as JSON payload.
    pub async fn raw_write<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: Option<&B>,
        opts: &WriteOptions,
    ) -> Result<(T, WriteMeta)> {
        self.put(endpoint, body, opts, &[]).await
    }

    /// Issue a raw `DELETE` request.
    pub async fn raw_delete(
        &self,
        endpoint: &str,
        opts: &WriteOptions,
    ) -> Result<WriteMeta> {
        let (_ok, meta) = self.delete(endpoint, opts, &[]).await?;
        Ok(meta)
    }
}
