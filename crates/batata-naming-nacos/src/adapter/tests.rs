//! Integration tests proving the adapter works with the legacy NamingServiceProvider trait

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use batata_api::naming::{NamingServiceProvider, RegisterSource};

    use crate::service::NacosNamingServiceImpl;

    /// Create a NacosNamingServiceImpl and use it through the legacy trait.
    /// This proves the adapter is a valid drop-in replacement.
    fn create_provider() -> Arc<dyn NamingServiceProvider> {
        Arc::new(NacosNamingServiceImpl::new())
    }

    fn api_instance(ip: &str, port: i32) -> batata_api::naming::Instance {
        batata_api::naming::Instance {
            instance_id: format!("{ip}#{port}#DEFAULT#test-svc"),
            ip: ip.to_string(),
            port,
            weight: 1.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-svc".to_string(),
            metadata: Default::default(),
            register_source: RegisterSource::Batata,
        }
    }

    #[test]
    fn test_adapter_register_and_get() {
        let provider = create_provider();
        let inst = api_instance("10.0.0.1", 8080);

        assert!(provider.register_instance("public", "DEFAULT_GROUP", "test-svc", inst));

        let instances = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].ip, "10.0.0.1");
        assert_eq!(instances[0].port, 8080);
        assert!(instances[0].healthy);
        assert_eq!(instances[0].register_source, RegisterSource::Batata);
    }

    #[test]
    fn test_adapter_deregister() {
        let provider = create_provider();
        let inst = api_instance("10.0.0.1", 8080);

        provider.register_instance("public", "DEFAULT_GROUP", "test-svc", inst.clone());
        assert!(provider.service_exists("public", "DEFAULT_GROUP", "test-svc"));

        provider.deregister_instance("public", "DEFAULT_GROUP", "test-svc", &inst);

        let instances = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(instances.is_empty());
    }

    #[test]
    fn test_adapter_get_service_with_protection() {
        let provider = create_provider();

        let healthy = api_instance("10.0.0.1", 8080);
        let mut unhealthy = api_instance("10.0.0.2", 8080);
        unhealthy.healthy = false;

        provider.register_instance("public", "DEFAULT_GROUP", "test-svc", healthy);
        provider.register_instance("public", "DEFAULT_GROUP", "test-svc", unhealthy);

        // Set high threshold
        provider.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-svc", 0.8);

        let (svc, info) = provider.get_service_with_protection_info(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "",
            true,
        );

        // Protection triggered (0.5 < 0.8), so all instances returned
        assert!(info.triggered);
        assert_eq!(svc.hosts.len(), 2);
    }

    #[test]
    fn test_adapter_list_services() {
        let provider = create_provider();

        provider.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-a",
            api_instance("10.0.0.1", 8080),
        );
        provider.register_instance(
            "public",
            "DEFAULT_GROUP",
            "svc-b",
            api_instance("10.0.0.2", 8080),
        );

        let (total, names) = provider.list_services("public", "DEFAULT_GROUP", 1, 10);
        assert_eq!(total, 2);
        assert!(names.contains(&"svc-a".to_string()));
        assert!(names.contains(&"svc-b".to_string()));
    }

    #[test]
    fn test_adapter_subscribe_and_get() {
        let provider = create_provider();

        provider.subscribe("conn-1", "public", "DEFAULT_GROUP", "test-svc");
        let subs = provider.get_subscribers("public", "DEFAULT_GROUP", "test-svc");
        assert_eq!(subs, vec!["conn-1"]);

        provider.unsubscribe("conn-1", "public", "DEFAULT_GROUP", "test-svc");
        let subs = provider.get_subscribers("public", "DEFAULT_GROUP", "test-svc");
        assert!(subs.is_empty());
    }

    #[test]
    fn test_adapter_heartbeat() {
        let provider = create_provider();
        let inst = api_instance("10.0.0.1", 8080);

        provider.register_instance("public", "DEFAULT_GROUP", "test-svc", inst.clone());

        // Mark unhealthy
        provider.update_instance_health(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            "10.0.0.1",
            8080,
            "DEFAULT",
            false,
        );

        let instances = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(!instances[0].healthy);

        // Heartbeat restores health
        assert!(provider.heartbeat("public", "DEFAULT_GROUP", "test-svc", inst));

        let instances = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(instances[0].healthy);
    }

    #[test]
    fn test_adapter_batch_operations() {
        let provider = create_provider();

        let instances = vec![
            api_instance("10.0.0.1", 8080),
            api_instance("10.0.0.2", 8080),
            api_instance("10.0.0.3", 8080),
        ];

        assert!(provider.batch_register_instances(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            instances.clone(),
        ));

        let result = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(result.len(), 3);

        // Batch deregister 2
        assert!(provider.batch_deregister_instances(
            "public",
            "DEFAULT_GROUP",
            "test-svc",
            vec![
                api_instance("10.0.0.1", 8080),
                api_instance("10.0.0.2", 8080)
            ],
        ));

        let result = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ip, "10.0.0.3");
    }

    #[test]
    fn test_adapter_connection_tracking() {
        let provider = create_provider();
        let inst = api_instance("10.0.0.1", 8080);

        provider.register_instance("public", "DEFAULT_GROUP", "test-svc", inst);

        let service_key = "public@@DEFAULT_GROUP@@test-svc";
        let instance_key = "10.0.0.1#8080#DEFAULT";
        provider.add_connection_instance("conn-1", service_key, instance_key);

        // Disconnect should clean up
        let affected = provider.deregister_all_by_connection("conn-1");
        assert_eq!(affected.len(), 1);

        let result = provider.get_instances("public", "DEFAULT_GROUP", "test-svc", "", false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_adapter_cluster_config() {
        let provider = create_provider();

        provider
            .create_cluster_config(
                "public",
                "DEFAULT_GROUP",
                "test-svc",
                "cluster-1",
                "TCP",
                8080,
                true,
                Default::default(),
            )
            .unwrap();

        let config =
            provider.get_cluster_config("public", "DEFAULT_GROUP", "test-svc", "cluster-1");
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.name, "cluster-1");
        assert_eq!(config.health_check_type, "TCP");
        assert_eq!(config.check_port, 8080);
        assert!(config.use_instance_port);
    }

    #[test]
    fn test_adapter_metadata() {
        let provider = create_provider();
        let inst = api_instance("10.0.0.1", 8080);
        provider.register_instance("public", "DEFAULT_GROUP", "test-svc", inst);

        // Set and get metadata
        let meta = provider.get_service_metadata("public", "DEFAULT_GROUP", "test-svc");
        assert!(meta.is_some());

        // Update protect threshold
        provider.update_service_protect_threshold("public", "DEFAULT_GROUP", "test-svc", 0.5);
        let meta = provider
            .get_service_metadata("public", "DEFAULT_GROUP", "test-svc")
            .unwrap();
        assert!((meta.protect_threshold - 0.5).abs() < f32::EPSILON);
    }
}
