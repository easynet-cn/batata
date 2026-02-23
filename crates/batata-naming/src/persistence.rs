//! Metadata persistence for cluster configurations and service metadata
//!
//! This module provides interfaces and implementations for persisting
//! cluster configurations and service metadata to storage.

pub mod file;

use async_trait::async_trait;
use crate::service::{ClusterConfig, ServiceMetadata};

/// Metadata persistence trait
#[async_trait::async_trait]
pub trait MetadataPersistence: Send + Sync {
    /// Save cluster configuration
    async fn save_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: &ClusterConfig,
    ) -> Result<(), String>;

    /// Delete cluster configuration
    async fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Result<(), String>;

    /// Load all cluster configurations
    async fn load_cluster_configs(
        &self,
    ) -> Result<Vec<(String, String, String, String, ClusterConfig)>, String>;

    /// Save service metadata
    async fn save_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: &ServiceMetadata,
    ) -> Result<(), String>;

    /// Load all service metadata
    async fn load_service_metadata(
        &self,
    ) -> Result<Vec<(String, String, String, ServiceMetadata)>, String>;
}

/// No-op persistence for in-memory only mode
pub struct NoopPersistence;

#[async_trait::async_trait]
impl MetadataPersistence for NoopPersistence {
    async fn save_cluster_config(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
        _cluster_name: &str,
        _config: &ClusterConfig,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn delete_cluster_config(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
        _cluster_name: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn load_cluster_configs(
        &self,
    ) -> Result<Vec<(String, String, String, String, ClusterConfig)>, String> {
        Ok(Vec::new())
    }

    async fn save_service_metadata(
        &self,
        _namespace: &str,
        _group_name: &str,
        _service_name: &str,
        _metadata: &ServiceMetadata,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn load_service_metadata(
        &self,
    ) -> Result<Vec<(String, String, String, ServiceMetadata)>, String> {
        Ok(Vec::new())
    }
}
