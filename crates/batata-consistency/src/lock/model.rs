//! Distributed Lock Data Model (ADV-001)

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lock state enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    /// Lock is available
    Unlocked,
    /// Lock is held by an owner
    Locked,
    /// Lock is being acquired (in transition)
    Acquiring,
    /// Lock is being released (in transition)
    Releasing,
    /// Lock expired and is being cleaned up
    Expired,
}

impl Default for LockState {
    fn default() -> Self {
        Self::Unlocked
    }
}

/// Distributed lock entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedLock {
    /// Unique lock key (namespace::name format)
    pub key: String,
    /// Lock name
    pub name: String,
    /// Namespace
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// Current lock state
    pub state: LockState,
    /// Current owner (client ID or session ID)
    #[serde(default)]
    pub owner: Option<String>,
    /// Owner metadata (additional info about the owner)
    #[serde(default)]
    pub owner_metadata: Option<String>,
    /// Lock version (incremented on each state change)
    #[serde(default)]
    pub version: u64,
    /// Fence token (monotonically increasing, used for fencing)
    #[serde(default)]
    pub fence_token: u64,
    /// Lock acquisition timestamp (Unix millis)
    #[serde(default)]
    pub acquired_at: Option<i64>,
    /// Lock expiration timestamp (Unix millis)
    #[serde(default)]
    pub expires_at: Option<i64>,
    /// Time-to-live in milliseconds
    #[serde(default = "default_ttl")]
    pub ttl_ms: u64,
    /// Whether auto-renewal is enabled
    #[serde(default)]
    pub auto_renew: bool,
    /// Last renewal timestamp
    #[serde(default)]
    pub last_renewed_at: Option<i64>,
    /// Number of times the lock has been renewed
    #[serde(default)]
    pub renewal_count: u32,
    /// Maximum renewals allowed (0 = unlimited)
    #[serde(default)]
    pub max_renewals: u32,
    /// Creation timestamp
    pub created_at: i64,
    /// Last update timestamp
    pub updated_at: i64,
}

fn default_namespace() -> String {
    "public".to_string()
}

fn default_ttl() -> u64 {
    30000 // 30 seconds
}

impl Default for DistributedLock {
    fn default() -> Self {
        let now = current_timestamp();
        Self {
            key: String::new(),
            name: String::new(),
            namespace: "public".to_string(),
            state: LockState::Unlocked,
            owner: None,
            owner_metadata: None,
            version: 0,
            fence_token: 0,
            acquired_at: None,
            expires_at: None,
            ttl_ms: 30000,
            auto_renew: false,
            last_renewed_at: None,
            renewal_count: 0,
            max_renewals: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

impl DistributedLock {
    /// Create a new lock with the given key
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        let namespace = namespace.into();
        let name = name.into();
        let key = format!("{}::{}", namespace, name);
        Self {
            key,
            name,
            namespace,
            ..Default::default()
        }
    }

    /// Check if the lock is currently held
    pub fn is_locked(&self) -> bool {
        self.state == LockState::Locked && !self.is_expired()
    }

    /// Check if the lock has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            current_timestamp() >= expires_at
        } else {
            false
        }
    }

    /// Check if the given owner holds this lock
    pub fn is_owned_by(&self, owner: &str) -> bool {
        self.is_locked() && self.owner.as_deref() == Some(owner)
    }

    /// Calculate remaining TTL in milliseconds
    pub fn remaining_ttl_ms(&self) -> u64 {
        if let Some(expires_at) = self.expires_at {
            let now = current_timestamp();
            if expires_at > now {
                return (expires_at - now) as u64;
            }
        }
        0
    }

    /// Acquire the lock for a given owner
    pub fn acquire(&mut self, owner: impl Into<String>) -> bool {
        if self.is_locked() {
            return false;
        }

        let now = current_timestamp();
        self.state = LockState::Locked;
        self.owner = Some(owner.into());
        self.acquired_at = Some(now);
        self.expires_at = Some(now + self.ttl_ms as i64);
        self.version += 1;
        self.fence_token += 1;
        self.updated_at = now;
        self.renewal_count = 0;
        self.last_renewed_at = None;
        true
    }

