//! Server lifecycle status (shared between the server and core crates).
//!
//! This lives in `batata-common` (not in a higher crate) because both layers need it:
//! the server owns and refreshes the status, while the gRPC request path in
//! `batata-core` must read it to gate traffic while the node is not ready.
//!
//! Nacos parity: Nacos gates gRPC traffic on `ApplicationUtils.isStarted()`
//! (`GrpcRequestAcceptor`) and refreshes a richer `ServerStatus` (UP / DOWN /
//! STARTING / PAUSED / WRITE_ONLY / READ_ONLY) every 5s from
//! `CPProtocol.isReady() && DistroProtocol.isInitialized()`
//! (`naming/cluster/ServerStatusManager`). This type models the same lifecycle.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicI8, AtomicU8, Ordering};

/// Server lifecycle status.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    /// The server is still starting up.
    Starting = 0,
    /// The server is up and serving traffic.
    Up = 1,
    /// The server is down (e.g. a required subsystem is not ready).
    Down = 2,
    /// The server is draining: finishing in-flight requests before shutdown.
    Draining = 3,
    /// Operator-set: only read (GET) traffic is served.
    ReadOnly = 4,
    /// Operator-set: only write (non-GET) traffic is served.
    WriteOnly = 5,
    /// Operator-set: traffic is paused entirely.
    Paused = 6,
}

impl ServerStatus {
    /// Convert the stored `u8` back into a `ServerStatus`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Up,
            2 => Self::Down,
            3 => Self::Draining,
            4 => Self::ReadOnly,
            5 => Self::WriteOnly,
            6 => Self::Paused,
            _ => Self::Starting,
        }
    }

    /// Whether this status is an operator-controlled mode rather than a lifecycle state.
    pub fn is_operator_mode(&self) -> bool {
        matches!(self, Self::ReadOnly | Self::WriteOnly | Self::Paused)
    }
}

impl fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "STARTING"),
            Self::Up => write!(f, "UP"),
            Self::Down => write!(f, "DOWN"),
            Self::Draining => write!(f, "DRAINING"),
            Self::ReadOnly => write!(f, "READ_ONLY"),
            Self::WriteOnly => write!(f, "WRITE_ONLY"),
            Self::Paused => write!(f, "PAUSED"),
        }
    }
}

impl FromStr for ServerStatus {
    type Err = String;

    /// Parse from the `Display` string (case-sensitive, matching the enum names).
    ///
    /// Nacos parity: this is exactly `ServerStatus.valueOf(String)`, so the only
    /// accepted values are the enum identifiers — `UP`, `DOWN`, `STARTING`,
    /// `DRAINING`, `READ_ONLY`, `WRITE_ONLY`, `PAUSED`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "STARTING" => Ok(Self::Starting),
            "UP" => Ok(Self::Up),
            "DOWN" => Ok(Self::Down),
            "DRAINING" => Ok(Self::Draining),
            "READ_ONLY" => Ok(Self::ReadOnly),
            "WRITE_ONLY" => Ok(Self::WriteOnly),
            "PAUSED" => Ok(Self::Paused),
            other => Err(format!("unknown server status: {}", other)),
        }
    }
}

/// Thread-safe server status manager.
///
/// The status is an `AtomicU8` so that `is_up()` is a single atomic load with zero
/// contention — it sits on the hot path of every incoming request. The optional error
/// message is behind an `RwLock` because it is only read on the cold rejection path.
pub struct ServerStatusManager {
    status: Arc<AtomicU8>,
    error_msg: Arc<RwLock<Option<String>>>,
    /// Operator override — Nacos `switchDomain.overriddenServerStatus` parity.
    ///
    /// `-1` means "no override"; any other value is the forced `ServerStatus` (stored as
    /// its `u8` discriminant). When set, the readiness refresher adopts this value verbatim
    /// and skips the UP/DOWN derivation.
    overridden: Arc<AtomicI8>,
}

