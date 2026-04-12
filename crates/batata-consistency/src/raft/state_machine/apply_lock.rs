//! Lock-domain apply handlers for the Raft state machine (ADV-005).
//!
//! Split out of `mod.rs` purely for file-size / review hygiene — no
//! behavioral change. Lock values are stored as JSON blobs rather than
//! typed bincode records because they embed dynamic fence/metadata fields
//! and are touched by a narrow set of operators (acquire/release/renew/
//! force_release/expire).

use tracing::{debug, error};

use super::RocksStateMachine;
use super::json_to_bytes;
use crate::raft::request::RaftResponse;

impl RocksStateMachine {
    pub(super) fn apply_lock_acquire(
        &self,
        namespace: &str,
        name: &str,
        owner: &str,
        ttl_ms: u64,
        fence_token: u64,
        owner_metadata: Option<String>,
    ) -> RaftResponse {
        let key = Self::lock_key(namespace, name);
        let now = chrono::Utc::now().timestamp_millis();
        let expires_at = now + ttl_ms as i64;

        // Check if lock exists and is held
        if let Ok(Some(bytes)) = self.db.get_cf(self.cf_locks(), key.as_bytes())
            && let Ok(existing) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            // Check if lock is still valid (not expired)
            if let Some(exp) = existing["expires_at"].as_i64()
                && exp > now
            {
                // Lock is held, check if same owner
                if existing["owner"].as_str() == Some(owner) {
                    // Reentrant acquire: bump the count, refresh TTL, keep
                    // the original fence_token and acquired_at so fencing
                    // remains stable across nested lock() calls (matches
                    // Nacos AtomicLockService.lock_count semantics).
                    let prev_count = existing["lock_count"].as_u64().unwrap_or(1);
                    let new_count = prev_count.saturating_add(1);
                    let stable_fence_token =
                        existing["fence_token"].as_u64().unwrap_or(fence_token);
                    let value = serde_json::json!({
                        "namespace": namespace,
                        "name": name,
                        "owner": owner,
                        "state": "Locked",
                        "fence_token": stable_fence_token,
                        "lock_count": new_count,
                        "ttl_ms": ttl_ms,
                        "acquired_at": existing["acquired_at"],
                        "expires_at": expires_at,
                        "renewal_count": existing["renewal_count"].as_u64().unwrap_or(0),
                        "owner_metadata": owner_metadata,
                    });

                    match self.db.put_cf_opt(
                        self.cf_locks(),
                        key.as_bytes(),
                        json_to_bytes(&value),
                        &self.write_opts,
                    ) {
                        Ok(_) => {
                            debug!(
                                "Lock reentrant-acquired by same owner: {} (depth={})",
                                key, new_count
                            );
                            return RaftResponse::success_with_data(
                                serde_json::to_vec(&value).unwrap_or_default(),
                            );
                        }
                        Err(e) => {
                            error!("Failed to acquire lock: {}", e);
                            return RaftResponse::failure(format!("Failed to acquire lock: {}", e));
                        }
                    }
                }
                // Lock is held by different owner
                return RaftResponse::failure(format!(
                    "Lock is held by {}",
                    existing["owner"].as_str().unwrap_or("unknown")
                ));
            }
        }

