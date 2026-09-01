use std::collections::HashMap;
use std::sync::Arc;

use batata_plugin::{PluginContext, PluginStateProvider, ProtocolAdapterPlugin};

use crate::model::config::{ApolloPluginConfig, AuthConfig};
use crate::persistence::{
    ApolloPersistenceService, EmbeddedApolloPersistence, SqlApolloPersistence,
};

#[derive(Clone)]
/// Represents the `ApolloPluginInner` entity.
pub struct ApolloPluginInner {
    /// The `persistence` field.
    pub persistence: Arc<dyn ApolloPersistenceService>,
}

/// Represents the `ApolloPlugin` entity.
pub struct ApolloPlugin {
    config: ApolloPluginConfig,
    inner: std::sync::OnceLock<ApolloPluginInner>,
}

impl ApolloPlugin {
    /// Creates an `ApolloPlugin` from a plugin configuration.
    pub fn from_plugin_config(config: ApolloPluginConfig) -> Self {
        Self {
            config,
            inner: std::sync::OnceLock::new(),
        }
    }

    fn inner(&self) -> &ApolloPluginInner {
        self.inner
            .get()
            .expect("ApolloPlugin::init() must be called before accessing services")
    }
}

#[async_trait::async_trait]
impl ProtocolAdapterPlugin for ApolloPlugin {
    fn name(&self) -> &str {
        "apollo-compatibility"
    }

