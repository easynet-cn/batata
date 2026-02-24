//! Load balancer for service discovery
//!
//! Provides weighted random selection from healthy instances.

use batata_api::naming::model::Instance;
use batata_api::naming::model::ServiceInfo;
use rand::seq::SliceRandom;
use rand::Rng;
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
    pub fn select_all(service_info: &ServiceInfo) -> Result<Vec<Instance>, String> {
        let hosts = service_info.hosts.clone();
        if hosts.is_empty() {
            return Err(format!("no host to srv for serviceInfo: {}", service_info.name));
        }
        Ok(hosts)
    }

    /// Randomly select one instance from service
    pub fn select_host(service_info: &ServiceInfo) -> Result<Instance, String> {
        let hosts = Self::select_all(service_info)?;
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
                weight: host.weight.unwrap_or(1.0),
            })
            .collect();

        if host_with_weight.is_empty() {
            return Err("no healthy instances available".to_string());
        }

        // Random weighted selection
        let total_weight: f64 = host_with_weight.iter().map(|h| h.weight).sum();
        let mut rng = rand::thread_rng();
        let random = rng.gen::<f64>() * total_weight;

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

    /// Select by instance name (deterministic)
    pub fn select_by_name(
        service_info: &ServiceInfo,
        instance_name: &str,
    ) -> Result<Instance, String> {
        let hosts = Self::select_all(service_info)?;
        hosts
            .iter()
            .find(|host| host.instance_name.as_deref() == Some(instance_name))
            .cloned()
            .ok_or_else(|| format!("instance '{}' not found", instance_name))
    }

    /// Select healthy instances only
    pub fn select_healthy(service_info: &ServiceInfo) -> Result<Vec<Instance>, String> {
        let hosts = Self::select_all(service_info)?;
        let healthy: Vec<Instance> = hosts
            .iter()
            .filter(|host| host.healthy)
            .cloned()
            .collect();

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

        healthy.choose(&mut rand::thread_rng()).cloned().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_instance(id: &str, weight: f64, healthy: bool) -> Instance {
        Instance {
            instance_id: Some(id.to_string()),
            ip: "127.0.0.1".to_string(),
            port: 8080,
            weight: Some(weight),
            healthy,
            ..Default::default()
        }
    }

    fn create_mock_service(instances: Vec<Instance>) -> ServiceInfo {
        ServiceInfo {
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
        assert_eq!(healthy[0].instance_id.as_ref().unwrap(), "1");
        assert_eq!(healthy[1].instance_id.as_ref().unwrap(), "3");
    }

    #[test]
    fn test_select_by_name() {
        let instances = vec![
            create_mock_instance("instance-a", 1.0, true),
            create_mock_instance("instance-b", 1.0, true),
        ];
        let service = create_mock_service(instances);

        let selected = Balancer::select_by_name(&service, "instance-b").unwrap();
        assert_eq!(selected.instance_id.as_ref().unwrap(), "instance-b");
    }

    #[test]
    fn test_random_weighted() {
        let instances = vec![
            create_mock_instance("1", 1.0, true),  // 1/6 weight
            create_mock_instance("2", 2.0, true),  // 2/6 weight
            create_mock_instance("3", 3.0, true),  // 3/6 weight
        ];
        let service = create_mock_service(instances);

        // Run multiple times to verify distribution
        let mut counts = [0usize; 3];
        for _ in 0..1000 {
            let selected = Balancer::select_host(&service).unwrap();
            match selected.instance_id.as_deref() {
                Some("1") => counts[0] += 1,
                Some("2") => counts[1] += 1,
                Some("3") => counts[2] += 1,
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
}