        // Lock is free or expired, acquire it fresh (lock_count starts at 1)
        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": owner,
            "state": "Locked",
            "fence_token": fence_token,
            "lock_count": 1,
            "ttl_ms": ttl_ms,
            "acquired_at": now,
            "expires_at": expires_at,
            "renewal_count": 0,
            "owner_metadata": owner_metadata,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock acquired: {}", key);
                RaftResponse::success_with_data(serde_json::to_vec(&value).unwrap_or_default())
            }
            Err(e) => {
                error!("Failed to acquire lock: {}", e);
                RaftResponse::failure(format!("Failed to acquire lock: {}", e))
            }
        }
    }

    pub(super) fn apply_lock_release(
        &self,
        namespace: &str,
        name: &str,
        owner: &str,
        fence_token: Option<u64>,
    ) -> RaftResponse {
        let key = Self::lock_key(namespace, name);

        // Get existing lock
        let existing = match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        let Some(existing) = existing else {
            return RaftResponse::failure("Lock not found");
        };

        // Check owner
        if existing["owner"].as_str() != Some(owner) {
            return RaftResponse::failure("Not the lock owner");
        }

        // Check fence token if provided
        if let Some(expected_token) = fence_token
            && existing["fence_token"].as_u64() != Some(expected_token)
        {
            return RaftResponse::failure("Fence token mismatch");
        }

        // Reentrant release: decrement lock_count. Only fully release when
        // it reaches zero. Legacy entries without the field are treated as
        // depth=1 for backward compatibility.
        let prev_count = existing["lock_count"].as_u64().unwrap_or(1);
        if prev_count > 1 {
            let new_count = prev_count - 1;
            let mut updated = existing.clone();
            updated["lock_count"] = serde_json::json!(new_count);
            match self.db.put_cf_opt(
                self.cf_locks(),
                key.as_bytes(),
                json_to_bytes(&updated),
                &self.write_opts,
            ) {
                Ok(_) => {
                    debug!("Lock reentrant-released: {} (depth={})", key, new_count);
                    return RaftResponse::success_with_data(
                        serde_json::to_vec(&updated).unwrap_or_default(),
                    );
                }
                Err(e) => {
                    error!("Failed to release lock: {}", e);
                    return RaftResponse::failure(format!("Failed to release lock: {}", e));
                }
            }
        }

        // Fully release the lock (count was 1 or legacy)
        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": null,
            "state": "Free",
            "fence_token": existing["fence_token"],
            "lock_count": 0,
            "ttl_ms": existing["ttl_ms"],
            "acquired_at": null,
            "expires_at": null,
            "renewal_count": 0,
            "owner_metadata": null,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock fully released: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to release lock: {}", e);
                RaftResponse::failure(format!("Failed to release lock: {}", e))
            }
        }
    }

    pub(super) fn apply_lock_renew(
        &self,
        namespace: &str,
        name: &str,
        owner: &str,
        ttl_ms: Option<u64>,
    ) -> RaftResponse {
        let key = Self::lock_key(namespace, name);
        let now = chrono::Utc::now().timestamp_millis();

        // Get existing lock
        let existing = match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        let Some(mut existing) = existing else {
            return RaftResponse::failure("Lock not found");
        };

        // Check owner
        if existing["owner"].as_str() != Some(owner) {
            return RaftResponse::failure("Not the lock owner");
        }

        // Check if lock is still valid
        if let Some(exp) = existing["expires_at"].as_i64()
            && exp <= now
        {
            return RaftResponse::failure("Lock has expired");
        }

        // Update TTL and expires_at
        let new_ttl = ttl_ms.unwrap_or_else(|| existing["ttl_ms"].as_u64().unwrap_or(30000));
        let expires_at = now + new_ttl as i64;
        let renewal_count = existing["renewal_count"].as_u64().unwrap_or(0) + 1;

        existing["ttl_ms"] = serde_json::json!(new_ttl);
        existing["expires_at"] = serde_json::json!(expires_at);
        existing["renewal_count"] = serde_json::json!(renewal_count);

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&existing),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock renewed: {} (renewal #{})", key, renewal_count);
                RaftResponse::success_with_data(serde_json::to_vec(&existing).unwrap_or_default())
            }
            Err(e) => {
                error!("Failed to renew lock: {}", e);
                RaftResponse::failure(format!("Failed to renew lock: {}", e))
            }
        }
    }

    pub(super) fn apply_lock_force_release(&self, namespace: &str, name: &str) -> RaftResponse {
        let key = Self::lock_key(namespace, name);

        // Force release regardless of owner
        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": null,
            "state": "Free",
            "fence_token": 0,
            "ttl_ms": 0,
            "acquired_at": null,
            "expires_at": null,
            "renewal_count": 0,
            "owner_metadata": null,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock force released: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to force release lock: {}", e);
                RaftResponse::failure(format!("Failed to force release lock: {}", e))
            }
        }
    }

    pub(super) fn apply_lock_expire(&self, namespace: &str, name: &str) -> RaftResponse {
        let key = Self::lock_key(namespace, name);

        // Check existence without deserialization (saves JSON parse overhead)
        match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(_)) => {}                    // Key exists, proceed to expire
            _ => return RaftResponse::success(), // Nothing to expire
        }

        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": null,
            "state": "Expired",
            "fence_token": 0,
            "ttl_ms": 0,
            "acquired_at": null,
            "expires_at": null,
            "renewal_count": 0,
            "owner_metadata": null,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock expired: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to expire lock: {}", e);
                RaftResponse::failure(format!("Failed to expire lock: {}", e))
            }
        }
    }
}
