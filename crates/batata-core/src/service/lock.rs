// In-memory distributed lock service
// Provides simple lock acquire/release with automatic expiry

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tracing::{debug, info};

/// Upper bound for a RENEW-supplied TTL (milliseconds), matching Nacos
/// `MAX_AUTO_EXPIRE_TIME` (30 minutes). A single renew must not be able to
/// extend a lock indefinitely. Shared with the RENEW handler so the Raft and
/// in-memory paths clamp identically.
pub(crate) const MAX_LOCK_EXPIRE_MS: i64 = 1_800_000;

/// An acquired lock entry with reentrant lock support and fencing tokens
pub(crate) struct LockEntry {
    owner: String,
    /// Number of times the same owner has acquired this lock (reentrant)
    lock_count: u32,
    /// Monotonically increasing fencing token assigned on first acquire
    fencing_token: u64,
    acquired_at: Instant,
    ttl: Duration,
}

impl LockEntry {
    fn is_expired(&self) -> bool {
        self.acquired_at.elapsed() > self.ttl
    }
}

/// In-memory distributed lock service using DashMap
///
/// Supports reentrant locking (same owner can acquire multiple times) and
/// fencing tokens (monotonically increasing u64 returned on acquire).
pub struct LockService {
    pub(crate) locks: Arc<DashMap<String, LockEntry>>,
    /// Global monotonically increasing counter for fencing tokens
    pub(crate) fencing_counter: Arc<AtomicU64>,
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

