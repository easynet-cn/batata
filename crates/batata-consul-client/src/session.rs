use std::sync::Arc;

use tokio::sync::watch;

use crate::client::ConsulClient;
use crate::error::{ConsulError, Result};
use crate::model::{QueryMeta, QueryOptions, SessionEntry, WriteMeta, WriteOptions};

/// Session API operations
impl ConsulClient {
    /// Create a new session, returns the session ID
    pub async fn session_create(
        &self,
        entry: &SessionEntry,
        opts: &WriteOptions,
    ) -> Result<(String, WriteMeta)> {
        let (resp, meta): (serde_json::Value, WriteMeta) = self
            .put("/v1/session/create", Some(entry), opts, &[])
            .await?;

        let id = resp
            .get("ID")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok((id, meta))
    }

    /// Destroy a session
    pub async fn session_destroy(&self, id: &str, opts: &WriteOptions) -> Result<WriteMeta> {
        let path = format!("/v1/session/destroy/{}", id);
        self.put_no_response(&path, opts, &[]).await
    }

    /// Renew a session
    pub async fn session_renew(
        &self,
        id: &str,
        opts: &WriteOptions,
    ) -> Result<(Vec<SessionEntry>, WriteMeta)> {
        let path = format!("/v1/session/renew/{}", id);
        self.put::<Vec<SessionEntry>, ()>(&path, None, opts, &[])
            .await
    }

    /// Get info about a specific session
    pub async fn session_info(
        &self,
        id: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<SessionEntry>, QueryMeta)> {
        let path = format!("/v1/session/info/{}", id);
        self.get(&path, opts).await
    }

    /// List sessions for a node
    pub async fn session_node(
        &self,
        node: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<SessionEntry>, QueryMeta)> {
        let path = format!("/v1/session/node/{}", node);
        self.get(&path, opts).await
    }

    /// List all active sessions
    pub async fn session_list(
        &self,
        opts: &QueryOptions,
    ) -> Result<(Vec<SessionEntry>, QueryMeta)> {
        self.get("/v1/session/list", opts).await
    }

    /// Create a new session with no associated health checks.
    /// This is equivalent to creating a session with an empty NodeChecks list,
    /// which prevents automatic invalidation when node checks fail.
    pub async fn session_create_no_checks(
        &self,
        entry: &SessionEntry,
        opts: &WriteOptions,
    ) -> Result<(String, WriteMeta)> {
        let mut body = serde_json::Map::new();
        body.insert("NodeChecks".to_string(), serde_json::Value::Array(vec![]));
        if let Some(ref name) = entry.name
            && !name.is_empty()
        {
            body.insert("Name".to_string(), serde_json::json!(name));
        }
        if let Some(ref node) = entry.node
            && !node.is_empty()
        {
            body.insert("Node".to_string(), serde_json::json!(node));
        }
        if let Some(lock_delay) = entry.lock_delay
            && lock_delay > 0
        {
            body.insert(
                "LockDelay".to_string(),
                serde_json::json!(format!("{}ms", lock_delay)),
            );
        }
        if let Some(ref behavior) = entry.behavior
            && !behavior.is_empty()
        {
            body.insert("Behavior".to_string(), serde_json::json!(behavior));
        }
        if let Some(ref ttl) = entry.ttl
            && !ttl.is_empty()
        {
            body.insert("TTL".to_string(), serde_json::json!(ttl));
        }

        let json_body = serde_json::Value::Object(body);
        let (resp, meta): (serde_json::Value, WriteMeta) = self
            .put("/v1/session/create", Some(&json_body), opts, &[])
            .await?;

        let id = resp
            .get("ID")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok((id, meta))
    }

    /// Periodically renew a session until the done receiver signals or the session expires.
    ///
    /// This spawns a background tokio task that renews the session at TTL/2 intervals.
    /// When the `done_rx` receiver gets a value (or is dropped), the session is destroyed
    /// and the task exits. Returns a `JoinHandle` that resolves to the final result.
    ///
    /// Follows the Go client's `RenewPeriodic` semantics:
    /// - Renews at TTL/2 intervals
    /// - On error, retries after 1 second
    /// - If total time since last successful renewal exceeds TTL, returns error
    /// - If session no longer exists (empty renewal response), returns SessionExpired error
    pub fn session_renew_periodic(
        self: &Arc<Self>,
        initial_ttl: std::time::Duration,
        session_id: String,
        opts: WriteOptions,
        mut done_rx: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let mut ttl = initial_ttl;
            let mut wait_dur = ttl / 2;
            let mut last_renew_time = std::time::Instant::now();
            let mut last_err: Option<ConsulError> = None;

            loop {
                if last_renew_time.elapsed() > ttl {
                    return Err(last_err
                        .unwrap_or_else(|| ConsulError::Other("session expired".to_string())));
                }

                tokio::select! {
                    _ = tokio::time::sleep(wait_dur) => {
                        match client.session_renew(&session_id, &opts).await {
                            Ok((entries, _)) => {
                                if entries.is_empty() {
                                    return Err(ConsulError::Other("session expired".to_string()));
                                }
                                // Update TTL from server response
                                if let Some(ref entry_ttl) = entries[0].ttl
                                    && let Some(parsed) = parse_consul_duration(entry_ttl)
                                {
                                    ttl = parsed;
                                    wait_dur = ttl / 2;
                                }
                                last_renew_time = std::time::Instant::now();
                                last_err = None;
                            }
                            Err(e) => {
                                wait_dur = std::time::Duration::from_secs(1);
                                last_err = Some(e);
                            }
                        }
                    }
                    _ = done_rx.changed() => {
                        // Attempt to destroy the session on graceful stop
                        let _ = client.session_destroy(&session_id, &opts).await;
                        return Ok(());
                    }
                }
            }
        })
    }
}

/// Parse a Consul TTL duration string like "15s", "500ms", "10m" into a Duration
fn parse_consul_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        if let Some(ms) = secs.strip_suffix('m') {
            // "500ms" case
            ms.parse::<u64>().ok().map(std::time::Duration::from_millis)
        } else {
            secs.parse::<u64>().ok().map(std::time::Duration::from_secs)
        }
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.parse::<u64>()
            .ok()
            .map(|m| std::time::Duration::from_secs(m * 60))
    } else if let Some(hours) = s.strip_suffix('h') {
        hours
            .parse::<u64>()
            .ok()
            .map(|h| std::time::Duration::from_secs(h * 3600))
    } else {
        // Try parsing as seconds
        s.parse::<u64>().ok().map(std::time::Duration::from_secs)
    }
}