impl ServerStatusManager {
    /// Create a manager that starts in `ServerStatus::Starting`.
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(ServerStatus::Starting as u8)),
            error_msg: Arc::new(RwLock::new(None)),
            overridden: Arc::new(AtomicI8::new(-1)),
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

    /// Force the server into a specific status, overriding the readiness computation.
    ///
    /// Nacos parity: `switchDomain.overriddenServerStatus`. Once set, the 5s readiness
    /// refresher adopts this value verbatim and skips the UP/DOWN derivation, exactly like
    /// `ServerStatusManager.refreshServerStatus()`:
    /// `if (isNotBlank(overriddenServerStatus)) { status = value; return; }`.
    pub fn set_overridden(&self, status: ServerStatus) {
        self.overridden.store(status as i8, Ordering::Relaxed);
        self.status.store(status as u8, Ordering::Relaxed);
        if let Ok(mut g) = self.error_msg.write() {
            *g = None;
        }
    }

    /// Clear the operator override (Nacos: `update(overriddenServerStatus, "null")`).
    ///
    /// After clearing, the refresher reverts to computing UP/DOWN from subsystem readiness.
    pub fn clear_overridden(&self) {
        self.overridden.store(-1, Ordering::Relaxed);
    }

    /// Returns the active operator override, if any.
    pub fn overridden(&self) -> Option<ServerStatus> {
        let v = self.overridden.load(Ordering::Relaxed);
        if v < 0 {
            None
        } else {
            Some(ServerStatus::from_u8(v as u8))
        }
    }

    /// Mark the server ready to serve traffic.
    pub fn set_up(&self) {
        self.status.store(ServerStatus::Up as u8, Ordering::Relaxed);
    }

    /// Mark the server not ready (a required subsystem is unavailable).
    pub fn set_down(&self) {
        self.status
            .store(ServerStatus::Down as u8, Ordering::Relaxed);
    }

    /// Mark the server as draining before shutdown.
    pub fn set_draining(&self) {
        self.status
            .store(ServerStatus::Draining as u8, Ordering::Relaxed);
    }

    /// Mark the server as starting again.
    pub fn set_starting(&self) {
        self.status
            .store(ServerStatus::Starting as u8, Ordering::Relaxed);
    }

    /// Operator mode: serve only read (GET) traffic.
    ///
    /// Nacos parity: `ServerStatus.READ_ONLY`, settable via the overridden server status.
    pub fn set_read_only(&self) {
        self.status
            .store(ServerStatus::ReadOnly as u8, Ordering::Relaxed);
    }

    /// Operator mode: serve only write (non-GET) traffic.
    ///
    /// Nacos parity: `ServerStatus.WRITE_ONLY`, settable via the overridden server status.
    pub fn set_write_only(&self) {
        self.status
            .store(ServerStatus::WriteOnly as u8, Ordering::Relaxed);
    }

    /// Operator mode: pause all traffic.
    ///
    /// Nacos parity: `ServerStatus.PAUSED`, settable via the overridden server status.
    pub fn set_paused(&self) {
        self.status
            .store(ServerStatus::Paused as u8, Ordering::Relaxed);
    }

    /// Get the current error message (cold path — only called on rejection).
    pub async fn error_msg(&self) -> Option<String> {
        self.error_msg.read().ok().and_then(|g| g.clone())
    }

    /// Set or clear the error message.
    pub async fn set_error_msg(&self, msg: Option<String>) {
        if let Ok(mut g) = self.error_msg.write() {
            *g = msg;
        }
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
            overridden: self.overridden.clone(),
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
    fn test_set_draining() {
        let mgr = ServerStatusManager::new();
        mgr.set_up();
        mgr.set_draining();
        assert_eq!(mgr.status(), ServerStatus::Draining);
        assert!(!mgr.is_up());
    }

    #[test]
    fn test_display() {
        assert_eq!(ServerStatus::Starting.to_string(), "STARTING");
        assert_eq!(ServerStatus::Up.to_string(), "UP");
        assert_eq!(ServerStatus::Down.to_string(), "DOWN");
        assert_eq!(ServerStatus::Draining.to_string(), "DRAINING");
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

    #[test]
    fn test_override_takes_precedence() {
        let mgr = ServerStatusManager::new();
        mgr.set_up();
        assert_eq!(mgr.overridden(), None);

        // Operator override wins over the lifecycle state.
        mgr.set_overridden(ServerStatus::ReadOnly);
        assert_eq!(mgr.status(), ServerStatus::ReadOnly);
        assert_eq!(mgr.overridden(), Some(ServerStatus::ReadOnly));
        assert!(!mgr.is_up());

        // Clearing reverts to "no override".
        mgr.clear_overridden();
        assert_eq!(mgr.overridden(), None);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(ServerStatus::from_str("UP").unwrap(), ServerStatus::Up);
        assert_eq!(
            ServerStatus::from_str("READ_ONLY").unwrap(),
            ServerStatus::ReadOnly
        );
        assert_eq!(
            ServerStatus::from_str("PAUSED").unwrap(),
            ServerStatus::Paused
        );
        assert_eq!(
            ServerStatus::from_str("WRITE_ONLY").unwrap(),
            ServerStatus::WriteOnly
        );
        assert!(ServerStatus::from_str("NOPE").is_err());
        // Case-sensitive, like Nacos `ServerStatus.valueOf`.
        assert!(ServerStatus::from_str("up").is_err());
    }
}
