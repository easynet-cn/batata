use std::sync::atomic::{AtomicUsize, Ordering};

use reqwest::{Client, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::{debug, warn};

use crate::config::ConsulClientConfig;
use crate::error::{ConsulError, Result};
use crate::model::{QueryMeta, QueryOptions, WriteMeta, WriteOptions};

const HEADER_CONSUL_INDEX: &str = "X-Consul-Index";
const HEADER_CONSUL_KNOWN_LEADER: &str = "X-Consul-KnownLeader";
const HEADER_CONSUL_LAST_CONTACT: &str = "X-Consul-LastContact";
const HEADER_CONSUL_TOKEN: &str = "X-Consul-Token";
const HEADER_CACHE_CONTROL: &str = "X-Cache";
const HEADER_AGE: &str = "Age";

/// HTTP status codes that are retryable
const RETRYABLE_STATUS_CODES: &[u16] = &[429, 500, 502, 503, 504];

/// Core Consul HTTP client
pub struct ConsulClient {
    pub(crate) client: Client,
    pub(crate) config: ConsulClientConfig,
    /// Current server index for address rotation
    current_addr_index: AtomicUsize,
    /// Request metrics
    pub metrics: std::sync::Arc<crate::metrics::ConsulClientMetrics>,
}

impl ConsulClient {
    /// Create a new Consul client with TLS and retry support
    pub fn new(config: ConsulClientConfig) -> Result<Self> {
        let mut builder = Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.read_timeout)
            .no_proxy();

        // TLS configuration (matches Go client TLSConfig)
        if let Some(ca_cert_path) = &config.tls_ca_cert {
            let ca_bytes = std::fs::read(ca_cert_path)
                .map_err(|e| ConsulError::Other(format!("Failed to read CA cert: {}", e)))?;
            let ca_cert = reqwest::Certificate::from_pem(&ca_bytes)
                .map_err(|e| ConsulError::Other(format!("Invalid CA cert: {}", e)))?;
            builder = builder.add_root_certificate(ca_cert);
        }

        if let (Some(cert_path), Some(key_path)) = (&config.tls_client_cert, &config.tls_client_key)
        {
            let cert_bytes = std::fs::read(cert_path)
                .map_err(|e| ConsulError::Other(format!("Failed to read client cert: {}", e)))?;
            let key_bytes = std::fs::read(key_path)
                .map_err(|e| ConsulError::Other(format!("Failed to read client key: {}", e)))?;
            let mut pem = cert_bytes;
            pem.extend_from_slice(&key_bytes);
            let identity = reqwest::Identity::from_pem(&pem)
                .map_err(|e| ConsulError::Other(format!("Invalid client identity: {}", e)))?;
            builder = builder.identity(identity);
        }

        if config.tls_insecure_skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder.build().map_err(ConsulError::Http)?;

        Ok(Self {
            client,
            config,
            current_addr_index: AtomicUsize::new(0),
            metrics: std::sync::Arc::new(crate::metrics::ConsulClientMetrics::new()),
        })
    }

    /// Build the full URL for a given path using the current server address
    pub(crate) fn url(&self, path: &str) -> String {
        let addr = self.current_address();
        format!("{}{}", addr.trim_end_matches('/'), path)
    }

    /// Get the current server address (supports rotation for failover)
    fn current_address(&self) -> &str {
        let addresses = if self.config.addresses.is_empty() {
            return self.config.address.as_str();
        } else {
            &self.config.addresses
        };
        let idx = self.current_addr_index.load(Ordering::Relaxed);
        &addresses[idx % addresses.len()]
    }

    /// Switch to the next server address (called on connection failure)
    fn rotate_address(&self) {
        if self.config.addresses.len() > 1 {
            let old = self.current_addr_index.fetch_add(1, Ordering::Relaxed);
            let new_idx = (old + 1) % self.config.addresses.len();
            warn!(
                "Rotating to next Consul server: {}",
                &self.config.addresses[new_idx]
            );
        }
    }

    /// Get the effective token (request-level override > token_file > client default)
    pub(crate) fn effective_token<'a>(&'a self, override_token: &'a str) -> String {
        if !override_token.is_empty() {
            override_token.to_string()
        } else {
            self.config.effective_token()
        }
    }

    /// Check if an HTTP status code is retryable
    fn is_retryable_status(status: u16) -> bool {
        RETRYABLE_STATUS_CODES.contains(&status)
    }

    /// Apply QueryOptions to a request as query parameters.
    /// Uses &'static str for parameter keys to avoid allocations.
    pub(crate) fn apply_query_options(
        &self,
        params: &mut Vec<(&'static str, String)>,
        opts: &QueryOptions,
    ) {
        let dc = if opts.datacenter.is_empty() {
            &self.config.datacenter
        } else {
            &opts.datacenter
        };
        if !dc.is_empty() {
            params.push(("dc", dc.to_string()));
        }
        if opts.wait_index > 0 {
            params.push(("index", opts.wait_index.to_string()));
        }
        if let Some(wait) = opts.wait_time.or(self.config.wait_time) {
            let secs = wait.as_secs();
            if secs > 0 {
                params.push(("wait", format!("{secs}s")));
            }
        }
        if !opts.filter.is_empty() {
            params.push(("filter", opts.filter.clone()));
        }
        let ns = if opts.namespace.is_empty() {
            &self.config.namespace
        } else {
            &opts.namespace
        };
        if !ns.is_empty() {
            params.push(("ns", ns.to_string()));
        }
        let partition = if opts.partition.is_empty() {
            &self.config.partition
        } else {
            &opts.partition
        };
        if !partition.is_empty() {
            params.push(("partition", partition.to_string()));
        }
        if opts.require_consistent {
            params.push(("consistent", String::new()));
        } else if opts.allow_stale {
            params.push(("stale", String::new()));
        }
        if !opts.near.is_empty() {
            params.push(("near", opts.near.clone()));
        }
    }

    /// Apply WriteOptions as query parameters.
    /// Uses &'static str for parameter keys to avoid allocations.
    pub(crate) fn apply_write_options(
        &self,
        params: &mut Vec<(&'static str, String)>,
        opts: &WriteOptions,
    ) {
        let dc = if opts.datacenter.is_empty() {
            &self.config.datacenter
        } else {
            &opts.datacenter
        };
        if !dc.is_empty() {
            params.push(("dc", dc.to_string()));
        }
        let ns = if opts.namespace.is_empty() {
            &self.config.namespace
        } else {
            &opts.namespace
        };
        if !ns.is_empty() {
            params.push(("ns", ns.to_string()));
        }
        let partition = if opts.partition.is_empty() {
            &self.config.partition
        } else {
            &opts.partition
        };
        if !partition.is_empty() {
            params.push(("partition", partition.to_string()));
        }
    }

    /// Parse response headers into QueryMeta
    pub(crate) fn parse_query_meta(response: &Response) -> QueryMeta {
        let headers = response.headers();
        let last_index = headers
            .get(HEADER_CONSUL_INDEX)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let last_contact = headers
            .get(HEADER_CONSUL_LAST_CONTACT)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let known_leader = headers
            .get(HEADER_CONSUL_KNOWN_LEADER)
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "true")
            .unwrap_or(false);
        let cache_hit = headers
            .get(HEADER_CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "HIT")
            .unwrap_or(false);
        let cache_age = headers
            .get(HEADER_AGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        QueryMeta {
            last_index,
            last_contact,
            known_leader,
            cache_hit,
            cache_age,
        }
    }

    // --- Public request methods ---

    /// GET request returning deserialized JSON and QueryMeta.
    /// Includes automatic retry with exponential backoff for transient errors,
    /// and server address rotation on connection failure.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        opts: &QueryOptions,
    ) -> Result<(T, QueryMeta)> {
        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);
        let token = self.effective_token(&opts.token);

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            let url = self.url(path);
            debug!("GET {} (attempt {})", url, attempt);

            let mut req = self.client.get(&url);
            if !token.is_empty() {
                req = req.header(HEADER_CONSUL_TOKEN, &token);
            }
            if !params.is_empty() {
                req = req.query(&params);
            }

            match req.send().await {
                Ok(response) => {
                    let meta = Self::parse_query_meta(&response);

                    if response.status() == StatusCode::NOT_FOUND {
                        self.metrics.record_failure();
                        return Err(ConsulError::NotFound);
                    }
                    if !response.status().is_success() {
                        let status = response.status().as_u16();
                        if Self::is_retryable_status(status) && attempt < self.config.max_retries {
                            self.metrics.record_retry();
                            let delay = self.retry_delay(attempt);
                            warn!("Retryable error ({}), retrying in {:?}", status, delay);
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        self.metrics.record_failure();
                        let body = response.text().await.unwrap_or_default();
                        return Err(ConsulError::Api {
                            status,
                            message: body,
                        });
                    }

                    let body = response.json::<T>().await.map_err(ConsulError::Http)?;
                    self.metrics.record_success();
                    return Ok((body, meta));
                }
                Err(e) => {
                    if attempt < self.config.max_retries {
                        warn!("Request failed: {}, rotating server", e);
                        self.rotate_address();
                        self.metrics.record_rotation();
                        self.metrics.record_retry();
                        let delay = self.retry_delay(attempt);
                        tokio::time::sleep(delay).await;
                        last_error = Some(ConsulError::Http(e));
                        continue;
                    }
                    self.metrics.record_failure();
                    return Err(ConsulError::Http(e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ConsulError::Other("All retries exhausted".to_string())))
    }

    /// Calculate retry delay with exponential backoff and jitter
    fn retry_delay(&self, attempt: u32) -> std::time::Duration {
        let base = self.config.retry_backoff_base_ms;
        let max = self.config.retry_backoff_max_ms;
        let delay_ms = std::cmp::min(base * 2u64.pow(attempt), max);
        // Add jitter (±25%)
        let jitter = delay_ms / 4;
        let actual = delay_ms + (rand_u64() % (jitter * 2 + 1)).saturating_sub(jitter);
        std::time::Duration::from_millis(actual)
    }

    /// GET request returning optional result (None on 404)
    pub async fn get_optional<T: DeserializeOwned>(
        &self,
        path: &str,
        opts: &QueryOptions,
    ) -> Result<(Option<T>, QueryMeta)> {
        match self.get::<T>(path, opts).await {
            Ok((body, meta)) => Ok((Some(body), meta)),
            Err(ConsulError::NotFound) => Ok((None, QueryMeta::default())),
            Err(e) => Err(e),
        }
    }

    /// GET request returning raw string body and QueryMeta
    pub async fn get_raw(&self, path: &str, opts: &QueryOptions) -> Result<(String, QueryMeta)> {
        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        let mut req = self.client.get(&url);
        if !token.is_empty() {
            req = req.header(HEADER_CONSUL_TOKEN, token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;
        let meta = Self::parse_query_meta(&response);

        if response.status() == StatusCode::NOT_FOUND {
            return Err(ConsulError::NotFound);
        }
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        let body = response.text().await.map_err(ConsulError::Http)?;
        Ok((body, meta))
    }

    /// PUT request with JSON body, returning deserialized response and WriteMeta
    pub async fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: Option<&B>,
        opts: &WriteOptions,
        extra_params: &[(&'static str, String)],
    ) -> Result<(T, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend(extra_params.iter().map(|(k, v)| (*k, v.clone())));

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        debug!("PUT {}", url);

        let mut req = self.client.put(&url);
        if !token.is_empty() {
            req = req.header(HEADER_CONSUL_TOKEN, token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }
        if let Some(b) = body {
            req = req.json(b);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;

        if response.status() == StatusCode::NOT_FOUND {
            return Err(ConsulError::NotFound);
        }
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        let result = response.json::<T>().await.map_err(ConsulError::Http)?;
        let meta = WriteMeta {
            request_time: start.elapsed(),
        };
        Ok((result, meta))
    }

    /// PUT request with raw bytes body
    pub async fn put_bytes(
        &self,
        path: &str,
        body: Vec<u8>,
        opts: &WriteOptions,
        extra_params: &[(&'static str, String)],
    ) -> Result<(bool, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend(extra_params.iter().map(|(k, v)| (*k, v.clone())));

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        let mut req = self.client.put(&url).body(body);
        if !token.is_empty() {
            req = req.header(HEADER_CONSUL_TOKEN, token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        let text = response.text().await.map_err(ConsulError::Http)?;
        let result = text.trim() == "true";
        let meta = WriteMeta {
            request_time: start.elapsed(),
        };
        Ok((result, meta))
    }

    /// PUT request without response body (returns status only)
    pub async fn put_no_response(
        &self,
        path: &str,
        opts: &WriteOptions,
        extra_params: &[(&'static str, String)],
    ) -> Result<WriteMeta> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend(extra_params.iter().map(|(k, v)| (*k, v.clone())));

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        let mut req = self.client.put(&url);
        if !token.is_empty() {
            req = req.header(HEADER_CONSUL_TOKEN, token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        Ok(WriteMeta {
            request_time: start.elapsed(),
        })
    }

    /// DELETE request
    pub async fn delete(
        &self,
        path: &str,
        opts: &WriteOptions,
        extra_params: &[(&'static str, String)],
    ) -> Result<(bool, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend(extra_params.iter().map(|(k, v)| (*k, v.clone())));

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        debug!("DELETE {}", url);

        let mut req = self.client.delete(&url);
        if !token.is_empty() {
            req = req.header(HEADER_CONSUL_TOKEN, token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        let text = response.text().await.map_err(ConsulError::Http)?;
        let result = text.trim() == "true";
        let meta = WriteMeta {
            request_time: start.elapsed(),
        };
        Ok((result, meta))
    }

    /// POST request with JSON body and deserialized response
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: Option<&B>,
        opts: &WriteOptions,
        extra_params: &[(&'static str, String)],
    ) -> Result<(T, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend(extra_params.iter().map(|(k, v)| (*k, v.clone())));

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        debug!("POST {}", url);

        let mut req = self.client.post(&url);
        if !token.is_empty() {
            req = req.header(HEADER_CONSUL_TOKEN, token);
        }
        if !params.is_empty() {
            req = req.query(&params);
        }
        if let Some(b) = body {
            req = req.json(b);
        }

        let response = req.send().await.map_err(ConsulError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ConsulError::Api {
                status,
                message: body,
            });
        }

        let result = response.json::<T>().await.map_err(ConsulError::Http)?;
        let meta = WriteMeta {
            request_time: start.elapsed(),
        };
        Ok((result, meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_building() {
        let config = ConsulClientConfig::new("http://localhost:8500");
        let client = ConsulClient::new(config).unwrap();
        assert_eq!(
            client.url("/v1/kv/test"),
            "http://localhost:8500/v1/kv/test"
        );
    }

    #[test]
    fn test_url_trailing_slash() {
        let config = ConsulClientConfig::new("http://localhost:8500/");
        let client = ConsulClient::new(config).unwrap();
        assert_eq!(
            client.url("/v1/kv/test"),
            "http://localhost:8500/v1/kv/test"
        );
    }

    #[test]
    fn test_effective_token() {
        let config = ConsulClientConfig::new("http://localhost:8500").with_token("default-token");
        let client = ConsulClient::new(config).unwrap();

        assert_eq!(client.effective_token(""), "default-token");
        assert_eq!(client.effective_token("override"), "override");
    }

    #[test]
    fn test_apply_query_options() {
        let config = ConsulClientConfig::new("http://localhost:8500").with_datacenter("dc1");
        let client = ConsulClient::new(config).unwrap();

        let opts = QueryOptions {
            wait_index: 42,
            filter: "Service.Tags contains \"v2\"".to_string(),
            ..Default::default()
        };

        let mut params = Vec::new();
        client.apply_query_options(&mut params, &opts);

        assert!(params.iter().any(|(k, v)| *k == "dc" && v == "dc1"));
        assert!(params.iter().any(|(k, v)| *k == "index" && v == "42"));
        assert!(params.iter().any(|(k, _)| *k == "filter"));
    }
}

/// Simple pseudo-random u64 based on time (no external dependency needed)
fn rand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    // xorshift64
    let mut x = seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}
