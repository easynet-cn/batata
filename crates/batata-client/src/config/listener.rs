//! Config change listener trait and notification types

/// Information about a config change delivered to listeners.
#[derive(Clone, Debug)]
pub struct ConfigResponse {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
}

/// Trait for receiving config change notifications.
///
/// Implement this trait to be notified when a config value changes on the server.
pub trait ConfigChangeListener: Send + Sync + 'static {
    /// Called when the config content has changed.
    fn receive_config_info(&self, config_info: ConfigResponse);
}

/// A simple listener that invokes a closure.
pub struct FnConfigChangeListener<F>
where
    F: Fn(ConfigResponse) + Send + Sync + 'static,
{
    f: F,
}

impl<F> FnConfigChangeListener<F>
where
    F: Fn(ConfigResponse) + Send + Sync + 'static,
{
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> ConfigChangeListener for FnConfigChangeListener<F>
where
    F: Fn(ConfigResponse) + Send + Sync + 'static,
{
    fn receive_config_info(&self, config_info: ConfigResponse) {
        (self.f)(config_info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_fn_listener() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let listener = FnConfigChangeListener::new(move |info: ConfigResponse| {
            assert_eq!(info.data_id, "test-id");
            called_clone.store(true, Ordering::SeqCst);
        });

        listener.receive_config_info(ConfigResponse {
            data_id: "test-id".to_string(),
            group: "DEFAULT_GROUP".to_string(),
            tenant: "".to_string(),
            content: "test content".to_string(),
        });

        assert!(called.load(Ordering::SeqCst));
    }
}
