//! Embedded implementation of ServiceRegistryPersistence (database-discovery
//! mode storage) inside `CF_APOLLO_SERVICE_REGISTRY`.
//!
//! Layout: `sr:{service_name}:{uri}` → JSON [`ServiceRegistryEntry`]; one row
//! per (service, uri) — heartbeat overwrites in place, refreshing last_time.

use std::sync::Arc;

use async_trait::async_trait;
use rocksdb::DB;

use batata_consistency::raft::state_machine::CF_APOLLO_SERVICE_REGISTRY;

use crate::persistence::embedded::JsonStore;
use crate::persistence::traits::service_registry::{ServiceRegistryEntry, ServiceRegistryPersistence};

const KEY_PREFIX: &str = "sr:";

/// Represents the `ServiceRegistryEmbedded` entity.
pub struct ServiceRegistryEmbedded {
    db: Arc<DB>,
}

impl ServiceRegistryEmbedded {
    /// Creates a new `ServiceRegistryEmbedded`.
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    fn key(service_name: &str, uri: &str) -> String {
        format!("{}{}:{}", KEY_PREFIX, service_name, uri)
    }
}

#[async_trait]
impl ServiceRegistryPersistence for ServiceRegistryEmbedded {
    async fn heartbeat(
        &self,
        service_name: &str,
        uri: &str,
        cluster: &str,
    ) -> anyhow::Result<()> {
        let store = JsonStore::new(self.db.clone(), CF_APOLLO_SERVICE_REGISTRY);
        let now = chrono::Utc::now().timestamp_millis();
        let mut entry: Option<ServiceRegistryEntry> =
            store.get(Self::key(service_name, uri).as_bytes())?;
        match entry.as_mut() {
            Some(e) => {
                e.cluster = cluster.to_string();
                e.data_change_last_time = now;
            }
            None => {
                entry = Some(ServiceRegistryEntry {
                    id: 0,
                    service_name: service_name.to_string(),
                    uri: uri.to_string(),
                    cluster: cluster.to_string(),
                    metadata: None,
                    data_change_last_time: now,
                });
            }
        }
        store.put(Self::key(service_name, uri).as_bytes(), entry.as_ref().unwrap())?;
        Ok(())
    }

    async fn deregister(&self, service_name: &str, uri: &str) -> anyhow::Result<()> {
        JsonStore::new(self.db.clone(), CF_APOLLO_SERVICE_REGISTRY)
            .delete(Self::key(service_name, uri).as_bytes())
    }

    async fn find_alive(
        &self,
        service_name: &str,
        window_secs: i64,
    ) -> anyhow::Result<Vec<ServiceRegistryEntry>> {
        let store = JsonStore::new(self.db.clone(), CF_APOLLO_SERVICE_REGISTRY);
        let cutoff = chrono::Utc::now().timestamp_millis() - window_secs * 1000;
        let prefix = format!("{}{}:", KEY_PREFIX, service_name);
        Ok(store
            .scan_prefix::<ServiceRegistryEntry>(prefix.as_bytes())?
            .into_iter()
            .filter(|e| e.data_change_last_time > cutoff)
            .collect())
    }
}