    /// Release the lock
    pub fn release(&mut self, owner: &str) -> bool {
        if !self.is_owned_by(owner) {
            return false;
        }

        self.state = LockState::Unlocked;
        self.owner = None;
        self.owner_metadata = None;
        self.acquired_at = None;
        self.expires_at = None;
        self.version += 1;
        self.updated_at = current_timestamp();
        true
    }

    /// Force release the lock (admin operation)
    pub fn force_release(&mut self) {
        self.state = LockState::Unlocked;
        self.owner = None;
        self.owner_metadata = None;
        self.acquired_at = None;
        self.expires_at = None;
        self.version += 1;
        self.updated_at = current_timestamp();
    }

    /// Renew the lock (ADV-003)
    pub fn renew(&mut self, owner: &str) -> bool {
        if !self.is_owned_by(owner) {
            return false;
        }

        // Check max renewals
        if self.max_renewals > 0 && self.renewal_count >= self.max_renewals {
            return false;
        }

        let now = current_timestamp();
        self.expires_at = Some(now + self.ttl_ms as i64);
        self.last_renewed_at = Some(now);
        self.renewal_count += 1;
        self.version += 1;
        self.updated_at = now;
        true
    }

    /// Handle expiration (ADV-004)
    pub fn expire(&mut self) {
        if self.is_expired() {
            self.state = LockState::Expired;
            self.owner = None;
            self.owner_metadata = None;
            self.version += 1;
            self.updated_at = current_timestamp();
        }
    }
}

/// Lock acquisition request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockAcquireRequest {
    /// Lock namespace
    pub namespace: String,
    /// Lock name
    pub name: String,
    /// Owner (client ID)
    pub owner: String,
    /// Owner metadata
    #[serde(default)]
    pub owner_metadata: Option<String>,
    /// Time-to-live in milliseconds
    #[serde(default = "default_ttl")]
    pub ttl_ms: u64,
    /// Enable auto-renewal
    #[serde(default)]
    pub auto_renew: bool,
    /// Maximum wait time in milliseconds (0 = non-blocking)
    #[serde(default)]
    pub wait_ms: u64,
    /// Maximum renewals (0 = unlimited)
    #[serde(default)]
    pub max_renewals: u32,
}

impl Default for LockAcquireRequest {
    fn default() -> Self {
        Self {
            namespace: "public".to_string(),
            name: String::new(),
            owner: String::new(),
            owner_metadata: None,
            ttl_ms: 30000,
            auto_renew: false,
            wait_ms: 0,
            max_renewals: 0,
        }
    }
}

/// Lock acquisition result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockAcquireResult {
    /// Whether the lock was acquired
    pub acquired: bool,
    /// The lock (if acquired)
    pub lock: Option<DistributedLock>,
    /// Fence token (for fencing operations)
    pub fence_token: u64,
    /// Current owner (if lock is held by someone else)
    pub current_owner: Option<String>,
    /// Remaining wait time if waiting
    pub remaining_wait_ms: u64,
    /// Error message (if any)
    pub error: Option<String>,
}

impl Default for LockAcquireResult {
    fn default() -> Self {
        Self {
            acquired: false,
            lock: None,
            fence_token: 0,
            current_owner: None,
            remaining_wait_ms: 0,
            error: None,
        }
    }
}

/// Lock release request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockReleaseRequest {
    /// Lock namespace
    pub namespace: String,
    /// Lock name
    pub name: String,
    /// Owner (client ID)
    pub owner: String,
    /// Expected fence token (for fencing)
    #[serde(default)]
    pub fence_token: Option<u64>,
}

/// Lock release result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockReleaseResult {
    /// Whether the lock was released
    pub released: bool,
    /// Error message (if any)
    pub error: Option<String>,
}

