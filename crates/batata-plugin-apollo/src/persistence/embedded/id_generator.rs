use std::sync::atomic::{AtomicI32, Ordering};

pub struct IdGenerator {
    next_id: AtomicI32,
}

impl IdGenerator {
    pub fn new(start_id: i32) -> Self {
        Self {
            next_id: AtomicI32::new(start_id),
        }
    }

    pub fn next_id(&self) -> i32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}
