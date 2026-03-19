//! Connection limit checker implementation that bridges batata-core's
//! ConnectionLimitChecker trait with batata-plugin's ControlPlugin.

use std::sync::Arc;

use batata_core::service::remote::ConnectionLimitChecker;
use batata_plugin::{ControlContext, ControlPlugin};

/// Connection limit checker backed by the control plugin.
pub struct ControlPluginConnectionLimiter {
    control_plugin: Arc<dyn ControlPlugin>,
}

impl ControlPluginConnectionLimiter {
    pub fn new(control_plugin: Arc<dyn ControlPlugin>) -> Self {
        Self { control_plugin }
    }
}

#[async_trait::async_trait]
impl ConnectionLimitChecker for ControlPluginConnectionLimiter {
    async fn check_connection(&self, client_ip: &str, client_id: &str) -> bool {
        let ctx = ControlContext::new()
            .with_ip(client_ip)
            .with_client_id(client_id);

        let result = self.control_plugin.check_connection_limit(&ctx).await;
        result.allowed
    }

    async fn release_connection(&self, client_ip: &str, client_id: &str) {
        let ctx = ControlContext::new()
            .with_ip(client_ip)
            .with_client_id(client_id);

        self.control_plugin.release_connection(&ctx).await;
    }
}
