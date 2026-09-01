use std::sync::atomic::{AtomicI64, Ordering};

/// Represents the `IdGenerator` entity.
///
/// IDs are 64-bit to match the `i64` primary keys used across all Apollo
/// entities (PostgreSQL/MySQL `BIGINT`), avoiding the `INT` 2.1-billion ceiling
/// on high-churn tables such as `release_history` and `commit`.
pub struct IdGenerator {
    next_id: AtomicI64,
}

impl IdGenerator {
    /// Creates a new `IdGenerator`.
    pub fn new(start_id: i64) -> Self {
        Self {
            next_id: AtomicI64::new(start_id),
        }
    }

    /// Returns the next monotonically increasing 64-bit id.
    pub fn next_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}
