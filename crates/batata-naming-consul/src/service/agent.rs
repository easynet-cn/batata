//! Consul Agent Service — local service registration and management

use crate::error::ConsulNamingError;
use crate::model::{
    AgentService, AgentServiceRegistration, CheckDefinition, CheckStatus, HealthCheck,
};

use super::{ConsulNamingServiceImpl, StoredService};

impl ConsulNamingServiceImpl {
    /// Register a service on the local agent
    pub fn register_service(
        &self,
        registration: AgentServiceRegistration,
    ) -> Result<(), ConsulNamingError> {
        let service_id = registration
            .id
            .clone()
            .unwrap_or_else(|| registration.name.clone());

        let modify_index = self.index.next();

        // Build AgentService from registration
        let agent_service = AgentService {
            id: service_id.clone(),
            service: registration.name.clone(),
            tags: registration.tags.clone(),
            port: registration.port,
            address: registration.address.clone(),
            meta: registration.meta.clone(),
            weights: registration.weights.clone().unwrap_or_default(),
            enable_tag_override: registration.enable_tag_override,
            kind: registration.kind,
            datacenter: self.dc_config.datacenter.clone(),
        };

        // Process health checks
        let mut checks = Vec::new();
        if let Some(check) = &registration.check {
            checks.push(self.build_health_check(&service_id, &registration.name, check));
        }
        if let Some(check_list) = &registration.checks {
            for check in check_list {
                checks.push(self.build_health_check(&service_id, &registration.name, check));
            }
        }

        // Register checks in the check registry
        for check in &checks {
            self.checks.insert(check.check_id.clone(), check.clone());
        }

        let create_index = self
            .services
            .get(&service_id)
            .map(|s| s.create_index)
            .unwrap_or(modify_index);

        self.services.insert(
            service_id,
            StoredService {
                service: agent_service,
                checks,
                create_index,
                modify_index,
            },
        );

        self.notify_service_change(&registration.name);
        Ok(())
    }

    /// Deregister a service by ID
    pub fn deregister_service(&self, service_id: &str) -> Result<(), ConsulNamingError> {
        if let Some((_, stored)) = self.services.remove(service_id) {
            // Remove associated checks
            for check in &stored.checks {
                self.checks.remove(&check.check_id);
            }
            self.notify_service_change(&stored.service.service);
            Ok(())
        } else {
            // Idempotent — not an error
            Ok(())
        }
    }

    /// List all services on this agent
    pub fn list_services(&self) -> std::collections::HashMap<String, AgentService> {
        self.services
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().service.clone()))
            .collect()
    }

    /// Get a specific service by ID
    pub fn get_service(&self, service_id: &str) -> Option<AgentService> {
        self.services.get(service_id).map(|s| s.service.clone())
    }

    /// Get all service names with their tags
    pub fn list_service_names(&self) -> std::collections::HashMap<String, Vec<String>> {
        let mut result: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for entry in self.services.iter() {
            let svc = &entry.value().service;
            result
                .entry(svc.service.clone())
                .or_default()
                .extend(svc.tags.iter().cloned());
        }
        // Deduplicate tags
        for tags in result.values_mut() {
            tags.sort();
            tags.dedup();
        }
        result
    }

    /// Build a HealthCheck from a CheckDefinition
    fn build_health_check(
        &self,
        service_id: &str,
        service_name: &str,
        def: &CheckDefinition,
    ) -> HealthCheck {
        let check_id = def
            .check_id
            .clone()
            .unwrap_or_else(|| format!("service:{service_id}"));

        let check_type = if def.ttl.is_some() {
            "ttl"
        } else if def.http.is_some() {
            "http"
        } else if def.tcp.is_some() {
            "tcp"
        } else if def.grpc.is_some() {
            "grpc"
        } else {
            "unknown"
        };

        let initial_status = def.status.unwrap_or(CheckStatus::Critical);

        HealthCheck {
            check_id,
            name: def.name.clone(),
            node: self.dc_config.node_name.clone(),
            status: initial_status,
            notes: def.notes.clone(),
            output: String::new(),
            service_id: service_id.to_string(),
            service_name: service_name.to_string(),
            check_type: check_type.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ServiceKind;

    fn test_registration(name: &str, port: u16) -> AgentServiceRegistration {
        AgentServiceRegistration {
            id: Some(format!("{name}-1")),
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
    fn test_register_and_list() {
        let svc = ConsulNamingServiceImpl::default();

        svc.register_service(test_registration("web-api", 8080))
            .unwrap();
        svc.register_service(test_registration("db", 5432)).unwrap();

        let services = svc.list_services();
        assert_eq!(services.len(), 2);
        assert!(services.contains_key("web-api-1"));
        assert!(services.contains_key("db-1"));
    }

    #[test]
    fn test_deregister() {
        let svc = ConsulNamingServiceImpl::default();
        svc.register_service(test_registration("web-api", 8080))
            .unwrap();

        assert!(svc.get_service("web-api-1").is_some());
        svc.deregister_service("web-api-1").unwrap();
        assert!(svc.get_service("web-api-1").is_none());
    }

    #[test]
    fn test_service_names_with_tags() {
        let svc = ConsulNamingServiceImpl::default();

        let mut reg1 = test_registration("web-api", 8080);
        reg1.tags = vec!["v1".to_string(), "prod".to_string()];
        let mut reg2 = test_registration("web-api", 8081);
        reg2.id = Some("web-api-2".to_string());
        reg2.tags = vec!["v2".to_string(), "prod".to_string()];

        svc.register_service(reg1).unwrap();
        svc.register_service(reg2).unwrap();

        let names = svc.list_service_names();
        assert_eq!(names.len(), 1);
        let tags = names.get("web-api").unwrap();
        assert!(tags.contains(&"prod".to_string()));
        assert!(tags.contains(&"v1".to_string()));
        assert!(tags.contains(&"v2".to_string()));
    }

    #[test]
    fn test_register_with_checks() {
        let svc = ConsulNamingServiceImpl::default();

        let mut reg = test_registration("web-api", 8080);
        reg.check = Some(CheckDefinition {
            check_id: Some("web-api-http".to_string()),
            name: "HTTP check".to_string(),
            service_id: None,
            ttl: None,
            http: Some("http://localhost:8080/health".to_string()),
            method: None,
            header: None,
            tcp: None,
            grpc: None,
            grpc_use_tls: false,
            interval: Some("10s".to_string()),
            timeout: Some("5s".to_string()),
            deregister_critical_service_after: None,
            status: Some(CheckStatus::Passing),
            notes: String::new(),
        });

        svc.register_service(reg).unwrap();

        // Check was registered
        let checks: Vec<_> = svc.checks.iter().map(|e| e.key().clone()).collect();
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0], "web-api-http");

        // Deregister removes checks too
        svc.deregister_service("web-api-1").unwrap();
        assert_eq!(svc.checks.len(), 0);
    }
}
