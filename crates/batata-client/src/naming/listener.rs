//! Naming event listener trait and event types

use batata_api::naming::model::Instance;

/// Event delivered to naming listeners when a service's instance list changes.
#[derive(Clone, Debug)]
pub struct NamingEvent {
    pub service_name: String,
    pub group_name: String,
    pub clusters: String,
    pub instances: Vec<Instance>,
}

/// Trait for receiving naming service change events.
///
/// Implement this to be notified when a subscribed service's instance list changes.
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
        });

        assert!(called.load(Ordering::SeqCst));
    }
}