        Self {
            locks,
            fencing_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Acquire a lock for the given key.
    ///
    /// Returns `Some(fencing_token)` if the lock was acquired, `None` if already
    /// held by another owner. Supports reentrant locking: if the same owner calls
    /// acquire again, the lock count is incremented and the existing fencing token
    /// is returned. The TTL is refreshed on each reentrant acquire.
    pub fn acquire(&self, key: &str, owner: &str, expired_time_ms: i64) -> Option<u64> {
        use dashmap::mapref::entry::Entry;

        let ttl = Duration::from_millis(expired_time_ms.max(0) as u64);
        let now = Instant::now();

        match self.locks.entry(key.to_string()) {
            Entry::Occupied(mut occupied) => {
                let existing = occupied.get_mut();
                if existing.owner == owner {
                    // Reentrant acquire: increment count, refresh TTL
                    existing.lock_count += 1;
                    existing.acquired_at = now;
                    existing.ttl = ttl;
                    let token = existing.fencing_token;
                    debug!(key = %key, owner = %owner, lock_count = existing.lock_count, "Lock re-acquired (reentrant)");
                    Some(token)
                } else if existing.is_expired() {
                    // Expired lock held by another owner -- replace it
                    let token = self.fencing_counter.fetch_add(1, Ordering::Relaxed);
                    *existing = LockEntry {
                        owner: owner.to_string(),
                        lock_count: 1,
                        fencing_token: token,
                        acquired_at: now,
                        ttl,
                    };
                    debug!(key = %key, owner = %owner, fencing_token = token, "Lock acquired (replaced expired)");
                    Some(token)
                } else {
                    // Held by another owner and not expired
                    None
                }
            }
            Entry::Vacant(vacant) => {
                // New lock acquisition
                let token = self.fencing_counter.fetch_add(1, Ordering::Relaxed);
                vacant.insert(LockEntry {
                    owner: owner.to_string(),
                    lock_count: 1,
                    fencing_token: token,
                    acquired_at: now,
                    ttl,
                });
                debug!(key = %key, owner = %owner, fencing_token = token, "Lock acquired");
                Some(token)
            }
        }
    }

    /// Release a lock for the given key.
    ///
    /// For reentrant locks, this decrements the lock count. The lock is only
    /// fully released when the count reaches zero. Returns `true` if the lock
    /// was (partially or fully) released, `false` if not held by this owner.
    pub fn release(&self, key: &str, owner: &str) -> bool {
        if let Some(mut entry) = self.locks.get_mut(key) {
            if entry.owner != owner {
                return false;
            }
            if entry.lock_count > 1 {
                entry.lock_count -= 1;
                debug!(key = %key, owner = %owner, lock_count = entry.lock_count, "Lock count decremented");
                return true;
            }
            // lock_count == 1: fully release
            drop(entry);
            self.locks.remove(key);
            debug!(key = %key, owner = %owner, "Lock fully released");
            return true;
        }
        false
    }

    /// Renew an existing lock's lease.
    ///
    /// `expired_time_ms` is interpreted as a *duration* (not an absolute
    /// timestamp): when positive it becomes the new TTL (clamped to
    /// [`MAX_LOCK_EXPIRE_MS`]); when non-positive the existing TTL is kept.
    /// The lease start (`acquired_at`) is always refreshed.
    ///
    /// Returns `false` when the lock does not exist, is owned by someone else,
    /// or has already expired. The reentrant `lock_count` is deliberately
    /// **never** modified — a renew is not an acquire. The whole
    /// check-and-refresh runs under a single `DashMap` write guard so it is
    /// atomic with respect to the background expiry cleanup task.
    pub fn renew(&self, key: &str, owner: &str, expired_time_ms: i64) -> bool {
        let Some(mut held) = self.locks.get_mut(key) else {
            return false;
        };
        if held.owner != owner {
            return false;
        }
        if held.is_expired() {
            return false;
        }
        if expired_time_ms > 0 {
            // RENEW treats expiredTime as a duration; clamp to GA maxExpireTime.
            held.ttl = Duration::from_millis(expired_time_ms.min(MAX_LOCK_EXPIRE_MS) as u64);
        }
        // Only refresh the lease timestamp. Never touch `lock_count` here:
        // renewing must not behave like a reentrant acquire().
        held.acquired_at = Instant::now();
        true
    }

    /// Force-release a lock regardless of the lock count.
    /// Returns `true` if the lock existed and was held by this owner.
    pub fn force_release(&self, key: &str, owner: &str) -> bool {
        if let Some(entry) = self.locks.get(key) {
            if entry.owner != owner {
                return false;
            }
            drop(entry);
            self.locks.remove(key);
            debug!(key = %key, owner = %owner, "Lock force-released");
            return true;
        }
        false
    }

    /// Get the current fencing token for a lock (if held).
    pub fn get_fencing_token(&self, key: &str) -> Option<u64> {
        self.locks
            .get(key)
            .filter(|e| !e.is_expired())
            .map(|e| e.fencing_token)
    }

    /// Get the current lock count for a key (0 if not held or expired).
    pub fn get_lock_count(&self, key: &str) -> u32 {
        self.locks
            .get(key)
            .filter(|e| !e.is_expired())
            .map(|e| e.lock_count)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Create a LockService without spawning the background task (for sync tests)
    fn test_lock_service() -> LockService {
        LockService {
            locks: Arc::new(DashMap::new()),
            fencing_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    #[test]
    fn test_acquire_and_release() {
        let svc = test_lock_service();

        let token = svc.acquire("key1", "owner1", 60000);
        assert!(token.is_some());
        assert!(svc.release("key1", "owner1"));
    }

    #[test]
    fn test_acquire_conflict() {
        let svc = test_lock_service();

        let token = svc.acquire("key1", "owner1", 60000);
        assert!(token.is_some());
        // Another owner cannot acquire
        assert!(svc.acquire("key1", "owner2", 60000).is_none());
        // Same owner can re-acquire (reentrant)
        let token2 = svc.acquire("key1", "owner1", 60000);
        assert!(token2.is_some());
        // Reentrant acquire returns the same fencing token
        assert_eq!(token, token2);
    }

    #[test]
    fn test_release_wrong_owner() {
        let svc = test_lock_service();

        assert!(svc.acquire("key1", "owner1", 60000).is_some());
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
        assert!(svc.acquire("key1", "owner1", 0).is_some());
        // Another owner can now acquire since lock is expired
        assert!(svc.acquire("key1", "owner2", 60000).is_some());
    }

    #[test]
    fn test_reentrant_lock() {
        let svc = test_lock_service();

        // First acquire
        let token1 = svc.acquire("key1", "owner1", 60000).unwrap();
        assert_eq!(svc.get_lock_count("key1"), 1);

        // Second acquire (reentrant) returns same token
        let token2 = svc.acquire("key1", "owner1", 60000).unwrap();
        assert_eq!(token1, token2);
        assert_eq!(svc.get_lock_count("key1"), 2);

        // Third acquire (reentrant)
        let token3 = svc.acquire("key1", "owner1", 60000).unwrap();
        assert_eq!(token1, token3);
        assert_eq!(svc.get_lock_count("key1"), 3);

        // Release decrements count
        assert!(svc.release("key1", "owner1"));
        assert_eq!(svc.get_lock_count("key1"), 2);

        assert!(svc.release("key1", "owner1"));
        assert_eq!(svc.get_lock_count("key1"), 1);

        // Final release removes the lock entirely
        assert!(svc.release("key1", "owner1"));
        assert_eq!(svc.get_lock_count("key1"), 0);

        // Lock is gone, another owner can acquire
        assert!(svc.acquire("key1", "owner2", 60000).is_some());
    }

    #[test]
    fn test_fencing_token_monotonically_increasing() {
        let svc = test_lock_service();

        let t1 = svc.acquire("key1", "owner1", 60000).unwrap();
        svc.release("key1", "owner1");

        let t2 = svc.acquire("key1", "owner2", 60000).unwrap();
        svc.release("key1", "owner2");

        let t3 = svc.acquire("key2", "owner1", 60000).unwrap();

        assert!(t2 > t1, "Fencing tokens must be monotonically increasing");
        assert!(t3 > t2, "Fencing tokens must be monotonically increasing");
    }

    #[test]
    fn test_get_fencing_token() {
        let svc = test_lock_service();

        assert!(svc.get_fencing_token("key1").is_none());

        let token = svc.acquire("key1", "owner1", 60000).unwrap();
        assert_eq!(svc.get_fencing_token("key1"), Some(token));

        svc.release("key1", "owner1");
        assert!(svc.get_fencing_token("key1").is_none());
    }

    #[test]
    fn test_force_release() {
        let svc = test_lock_service();

        // Acquire 3 times (reentrant)
        svc.acquire("key1", "owner1", 60000);
        svc.acquire("key1", "owner1", 60000);
        svc.acquire("key1", "owner1", 60000);
        assert_eq!(svc.get_lock_count("key1"), 3);

        // Force release removes regardless of count
        assert!(svc.force_release("key1", "owner1"));
        assert_eq!(svc.get_lock_count("key1"), 0);
    }

    #[test]
    fn test_force_release_wrong_owner() {
        let svc = test_lock_service();

        svc.acquire("key1", "owner1", 60000);
        assert!(!svc.force_release("key1", "owner2"));
        // Lock should still be held
        assert_eq!(svc.get_lock_count("key1"), 1);
    }

    #[test]
    fn test_expired_lock_fencing_token() {
        let svc = test_lock_service();

        // Acquire with 0ms TTL (immediately expired)
        svc.acquire("key1", "owner1", 0);
        // Expired locks should not return a fencing token
        assert!(svc.get_fencing_token("key1").is_none());
        assert_eq!(svc.get_lock_count("key1"), 0);
    }

    #[test]
    fn test_renew_refreshes_lease_without_changing_lock_count() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-1", 1000);
        svc.acquire("k", "owner-1", 1000); // reentrant -> count = 2
        assert_eq!(svc.get_lock_count("k"), 2);

        assert!(svc.renew("k", "owner-1", 60000));
        assert_eq!(
            svc.get_lock_count("k"),
            2,
            "renew must never change lock_count"
        );
        assert_eq!(
            svc.locks.get("k").unwrap().ttl,
            Duration::from_millis(60000)
        );
        // Lease start was refreshed, so the lock is live again.
        assert!(svc.get_fencing_token("k").is_some());
    }

    #[test]
    fn test_renew_negative_expired_time_keeps_ttl() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-1", 5000);
        assert!(svc.renew("k", "owner-1", -1));
        assert_eq!(svc.locks.get("k").unwrap().ttl, Duration::from_millis(5000));
    }

    #[test]
    fn test_renew_clamps_ttl_to_max() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-1", 1000);
        assert!(svc.renew("k", "owner-1", MAX_LOCK_EXPIRE_MS + 1000));
        assert_eq!(
            svc.locks.get("k").unwrap().ttl,
            Duration::from_millis(MAX_LOCK_EXPIRE_MS as u64),
            "renew must clamp expiredTime to maxExpireTime"
        );
    }

    #[test]
    fn test_renew_missing_lock_returns_false() {
        let svc = test_lock_service();
        assert!(!svc.renew("missing", "owner-1", 30000));
    }

    #[test]
    fn test_renew_wrong_owner_returns_false() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-1", 60000);
        assert!(!svc.renew("k", "owner-2", 30000));
        assert_eq!(svc.get_lock_count("k"), 1);
    }

    #[test]
    fn test_renew_expired_lock_returns_false() {
        let svc = test_lock_service();
        svc.acquire("k", "owner-1", 0); // ttl 0 -> immediately expired
        std::thread::sleep(Duration::from_millis(5));
        assert!(
            !svc.renew("k", "owner-1", 30000),
            "an already-expired lock cannot be renewed"
        );
        // The stale entry can be taken over by another owner.
        assert!(svc.acquire("k", "owner-2", 60000).is_some());
    }
}
