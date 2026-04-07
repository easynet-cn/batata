//! Health check interceptor chain (matches Nacos HealthCheckInterceptorChain)
//!
//! This module provides a chain of interceptors that gate health check execution.
//! Interceptors are executed in order; if any interceptor returns `true` (block),
//! the health check is skipped. This is used to:
//! - Skip health checks when globally disabled
//! - Skip health checks on non-responsible nodes in cluster mode
//!
//! Reference: Nacos `HealthCheckResponsibleInterceptor` + `HealthCheckEnableInterceptor`

use std::sync::Arc;

use batata_core::service::distro::DistroMapper;

use super::config::HealthCheckConfig;

/// Trait for health check interceptors.
///
/// Interceptors are evaluated in ascending `order()`. If `intercept()` returns
/// `true`, the health check task is **blocked** (skipped). If all interceptors
/// return `false`, the task proceeds normally.
pub trait HealthCheckInterceptor: Send + Sync {
    /// Returns `true` to **block** execution, `false` to allow.
    fn intercept(&self, responsible_id: &str) -> bool;

    /// Execution order (lower = earlier). Matches Nacos ordering convention.
    fn order(&self) -> i32;
}

/// Interceptor that blocks health checks when globally disabled.
///
/// Matches Nacos `HealthCheckEnableInterceptor` (order: `Integer.MIN_VALUE`).
pub struct HealthCheckEnableInterceptor {
    config: Arc<HealthCheckConfig>,
}

impl HealthCheckEnableInterceptor {
    pub fn new(config: Arc<HealthCheckConfig>) -> Self {
        Self { config }
    }
}

impl HealthCheckInterceptor for HealthCheckEnableInterceptor {
    fn intercept(&self, _responsible_id: &str) -> bool {
        !self.config.is_enabled()
    }

    fn order(&self) -> i32 {
        i32::MIN
    }
}

/// Interceptor that blocks health checks on non-responsible nodes.
///
/// Uses `DistroMapper` to determine which cluster node owns a given key.
/// In standalone mode (no members), always allows execution.
///
/// Matches Nacos `HealthCheckResponsibleInterceptor` (order: `Integer.MIN_VALUE + 1`).
pub struct HealthCheckResponsibleInterceptor {
    distro_mapper: Arc<DistroMapper>,
    local_address: String,
}

impl HealthCheckResponsibleInterceptor {
    pub fn new(distro_mapper: Arc<DistroMapper>, local_address: String) -> Self {
        Self {
            distro_mapper,
            local_address,
        }
    }
}

impl HealthCheckInterceptor for HealthCheckResponsibleInterceptor {
    fn intercept(&self, responsible_id: &str) -> bool {
        // Block if this node is NOT responsible for the given key
        !self
            .distro_mapper
            .is_responsible(responsible_id, &self.local_address)
    }

    fn order(&self) -> i32 {
        i32::MIN + 1
    }
}

/// Chain of interceptors that gates health check execution.
///
/// Evaluates interceptors in ascending order. If any returns `true` (block),
/// `should_execute()` returns `false` and the health check should be skipped.
///
/// Matches Nacos `HealthCheckInterceptorChain`.
pub struct HealthCheckInterceptorChain {
    interceptors: Vec<Box<dyn HealthCheckInterceptor>>,
}

impl HealthCheckInterceptorChain {
    /// Create a new chain with the given interceptors, sorted by order.
    pub fn new(mut interceptors: Vec<Box<dyn HealthCheckInterceptor>>) -> Self {
        interceptors.sort_by_key(|i| i.order());
        Self { interceptors }
    }

    /// Create a standalone chain that always allows execution.
    ///
    /// Used when running in standalone mode (no cluster).
    pub fn standalone(config: Arc<HealthCheckConfig>) -> Self {
        let interceptors: Vec<Box<dyn HealthCheckInterceptor>> =
            vec![Box::new(HealthCheckEnableInterceptor::new(config))];
        Self::new(interceptors)
    }

