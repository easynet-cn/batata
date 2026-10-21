//! Naming event listener trait and event types

use batata_api::naming::model::Instance;

use super::instances_diff::InstancesDiff;

/// Event delivered to naming listeners when a service's instance list changes.
#[derive(Clone, Debug)]
pub struct NamingEvent {
    /// Name of the service that changed.
    pub service_name: String,
    /// Group of the service.
    pub group_name: String,
    /// Cluster filter for the subscription (empty = all clusters).
    pub clusters: String,
    /// Current full instance list for the service.
    pub instances: Vec<Instance>,
    /// Diff between old and new instances (None on first subscription)
    pub diff: Option<InstancesDiff>,
}

/// Trait for receiving naming service change events.
///
/// Implement this to be notified when a subscribed service's instance list changes.
///
/// # Callback context
///
/// `on_event` is invoked synchronously from the client's push-dispatch task,
/// i.e. on a Tokio runtime thread. The callback must therefore **never block**:
///
/// - No `tokio::sync::Mutex::blocking_lock()` — it panics inside a runtime and
///   stalls every other task on a `current_thread` runtime.
/// - No long-held `std::sync::Mutex` that is also taken across an `.await` in
///   the caller — that deadlocks the runtime (the notification cannot be
///   delivered while the caller waits for it).
/// - No `block_on` / blocking wait on an async call into the same client: the
///   callback is synchronous, and the work it waits on needs the runtime too.
///
/// The subscription lock is released *before* the callback runs, so
/// subscribing/unsubscribing from a callback does not deadlock — but
/// dispatching anything async still has to go through `tokio::spawn`.
///
/// Use an atomic, a channel, or spawn a task instead.
pub trait EventListener: Send + Sync + 'static {
    /// Called when the service's instance list has changed.
    fn on_event(&self, event: NamingEvent);
}

/// A simple listener that invokes a closure.
pub struct FnEventListener<F>
where
    F: Fn(NamingEvent) + Send + Sync + 'static,
{
    f: F,
}

impl<F> FnEventListener<F>
where
    F: Fn(NamingEvent) + Send + Sync + 'static,
{
    /// Create a listener that invokes the given closure on each event.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> EventListener for FnEventListener<F>
where
    F: Fn(NamingEvent) + Send + Sync + 'static,
{
    fn on_event(&self, event: NamingEvent) {
        (self.f)(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_fn_event_listener() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let listener = FnEventListener::new(move |event: NamingEvent| {
            assert_eq!(event.service_name, "my-service");
            called_clone.store(true, Ordering::SeqCst);
        });

        listener.on_event(NamingEvent {
            service_name: "my-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            clusters: String::new(),
            instances: vec![],
            diff: None,
        });

        assert!(called.load(Ordering::SeqCst));
    }
}
