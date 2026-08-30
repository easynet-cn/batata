use std::sync::atomic::{AtomicI32, Ordering};

/// Represents the `IdGenerator` entity.
pub struct IdGenerator {
    next_id: AtomicI32,
}

impl IdGenerator {
    /// Creates a new `IdGenerator`.
    pub fn new(start_id: i32) -> Self {
        Self {
            next_id: AtomicI32::new(start_id),
        }
    }

    /// Performs the `next_id` operation.
    pub fn next_id(&self) -> i32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}
