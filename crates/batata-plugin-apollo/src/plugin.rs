use std::collections::HashMap;
use std::sync::Arc;

use batata_plugin::{PluginContext, PluginStateProvider, ProtocolAdapterPlugin};

use crate::model::config::ApolloPluginConfig;
use crate::persistence::{
    ApolloPersistence, EmbeddedApolloPersistence, ExternalDbApolloPersistence,
};

#[derive(Clone)]
pub struct ApolloPluginInner {
    pub persistence: Arc<dyn ApolloPersistence>,
}

pub struct ApolloPlugin {
    config: ApolloPluginConfig,
    inner: std::sync::OnceLock<ApolloPluginInner>,
}

impl ApolloPlugin {
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
        Vec::new()
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

        let persistence: Arc<dyn ApolloPersistence> = match storage_mode {
            Some(mode) if *mode == batata_persistence::model::StorageMode::ExternalDb => {
                let db_conn = db
                    .ok_or_else(|| anyhow::anyhow!("Database connection not available"))?;

                tracing::info!("Running Apollo database migrations...");
                crate::migration::run_apollo_migrations_with_lock(db_conn.as_ref()).await?;
                tracing::info!("Apollo database migrations completed");

                Arc::new(ExternalDbApolloPersistence::new(db_conn))
            }
            _ => {
                let db = rocks_db
                    .ok_or_else(|| anyhow::anyhow!("RocksDB not available"))?;
                Arc::new(EmbeddedApolloPersistence::new(db))
            }
        };

        let inner = ApolloPluginInner { persistence };
        self.inner
            .set(inner)
            .map_err(|_| anyhow::anyhow!("ApolloPlugin::init() called more than once"))?;

        tracing::info!("Apollo compatibility plugin initialized successfully");
        Ok(())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        tracing::info!("Apollo compatibility plugin shutting down");
        Ok(())
    }

    fn configure(&self, cfg: &mut actix_web::web::ServiceConfig) {
        let inner = self.inner();
        cfg.app_data(actix_web::web::Data::from(inner.persistence.clone()))
            .service(crate::route::routes());
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
