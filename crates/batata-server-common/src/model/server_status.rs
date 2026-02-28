//! Server status lifecycle management
//!
//! Tracks the server's startup state (Starting → Up/Down) and provides
//! lock-free reads for the hot path (every HTTP request via TrafficReviseFilter).

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use tokio::sync::RwLock;

/// Server lifecycle status.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    Starting = 0,
    Up = 1,
    Down = 2,
}

impl ServerStatus {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Up,
            2 => Self::Down,
            _ => Self::Starting,
        }
    }
}

impl fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "STARTING"),
            Self::Up => write!(f, "UP"),
            Self::Down => write!(f, "DOWN"),
        }
    }
}

/// Thread-safe server status manager.
///
/// Uses an `AtomicU8` for the status so that `is_up()` is a single atomic load
/// with zero contention — this sits on the hot path of every incoming request.
/// The optional error message is behind a `RwLock` since it is only read on
/// the cold (rejection) path.
pub struct ServerStatusManager {
    status: Arc<AtomicU8>,
    error_msg: Arc<RwLock<Option<String>>>,
}

impl ServerStatusManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(ServerStatus::Starting as u8)),
            error_msg: Arc::new(RwLock::new(None)),
        }
    }

    /// Current status (atomic load).
    pub fn status(&self) -> ServerStatus {
        ServerStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    /// Returns `true` when the server is fully ready.
    pub fn is_up(&self) -> bool {
        self.status.load(Ordering::Relaxed) == ServerStatus::Up as u8
    }

    pub fn set_up(&self) {
        self.status.store(ServerStatus::Up as u8, Ordering::Relaxed);
    }

    pub fn set_down(&self) {
        self.status
            .store(ServerStatus::Down as u8, Ordering::Relaxed);
    }

    pub fn set_starting(&self) {
        self.status
            .store(ServerStatus::Starting as u8, Ordering::Relaxed);
    }

    /// Get the current error message (cold path — only called on rejection).
    pub async fn error_msg(&self) -> Option<String> {
        self.error_msg.read().await.clone()
    }

    /// Set or clear the error message.
    pub async fn set_error_msg(&self, msg: Option<String>) {
        *self.error_msg.write().await = msg;
    }
}

impl Default for ServerStatusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ServerStatusManager {
    fn clone(&self) -> Self {
        Self {
            status: self.status.clone(),
            error_msg: self.error_msg.clone(),
        }
    }
}

impl fmt::Debug for ServerStatusManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerStatusManager")
            .field("status", &self.status())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_status_is_starting() {
        let mgr = ServerStatusManager::new();
        assert_eq!(mgr.status(), ServerStatus::Starting);
        assert!(!mgr.is_up());
    }

    #[test]
    fn test_set_up() {
        let mgr = ServerStatusManager::new();
        mgr.set_up();
        assert_eq!(mgr.status(), ServerStatus::Up);
        assert!(mgr.is_up());
    }

    #[test]
    fn test_set_down() {
        let mgr = ServerStatusManager::new();
        mgr.set_up();
        mgr.set_down();
        assert_eq!(mgr.status(), ServerStatus::Down);
        assert!(!mgr.is_up());
    }

    #[test]
    fn test_set_starting() {
        let mgr = ServerStatusManager::new();
        mgr.set_up();
        mgr.set_starting();
        assert_eq!(mgr.status(), ServerStatus::Starting);
    }

    #[test]
    fn test_display() {
        assert_eq!(ServerStatus::Starting.to_string(), "STARTING");
        assert_eq!(ServerStatus::Up.to_string(), "UP");
        assert_eq!(ServerStatus::Down.to_string(), "DOWN");
    }

    #[test]
    fn test_clone_shares_state() {
        let mgr = ServerStatusManager::new();
        let clone = mgr.clone();
        mgr.set_up();
        assert!(clone.is_up());
    }

    #[tokio::test]
    async fn test_error_msg() {
        let mgr = ServerStatusManager::new();
        assert!(mgr.error_msg().await.is_none());

        mgr.set_error_msg(Some("db not ready".to_string())).await;
        assert_eq!(mgr.error_msg().await, Some("db not ready".to_string()));

        mgr.set_error_msg(None).await;
        assert!(mgr.error_msg().await.is_none());
    }
}
