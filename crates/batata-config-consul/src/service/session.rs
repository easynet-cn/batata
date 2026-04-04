//! Consul Session Service — distributed locking primitives

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use crate::error::ConsulKvError;
use crate::model::{Session, SessionRequest};

/// In-memory Consul session service
#[derive(Clone)]
pub struct ConsulSessionServiceImpl {
    sessions: Arc<DashMap<String, Session>>,
    index: Arc<AtomicU64>,
    node_name: String,
}

impl ConsulSessionServiceImpl {
    pub fn new(node_name: String) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            index: Arc::new(AtomicU64::new(1)),
            node_name,
        }
    }

    fn next_index(&self) -> u64 {
        self.index.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Create a new session
    pub fn create_session(&self, request: SessionRequest) -> Result<String, ConsulKvError> {
        let id = generate_session_id();
        let idx = self.next_index();

        let session = Session {
            id: id.clone(),
            name: request.name,
            node: if request.node.is_empty() {
                self.node_name.clone()
            } else {
                request.node
            },
            checks: request.checks,
            lock_delay: if request.lock_delay.is_empty() {
                "15s".to_string()
            } else {
                request.lock_delay
            },
            behavior: request.behavior,
            ttl: request.ttl,
            create_index: idx,
            modify_index: idx,
        };

        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Destroy a session
    pub fn destroy_session(&self, session_id: &str) -> Result<(), ConsulKvError> {
        self.sessions.remove(session_id);
        Ok(())
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>, ConsulKvError> {
        Ok(self.sessions.get(session_id).map(|s| s.clone()))
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Result<Vec<Session>, ConsulKvError> {
        Ok(self
            .sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect())
    }

    /// List sessions for a specific node
    pub fn list_node_sessions(&self, node: &str) -> Result<Vec<Session>, ConsulKvError> {
        Ok(self
            .sessions
            .iter()
            .filter(|entry| entry.value().node == node)
            .map(|entry| entry.value().clone())
            .collect())
    }

    /// Renew a session
    pub fn renew_session(&self, session_id: &str) -> Result<Session, ConsulKvError> {
        if let Some(mut entry) = self.sessions.get_mut(session_id) {
            entry.modify_index = self.next_index();
            Ok(entry.clone())
        } else {
            Err(ConsulKvError::SessionNotFound(session_id.to_string()))
        }
    }
}

impl Default for ConsulSessionServiceImpl {
    fn default() -> Self {
        Self::new("batata-node".to_string())
    }
}

fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:032x}", t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SessionBehavior;

    #[test]
    fn test_session_lifecycle() {
        let svc = ConsulSessionServiceImpl::default();

        let id = svc
            .create_session(SessionRequest {
                name: "test-session".to_string(),
                node: String::new(),
                checks: vec!["serfHealth".to_string()],
                lock_delay: String::new(),
                behavior: SessionBehavior::Release,
                ttl: "30s".to_string(),
            })
            .unwrap();

        // Get session
        let session = svc.get_session(&id).unwrap().unwrap();
        assert_eq!(session.name, "test-session");
        assert_eq!(session.node, "batata-node");
        assert_eq!(session.lock_delay, "15s"); // default

        // List sessions
        let sessions = svc.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);

        // Destroy
        svc.destroy_session(&id).unwrap();
        assert!(svc.get_session(&id).unwrap().is_none());
    }

    #[test]
    fn test_renew_session() {
        let svc = ConsulSessionServiceImpl::default();

        let id = svc
            .create_session(SessionRequest {
                name: "test".to_string(),
                ..Default::default()
            })
            .unwrap();

        let original = svc.get_session(&id).unwrap().unwrap();
        let renewed = svc.renew_session(&id).unwrap();
        assert!(renewed.modify_index > original.modify_index);
    }

    #[test]
    fn test_renew_nonexistent() {
        let svc = ConsulSessionServiceImpl::default();
        let result = svc.renew_session("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_node_sessions() {
        let svc = ConsulSessionServiceImpl::default();

        svc.create_session(SessionRequest {
            name: "s1".to_string(),
            node: "node-a".to_string(),
            ..Default::default()
        })
        .unwrap();
        svc.create_session(SessionRequest {
            name: "s2".to_string(),
            node: "node-b".to_string(),
            ..Default::default()
        })
        .unwrap();

        let sessions = svc.list_node_sessions("node-a").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "s1");
    }
}
