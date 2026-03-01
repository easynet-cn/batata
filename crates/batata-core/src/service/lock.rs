// In-memory distributed lock service
// Provides simple lock acquire/release with automatic expiry

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::{debug, info};

/// An acquired lock entry
pub(crate) struct LockEntry {
    owner: String,
    acquired_at: Instant,
    ttl: Duration,
}

impl LockEntry {
    fn is_expired(&self) -> bool {
        self.acquired_at.elapsed() > self.ttl
    }
}

/// In-memory distributed lock service using DashMap
pub struct LockService {
    pub(crate) locks: Arc<DashMap<String, LockEntry>>,
}

impl Default for LockService {
    fn default() -> Self {
        Self::new()
    }
}

impl LockService {
    /// Create a new lock service and start the background expiry task
    pub fn new() -> Self {
        let locks: Arc<DashMap<String, LockEntry>> = Arc::new(DashMap::new());

        // Background task to clean up expired locks
        let locks_clone = locks.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                let expired_keys: Vec<String> = locks_clone
                    .iter()
                    .filter(|entry| entry.value().is_expired())
                    .map(|entry| entry.key().clone())
                    .collect();

                for key in &expired_keys {
                    locks_clone.remove(key);
                }

                if !expired_keys.is_empty() {
                    debug!(
                        count = expired_keys.len(),
                        "Cleaned up expired lock entries"
                    );
                }
            }
        });

        info!("LockService initialized with background expiry task");

        Self { locks }
    }

    /// Acquire a lock for the given key
    ///
    /// Returns `true` if the lock was acquired, `false` if already held by another owner
    pub fn acquire(&self, key: &str, owner: &str, expired_time_ms: i64) -> bool {
        let ttl = Duration::from_millis(expired_time_ms.max(0) as u64);

        // Check if existing lock is held and not expired
        if let Some(existing) = self.locks.get(key)
            && !existing.is_expired()
            && existing.owner != owner
        {
            return false;
        }

        // Acquire or re-acquire the lock
        self.locks.insert(
            key.to_string(),
            LockEntry {
                owner: owner.to_string(),
                acquired_at: Instant::now(),
                ttl,
            },
        );

        debug!(key = %key, owner = %owner, "Lock acquired");
        true
    }

    /// Release a lock for the given key
    ///
    /// Returns `true` if the lock was released, `false` if not held by this owner
    pub fn release(&self, key: &str, owner: &str) -> bool {
        if let Some(entry) = self.locks.get(key) {
            if entry.owner != owner {
                return false;
            }
            drop(entry);
            self.locks.remove(key);
            debug!(key = %key, owner = %owner, "Lock released");
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Create a LockService without spawning the background task (for sync tests)
    fn test_lock_service() -> LockService {
        LockService {
            locks: Arc::new(DashMap::new()),
        }
    }

    #[test]
    fn test_acquire_and_release() {
        let svc = test_lock_service();

        assert!(svc.acquire("key1", "owner1", 60000));
        assert!(svc.release("key1", "owner1"));
    }

    #[test]
    fn test_acquire_conflict() {
        let svc = test_lock_service();

        assert!(svc.acquire("key1", "owner1", 60000));
        // Another owner cannot acquire
        assert!(!svc.acquire("key1", "owner2", 60000));
        // Same owner can re-acquire
        assert!(svc.acquire("key1", "owner1", 60000));
    }

    #[test]
    fn test_release_wrong_owner() {
        let svc = test_lock_service();

        assert!(svc.acquire("key1", "owner1", 60000));
        // Wrong owner cannot release
        assert!(!svc.release("key1", "owner2"));
        // Correct owner can release
        assert!(svc.release("key1", "owner1"));
    }

    #[test]
    fn test_release_nonexistent() {
        let svc = test_lock_service();
        assert!(!svc.release("nonexistent", "owner1"));
    }

    #[test]
    fn test_acquire_after_expiry() {
        let svc = test_lock_service();

        // Acquire with 0ms TTL (immediately expired)
        assert!(svc.acquire("key1", "owner1", 0));
        // Another owner can now acquire since lock is expired
        assert!(svc.acquire("key1", "owner2", 60000));
    }
}
