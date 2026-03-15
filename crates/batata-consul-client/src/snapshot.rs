//! Snapshot API for backup and restore

use crate::client::ConsulClient;
use crate::error::{ConsulError, Result};
use crate::model::{QueryMeta, QueryOptions, WriteMeta, WriteOptions};

/// Snapshot operations
impl ConsulClient {
    /// Save a snapshot (returns raw bytes)
    pub async fn snapshot_save(&self, opts: &QueryOptions) -> Result<(Vec<u8>, QueryMeta)> {
        let url = self.url("/v1/snapshot");
        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);

        let token = self.effective_token(&opts.token);

        let mut req = self.client.get(&url);
        if !token.is_empty() {
            req = req.header("X-Consul-Token", token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let resp = req.send().await.map_err(ConsulError::Http)?;
        let meta = Self::parse_query_meta(&resp);

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        let bytes = resp.bytes().await.map_err(ConsulError::Http)?;
        Ok((bytes.to_vec(), meta))
    }

    /// Restore a snapshot from raw bytes
    pub async fn snapshot_restore(&self, data: Vec<u8>, opts: &WriteOptions) -> Result<WriteMeta> {
        let start = std::time::Instant::now();
        let url = self.url("/v1/snapshot");
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);

        let token = self.effective_token(&opts.token);

        let mut req = self.client.put(&url).body(data);
        if !token.is_empty() {
            req = req.header("X-Consul-Token", token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let resp = req.send().await.map_err(ConsulError::Http)?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        Ok(WriteMeta {
            request_time: start.elapsed(),
        })
    }
}
