//! Core health check result handler
//!
//! Implements `HealthCheckResultHandler` for the batata core (Nacos) naming service.
//! Updates `Instance.healthy` in `NamingService` when check status changes.

use std::sync::Arc;

use batata_api::naming::NamingServiceProvider;
use batata_plugin::HealthCheckResultHandler;

/// Core result handler — updates NamingService instance health.
///
/// This is the built-in handler for batata's core Nacos-compatible naming service.
/// It maps the tri-state health (passing/warning/critical) to Nacos's binary
/// healthy/unhealthy model: passing/warning → healthy, critical → unhealthy.
pub struct CoreResultHandler {
    naming_service: Arc<dyn NamingServiceProvider>,
}

impl CoreResultHandler {
    pub fn new(naming_service: Arc<dyn NamingServiceProvider>) -> Self {
        Self { naming_service }
    }
}

impl HealthCheckResultHandler for CoreResultHandler {
    fn on_health_changed(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
        healthy: bool,
    ) {
        self.naming_service
            .update_instance_health(namespace, group, service, ip, port, cluster, healthy);
    }

    fn on_deregister(
        &self,
        namespace: &str,
        group: &str,
        service: &str,
        ip: &str,
        port: i32,
        cluster: &str,
    ) {
        let instance = batata_api::naming::Instance {
            ip: ip.to_string(),
            port,
            cluster_name: cluster.to_string(),
            ephemeral: false,
            ..Default::default()
        };
        self.naming_service
            .deregister_instance(namespace, group, service, &instance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_result_handler() {
        let naming = Arc::new(crate::service::NamingService::new());

        // Register an instance
        let instance = batata_api::naming::Instance {
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            healthy: true,
            ..Default::default()
        };
        naming.register_instance("public", "DEFAULT_GROUP", "test-svc", instance);

        let handler = CoreResultHandler::new(naming.clone());

        // Mark unhealthy
        handler.on_health_changed(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
            false,
        );

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(!instances[0].healthy);

        // Mark healthy again
        handler.on_health_changed(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
            true,
        );

        let instances = naming.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(instances[0].healthy);
    }

    #[test]
    fn test_core_result_handler_deregister() {
        let naming = Arc::new(crate::service::NamingService::new());

        let instance = batata_api::naming::Instance {
            ip: "10.0.0.1".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        };
        naming.register_instance("public", "DEFAULT_GROUP", "test-svc", instance);
        assert_eq!(
            naming
                .get_instances("public", "DEFAULT_GROUP", "test-svc", "", false)
                .len(),
            1
        );

        let handler = CoreResultHandler::new(naming.clone());
        handler.on_deregister(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
        );

        assert!(
            naming
                .get_instances("public", "DEFAULT_GROUP", "test-svc", "", false)
                .is_empty()
        );
    }
}
