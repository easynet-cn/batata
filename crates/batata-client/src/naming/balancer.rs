//! Load balancer for service discovery
//!
//! Provides weighted random selection from healthy instances.

use batata_api::naming::model::{Instance, Service};
use rand::Rng;
use rand::seq::IndexedRandom;
use tracing::debug;

/// Weighted pair for load balancing
#[derive(Debug, Clone)]
struct WeightedInstance {
    instance: Instance,
    weight: f64,
}

/// Load balancer using random weighted selection
pub struct Balancer;

impl Balancer {
    /// Select all instances from service
    pub fn select_all(service: &Service) -> Result<Vec<Instance>, String> {
        let hosts = service.hosts.clone();
        if hosts.is_empty() {
            return Err(format!("no host to srv for service: {}", service.name));
        }
        Ok(hosts)
    }

    /// Randomly select one instance from service
    pub fn select_host(service: &Service) -> Result<Instance, String> {
        let hosts = Self::select_all(service)?;
        Self::get_host_by_random_weight(&hosts)
    }

    /// Return one host from list by random weight
    fn get_host_by_random_weight(hosts: &[Instance]) -> Result<Instance, String> {
        debug!("entry randomWithWeight");
        if hosts.is_empty() {
            debug!("hosts is empty");
            return Err("no healthy instances available".to_string());
        }

        // Filter healthy instances with weights
        let host_with_weight: Vec<WeightedInstance> = hosts
            .iter()
            .filter(|host| host.healthy)
            .map(|host| WeightedInstance {
                instance: host.clone(),
                weight: host.weight,
            })
            .collect();

        if host_with_weight.is_empty() {
            return Err("no healthy instances available".to_string());
        }

        // Random weighted selection
        let total_weight: f64 = host_with_weight.iter().map(|h| h.weight).sum();
        let mut rng = rand::rng();
        let random = rng.random::<f64>() * total_weight;

        let mut weight_sum = 0.0;
        for weighted in &host_with_weight {
            weight_sum += weighted.weight;
            if random <= weight_sum {
                return Ok(weighted.instance.clone());
            }
        }

        // Fallback to last instance
        Ok(host_with_weight.last().unwrap().instance.clone())
    }

    /// Select healthy instances only
    pub fn select_healthy(service: &Service) -> Result<Vec<Instance>, String> {
        let hosts = Self::select_all(service)?;
        let healthy: Vec<Instance> = hosts.iter().filter(|host| host.healthy).cloned().collect();

        if healthy.is_empty() {
            return Err("no healthy instances available".to_string());
        }

        Ok(healthy)
    }

    /// Randomly select one instance from a list
    pub fn random_instance(instances: &[Instance]) -> Option<Instance> {
        let healthy: Vec<&Instance> = instances.iter().filter(|h| h.healthy).collect();
        if healthy.is_empty() {
            return None;
        }

        healthy.choose(&mut rand::rng()).cloned().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_instance(id: &str, weight: f64, healthy: bool) -> Instance {
        Instance {
            instance_id: id.to_string(),
            ip: "127.0.0.1".to_string(),
            port: 8080,
            weight,
            healthy,
            ..Default::default()
        }
    }

    fn create_mock_service(instances: Vec<Instance>) -> Service {
        Service {
            name: "test-service".to_string(),
            hosts: instances,
            ..Default::default()
        }
    }

    #[test]
    fn test_select_all() {
        let instances = vec![
            create_mock_instance("1", 1.0, true),
            create_mock_instance("2", 1.0, true),
        ];
        let service = create_mock_service(instances);

        let selected = Balancer::select_all(&service).unwrap();
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn test_select_healthy() {
        let instances = vec![
            create_mock_instance("1", 1.0, true),
            create_mock_instance("2", 1.0, false),
            create_mock_instance("3", 1.0, true),
        ];
        let service = create_mock_service(instances);

        let healthy = Balancer::select_healthy(&service).unwrap();
        assert_eq!(healthy.len(), 2);
    }

    #[test]
    fn test_random_weighted() {
        let instances = vec![
            create_mock_instance("1", 1.0, true),
            create_mock_instance("2", 2.0, true),
            create_mock_instance("3", 3.0, true),
        ];
        let service = create_mock_service(instances);

        // Run multiple times to verify distribution
        let mut counts = [0usize; 3];
        for _ in 0..1000 {
            let selected = Balancer::select_host(&service).unwrap();
            match selected.instance_id.as_str() {
                "1" => counts[0] += 1,
                "2" => counts[1] += 1,
                "3" => counts[2] += 1,
                _ => {}
            }
        }

        // Check that instance 3 is selected most often (highest weight)
        assert!(counts[2] > counts[1]);
        assert!(counts[1] > counts[0]);
    }

    #[test]
    fn test_no_healthy_instances() {
        let instances = vec![
            create_mock_instance("1", 1.0, false),
            create_mock_instance("2", 1.0, false),
        ];
        let service = create_mock_service(instances);

        let result = Balancer::select_host(&service);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no healthy instances"));
    }

    #[test]
    fn test_select_all_empty() {
        let service = create_mock_service(vec![]);
        let result = Balancer::select_all(&service);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no host to srv"));
    }

    #[test]
    fn test_select_healthy_empty() {
        let service = create_mock_service(vec![]);
        let result = Balancer::select_healthy(&service);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_healthy_none_healthy() {
        let instances = vec![
            create_mock_instance("1", 1.0, false),
            create_mock_instance("2", 1.0, false),
        ];
        let service = create_mock_service(instances);
        let result = Balancer::select_healthy(&service);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no healthy instances"));
    }

    #[test]
    fn test_select_host_single() {
        let instances = vec![create_mock_instance("only-one", 1.0, true)];
        let service = create_mock_service(instances);

        // Should always return the same instance
        for _ in 0..10 {
            let selected = Balancer::select_host(&service).unwrap();
            assert_eq!(selected.instance_id, "only-one");
        }
    }

    #[test]
    fn test_random_instance_healthy_only() {
        let instances = vec![
            create_mock_instance("healthy", 1.0, true),
            create_mock_instance("unhealthy", 1.0, false),
        ];

        // Should only return healthy instances
        for _ in 0..20 {
            let selected = Balancer::random_instance(&instances).unwrap();
            assert_eq!(selected.instance_id, "healthy");
        }
    }

    #[test]
    fn test_random_instance_empty() {
        let result = Balancer::random_instance(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_random_instance_all_unhealthy() {
        let instances = vec![
            create_mock_instance("1", 1.0, false),
            create_mock_instance("2", 1.0, false),
        ];
        let result = Balancer::random_instance(&instances);
        assert!(result.is_none());
    }

    #[test]
    fn test_select_all_includes_unhealthy() {
        let instances = vec![
            create_mock_instance("1", 1.0, true),
            create_mock_instance("2", 1.0, false),
        ];
        let service = create_mock_service(instances);

        let all = Balancer::select_all(&service).unwrap();
        assert_eq!(all.len(), 2); // Includes unhealthy
    }

    #[test]
    fn test_weighted_selection_zero_weight() {
        let instances = vec![
            create_mock_instance("zero", 0.0, true),
            create_mock_instance("normal", 1.0, true),
        ];
        let service = create_mock_service(instances);

        // The zero-weight instance should rarely be selected
        let mut normal_count = 0;
        for _ in 0..100 {
            let selected = Balancer::select_host(&service).unwrap();
            if selected.instance_id == "normal" {
                normal_count += 1;
            }
        }
        assert!(normal_count > 90);
    }
}
