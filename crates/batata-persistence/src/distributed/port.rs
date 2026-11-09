//! Narrow write port between the distributed storage backend and Raft.
//!
//! `batata-persistence` still depends on `batata-consistency` for the wire
//! types (`RaftRequest`/`RaftResponse`), the column-family constants, the key
//! builders and the bincode record types. This port does **not** remove that
//! crate dependency; see `docs/architecture.md` ("Architecture Decision
//! Records"). What it does is stop the storage backend from naming the concrete
//! `RaftNode`, so the write path can be injected and exercised with a mock.

use std::sync::Arc;

use async_trait::async_trait;
use batata_consistency::raft::node::RaftNode;
use batata_consistency::raft::request::{RaftRequest, RaftResponse};

/// Transport error returned by a Raft proposal.
pub type RaftWriteError = Box<dyn std::error::Error + Send + Sync>;

/// The consensus surface the distributed persistence backend needs.
///
/// Deliberately small: config/auth/AI writes are proposals and the health
/// check only needs to know whether a leader is serving. Reads go through
/// `RocksDbReader` directly and are not part of this port.
#[async_trait]
pub trait RaftStatePort: Send + Sync {
    /// Propose a write and return the state machine's response.
    async fn write(&self, request: RaftRequest) -> Result<RaftResponse, RaftWriteError>;

    /// Whether the group currently has an elected leader.
    fn has_leader(&self) -> bool;
}

/// Adapter exposing a concrete [`RaftNode`] through [`RaftStatePort`].
pub struct RaftNodePort(pub Arc<RaftNode>);

impl RaftNodePort {
    /// Wrap a concrete Raft node for injection into the persistence backend.
    pub fn new(node: Arc<RaftNode>) -> Self {
        Self(node)
    }
}

#[async_trait]
impl RaftStatePort for RaftNodePort {
    async fn write(&self, request: RaftRequest) -> Result<RaftResponse, RaftWriteError> {
        self.0.write(request).await
    }

    fn has_leader(&self) -> bool {
        self.0.leader_id().is_some()
    }
}

/// Classify a proposal outcome for the write helpers.
///
/// A transport error and a rejected proposal are both failures; only a
/// successful `RaftResponse` may be handed back to callers. Kept pure so the
/// fail-closed behaviour is directly testable.
pub fn classify_raft_write(
    outcome: Result<RaftResponse, RaftWriteError>,
) -> anyhow::Result<RaftResponse> {
    let response = outcome.map_err(|e| anyhow::anyhow!("Raft write failed: {}", e))?;
    if response.success {
        Ok(response)
    } else {
        Err(anyhow::anyhow!(
            "Raft write rejected: {}",
            response.message.unwrap_or_default()
        ))
    }
}

/// Turn leader availability into the persistence health result.
pub fn health_from_leader(has_leader: bool) -> anyhow::Result<()> {
    if has_leader {
        Ok(())
    } else {
        Err(anyhow::anyhow!("No Raft leader elected"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPort {
        response: Option<RaftResponse>,
        transport_error: Option<String>,
        leader: bool,
    }

    #[async_trait]
    impl RaftStatePort for MockPort {
        async fn write(&self, _request: RaftRequest) -> Result<RaftResponse, RaftWriteError> {
            if let Some(message) = &self.transport_error {
                return Err(std::io::Error::other(message.clone()).into());
            }
            Ok(self.response.clone().unwrap_or_default())
        }

        fn has_leader(&self) -> bool {
            self.leader
        }
    }

    fn success() -> RaftResponse {
        RaftResponse {
            success: true,
            data: Some(vec![1, 2, 3]),
            ..Default::default()
        }
    }

    #[test]
    fn classify_keeps_successful_response_data() {
        let response = classify_raft_write(Ok(success())).expect("success must pass");
        assert_eq!(response.data, Some(vec![1, 2, 3]));
    }

    #[test]
    fn classify_rejects_failed_proposal() {
        let rejected = RaftResponse {
            success: false,
            message: Some("conflict".to_string()),
            ..Default::default()
        };
        let err = classify_raft_write(Ok(rejected)).expect_err("failure must be fail-closed");
        assert!(err.to_string().contains("Raft write rejected: conflict"));
    }

    #[test]
    fn classify_rejects_transport_error() {
        let err = classify_raft_write(Err(std::io::Error::other("no leader").into()))
            .expect_err("transport error must be fail-closed");
        assert!(err.to_string().contains("Raft write failed: no leader"));
    }

    #[test]
    fn health_follows_leader_availability() {
        assert!(health_from_leader(true).is_ok());
        assert!(
            health_from_leader(false)
                .expect_err("leaderless group must be unhealthy")
                .to_string()
                .contains("No Raft leader elected")
        );
    }

    #[tokio::test]
    async fn mock_port_drives_the_classifier() {
        let ok_port = MockPort {
            response: Some(success()),
            transport_error: None,
            leader: true,
        };
        let outcome = ok_port.write(RaftRequest::Noop).await;
        assert!(classify_raft_write(outcome).is_ok());

        let broken_port = MockPort {
            response: None,
            transport_error: Some("forward to None".to_string()),
            leader: true,
        };
        let outcome = broken_port.write(RaftRequest::Noop).await;
        assert!(classify_raft_write(outcome).is_err());
        assert!(health_from_leader(broken_port.has_leader()).is_ok());
    }
}
