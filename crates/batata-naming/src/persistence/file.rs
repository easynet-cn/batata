//! File-based metadata persistence implementation
//!
//! This module provides file-based persistence for cluster configurations
//! and service metadata.

use super::MetadataPersistence;
use crate::service::{ClusterConfig, ServiceMetadata};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// File-based metadata persistence
pub struct FileMetadataPersistence {
    base_dir: String,
}

impl FileMetadataPersistence {
    /// Create a new file-based persistence
    pub fn new(base_dir: impl Into<String>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Get cluster config file path
    fn get_cluster_config_path(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> String {
        format!(
            "{}/clusters/{}@@{}@@{}@@{}.json",
            self.base_dir, namespace, group_name, service_name, cluster_name
        )
    }

    /// Get service metadata file path
    fn get_service_metadata_path(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> String {
        format!(
            "{}/services/{}@@{}@@{}.json",
            self.base_dir, namespace, group_name, service_name
        )
    }
}

#[async_trait::async_trait]
impl MetadataPersistence for FileMetadataPersistence {
    async fn save_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
        config: &ClusterConfig,
    ) -> Result<(), String> {
        let path = self.get_cluster_config_path(namespace, group_name, service_name, cluster_name);
        let json =
            serde_json::to_string_pretty(config).map_err(|e| format!("Serialize error: {}", e))?;

        // Create directory if not exists
        fs::create_dir_all(format!("{}/clusters", self.base_dir))
            .await
            .map_err(|e| format!("Create dir error: {}", e))?;

        // Write to file
        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| format!("Create file error: {}", e))?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| format!("Write file error: {}", e))?;

        Ok(())
    }

    async fn delete_cluster_config(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        cluster_name: &str,
    ) -> Result<(), String> {
        let path = self.get_cluster_config_path(namespace, group_name, service_name, cluster_name);

        // Ignore error if file doesn't exist
        let _ = fs::remove_file(&path).await;

        Ok(())
    }

    async fn load_cluster_configs(
        &self,
    ) -> Result<Vec<(String, String, String, String, ClusterConfig)>, String> {
        let cluster_dir = format!("{}/clusters", self.base_dir);
        let mut configs = Vec::new();

        if !Path::new(&cluster_dir).exists() {
            return Ok(configs);
        }

        let mut entries = fs::read_dir(&cluster_dir)
            .await
            .map_err(|e| format!("Read dir error: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Next entry error: {}", e))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Read file error: {}", e))?;

            let config: ClusterConfig =
                serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))?;

            // Parse filename: namespace@@group_name@@service_name@@cluster_name.json
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid filename")?;

            let parts: Vec<&str> = stem.split("@@").collect();
            if parts.len() == 4 {
                configs.push((
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                    parts[3].to_string(),
                    config,
                ));
            }
        }

        Ok(configs)
    }

    async fn save_service_metadata(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        metadata: &ServiceMetadata,
    ) -> Result<(), String> {
        let path = self.get_service_metadata_path(namespace, group_name, service_name);
        let json = serde_json::to_string_pretty(metadata)
            .map_err(|e| format!("Serialize error: {}", e))?;

        // Create directory if not exists
        fs::create_dir_all(format!("{}/services", self.base_dir))
            .await
            .map_err(|e| format!("Create dir error: {}", e))?;

        // Write to file
        let mut file = fs::File::create(&path)
            .await
            .map_err(|e| format!("Create file error: {}", e))?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| format!("Write file error: {}", e))?;

        Ok(())
    }

    async fn load_service_metadata(
        &self,
    ) -> Result<Vec<(String, String, String, ServiceMetadata)>, String> {
        let service_dir = format!("{}/services", self.base_dir);
        let mut metadatas = Vec::new();

        if !Path::new(&service_dir).exists() {
            return Ok(metadatas);
        }

        let mut entries = fs::read_dir(&service_dir)
            .await
            .map_err(|e| format!("Read dir error: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Next entry error: {}", e))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Read file error: {}", e))?;

            let metadata: ServiceMetadata =
                serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))?;

            // Parse filename: namespace@@group_name@@service_name.json
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid filename")?;

            let parts: Vec<&str> = stem.split("@@").collect();
            if parts.len() == 3 {
                metadatas.push((
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                    metadata,
                ));
            }
        }

        Ok(metadatas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_persistence() {
        let temp_dir = "/tmp/batata_test_persistence";
        let _ = std::fs::remove_dir_all(temp_dir);

        let persistence = FileMetadataPersistence::new(temp_dir);

        // Test save and load cluster config
        let config = ClusterConfig {
            name: "TEST".to_string(),
            health_check_type: "TCP".to_string(),
            ..Default::default()
        };

        persistence
            .save_cluster_config("ns1", "g1", "s1", "c1", &config)
            .await
            .unwrap();

        let configs = persistence.load_cluster_configs().await.unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].0, "ns1");
        assert_eq!(configs[0].4.name, "TEST");

        // Test delete
        persistence
            .delete_cluster_config("ns1", "g1", "s1", "c1")
            .await
            .unwrap();

        let configs = persistence.load_cluster_configs().await.unwrap();
        assert_eq!(configs.len(), 0);

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_persistence_metadata() {
        let temp_dir = "/tmp/batata_test_metadata";
        let _ = std::fs::remove_dir_all(temp_dir);

        let persistence = FileMetadataPersistence::new(temp_dir);

        let metadata = ServiceMetadata {
            protect_threshold: 0.5,
            ..Default::default()
        };

        persistence
            .save_service_metadata("ns1", "g1", "s1", &metadata)
            .await
            .unwrap();

        let metadatas = persistence.load_service_metadata().await.unwrap();
        assert_eq!(metadatas.len(), 1);
        assert_eq!(metadatas[0].3.protect_threshold, 0.5);

        // Cleanup
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
