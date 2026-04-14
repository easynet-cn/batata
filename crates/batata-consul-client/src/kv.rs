use crate::client::ConsulClient;
use crate::error::{ConsulError, Result};
use crate::model::{KVPair, QueryMeta, QueryOptions, WriteMeta, WriteOptions};

/// KV store operations
impl ConsulClient {
    /// Get a single KV entry by key
    pub async fn kv_get(
        &self,
        key: &str,
        opts: &QueryOptions,
    ) -> Result<(Option<KVPair>, QueryMeta)> {
        let path = format!("/v1/kv/{}", key);
        match self.get::<Vec<KVPair>>(&path, opts).await {
            Ok((pairs, meta)) => Ok((pairs.into_iter().next(), meta)),
            Err(ConsulError::NotFound) => Ok((None, QueryMeta::default())),
            Err(e) => Err(e),
        }
    }

    /// List all KV entries under a prefix
    pub async fn kv_list(
        &self,
        prefix: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<KVPair>, QueryMeta)> {
        let path = format!("/v1/kv/{}", prefix);
        match self
            .get_with_extra(&path, opts, &[("recurse", String::new())])
            .await
        {
            Ok((pairs, meta)) => Ok((pairs, meta)),
            Err(ConsulError::NotFound) => Ok((Vec::new(), QueryMeta::default())),
            Err(e) => Err(e),
        }
    }

    /// Get just the keys under a prefix
    pub async fn kv_keys(
        &self,
        prefix: &str,
        separator: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<String>, QueryMeta)> {
        let path = format!("/v1/kv/{}", prefix);
        let mut extra = vec![("keys", String::new())];
        if !separator.is_empty() {
            extra.push(("separator", separator.to_string()));
        }
        match self
            .get_with_extra::<Vec<String>>(&path, opts, &extra)
            .await
        {
            Ok((keys, meta)) => Ok((keys, meta)),
            Err(ConsulError::NotFound) => Ok((Vec::new(), QueryMeta::default())),
            Err(e) => Err(e),
        }
    }

    /// Put a KV entry. Returns true on success.
    pub async fn kv_put(&self, pair: &KVPair, opts: &WriteOptions) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", pair.key);
        let mut extra = Vec::new();
        if pair.flags > 0 {
            extra.push(("flags", pair.flags.to_string()));
        }
        let value = pair.value_bytes().unwrap_or_default();
        self.put_bytes(&path, value, opts, &extra).await
    }

    /// Put a KV entry with CAS (Check-And-Set). Returns true if the CAS succeeded.
    pub async fn kv_cas(&self, pair: &KVPair, opts: &WriteOptions) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", pair.key);
        let mut extra = vec![("cas", pair.modify_index.to_string())];
        if pair.flags > 0 {
            extra.push(("flags", pair.flags.to_string()));
        }
        let value = pair.value_bytes().unwrap_or_default();
        self.put_bytes(&path, value, opts, &extra).await
    }

    /// Acquire a lock on a key with a session
    pub async fn kv_acquire(
        &self,
        pair: &KVPair,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", pair.key);
        let session = pair.session.as_deref().unwrap_or("");
        let mut extra = vec![("acquire", session.to_string())];
        if pair.flags > 0 {
            extra.push(("flags", pair.flags.to_string()));
        }
        let value = pair.value_bytes().unwrap_or_default();
        self.put_bytes(&path, value, opts, &extra).await
    }

    /// Release a lock on a key
    pub async fn kv_release(
        &self,
        pair: &KVPair,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", pair.key);
        let session = pair.session.as_deref().unwrap_or("");
        let mut extra = vec![("release", session.to_string())];
        if pair.flags > 0 {
            extra.push(("flags", pair.flags.to_string()));
        }
        let value = pair.value_bytes().unwrap_or_default();
        self.put_bytes(&path, value, opts, &extra).await
    }

    /// Delete a single key
    pub async fn kv_delete(&self, key: &str, opts: &WriteOptions) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", key);
        self.delete(&path, opts, &[]).await
    }

    /// Delete all keys under a prefix
    pub async fn kv_delete_tree(
        &self,
        prefix: &str,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", prefix);
        self.delete(&path, opts, &[("recurse", String::new())])
            .await
    }

    /// Delete a key with CAS
    pub async fn kv_delete_cas(
        &self,
        key: &str,
        modify_index: u64,
        opts: &WriteOptions,
    ) -> Result<(bool, WriteMeta)> {
        let path = format!("/v1/kv/{}", key);
        self.delete(&path, opts, &[("cas", modify_index.to_string())])
            .await
    }
}

// Helper: GET with extra query params beyond QueryOptions
impl ConsulClient {
    pub async fn get_with_extra<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        opts: &QueryOptions,
        extra_params: &[(&'static str, String)],
    ) -> Result<(T, QueryMeta)> {
        use reqwest::StatusCode;

        let mut params = Vec::new();
        self.apply_query_options(&mut params, opts);
        params.extend(extra_params.iter().map(|(k, v)| (*k, v.clone())));

        let token = self.effective_token(&opts.token);
        let url = self.url(path);

        let mut req = self.client.get(&url);
        if !token.is_empty() {
            req = req.header("X-Consul-Token", token);
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
}
