//! Naming service data models
//!
//! This module re-exports types from batata_api::naming::model for unified type system.

// Re-export all naming model types from batata-api
pub use batata_api::naming::model::{
    // Constants
    DE_REGISTER_INSTANCE,
    // Core types
    HeartbeatForm,
    INSTANCE_TYPE_EPHEMERAL,
    INSTANCE_TYPE_PERSISTENT,
    Instance,
    InstanceRegisterForm,
    REGISTER_INSTANCE,
    Service,
    ServiceInfo,
    ServiceQuery,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_default() {
        let inst = Instance::default();
        assert!(inst.ip.is_empty());
        assert_eq!(inst.port, 0);
        assert_eq!(inst.weight, 0.0);
    }

    #[test]
    fn test_instance_new() {
        let inst = Instance::new("127.0.0.1".to_string(), 8080);
        assert_eq!(inst.ip, "127.0.0.1");
        assert_eq!(inst.port, 8080);
        assert_eq!(inst.weight, 1.0);
        assert!(inst.healthy);
        assert!(inst.enabled);
        assert!(inst.ephemeral);
    }

    #[test]
    fn test_instance_key() {
        let inst = Instance {
            ip: "192.168.1.100".to_string(),
            port: 8080,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        };
        assert_eq!(inst.key(), "192.168.1.100#8080#DEFAULT");
    }

    #[test]
    fn test_service_new() {
        let svc = Service::new("my-service".to_string(), "DEFAULT_GROUP".to_string());
        assert_eq!(svc.name, "my-service");
        assert_eq!(svc.group_name, "DEFAULT_GROUP");
        assert_eq!(svc.cache_millis, 10000);
    }

    #[test]
    fn test_service_healthy_hosts() {
        let svc = Service {
            hosts: vec![
                Instance {
                    healthy: true,
                    enabled: true,
                    ..Default::default()
                },
                Instance {
                    healthy: false,
                    enabled: true,
                    ..Default::default()
                },
                Instance {
                    healthy: true,
                    enabled: false,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(svc.healthy_hosts().len(), 1);
    }

    #[test]
    fn test_instance_register_form_to_instance() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            weight: 2.0,
            enabled: true,
            healthy: true,
            ephemeral: false,
            cluster_name: "PROD".to_string(),
            service_name: "api-gateway".to_string(),
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.ip, "10.0.0.1");
        assert_eq!(inst.port, 9000);
        assert_eq!(inst.weight, 2.0);
        assert_eq!(inst.cluster_name, "PROD");
        assert!(!inst.ephemeral);
    }

    #[test]
    fn test_instance_register_form_zero_weight() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            weight: 0.0,
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.weight, 1.0);
    }

    #[test]
    fn test_instance_register_form_empty_cluster_name() {
        let form = InstanceRegisterForm {
            ip: "10.0.0.1".to_string(),
            port: 9000,
            cluster_name: "".to_string(),
            ..Default::default()
        };
        let inst = form.to_instance();
        assert_eq!(inst.cluster_name, "DEFAULT");
    }

    #[test]
    fn test_instance_type_constants() {
        assert_eq!(INSTANCE_TYPE_EPHEMERAL, "ephemeral");
        assert_eq!(INSTANCE_TYPE_PERSISTENT, "persistent");
    }

    #[test]
    fn test_request_type_constants() {
        assert_eq!(REGISTER_INSTANCE, "registerInstance");
        assert_eq!(DE_REGISTER_INSTANCE, "deregisterInstance");
    }
}
