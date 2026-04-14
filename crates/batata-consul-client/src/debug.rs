//! Debug / pprof endpoints.
//!
//! Maps to Consul Go SDK's `api/debug.go`. All endpoints live under
//! `/debug/pprof/*` (note: not under `/v1`).

use crate::client::ConsulClient;
use crate::error::{ConsulError, Result};

impl ConsulClient {
    /// Collect a heap profile (pprof format) as raw bytes.
    ///
    /// Consul equivalent: `Debug.Heap()`.
    pub async fn debug_heap(&self) -> Result<Vec<u8>> {
        self.debug_pprof_bytes("heap", None).await
    }

    /// Collect a CPU profile of the specified duration (seconds).
    ///
    /// Consul equivalent: `Debug.Profile(seconds)`.
    pub async fn debug_profile(&self, seconds: u32) -> Result<Vec<u8>> {
        self.debug_pprof_bytes("profile", Some(seconds)).await
    }

    /// Collect an execution trace of the specified duration (seconds).
    pub async fn debug_trace(&self, seconds: u32) -> Result<Vec<u8>> {
        self.debug_pprof_bytes("trace", Some(seconds)).await
    }

    /// Dump all goroutines (pprof format).
    pub async fn debug_goroutine(&self) -> Result<Vec<u8>> {
        self.debug_pprof_bytes("goroutine", None).await
    }

    /// Fetch an arbitrary pprof profile by name with optional `seconds`
    /// query parameter. Accepts any profile name Consul supports
    /// (e.g., "allocs", "block", "mutex").
    pub async fn debug_pprof_bytes(
        &self,
        name: &str,
        seconds: Option<u32>,
    ) -> Result<Vec<u8>> {
        let path = format!("/debug/pprof/{}", name);
        let url = self.url(&path);
        let mut req = self.client.get(&url);
        if let Some(s) = seconds {
            req = req.query(&[("seconds", s.to_string())]);
        }
        let token = self.effective_token("");
        if !token.is_empty() {
            req = req.header("X-Consul-Token", token);
        }
        let resp = req.send().await.map_err(ConsulError::Http)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ConsulError::Api { status, message: body });
        }
        let bytes = resp.bytes().await.map_err(ConsulError::Http)?;
        Ok(bytes.to_vec())
    }
}
