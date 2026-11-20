//! Nacos config source for the Istio MCP/ADS server on port 18848.
//!
//! Upstream Nacos publishes a config event only for configs carrying Istio
//! tags. Batata's persistence layer does not expose that event with content,
//! so this source periodically re-reads tagged configs and hands a small
//! crate-local DTO to the MCP bridge. The bridge remains the single publisher
//! of snapshots, which prevents naming and config updates from racing.

use std::sync::Arc;

use async_trait::async_trait;
use batata_mesh::{IstioConfigSourceEntry, is_istio_config_tag};
use batata_persistence::PersistenceService;
use tracing::{debug, warn};

/// Supplies the current set of tagged Nacos configs used for Istio RDS.
#[async_trait]
pub trait IstioMcpConfigProvider: Send + Sync {
    /// Load every config whose advanced tags enable upstream Istio processing.
    async fn load_virtual_service_configs(&self) -> Vec<IstioConfigSourceEntry>;
}

/// Adapter that reads Istio-tagged configs from batata persistence.
pub struct PersistenceIstioMcpConfigProvider {
    persistence: Arc<dyn PersistenceService>,
}

impl PersistenceIstioMcpConfigProvider {
    /// Create a provider backed by the server's persistence service.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        Self { persistence }
    }
}

#[async_trait]
impl IstioMcpConfigProvider for PersistenceIstioMcpConfigProvider {
    async fn load_virtual_service_configs(&self) -> Vec<IstioConfigSourceEntry> {
        let mut namespaces: Vec<String> = match self.persistence.namespace_find_all().await {
            Ok(namespaces) => namespaces
                .into_iter()
                .map(|namespace| namespace.namespace_id)
                .collect(),
            Err(error) => {
                warn!(%error, "Failed to list config namespaces for Istio MCP; falling back to public");
                vec!["public".to_string()]
            }
        };
        if !namespaces.iter().any(|namespace| namespace == "public") {
            namespaces.push("public".to_string());
        }

        let mut entries = Vec::new();
        for namespace in namespaces {
            let configs = match self.persistence.config_find_by_namespace(&namespace).await {
                Ok(configs) => configs,
                Err(error) => {
                    warn!(namespace = %namespace, %error, "Failed to list configs for Istio MCP");
                    continue;
                }
            };

            for summary in configs {
                if !is_istio_config_tag(&summary.config_tags) {
                    continue;
                }

                let tenant = if summary.tenant.is_empty() {
                    "public".to_string()
                } else {
                    summary.tenant.clone()
                };
                let config = match self
                    .persistence
                    .config_find_one(&summary.data_id, &summary.group, &tenant)
                    .await
                {
                    Ok(Some(config)) => config,
                    Ok(None) => continue,
                    Err(error) => {
                        warn!(
                            namespace = %tenant,
                            group = %summary.group,
                            data_id = %summary.data_id,
                            %error,
                            "Failed to read tagged Istio config"
                        );
                        continue;
                    }
                };

                if !config.encrypted_data_key.is_empty() {
                    warn!(
                        namespace = %tenant,
                        group = %summary.group,
                        data_id = %summary.data_id,
                        "Skipping encrypted config for Istio MCP"
                    );
                    continue;
                }

                debug!(
                    namespace = %tenant,
                    group = %summary.group,
                    data_id = %summary.data_id,
                    config_tags = %summary.config_tags,
                    "Loaded tagged Istio config"
                );
                entries.push(IstioConfigSourceEntry {
                    data_id: config.data_id,
                    group: config.group,
                    namespace: config.tenant,
                    config_tags: config.config_tags,
                    content: config.content,
                });
            }
        }

        entries.sort_by(|a, b| {
            (&a.namespace, &a.group, &a.data_id).cmp(&(&b.namespace, &b.group, &b.data_id))
        });
        entries
    }
}
