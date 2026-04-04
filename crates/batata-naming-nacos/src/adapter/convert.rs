//! Type conversions between batata-api and batata-naming-nacos models

use batata_api::naming::model as api;

use crate::model::{
    ClusterConfig, HealthCheckType, NacosInstance, NacosService, ProtectionInfo, ServiceMetadata,
};

/// Convert from batata-api Instance to NacosInstance
pub fn from_api_instance(inst: api::Instance) -> NacosInstance {
    NacosInstance {
        instance_id: inst.instance_id,
        ip: inst.ip,
        port: inst.port,
        weight: inst.weight,
        healthy: inst.healthy,
        enabled: inst.enabled,
        ephemeral: inst.ephemeral,
        cluster_name: inst.cluster_name,
        service_name: inst.service_name,
        metadata: inst.metadata,
    }
}

/// Convert from NacosInstance to batata-api Instance
pub fn to_api_instance(inst: &NacosInstance, source: api::RegisterSource) -> api::Instance {
    api::Instance {
        instance_id: inst.instance_id.clone(),
        ip: inst.ip.clone(),
        port: inst.port,
        weight: inst.weight,
        healthy: inst.healthy,
        enabled: inst.enabled,
        ephemeral: inst.ephemeral,
        cluster_name: inst.cluster_name.clone(),
        service_name: inst.service_name.clone(),
        metadata: inst.metadata.clone(),
        register_source: source,
    }
}

/// Convert NacosService to batata-api Service
pub fn to_api_service(svc: &NacosService, source: api::RegisterSource) -> api::Service {
    api::Service {
        name: svc.name.clone(),
        group_name: svc.group_name.clone(),
        clusters: svc.clusters.clone(),
        cache_millis: svc.cache_millis,
        hosts: svc
            .hosts
            .iter()
            .map(|h| to_api_instance(h, source.clone()))
            .collect(),
        last_ref_time: svc.last_ref_time,
        checksum: svc.checksum.clone(),
        all_ips: svc.all_ips,
        reach_protection_threshold: svc.reach_protection_threshold,
    }
}

/// Convert ProtectionInfo
pub fn to_api_protection_info(info: &ProtectionInfo) -> api::ProtectionInfo {
    api::ProtectionInfo {
        threshold: info.threshold,
        total_instances: info.total_instances,
        healthy_instances: info.healthy_instances,
        healthy_ratio: info.healthy_ratio,
        triggered: info.triggered,
    }
}

/// Convert from batata-api ServiceMetadata to NacosInstance ServiceMetadata
pub fn from_api_service_metadata(meta: api::ServiceMetadata) -> ServiceMetadata {
    ServiceMetadata {
        protect_threshold: meta.protect_threshold,
        metadata: meta.metadata,
        selector_type: meta.selector_type,
        selector_expression: meta.selector_expression,
        ephemeral: meta.ephemeral,
        revision: meta.revision,
    }
}

/// Convert ServiceMetadata to batata-api ServiceMetadata
pub fn to_api_service_metadata(
    meta: &ServiceMetadata,
    source: api::RegisterSource,
) -> api::ServiceMetadata {
    api::ServiceMetadata {
        protect_threshold: meta.protect_threshold,
        metadata: meta.metadata.clone(),
        selector_type: meta.selector_type.clone(),
        selector_expression: meta.selector_expression.clone(),
        ephemeral: meta.ephemeral,
        revision: meta.revision,
        register_source: source,
    }
}

/// Convert from batata-api ClusterConfig to NacosInstance ClusterConfig
pub fn from_api_cluster_config(cfg: api::ClusterConfig) -> ClusterConfig {
    ClusterConfig {
        name: cfg.name,
        health_check_type: match cfg.health_check_type.as_str() {
            "TCP" | "tcp" => HealthCheckType::Tcp,
            "HTTP" | "http" => HealthCheckType::Http,
            "MYSQL" | "mysql" => HealthCheckType::Mysql,
            _ => HealthCheckType::None,
        },
        health_check_port: cfg.check_port as u16,
        use_instance_port: cfg.use_instance_port,
        metadata: cfg.metadata,
    }
}

/// Convert ClusterConfig to batata-api ClusterConfig
pub fn to_api_cluster_config(cfg: &ClusterConfig) -> api::ClusterConfig {
    api::ClusterConfig {
        name: cfg.name.clone(),
        service_name: String::new(),
        health_check_type: match cfg.health_check_type {
            HealthCheckType::Tcp => "TCP".to_string(),
            HealthCheckType::Http => "HTTP".to_string(),
            HealthCheckType::Mysql => "MYSQL".to_string(),
            HealthCheckType::None => "NONE".to_string(),
        },
        check_port: cfg.health_check_port as i32,
        default_port: 0,
        use_instance_port: cfg.use_instance_port,
        metadata: cfg.metadata.clone(),
    }
}
