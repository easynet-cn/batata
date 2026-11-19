//! Polling data source that feeds Nacos naming state into the Path A xDS bridge.
//!
//! [`NacosSyncBridge`](batata_mesh::sync::NacosSyncBridge) owns the service cache
//! and debounces snapshot updates, but it has no awareness of the naming
//! provider. This module supplies the missing producer: it periodically reads
//! the provider's service keys and emits `Updated` / `Removed` events.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use batata_api::naming::NamingServiceProvider;
use batata_mesh::sync::{NacosInstanceData, NacosServiceData, ServiceChangeEvent};

/// Polls the naming provider and keeps the bridge service cache in sync.
pub(crate) struct NacosXdsSource {
    provider: Arc<dyn NamingServiceProvider>,
    event_tx: mpsc::Sender<ServiceChangeEvent>,
    /// Last successfully published payload per service key.
    known: HashMap<String, NacosServiceData>,
}

impl NacosXdsSource {
    pub(crate) fn new(
        provider: Arc<dyn NamingServiceProvider>,
        event_tx: mpsc::Sender<ServiceChangeEvent>,
    ) -> Self {
        Self {
            provider,
            event_tx,
            known: HashMap::new(),
        }
    }

    /// Perform one full poll, emitting only actual changes.
    /// Returns `false` when the bridge receiver is gone.
    pub(crate) async fn poll_once(&mut self) -> bool {
        let keys = self.provider.get_all_service_keys();
        let mut current = HashMap::with_capacity(keys.len());

        for key in keys {
            let Some((namespace, group, service)) = split_service_key(&key) else {
                warn!(service_key = %key, "Skipping malformed naming service key");
                continue;
            };

            let service_info = self
                .provider
                .get_service(namespace, group, service, "", false);
            let instances = self
                .provider
                .get_instances_snapshot(namespace, group, service, "", false);
            let service_data = NacosServiceData {
                namespace_id: namespace.to_string(),
                group_name: group.to_string(),
                service_name: service.to_string(),
                protect_threshold: f64::from(service_info.protect_threshold),
                metadata: service_info.metadata,
                instances: instances
                    .iter()
                    .map(|instance| NacosInstanceData {
                        instance_id: instance.instance_id.clone(),
                        ip: instance.ip.clone(),
                        port: u16::try_from(instance.port).unwrap_or_default(),
                        weight: instance.weight,
                        healthy: instance.healthy,
                        enabled: instance.enabled,
                        ephemeral: instance.ephemeral,
                        cluster_name: instance.cluster_name.clone(),
                        service_name: instance.service_name.clone(),
                        metadata: instance.metadata.clone(),
                    })
                    .collect(),
            };

            if self.known.get(&key) != Some(&service_data) {
                let event = ServiceChangeEvent::Updated(service_data.clone());
                if self.event_tx.send(event).await.is_err() {
                    return false;
                }
            }
            current.insert(key, service_data);
        }

        let mut removed: Vec<String> = self
            .known
            .keys()
            .filter(|key| !current.contains_key(*key))
            .cloned()
            .collect();
        removed.sort();
        for key in removed {
            let Some((namespace, group, service)) = split_service_key(&key) else {
                continue;
            };
            let event = ServiceChangeEvent::Removed {
                namespace_id: namespace.to_string(),
                group_name: group.to_string(),
                service_name: service.to_string(),
            };
            if self.event_tx.send(event).await.is_err() {
                return false;
            }
        }

        self.known = current;
        true
    }
}

/// Split the naming store key produced by `build_service_key`.
fn split_service_key(key: &str) -> Option<(&str, &str, &str)> {
    let mut parts = key.splitn(3, "@@");
    let namespace = parts.next()?;
    let group = parts.next()?;
    let service = parts.next()?;
    Some((namespace, group, service))
}

