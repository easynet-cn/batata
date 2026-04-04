//! Consul Health Service — check management and health queries

use crate::model::{CheckStatus, HealthCheck};

use super::ConsulNamingServiceImpl;

impl ConsulNamingServiceImpl {
    // ========================================================================
    // Check Registration
    // ========================================================================

    /// Register a standalone health check
    pub fn register_check(&self, check: HealthCheck) {
        self.checks.insert(check.check_id.clone(), check);
    }

    /// Deregister a health check
    pub fn deregister_check(&self, check_id: &str) {
        self.checks.remove(check_id);
        // Also remove from service's check list
        for mut entry in self.services.iter_mut() {
            entry.value_mut().checks.retain(|c| c.check_id != check_id);
        }
    }

    // ========================================================================
    // TTL Check Updates
    // ========================================================================

    /// Update a check's status and output
    pub fn update_check(&self, check_id: &str, status: CheckStatus, output: &str) {
        // Update in check registry
        if let Some(mut check) = self.checks.get_mut(check_id) {
            check.status = status;
            check.output = output.to_string();
        }

        // Update in service's embedded checks
        for mut entry in self.services.iter_mut() {
            let stored = entry.value_mut();
            for check in &mut stored.checks {
                if check.check_id == check_id {
                    check.status = status;
                    check.output = output.to_string();
                    stored.modify_index = self.index.next();
                }
            }
        }
    }

    /// Pass a TTL check
    pub fn pass_check(&self, check_id: &str, note: &str) {
        self.update_check(check_id, CheckStatus::Passing, note);
    }

    /// Warn a TTL check
    pub fn warn_check(&self, check_id: &str, note: &str) {
        self.update_check(check_id, CheckStatus::Warning, note);
    }

    /// Fail a TTL check
    pub fn fail_check(&self, check_id: &str, note: &str) {
        self.update_check(check_id, CheckStatus::Critical, note);
    }

    // ========================================================================
    // Health Queries
    // ========================================================================

    /// Get all checks on the agent
    pub fn list_checks(&self) -> std::collections::HashMap<String, HealthCheck> {
        self.checks
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get checks for a specific service
    pub fn get_service_checks(&self, service_id: &str) -> Vec<HealthCheck> {
        self.services
            .get(service_id)
            .map(|s| s.checks.clone())
            .unwrap_or_default()
    }

    /// Get a single check by ID
    pub fn get_check(&self, check_id: &str) -> Option<HealthCheck> {
        self.checks.get(check_id).map(|c| c.clone())
    }

    /// Get all checks in a specific state
    pub fn get_checks_by_state(&self, state: CheckStatus) -> Vec<HealthCheck> {
        self.checks
            .iter()
            .filter(|entry| entry.value().status == state)
            .map(|entry| entry.value().clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentServiceRegistration, CheckDefinition, ServiceKind};

    #[test]
    fn test_ttl_check_lifecycle() {
        let svc = ConsulNamingServiceImpl::default();

        // Register service with TTL check
        let reg = AgentServiceRegistration {
            id: Some("web-1".to_string()),
            name: "web".to_string(),
            tags: vec![],
            port: 8080,
            address: "10.0.0.1".to_string(),
            meta: std::collections::HashMap::new(),
            enable_tag_override: false,
            weights: None,
            kind: ServiceKind::Typical,
            proxy: None,
            connect: None,
            tagged_addresses: std::collections::HashMap::new(),
            check: Some(CheckDefinition {
                check_id: Some("web-ttl".to_string()),
                name: "TTL check".to_string(),
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
                status: Some(CheckStatus::Critical),
                notes: String::new(),
            }),
            checks: None,
        };
        svc.register_service(reg).unwrap();

        // Initially critical
        let check = svc.get_check("web-ttl").unwrap();
        assert_eq!(check.status, CheckStatus::Critical);

        // Pass the check
        svc.pass_check("web-ttl", "all good");
        let check = svc.get_check("web-ttl").unwrap();
        assert_eq!(check.status, CheckStatus::Passing);
        assert_eq!(check.output, "all good");

        // Warn the check
        svc.warn_check("web-ttl", "high latency");
        let check = svc.get_check("web-ttl").unwrap();
        assert_eq!(check.status, CheckStatus::Warning);

        // Fail the check
        svc.fail_check("web-ttl", "timeout");
        let check = svc.get_check("web-ttl").unwrap();
        assert_eq!(check.status, CheckStatus::Critical);
    }

    #[test]
    fn test_checks_by_state() {
        let svc = ConsulNamingServiceImpl::default();

        svc.register_check(HealthCheck {
            check_id: "c1".to_string(),
            name: "Check 1".to_string(),
            node: "node1".to_string(),
            status: CheckStatus::Passing,
            notes: String::new(),
            output: String::new(),
            service_id: "svc1".to_string(),
            service_name: "svc".to_string(),
            check_type: "ttl".to_string(),
        });
        svc.register_check(HealthCheck {
            check_id: "c2".to_string(),
            name: "Check 2".to_string(),
            node: "node1".to_string(),
            status: CheckStatus::Critical,
            notes: String::new(),
            output: String::new(),
            service_id: "svc2".to_string(),
            service_name: "svc".to_string(),
            check_type: "ttl".to_string(),
        });

        let passing = svc.get_checks_by_state(CheckStatus::Passing);
        assert_eq!(passing.len(), 1);
        assert_eq!(passing[0].check_id, "c1");

        let critical = svc.get_checks_by_state(CheckStatus::Critical);
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].check_id, "c2");
    }
}
