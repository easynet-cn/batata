use reqwest::{Client, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::debug;

use crate::config::ConsulClientConfig;
use crate::error::{ConsulError, Result};
use crate::model::{QueryMeta, QueryOptions, WriteMeta, WriteOptions};

const HEADER_CONSUL_INDEX: &str = "X-Consul-Index";
const HEADER_CONSUL_KNOWN_LEADER: &str = "X-Consul-KnownLeader";
const HEADER_CONSUL_LAST_CONTACT: &str = "X-Consul-LastContact";
const HEADER_CONSUL_TOKEN: &str = "X-Consul-Token";
const HEADER_CACHE_CONTROL: &str = "X-Cache";
const HEADER_AGE: &str = "Age";

/// Core Consul HTTP client
pub struct ConsulClient {
    pub(crate) client: Client,
    pub(crate) config: ConsulClientConfig,
}

impl ConsulClient {
    /// Create a new Consul client
    pub fn new(config: ConsulClientConfig) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.read_timeout)
            .no_proxy()
            .build()
            .map_err(ConsulError::Http)?;

        Ok(Self { client, config })
    }

    /// Build the full URL for a given path
    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.address.trim_end_matches('/'), path)
    }

    /// Get the effective token (request-level override or client default)
    pub(crate) fn effective_token<'a>(&'a self, override_token: &'a str) -> &'a str {
        if override_token.is_empty() {
            &self.config.token
        } else {
            override_token
        }
    }

    /// Apply QueryOptions to a request as query parameters
    pub(crate) fn apply_query_options(
        &self,
        params: &mut Vec<(String, String)>,
        opts: &QueryOptions,
    ) {
        let dc = if opts.datacenter.is_empty() {
            &self.config.datacenter
        } else {
            &opts.datacenter
        };
        if !dc.is_empty() {
            params.push(("dc".to_string(), dc.to_string()));
        }
        if opts.wait_index > 0 {
            params.push(("index".to_string(), opts.wait_index.to_string()));
        }
        if let Some(wait) = opts.wait_time.or(self.config.wait_time) {
            let secs = wait.as_secs();
            if secs > 0 {
                params.push(("wait".to_string(), format!("{secs}s")));
            }
        }
        if !opts.filter.is_empty() {
            params.push(("filter".to_string(), opts.filter.clone()));
        }
        let ns = if opts.namespace.is_empty() {
            &self.config.namespace
        } else {
            &opts.namespace
        };
        if !ns.is_empty() {
            params.push(("ns".to_string(), ns.to_string()));
        }
        let partition = if opts.partition.is_empty() {
            &self.config.partition
        } else {
            &opts.partition
        };
        if !partition.is_empty() {
            params.push(("partition".to_string(), partition.to_string()));
        }
        if opts.require_consistent {
            params.push(("consistent".to_string(), String::new()));
        } else if opts.allow_stale {
            params.push(("stale".to_string(), String::new()));
        }
        if !opts.near.is_empty() {
            params.push(("near".to_string(), opts.near.clone()));
        }
    }

    /// Apply WriteOptions as query parameters
    pub(crate) fn apply_write_options(
        &self,
        params: &mut Vec<(String, String)>,
        opts: &WriteOptions,
    ) {
        let dc = if opts.datacenter.is_empty() {
            &self.config.datacenter
        } else {
            &opts.datacenter
        };
        if !dc.is_empty() {
            params.push(("dc".to_string(), dc.to_string()));
        }
        let ns = if opts.namespace.is_empty() {
            &self.config.namespace
        } else {
            &opts.namespace
        };
        if !ns.is_empty() {
            params.push(("ns".to_string(), ns.to_string()));
        }
        let partition = if opts.partition.is_empty() {
            &self.config.partition
        } else {
            &opts.partition
        };
        if !partition.is_empty() {
            params.push(("partition".to_string(), partition.to_string()));
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

    /// GET request returning deserialized JSON and QueryMeta
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        opts: &QueryOptions,
    ) -> Result<(T, QueryMeta)> {
        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        debug!("GET {}", url);

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

        let body = response.json::<T>().await.map_err(ConsulError::Http)?;
        Ok((body, meta))
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
        extra_params: &[(String, String)],
    ) -> Result<(T, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend_from_slice(extra_params);

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
        extra_params: &[(String, String)],
    ) -> Result<(bool, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend_from_slice(extra_params);

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
        extra_params: &[(String, String)],
    ) -> Result<WriteMeta> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend_from_slice(extra_params);

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
        extra_params: &[(String, String)],
    ) -> Result<(bool, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend_from_slice(extra_params);

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
        extra_params: &[(String, String)],
    ) -> Result<(T, WriteMeta)> {
        let start = std::time::Instant::now();
        let mut params = Vec::new();
        self.apply_write_options(&mut params, opts);
        params.extend_from_slice(extra_params);

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

        assert!(params.iter().any(|(k, v)| k == "dc" && v == "dc1"));
        assert!(params.iter().any(|(k, v)| k == "index" && v == "42"));
        assert!(params.iter().any(|(k, _)| k == "filter"));
    }
}