    fn protocol(&self) -> &str {
        "apollo"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn default_port(&self) -> u16 {
        self.config.port
    }

    fn http_workers(&self) -> usize {
        self.config.http_workers
    }

    fn required_column_families(&self) -> Vec<String> {
        use batata_consistency::raft::state_machine::*;
        vec![
            CF_APOLLO_APP.to_string(),
            CF_APOLLO_CLUSTER.to_string(),
            CF_APOLLO_NAMESPACE.to_string(),
            CF_APOLLO_ITEM.to_string(),
            CF_APOLLO_RELEASE.to_string(),
            CF_APOLLO_COMMIT.to_string(),
            CF_APOLLO_GRAY_RULE.to_string(),
            CF_APOLLO_INSTANCE.to_string(),
            CF_APOLLO_ACCESS_KEY.to_string(),
            CF_APOLLO_RELEASE_MSG.to_string(),
            CF_APOLLO_NAMESPACE_LOCK.to_string(),
            CF_APOLLO_RELEASE_HISTORY.to_string(),
            CF_APOLLO_APP_NAMESPACE.to_string(),
            CF_APOLLO_AUDIT.to_string(),
            CF_APOLLO_CONSUMER.to_string(),
            CF_APOLLO_CONSUMER_TOKEN.to_string(),
            CF_APOLLO_CONSUMER_AUDIT.to_string(),
            CF_APOLLO_CONSUMER_ROLE.to_string(),
            CF_APOLLO_PERMISSION.to_string(),
            CF_APOLLO_ROLE.to_string(),
            CF_APOLLO_ROLE_PERMISSION.to_string(),
            CF_APOLLO_USER_ROLE.to_string(),
            CF_APOLLO_USERS.to_string(),
            CF_APOLLO_FAVORITE.to_string(),
            CF_APOLLO_SERVER_CONFIG.to_string(),
            CF_APOLLO_INSTANCE_CONFIG.to_string(),
        ]
    }

    async fn init(&self, ctx: &PluginContext) -> anyhow::Result<()> {
        if self.inner.get().is_some() {
            tracing::info!("Apollo compatibility plugin already initialized");
            return Ok(());
        }

        tracing::info!("Initializing Apollo compatibility plugin");

        let storage_mode = ctx.get::<batata_persistence::model::StorageMode>("storage_mode");
        let db = ctx.get::<sea_orm::DatabaseConnection>("db");
        let rocks_db = ctx.get::<rocksdb::DB>("rocks_db");

        let persistence: Arc<dyn ApolloPersistenceService> = match storage_mode {
            Some(mode) if *mode == batata_persistence::model::StorageMode::ExternalDb => {
                let db_conn = db
                    .ok_or_else(|| anyhow::anyhow!("Database connection not available"))?;

                tracing::info!("Running Apollo database migrations...");
                crate::migration::run_apollo_migrations_with_lock(db_conn.as_ref()).await?;
                tracing::info!("Apollo database migrations completed");

                Arc::new(SqlApolloPersistence::new((*db_conn).clone()))
            }
            _ => {
                let db = rocks_db
                    .ok_or_else(|| anyhow::anyhow!("RocksDB not available"))?;
                Arc::new(EmbeddedApolloPersistence::new(db))
            }
        };

        let inner = ApolloPluginInner { persistence: persistence.clone() };
        self.inner
            .set(inner)
            .map_err(|_| anyhow::anyhow!("ApolloPlugin::init() called more than once"))?;

        // Upstream database-discovery mode: heartbeat self-registration every
        // apollo.service.registry.heartbeatIntervalInSecond (default 10s).
        // batata serves BOTH configservice and adminservice roles in one
        // process, so both service names are registered.
        let hb_persistence = persistence.clone();
        let port = self.config.port;
        tokio::spawn(async move {
            let uri = std::env::var("APOLLO_SERVICE_REGISTRY_URI")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{}", port));
            use crate::persistence::traits::ServiceRegistryPersistence;
            loop {
                for svc in ["apollo-configservice", "apollo-adminservice"] {
                    if let Err(e) =
                        ServiceRegistryPersistence::heartbeat(&*hb_persistence, svc, &uri, "default").await
                    {
                        tracing::warn!("service registry heartbeat failed for {}: {}", svc, e);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            }
        });

        tracing::info!("Apollo compatibility plugin initialized successfully");
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("Apollo compatibility plugin shutting down");
        // Deregister from the service registry on graceful shutdown.
        if let Some(inner) = self.inner.get() {
            use crate::persistence::traits::ServiceRegistryPersistence;
            let port = self.config.port;
            let uri = std::env::var("APOLLO_SERVICE_REGISTRY_URI")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{}", port));
            for svc in ["apollo-configservice", "apollo-adminservice"] {
                let _ = ServiceRegistryPersistence::deregister(&*inner.persistence, svc, &uri).await;
            }
        }
        Ok(())
    }

    fn configure(&self, cfg: &mut actix_web::web::ServiceConfig) {
        tracing::info!("Configuring Apollo plugin HTTP routes");
        let inner = self.inner();
        // Use Data::new (not Data::from) because handlers expect web::Data<Arc<dyn ApolloPersistenceService>>
        cfg.app_data(actix_web::web::Data::new(inner.persistence.clone()));
        // Authentication configuration shared with the auth middleware.
        cfg.app_data(actix_web::web::Data::new(AuthConfig::from_env()));

        if let Some(db) = inner.persistence.get_db_connection() {
            tracing::info!("Apollo plugin: registering DatabaseConnection as app data");
            cfg.app_data(actix_web::web::Data::new(db));
        }

        crate::route::configure_routes(cfg);
        tracing::info!("Apollo plugin HTTP routes configured");
    }

    async fn start_background_tasks(&self) -> anyhow::Result<()> {
        tracing::info!("Starting Apollo background tasks...");
        Ok(())
    }
}

impl PluginStateProvider for ApolloPlugin {
    fn plugin_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(3);
        state.insert(
            "apollo_enabled".to_string(),
            Some(format!("{}", self.config.enabled)),
        );
        state.insert(
            "apollo_port".to_string(),
            Some(format!("{}", self.config.port)),
        );
        state.insert(
            "apollo_http_workers".to_string(),
            Some(format!("{}", self.config.http_workers)),
        );
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apollo_plugin_trait_impl() {
        let config = ApolloPluginConfig::default();
        let plugin = ApolloPlugin::from_plugin_config(config);
        assert_eq!(ProtocolAdapterPlugin::name(&plugin), "apollo-compatibility");
        assert_eq!(plugin.protocol(), "apollo");
        assert!(plugin.is_enabled());
        assert_eq!(plugin.default_port(), 8080);
    }

    #[tokio::test]
    async fn test_apollo_plugin_from_config_init() {
        let config = ApolloPluginConfig {
            enabled: true,
            ..ApolloPluginConfig::default()
        };
        let plugin = ApolloPlugin::from_plugin_config(config);
        assert!(plugin.is_enabled());
        assert_eq!(plugin.protocol(), "apollo");
        assert_eq!(plugin.default_port(), 8080);
        assert!(plugin.inner.get().is_none());
    }

    #[test]
    fn test_apollo_plugin_state_provider() {
        let config = ApolloPluginConfig {
            enabled: true,
            port: 8080,
            http_workers: 4,
        };
        let plugin = ApolloPlugin::from_plugin_config(config);
        let state = plugin.plugin_state();
        assert_eq!(state.get("apollo_enabled"), Some(&Some("true".to_string())));
        assert_eq!(state.get("apollo_port"), Some(&Some("8080".to_string())));
        assert_eq!(state.get("apollo_http_workers"), Some(&Some("4".to_string())));
    }
}
