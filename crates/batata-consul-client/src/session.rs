use crate::client::ConsulClient;
use crate::error::Result;
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
}