/// Spawn the polling source. The returned sender stops the loop gracefully.
pub(crate) fn spawn_nacos_source(
    provider: Arc<dyn NamingServiceProvider>,
    event_tx: mpsc::Sender<ServiceChangeEvent>,
    interval: Duration,
) -> (oneshot::Sender<()>, JoinHandle<()>) {
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        let mut source = NacosXdsSource::new(provider, event_tx);
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    debug!("Nacos xDS naming source shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    if !source.poll_once().await {
                        break;
                    }
                }
            }
        }
    });
    (shutdown_tx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    use batata_api::naming::Instance;
    use batata_mesh::server::{XdsServer, XdsServerConfig};
    use batata_mesh::sync::{NacosSyncBridge, SyncBridgeConfig};
    use batata_naming::NamingService;
    use tokio::time::timeout;

    fn instance(service: &str, ip: &str, port: i32) -> Instance {
        Instance {
            instance_id: format!("{service}-{ip}"),
            ip: ip.to_string(),
            port,
            weight: 100.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: service.to_string(),
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_split_service_key() {
        assert_eq!(
            split_service_key("public@@DEFAULT_GROUP@@web"),
            Some(("public", "DEFAULT_GROUP", "web"))
        );
        assert_eq!(split_service_key("public@@DEFAULT_GROUP"), None);
        // Namespace IDs may be empty upstream, so only the three-part shape matters.
        assert_eq!(
            split_service_key("@@DEFAULT_GROUP@@web"),
            Some(("", "DEFAULT_GROUP", "web"))
        );
    }

    async fn next_event(rx: &mut mpsc::Receiver<ServiceChangeEvent>) -> ServiceChangeEvent {
        timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for a service event")
            .expect("service event channel closed")
    }

    #[tokio::test]
    async fn test_poll_once_emits_updates_and_removals() {
        let naming = Arc::new(NamingService::new());
        let web = instance("web", "10.0.0.1", 8080);
        naming.register_instance("public", "DEFAULT_GROUP", "web", web.clone());
        let provider: Arc<dyn NamingServiceProvider> = naming.clone();
        let (tx, mut rx) = mpsc::channel(16);
        let mut source = NacosXdsSource::new(provider, tx);

        assert!(source.poll_once().await);
        let ServiceChangeEvent::Updated(data) = next_event(&mut rx).await else {
            panic!("expected an updated service event");
        };
        assert_eq!(data.namespace_id, "public");
        assert_eq!(data.group_name, "DEFAULT_GROUP");
        assert_eq!(data.service_name, "web");
        assert_eq!(data.instances.len(), 1);
        assert_eq!(data.instances[0].port, 8080);

        // A second poll with no changes must not emit anything.
        assert!(source.poll_once().await);
        assert!(rx.try_recv().is_err());

        // A changed instance must be republished.
        assert!(naming.deregister_instance("public", "DEFAULT_GROUP", "web", &web));
        let changed = instance("web", "10.0.0.2", 9090);
        naming.register_instance("public", "DEFAULT_GROUP", "web", changed.clone());
        assert!(source.poll_once().await);
        let ServiceChangeEvent::Updated(data) = next_event(&mut rx).await else {
            panic!("expected an updated service event after instance change");
        };
        assert_eq!(data.instances[0].port, 9090);

        // Remove the service entirely and repoll.
        assert!(naming.deregister_instance("public", "DEFAULT_GROUP", "web", &changed));
        assert!(
            naming
                .delete_service("public", "DEFAULT_GROUP", "web")
                .is_ok()
        );
        assert!(source.poll_once().await);
        let ServiceChangeEvent::Removed {
            namespace_id,
            group_name,
            service_name,
        } = next_event(&mut rx).await
        else {
            panic!("expected a removed service event");
        };
        assert_eq!(namespace_id, "public");
        assert_eq!(group_name, "DEFAULT_GROUP");
        assert_eq!(service_name, "web");
    }

    async fn wait_for_cluster_count(server: &Arc<XdsServer>, expected: usize) {
        timeout(Duration::from_secs(3), async {
            loop {
                if let Some(snapshot) = server.snapshot_cache().get_snapshot("unused")
                    && snapshot.clusters.len() == expected
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("timed out waiting for xDS snapshot");
    }

    #[tokio::test]
    async fn test_source_events_reach_xds_snapshot() {
        let naming = Arc::new(NamingService::new());
        let web = instance("web", "10.0.0.1", 8080);
        naming.register_instance("public", "DEFAULT_GROUP", "web", web.clone());

        let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
        let mut bridge = NacosSyncBridge::new(
            xds_server.clone(),
            SyncBridgeConfig {
                sync_interval_ms: 20,
                ..Default::default()
            },
        );
        bridge.start().await.unwrap();

        let provider: Arc<dyn NamingServiceProvider> = naming.clone();
        let mut source = NacosXdsSource::new(provider, bridge.event_sender());
        assert!(source.poll_once().await);
        wait_for_cluster_count(&xds_server, 1).await;

        assert!(naming.deregister_instance("public", "DEFAULT_GROUP", "web", &web));
        assert!(
            naming
                .delete_service("public", "DEFAULT_GROUP", "web")
                .is_ok()
        );
        assert!(source.poll_once().await);
        wait_for_cluster_count(&xds_server, 0).await;

        bridge.stop().await;
    }

    #[tokio::test]
    async fn test_poll_once_stops_when_bridge_receiver_is_closed() {
        let naming = Arc::new(NamingService::new());
        naming.register_instance(
            "public",
            "DEFAULT_GROUP",
            "web",
            instance("web", "10.0.0.1", 8080),
        );
        let provider: Arc<dyn NamingServiceProvider> = naming;
        let (tx, rx) = mpsc::channel(16);
        drop(rx);
        let mut source = NacosXdsSource::new(provider, tx);

        assert!(!source.poll_once().await);
    }
}