    /// Create a cluster-aware chain with enable + responsibility checks.
    pub fn cluster(
        config: Arc<HealthCheckConfig>,
        distro_mapper: Arc<DistroMapper>,
        local_address: String,
    ) -> Self {
        let interceptors: Vec<Box<dyn HealthCheckInterceptor>> = vec![
            Box::new(HealthCheckEnableInterceptor::new(config)),
            Box::new(HealthCheckResponsibleInterceptor::new(
                distro_mapper,
                local_address,
            )),
        ];
        Self::new(interceptors)
    }

    /// Returns `true` if the health check should proceed.
    ///
    /// Evaluates all interceptors in order. If any blocks, returns `false`.
    pub fn should_execute(&self, responsible_id: &str) -> bool {
        for interceptor in &self.interceptors {
            if interceptor.intercept(responsible_id) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standalone_chain_allows_when_enabled() {
        let config = Arc::new(HealthCheckConfig::default());
        let chain = HealthCheckInterceptorChain::standalone(config);
        assert!(chain.should_execute("10.0.0.1:8080"));
    }

    #[test]
    fn test_standalone_chain_blocks_when_disabled() {
        let mut cfg = HealthCheckConfig::default();
        cfg.health_check_enabled = false;
        let config = Arc::new(cfg);
        let chain = HealthCheckInterceptorChain::standalone(config);
        assert!(!chain.should_execute("10.0.0.1:8080"));
    }

    #[test]
    fn test_cluster_chain_allows_responsible_node() {
        let config = Arc::new(HealthCheckConfig::default());
        let mapper = Arc::new(DistroMapper::new());
        mapper.update_members(vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ]);

        // Find which node is responsible for "test-key"
        let responsible = mapper.responsible_node("test-key").unwrap();

        let chain = HealthCheckInterceptorChain::cluster(
            config,
            mapper,
            responsible.clone(),
        );

        assert!(
            chain.should_execute("test-key"),
            "Responsible node should execute"
        );
    }

    #[test]
    fn test_cluster_chain_blocks_non_responsible_node() {
        let config = Arc::new(HealthCheckConfig::default());
        let mapper = Arc::new(DistroMapper::new());
        mapper.update_members(vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ]);

        let responsible = mapper.responsible_node("test-key").unwrap();

        // Use a different node as local
        let non_responsible = if responsible == "10.0.0.1:8848" {
            "10.0.0.2:8848"
        } else {
            "10.0.0.1:8848"
        };

        let chain = HealthCheckInterceptorChain::cluster(
            config,
            mapper,
            non_responsible.to_string(),
        );

        assert!(
            !chain.should_execute("test-key"),
            "Non-responsible node should NOT execute"
        );
    }

    #[test]
    fn test_cluster_chain_standalone_mode_no_members() {
        let config = Arc::new(HealthCheckConfig::default());
        let mapper = Arc::new(DistroMapper::new());
        // No members — standalone mode

        let chain = HealthCheckInterceptorChain::cluster(
            config,
            mapper,
            "10.0.0.1:8848".to_string(),
        );

        assert!(
            chain.should_execute("any-key"),
            "Standalone (no members) should always execute"
        );
    }

    #[test]
    fn test_enable_interceptor_order() {
        let config = Arc::new(HealthCheckConfig::default());
        let interceptor = HealthCheckEnableInterceptor::new(config);
        assert_eq!(interceptor.order(), i32::MIN);
    }

    #[test]
    fn test_responsible_interceptor_order() {
        let mapper = Arc::new(DistroMapper::new());
        let interceptor =
            HealthCheckResponsibleInterceptor::new(mapper, "10.0.0.1:8848".to_string());
        assert_eq!(interceptor.order(), i32::MIN + 1);
    }

    #[test]
    fn test_interceptor_chain_ordering() {
        // Verify interceptors are evaluated in correct order:
        // enable check (MIN) runs before responsibility check (MIN+1)
        let mut cfg = HealthCheckConfig::default();
        cfg.health_check_enabled = false;
        let config = Arc::new(cfg);
        let mapper = Arc::new(DistroMapper::new());

        let chain = HealthCheckInterceptorChain::cluster(
            config,
            mapper,
            "10.0.0.1:8848".to_string(),
        );

        // Should be blocked by enable interceptor first
        assert!(!chain.should_execute("any-key"));
    }
}
