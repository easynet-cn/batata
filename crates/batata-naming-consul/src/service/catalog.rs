//! Consul Catalog Service — cluster-wide service directory

use crate::model::{CatalogNode, CatalogService, ServiceEntry};

use super::ConsulNamingServiceImpl;

impl ConsulNamingServiceImpl {
    /// List all services in the catalog: name -> tags
    pub fn catalog_list_services(&self) -> (std::collections::HashMap<String, Vec<String>>, u64) {
        let names = self.list_service_names();
        let index = self.index.current();
        (names, index)
    }

    /// Get all instances of a service from the catalog
    pub fn catalog_get_service(
        &self,
        service_name: &str,
        tag: Option<&str>,
        passing_only: bool,
    ) -> (Vec<CatalogService>, u64) {
        let index = self.index.current();

        let results: Vec<CatalogService> = self
            .services
            .iter()
            .filter(|entry| {
                let svc = &entry.value().service;
                svc.service == service_name
                    && tag
                        .map(|t| svc.tags.contains(&t.to_string()))
                        .unwrap_or(true)
                    && (!passing_only || self.is_service_healthy(entry.key()))
            })
            .map(|entry| {
                let stored = entry.value();
                CatalogService {
                    id: self.dc_config.node_id.clone(),
                    node: self.dc_config.node_name.clone(),
                    address: stored.service.address.clone(),
                    datacenter: self.dc_config.datacenter.clone(),
                    service_id: stored.service.id.clone(),
                    service_name: stored.service.service.clone(),
                    service_address: stored.service.address.clone(),
                    service_port: stored.service.port,
                    service_tags: stored.service.tags.clone(),
                    service_meta: stored.service.meta.clone(),
                    service_weights: stored.service.weights.clone(),
                    service_kind: stored.service.kind,
                    service_enable_tag_override: stored.service.enable_tag_override,
                }
            })
            .collect();

        (results, index)
    }

    /// Get health entries for a service (with checks)
    pub fn catalog_get_service_health(
        &self,
        service_name: &str,
        tag: Option<&str>,
        passing_only: bool,
    ) -> (Vec<ServiceEntry>, u64) {
        let index = self.index.current();

        let results: Vec<ServiceEntry> = self
            .services
            .iter()
            .filter(|entry| {
                let svc = &entry.value().service;
                svc.service == service_name
                    && tag
                        .map(|t| svc.tags.contains(&t.to_string()))
                        .unwrap_or(true)
            })
            .filter(|entry| {
                if passing_only {
                    self.is_service_healthy(entry.key())
                } else {
                    true
                }
            })
            .map(|entry| {
                let stored = entry.value();
                ServiceEntry {
                    node: CatalogNode {
                        id: self.dc_config.node_id.clone(),
                        node: self.dc_config.node_name.clone(),
                        address: stored.service.address.clone(),
                        datacenter: self.dc_config.datacenter.clone(),
                        meta: std::collections::HashMap::new(),
                    },
                    service: stored.service.clone(),
                    checks: stored.checks.clone(),
                }
            })
            .collect();

        (results, index)
    }

    /// List all nodes
    pub fn catalog_list_nodes(&self) -> (Vec<CatalogNode>, u64) {
        let node = CatalogNode {
            id: self.dc_config.node_id.clone(),
            node: self.dc_config.node_name.clone(),
            address: "127.0.0.1".to_string(),
            datacenter: self.dc_config.datacenter.clone(),
            meta: std::collections::HashMap::new(),
        };
        (vec![node], self.index.current())
    }

    /// Check if all checks for a service are passing
    fn is_service_healthy(&self, service_id: &str) -> bool {
        self.services
            .get(service_id)
            .map(|stored| {
                stored.checks.is_empty() || stored.checks.iter().all(|c| c.status.is_passing())
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentServiceRegistration, CheckDefinition, CheckStatus, ServiceKind};

    fn test_reg(name: &str, id: &str, port: u16) -> AgentServiceRegistration {
        AgentServiceRegistration {
            id: Some(id.to_string()),
            name: name.to_string(),
            tags: vec!["web".to_string()],
            port,
            address: "10.0.0.1".to_string(),
            meta: std::collections::HashMap::new(),
            enable_tag_override: false,
            weights: None,
            kind: ServiceKind::Typical,
            proxy: None,
            connect: None,
            tagged_addresses: std::collections::HashMap::new(),
            check: None,
            checks: None,
        }
    }

    #[test]
    fn test_catalog_list_services() {
        let svc = ConsulNamingServiceImpl::default();
        svc.register_service(test_reg("api", "api-1", 8080))
            .unwrap();
        svc.register_service(test_reg("api", "api-2", 8081))
            .unwrap();
        svc.register_service(test_reg("db", "db-1", 5432)).unwrap();

        let (services, _) = svc.catalog_list_services();
        assert_eq!(services.len(), 2);
        assert!(services.contains_key("api"));
        assert!(services.contains_key("db"));
    }

    #[test]
    fn test_catalog_get_service() {
        let svc = ConsulNamingServiceImpl::default();
        svc.register_service(test_reg("api", "api-1", 8080))
            .unwrap();
        svc.register_service(test_reg("api", "api-2", 8081))
            .unwrap();

        let (results, _) = svc.catalog_get_service("api", None, false);
        assert_eq!(results.len(), 2);

        // Filter by tag
        let (results, _) = svc.catalog_get_service("api", Some("web"), false);
        assert_eq!(results.len(), 2);

        let (results, _) = svc.catalog_get_service("api", Some("nonexistent"), false);
        assert!(results.is_empty());
    }

    #[test]
    fn test_catalog_health_filtering() {
        let svc = ConsulNamingServiceImpl::default();

        let mut reg = test_reg("api", "api-1", 8080);
        reg.check = Some(CheckDefinition {
            check_id: Some("api-check".to_string()),
            name: "API check".to_string(),
            service_id: None,
            ttl: Some("15s".to_string()),
            http: None,
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            grpc_use_tls: false,
            interval: None,
            timeout: None,
            deregister_critical_service_after: None,
            status: Some(CheckStatus::Critical), // starts critical
            notes: String::new(),
        });
        svc.register_service(reg).unwrap();

        // passing_only should exclude critical
        let (results, _) = svc.catalog_get_service("api", None, true);
        assert!(results.is_empty());

        // Update to passing
        svc.update_check("api-check", CheckStatus::Passing, "ok");
        let (results, _) = svc.catalog_get_service("api", None, true);
        assert_eq!(results.len(), 1);
    }
}