/// Lock renewal request (ADV-003)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRenewRequest {
    /// Lock namespace
    pub namespace: String,
    /// Lock name
    pub name: String,
    /// Owner (client ID)
    pub owner: String,
    /// New TTL (optional, uses original if not specified)
    #[serde(default)]
    pub ttl_ms: Option<u64>,
}

/// Lock renewal result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRenewResult {
    /// Whether the lock was renewed
    pub renewed: bool,
    /// New expiration timestamp
    pub expires_at: Option<i64>,
    /// Renewal count
    pub renewal_count: u32,
    /// Error message (if any)
    pub error: Option<String>,
}

/// Lock query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockQueryRequest {
    /// Lock namespace
    pub namespace: String,
    /// Lock name (optional for listing)
    #[serde(default)]
    pub name: Option<String>,
    /// Filter by owner
    #[serde(default)]
    pub owner: Option<String>,
    /// Filter by state
    #[serde(default)]
    pub state: Option<LockState>,
    /// Include expired locks
    #[serde(default)]
    pub include_expired: bool,
    /// Maximum results
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

/// Lock statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockStats {
    /// Total locks created
    pub total_locks: u64,
    /// Currently held locks
    pub active_locks: u32,
    /// Total acquisitions
    pub total_acquisitions: u64,
    /// Total releases
    pub total_releases: u64,
    /// Total renewals
    pub total_renewals: u64,
    /// Expired locks (auto-released)
    pub expired_locks: u64,
    /// Failed acquisitions (lock contention)
    pub failed_acquisitions: u64,
    /// Average lock hold time in milliseconds
    pub avg_hold_time_ms: u64,
}

/// Raft command for lock operations (ADV-005)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LockCommand {
    /// Acquire a lock
    Acquire(LockAcquireRequest),
    /// Release a lock
    Release(LockReleaseRequest),
    /// Renew a lock
    Renew(LockRenewRequest),
    /// Force release (admin)
    ForceRelease { namespace: String, name: String },
    /// Expire check (background)
    ExpireCheck,
}

/// Raft response for lock operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LockCommandResponse {
    /// Acquire result
    Acquire(LockAcquireResult),
    /// Release result
    Release(LockReleaseResult),
    /// Renew result
    Renew(LockRenewResult),
    /// Force release result
    ForceRelease { success: bool },
    /// Expire check result
    ExpireCheck { expired: u32 },
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_acquire_release() {
        let mut lock = DistributedLock::new("public", "test-lock");

        // Acquire by owner1
        assert!(lock.acquire("owner1"));
        assert!(lock.is_locked());
        assert!(lock.is_owned_by("owner1"));
        assert!(!lock.is_owned_by("owner2"));

        // Try to acquire by owner2 - should fail
        assert!(!lock.acquire("owner2"));

        // Release by wrong owner - should fail
        assert!(!lock.release("owner2"));

        // Release by correct owner
        assert!(lock.release("owner1"));
        assert!(!lock.is_locked());

        // Now owner2 can acquire
        assert!(lock.acquire("owner2"));
    }

    #[test]
    fn test_lock_renewal() {
        let mut lock = DistributedLock::new("public", "test-lock");
        lock.max_renewals = 3;

        lock.acquire("owner1");

        // Renew 3 times
        assert!(lock.renew("owner1"));
        assert!(lock.renew("owner1"));
        assert!(lock.renew("owner1"));

        // 4th renewal should fail
        assert!(!lock.renew("owner1"));
        assert_eq!(lock.renewal_count, 3);
    }

    #[test]
    fn test_lock_expiration() {
        let mut lock = DistributedLock::new("public", "test-lock");
        lock.ttl_ms = 1; // 1ms TTL

        lock.acquire("owner1");

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_millis(10));

        assert!(lock.is_expired());
        lock.expire();
        assert_eq!(lock.state, LockState::Expired);
    }

    #[test]
    fn test_fence_token() {
        let mut lock = DistributedLock::new("public", "test-lock");

        lock.acquire("owner1");
        let token1 = lock.fence_token;

        lock.release("owner1");
        lock.acquire("owner2");
        let token2 = lock.fence_token;

        assert!(token2 > token1);
    }
}
