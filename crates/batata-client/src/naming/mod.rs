//! Service discovery (naming) service
//!
//! Provides `BatataNamingService` for instance registration/deregistration,
//! service queries, subscriptions, and server push handling.

pub mod balancer;
pub mod failover;
pub mod fuzzy_watch;
pub mod instances_diff;
pub mod listener;
pub mod protect_mode;
pub mod redo;
pub mod service_info_holder;

use std::sync::Arc;

use batata_api::{
    grpc::Payload,
    naming::model::{
        BATCH_DE_REGISTER_INSTANCE, BATCH_REGISTER_INSTANCE, BatchInstanceRequest,
        BatchInstanceResponse, DE_REGISTER_INSTANCE, Instance, InstanceRequest, InstanceResponse,
        NotifySubscriberRequest, NotifySubscriberResponse, QueryServiceResponse, REGISTER_INSTANCE,
        ServiceListRequest, ServiceListResponse, ServiceQueryRequest, SubscribeServiceRequest,
        SubscribeServiceResponse,
    },
    remote::model::ResponseTrait,
};
use dashmap::DashMap;
use tracing::{debug, error, info};

use self::instances_diff::InstancesDiff;

use crate::error::Result;
use crate::grpc::{GrpcClient, ServerPushHandler};

use self::listener::{EventListener, NamingEvent};
use self::redo::{InstanceRedoData, NamingGrpcRedoService, SubscriberRedoData};
use self::service_info_holder::{ServiceInfoHolder, build_service_key};

/// Batata naming service backed by gRPC (Nacos-compatible).
///
/// Uses `NamingGrpcRedoService` for state-tracked instance registration and
/// subscription redo, matching the Nacos Java SDK pattern.
pub struct BatataNamingService {
    grpc_client: Arc<GrpcClient>,
    service_info_holder: Arc<ServiceInfoHolder>,
    /// Subscriptions: key = "groupName@@serviceName"
    subscriptions: DashMap<String, Vec<Arc<dyn EventListener>>>,
    /// Redo service with state machine for instance/subscription tracking
    redo_service: Arc<NamingGrpcRedoService>,
    /// Whether to protect against empty service instance lists
    pub empty_protection: bool,
}

impl BatataNamingService {
    /// Create a new naming service with the given gRPC client.
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self {
            grpc_client,
            service_info_holder: Arc::new(ServiceInfoHolder::new()),
            subscriptions: DashMap::new(),
            redo_service: Arc::new(NamingGrpcRedoService::new()),
            empty_protection: false,
        }
    }

    /// Create a new naming service with empty protection enabled.
    pub fn with_empty_protection(grpc_client: Arc<GrpcClient>, empty_protection: bool) -> Self {
        Self {
            grpc_client,
            service_info_holder: Arc::new(ServiceInfoHolder::new()),
            subscriptions: DashMap::new(),
            redo_service: Arc::new(NamingGrpcRedoService::new()),
            empty_protection,
        }
    }

    /// Get the redo service (for external access).
    pub fn redo_service(&self) -> &Arc<NamingGrpcRedoService> {
        &self.redo_service
    }

    /// Register server push handlers on the gRPC client.
    ///
    /// Must be called after wrapping in `Arc` to enable the push handler
    /// to call back into the naming service when subscriber notifications arrive.
    pub fn register_push_handlers(self: &Arc<Self>) {
        self.grpc_client.register_push_handler(
            "NotifySubscriberRequest",
            NotifySubscriberHandler::new(self.clone()),
        );
    }

    /// Get the service info holder (for external access to cached data).
    pub fn service_info_holder(&self) -> &Arc<ServiceInfoHolder> {
        &self.service_info_holder
    }

    /// Register a service instance.
    pub async fn register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> Result<()> {
        let mut req = InstanceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.r#type = REGISTER_INSTANCE.to_string();
        req.instance = instance.clone();

        // Cache for redo BEFORE sending (matches Nacos pattern)
        let redo_key = build_instance_redo_key(namespace, group_name, service_name, &instance);
        self.redo_service.cache_instance_for_redo(
            redo_key.clone(),
            InstanceRedoData::new(
                namespace.to_string(),
                group_name.to_string(),
                service_name.to_string(),
                instance.clone(),
            ),
        );

        let _resp: InstanceResponse = self.grpc_client.request_typed(&req).await?;

        // Mark as registered after success
        self.redo_service.instance_registered(&redo_key);

        debug!(
            "Registered instance: namespace={}, group={}, service={}",
            namespace, group_name, service_name
        );

        Ok(())
    }

    /// Deregister a service instance.
    pub async fn deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instance: Instance,
    ) -> Result<()> {
        let redo_key = build_instance_redo_key(namespace, group_name, service_name, &instance);

        // Mark for deregistration in redo service
        self.redo_service.instance_deregister(&redo_key);

        let mut req = InstanceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.r#type = DE_REGISTER_INSTANCE.to_string();
        req.instance = instance;

        let _resp: InstanceResponse = self.grpc_client.request_typed(&req).await?;

        // Remove from redo tracking after successful deregistration
        self.redo_service.remove_instance_for_redo(&redo_key);

        debug!(
            "Deregistered instance: namespace={}, group={}, service={}",
            namespace, group_name, service_name
        );

        Ok(())
    }

    /// Query all instances for a service.
    pub async fn get_all_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Result<Vec<Instance>> {
        let mut req = ServiceQueryRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();

        let resp: QueryServiceResponse = self.grpc_client.request_typed(&req).await?;

        // Update local cache
        let key = build_service_key(group_name, service_name);
        self.service_info_holder
            .update(&key, resp.service_info.clone());

        Ok(resp.service_info.hosts)
    }

    /// Subscribe to service changes and get the initial instance list.
    pub async fn subscribe(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        clusters: &str,
        listener: Arc<dyn EventListener>,
    ) -> Result<Vec<Instance>> {
        let mut req = SubscribeServiceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.subscribe = true;
        req.clusters = clusters.to_string();

        // Cache subscription for redo
        let sub_key = build_service_key(group_name, service_name);
        self.redo_service.cache_subscriber_for_redo(
            sub_key.clone(),
            SubscriberRedoData::new(
                namespace.to_string(),
                group_name.to_string(),
                service_name.to_string(),
                clusters.to_string(),
            ),
        );

        let resp: SubscribeServiceResponse = self.grpc_client.request_typed(&req).await?;

        // Mark subscription as registered
        self.redo_service.subscriber_registered(&sub_key);

        // Cache the service info
        self.service_info_holder
            .update(&sub_key, resp.service_info.clone());

        // Store the listener
        self.subscriptions
            .entry(sub_key)
            .or_default()
            .push(listener);

        debug!(
            "Subscribed to service: namespace={}, group={}, service={}",
            namespace, group_name, service_name
        );

        Ok(resp.service_info.hosts)
    }

    /// Unsubscribe from service changes.
    pub async fn unsubscribe(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        clusters: &str,
    ) -> Result<()> {
        let mut req = SubscribeServiceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.subscribe = false;
        req.clusters = clusters.to_string();

        let _resp: SubscribeServiceResponse = self.grpc_client.request_typed(&req).await?;

        // Remove from redo tracking and listeners
        let key = build_service_key(group_name, service_name);
        self.redo_service.remove_subscriber_for_redo(&key);
        self.subscriptions.remove(&key);

        debug!(
            "Unsubscribed from service: namespace={}, group={}, service={}",
            namespace, group_name, service_name
        );

        Ok(())
    }

    /// Batch register multiple instances for a service.
    pub async fn batch_register_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> Result<()> {
        let mut req = BatchInstanceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.r#type = BATCH_REGISTER_INSTANCE.to_string();
        req.instances = instances.clone();

        let _resp: BatchInstanceResponse = self.grpc_client.request_typed(&req).await?;

        // Cache each for redo on reconnect and mark as registered
        for instance in &instances {
            let redo_key = build_instance_redo_key(namespace, group_name, service_name, instance);
            self.redo_service.cache_instance_for_redo(
                redo_key.clone(),
                InstanceRedoData::new(
                    namespace.to_string(),
                    group_name.to_string(),
                    service_name.to_string(),
                    instance.clone(),
                ),
            );
            self.redo_service.instance_registered(&redo_key);
        }

        debug!(
            "Batch registered {} instances: namespace={}, group={}, service={}",
            instances.len(),
            namespace,
            group_name,
            service_name
        );

        Ok(())
    }

    /// Batch deregister multiple instances from a service.
    pub async fn batch_deregister_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        instances: Vec<Instance>,
    ) -> Result<()> {
        let mut req = BatchInstanceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.r#type = BATCH_DE_REGISTER_INSTANCE.to_string();
        req.instances = instances.clone();

        let _resp: BatchInstanceResponse = self.grpc_client.request_typed(&req).await?;

        // Mark for removal in redo service
        for instance in &instances {
            let redo_key = build_instance_redo_key(namespace, group_name, service_name, instance);
            self.redo_service.remove_instance_for_redo(&redo_key);
        }

        debug!(
            "Batch deregistered {} instances: namespace={}, group={}, service={}",
            instances.len(),
            namespace,
            group_name,
            service_name
        );

        Ok(())
    }

    /// List services with pagination.
    pub async fn list_services(
        &self,
        namespace: &str,
        group_name: &str,
        page_no: i32,
        page_size: i32,
    ) -> Result<(i32, Vec<String>)> {
        let mut req = ServiceListRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.page_no = page_no;
        req.page_size = page_size;

        let resp: ServiceListResponse = self.grpc_client.request_typed(&req).await?;

        Ok((resp.count, resp.service_names))
    }

    /// Get healthy instances only for a service.
    /// Select instances filtered by health status.
    ///
    /// Equivalent to Nacos `selectInstances(serviceName, healthy)`.
    /// When `healthy=true`, returns only healthy & enabled instances.
    /// When `healthy=false`, returns only unhealthy or disabled instances.
    pub async fn select_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        healthy: bool,
    ) -> Result<Vec<Instance>> {
        let instances = self
            .get_all_instances(namespace, group_name, service_name)
            .await?;
        Ok(instances
            .into_iter()
            .filter(|i| {
                if healthy {
                    i.healthy && i.enabled
                } else {
                    !i.healthy || !i.enabled
                }
            })
            .collect())
    }

    /// Select instances filtered by health status and clusters.
    ///
    /// Equivalent to Nacos `selectInstances(serviceName, clusters, healthy)`.
    pub async fn select_instances_with_clusters(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        clusters: &[String],
        healthy: bool,
    ) -> Result<Vec<Instance>> {
        let instances = self
            .select_instances(namespace, group_name, service_name, healthy)
            .await?;
        if clusters.is_empty() {
            return Ok(instances);
        }
        Ok(instances
            .into_iter()
            .filter(|i| clusters.iter().any(|c| c == &i.cluster_name))
            .collect())
    }

    /// Select only healthy & enabled instances (convenience method).
    pub async fn select_healthy_instances(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Result<Vec<Instance>> {
        self.select_instances(namespace, group_name, service_name, true)
            .await
    }

    /// Select one random healthy instance from the service.
    ///
    /// Uses weight-based random selection among healthy & enabled instances.
    /// Returns `None` if no healthy instance is available.
    /// Equivalent to Nacos `selectOneHealthyInstance()`.
    pub async fn select_one_healthy_instance(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
    ) -> Result<Option<Instance>> {
        let healthy = self
            .select_healthy_instances(namespace, group_name, service_name)
            .await?;
        if healthy.is_empty() {
            return Ok(None);
        }
        // Weight-based random selection
        let total_weight: f64 = healthy.iter().map(|i| i.weight.max(0.0)).sum();
        if total_weight <= 0.0 {
            // All weights zero — uniform random
            let idx = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize)
                % healthy.len();
            return Ok(Some(healthy[idx].clone()));
        }
        // Weighted random
        let rand_val = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as f64
            / u32::MAX as f64)
            * total_weight;
        let mut accum = 0.0;
        for inst in &healthy {
            accum += inst.weight.max(0.0);
            if accum >= rand_val {
                return Ok(Some(inst.clone()));
            }
        }
        Ok(Some(healthy.last().unwrap().clone()))
    }

    /// Select one healthy instance with cluster filter.
    pub async fn select_one_healthy_instance_with_clusters(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        clusters: &[String],
    ) -> Result<Option<Instance>> {
        let all_healthy = self
            .select_healthy_instances(namespace, group_name, service_name)
            .await?;
        let filtered: Vec<Instance> = if clusters.is_empty() {
            all_healthy
        } else {
            all_healthy
                .into_iter()
                .filter(|i| clusters.iter().any(|c| c == &i.cluster_name))
                .collect()
        };
        if filtered.is_empty() {
            return Ok(None);
        }
        // Simple random for filtered list
        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize)
            % filtered.len();
        Ok(Some(filtered[idx].clone()))
    }

    /// Get all currently subscribed service keys.
    ///
    /// Returns a list of (namespace, group, service_name) tuples.
    /// Equivalent to Nacos `getSubscribeServices()`.
    pub fn get_subscribe_services(&self) -> Vec<(String, String, String)> {
        self.subscriptions
            .iter()
            .map(|e| {
                let key = e.key().clone();
                // key format: "namespace@@group@@service"
                let parts: Vec<&str> = key.splitn(3, "@@").collect();
                if parts.len() == 3 {
                    (
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[2].to_string(),
                    )
                } else {
                    (key, String::new(), String::new())
                }
            })
            .collect()
    }

    /// Check if the naming server is healthy.
    ///
    /// Returns "UP" if the gRPC connection is active, "DOWN" otherwise.
    /// Equivalent to Nacos `getServerStatus()`.
    // ========================================================================
    // Convenience methods matching Nacos Java SDK overloads
    // ========================================================================

    /// Register instance with just service name, IP, and port.
    /// Uses empty namespace and DEFAULT_GROUP.
    pub async fn register_instance_simple(
        &self,
        service_name: &str,
        ip: &str,
        port: i32,
    ) -> Result<()> {
        let instance = Instance::new(ip.to_string(), port);
        self.register_instance("", "DEFAULT_GROUP", service_name, instance)
            .await
    }

    /// Get all instances with explicit subscribe flag.
    /// When `subscribe=false`, always queries the server directly.
    pub async fn get_all_instances_with_subscribe(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        subscribe: bool,
    ) -> Result<Vec<Instance>> {
        if subscribe {
            // Try cache first
            let key = build_service_key(group_name, service_name);
            if let Some(cached) = self.service_info_holder.get(&key)
                && !cached.hosts.is_empty()
            {
                return Ok(cached.hosts);
            }
        }
        // Direct server query
        self.get_all_instances(namespace, group_name, service_name)
            .await
    }

    /// Select instances with explicit subscribe flag.
    /// Matches Nacos `selectInstances(serviceName, clusters, healthy, subscribe)`.
    pub async fn select_instances_full(
        &self,
        namespace: &str,
        group_name: &str,
        service_name: &str,
        clusters: &[String],
        healthy: bool,
        subscribe: bool,
    ) -> Result<Vec<Instance>> {
        let all = self
            .get_all_instances_with_subscribe(namespace, group_name, service_name, subscribe)
            .await?;
        let filtered = all
            .into_iter()
            .filter(|i| {
                let health_match = if healthy {
                    i.healthy && i.enabled
                } else {
                    !i.healthy || !i.enabled
                };
                let cluster_match =
                    clusters.is_empty() || clusters.iter().any(|c| c == &i.cluster_name);
                health_match && cluster_match
            })
            .collect();
        Ok(filtered)
    }

    pub async fn get_server_status(&self) -> String {
        if self.grpc_client.is_connected().await {
            "UP".to_string()
        } else {
            "DOWN".to_string()
        }
    }

    /// Gracefully shutdown the naming service.
    /// Clears subscriptions, registered instances, and cached service info.
    pub async fn shutdown(&self) {
        info!("Shutting down naming service...");
        self.subscriptions.clear();
        self.redo_service.shutdown();
        info!("Naming service shutdown complete");
    }

    /// Handle a `NotifySubscriberRequest` from the server.
    ///
    /// Updates the local cache, computes instance diff, and notifies all listeners.
    pub fn handle_notify_subscriber(&self, req: &NotifySubscriberRequest) {
        let key = build_service_key(&req.group_name, &req.service_name);

        // Empty service protection: reject updates that would set instances to empty
        if self.empty_protection
            && req.service_info.hosts.is_empty()
            && self.service_info_holder.get(&key).is_some()
        {
            tracing::warn!(
                "Rejected empty instance list for service {}@@{}, using cached instances",
                req.group_name,
                req.service_name
            );
            return;
        }

        // Compute diff before updating cache
        let diff = if let Some(old_service) = self.service_info_holder.get(&key) {
            Some(InstancesDiff::diff(
                &old_service.hosts,
                &req.service_info.hosts,
            ))
        } else {
            None
        };

        info!(
            "Service change notification: group={}, service={}, hosts={}, diff={}",
            req.group_name,
            req.service_name,
            req.service_info.hosts.len(),
            diff.as_ref().map_or(0, |d| d.change_count())
        );

        // Update local cache
        self.service_info_holder
            .update(&key, req.service_info.clone());

        // Notify listeners
        if let Some(listeners) = self.subscriptions.get(&key) {
            let event = NamingEvent {
                service_name: req.service_name.clone(),
                group_name: req.group_name.clone(),
                clusters: req.service_info.clusters.clone(),
                instances: req.service_info.hosts.clone(),
                diff,
            };

            for listener in listeners.iter() {
                listener.on_event(event.clone());
            }
        }
    }

    /// Re-register all instances and re-subscribe all services (called after reconnect).
    ///
    /// Uses the `NamingGrpcRedoService` state machine to find items that need
    /// re-registration or re-subscription, matching the Nacos Java pattern.
    pub async fn redo(&self) -> Result<()> {
        // Signal the redo service that we're connected
        self.redo_service.on_connected();

        // Re-register instances that need redo
        let redo_instances = self.redo_service.find_instance_redo_data();
        for data in &redo_instances {
            let mut req = InstanceRequest::new();
            req.naming_request.namespace = data.namespace.clone();
            req.naming_request.group_name = data.group_name.clone();
            req.naming_request.service_name = data.service_name.clone();
            req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
            req.r#type = REGISTER_INSTANCE.to_string();
            req.instance = data.instance.clone();

            match self
                .grpc_client
                .request_typed::<_, InstanceResponse>(&req)
                .await
            {
                Ok(_) => {
                    data.set_registered(true);
                    debug!("Re-registered instance: service={}", data.service_name);
                }
                Err(e) => error!(
                    "Failed to re-register instance: service={}, error={}",
                    data.service_name, e
                ),
            }
        }

        // Re-subscribe services that need redo
        let redo_subs = self.redo_service.find_subscriber_redo_data();
        for (key, data) in &redo_subs {
            let mut req = SubscribeServiceRequest::new();
            req.naming_request.namespace = data.namespace.clone();
            req.naming_request.group_name = data.group_name.clone();
            req.naming_request.service_name = data.service_name.clone();
            req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
            req.subscribe = true;
            req.clusters = data.clusters.clone();

            match self
                .grpc_client
                .request_typed::<_, SubscribeServiceResponse>(&req)
                .await
            {
                Ok(resp) => {
                    data.set_registered(true);
                    self.service_info_holder.update(key, resp.service_info);
                    debug!("Re-subscribed to service: {}", key);
                }
                Err(e) => {
                    error!("Failed to re-subscribe to service: {}, error={}", key, e);
                }
            }
        }

        info!(
            "Redo complete: {} instances, {} subscriptions",
            redo_instances.len(),
            redo_subs.len()
        );

        Ok(())
    }
}

/// Build a redo key for a registered instance.
fn build_instance_redo_key(
    namespace: &str,
    group_name: &str,
    service_name: &str,
    instance: &Instance,
) -> String {
    format!(
        "{}#{}#{}#{}",
        namespace,
        group_name,
        service_name,
        instance.key()
    )
}

/// Server push handler for `NotifySubscriberRequest`.
pub struct NotifySubscriberHandler {
    naming_service: Arc<BatataNamingService>,
}

impl NotifySubscriberHandler {
    pub fn new(naming_service: Arc<BatataNamingService>) -> Self {
        Self { naming_service }
    }
}

impl ServerPushHandler for NotifySubscriberHandler {
    fn handle(&self, payload: &Payload) -> Option<Payload> {
        let req: NotifySubscriberRequest = crate::grpc::deserialize_payload(payload);
        self.naming_service.handle_notify_subscriber(&req);

        // Send acknowledgment
        let resp = NotifySubscriberResponse::new();
        Some(resp.build_payload())
    }
}

// ============================================================================
// NamingService trait implementation
// ============================================================================

#[async_trait::async_trait]
impl crate::traits::NamingService for BatataNamingService {
    async fn register_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instance: Instance,
    ) -> Result<()> {
        self.register_instance("", group_name, service_name, instance)
            .await
    }

    async fn deregister_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instance: Instance,
    ) -> Result<()> {
        self.deregister_instance("", group_name, service_name, instance)
            .await
    }

    async fn batch_register_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instances: Vec<Instance>,
    ) -> Result<()> {
        self.batch_register_instance("", group_name, service_name, instances)
            .await
    }

    async fn batch_deregister_instance(
        &self,
        service_name: &str,
        group_name: &str,
        instances: Vec<Instance>,
    ) -> Result<()> {
        self.batch_deregister_instance("", group_name, service_name, instances)
            .await
    }

    async fn get_all_instances(
        &self,
        service_name: &str,
        group_name: &str,
    ) -> Result<Vec<Instance>> {
        self.get_all_instances("", group_name, service_name).await
    }

    async fn get_all_instances_with_clusters(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &[String],
    ) -> Result<Vec<Instance>> {
        let all = self.get_all_instances("", group_name, service_name).await?;
        if clusters.is_empty() {
            return Ok(all);
        }
        Ok(all
            .into_iter()
            .filter(|i| clusters.iter().any(|c| c == &i.cluster_name))
            .collect())
    }

    async fn select_instances(
        &self,
        service_name: &str,
        group_name: &str,
        healthy: bool,
    ) -> Result<Vec<Instance>> {
        self.select_instances("", group_name, service_name, healthy)
            .await
    }

    async fn select_instances_with_clusters(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &[String],
        healthy: bool,
    ) -> Result<Vec<Instance>> {
        self.select_instances_with_clusters("", group_name, service_name, clusters, healthy)
            .await
    }

    async fn select_one_healthy_instance(
        &self,
        service_name: &str,
        group_name: &str,
    ) -> Result<Option<Instance>> {
        self.select_one_healthy_instance("", group_name, service_name)
            .await
    }

    async fn subscribe(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &str,
        listener: Arc<dyn EventListener>,
    ) -> Result<Vec<Instance>> {
        self.subscribe("", group_name, service_name, clusters, listener)
            .await
    }

    async fn unsubscribe(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &str,
    ) -> Result<()> {
        self.unsubscribe("", group_name, service_name, clusters)
            .await
    }

    async fn get_services_of_server(
        &self,
        page_no: i32,
        page_size: i32,
        group_name: &str,
    ) -> Result<crate::traits::ListView<String>> {
        let (count, names) = self
            .list_services("", group_name, page_no, page_size)
            .await?;
        Ok(crate::traits::ListView { count, data: names })
    }

    async fn get_server_status(&self) -> String {
        self.get_server_status().await
    }

    async fn shut_down(&self) {
        self.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::service_info_holder::build_service_key;
    use super::*;
    use batata_api::naming::model::{Instance, NotifySubscriberRequest, Service};

    #[test]
    fn test_build_service_key() {
        assert_eq!(
            build_service_key("DEFAULT_GROUP", "my-service"),
            "DEFAULT_GROUP@@my-service"
        );
    }

    fn make_instance(ip: &str, port: i32, healthy: bool) -> Instance {
        Instance {
            ip: ip.to_string(),
            port,
            healthy,
            weight: 1.0,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_build_instance_redo_key() {
        let instance = make_instance("10.0.0.1", 8080, true);
        let key = build_instance_redo_key("public", "DEFAULT_GROUP", "my-service", &instance);
        assert!(key.starts_with("public#DEFAULT_GROUP#my-service#"));
    }

    #[test]
    fn test_build_instance_redo_key_different_instances() {
        let inst1 = make_instance("10.0.0.1", 8080, true);
        let inst2 = make_instance("10.0.0.2", 8081, true);
        let key1 = build_instance_redo_key("ns", "group", "svc", &inst1);
        let key2 = build_instance_redo_key("ns", "group", "svc", &inst2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_handle_notify_subscriber_first_notification() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client);

        let service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            service,
        );

        naming_service.handle_notify_subscriber(&req);

        // Cache should be updated
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let cached = naming_service.service_info_holder.get(&key).unwrap();
        assert_eq!(cached.hosts.len(), 1);
        assert_eq!(cached.hosts[0].ip, "10.0.0.1");
    }

    #[test]
    fn test_handle_notify_subscriber_with_diff() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client);

        // Pre-populate cache
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let old_service = Service {
            name: "test-service".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };
        naming_service.service_info_holder.update(&key, old_service);

        // Notify with new instances
        let new_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![
                make_instance("10.0.0.1", 8080, true),
                make_instance("10.0.0.2", 8080, true),
            ],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            new_service,
        );

        naming_service.handle_notify_subscriber(&req);

        // Cache updated
        let cached = naming_service.service_info_holder.get(&key).unwrap();
        assert_eq!(cached.hosts.len(), 2);
    }

    #[test]
    fn test_handle_notify_subscriber_with_listener() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client);

        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        let listener = Arc::new(listener::FnEventListener::new(
            move |event: listener::NamingEvent| {
                assert_eq!(event.service_name, "test-service");
                assert_eq!(event.group_name, "DEFAULT_GROUP");
                assert_eq!(event.instances.len(), 1);
                called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            },
        ));

        let key = build_service_key("DEFAULT_GROUP", "test-service");
        naming_service
            .subscriptions
            .entry(key)
            .or_default()
            .push(listener);

        let service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            service,
        );

        naming_service.handle_notify_subscriber(&req);
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_handle_notify_subscriber_no_listener() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client);

        let service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            service,
        );

        // Should not panic even with no listeners
        naming_service.handle_notify_subscriber(&req);

        // Cache should still be updated
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        assert!(naming_service.service_info_holder.get(&key).is_some());
    }

    #[tokio::test]
    async fn test_shutdown_clears_subscriptions_and_instances() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client);

        // Add a subscription
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let listener = Arc::new(listener::FnEventListener::new(
            |_: listener::NamingEvent| {},
        ));
        naming_service
            .subscriptions
            .entry(key)
            .or_default()
            .push(listener);

        // Add a registered instance via redo service
        let instance = make_instance("10.0.0.1", 8080, true);
        let redo_key =
            build_instance_redo_key("public", "DEFAULT_GROUP", "test-service", &instance);
        naming_service.redo_service.cache_instance_for_redo(
            redo_key,
            super::redo::InstanceRedoData::new(
                "public".to_string(),
                "DEFAULT_GROUP".to_string(),
                "test-service".to_string(),
                instance,
            ),
        );

        assert_eq!(naming_service.subscriptions.len(), 1);
        assert_eq!(naming_service.redo_service.instance_count(), 1);

        naming_service.shutdown().await;

        assert_eq!(naming_service.subscriptions.len(), 0);
        assert_eq!(naming_service.redo_service.instance_count(), 0);
    }

    #[test]
    fn test_empty_protection_rejects_empty_instances() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::with_empty_protection(client, true);

        // Pre-populate cache with an instance
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let old_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };
        naming_service.service_info_holder.update(&key, old_service);

        // Notify with empty instance list — should be rejected
        let empty_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            empty_service,
        );

        naming_service.handle_notify_subscriber(&req);

        // Cache should still have the old instance
        let cached = naming_service.service_info_holder.get(&key).unwrap();
        assert_eq!(cached.hosts.len(), 1);
        assert_eq!(cached.hosts[0].ip, "10.0.0.1");
    }

    #[test]
    fn test_empty_protection_allows_non_empty_update() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::with_empty_protection(client, true);

        // Pre-populate cache
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let old_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };
        naming_service.service_info_holder.update(&key, old_service);

        // Notify with non-empty instance list — should be accepted
        let new_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.2", 8080, true)],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            new_service,
        );

        naming_service.handle_notify_subscriber(&req);

        // Cache should be updated to new instance
        let cached = naming_service.service_info_holder.get(&key).unwrap();
        assert_eq!(cached.hosts.len(), 1);
        assert_eq!(cached.hosts[0].ip, "10.0.0.2");
    }

    #[test]
    fn test_empty_protection_disabled_allows_empty() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client); // empty_protection = false

        // Pre-populate cache
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let old_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![make_instance("10.0.0.1", 8080, true)],
            ..Default::default()
        };
        naming_service.service_info_holder.update(&key, old_service);

        // Notify with empty — should be accepted since protection is off
        let empty_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            empty_service,
        );

        naming_service.handle_notify_subscriber(&req);

        let cached = naming_service.service_info_holder.get(&key).unwrap();
        assert_eq!(cached.hosts.len(), 0);
    }

    #[test]
    fn test_handle_notify_subscriber_diff_added_and_removed() {
        let config = crate::grpc::GrpcClientConfig::default();
        let client = Arc::new(crate::grpc::GrpcClient::new(config).unwrap());
        let naming_service = BatataNamingService::new(client);

        // Pre-populate with inst1 and inst2
        let key = build_service_key("DEFAULT_GROUP", "test-service");
        let old_service = Service {
            hosts: vec![
                make_instance("10.0.0.1", 8080, true),
                make_instance("10.0.0.2", 8080, true),
            ],
            ..Default::default()
        };
        naming_service.service_info_holder.update(&key, old_service);

        let diff_received = Arc::new(std::sync::Mutex::new(None));
        let diff_clone = diff_received.clone();
        let listener = Arc::new(listener::FnEventListener::new(
            move |event: listener::NamingEvent| {
                *diff_clone.lock().unwrap() = event.diff;
            },
        ));
        naming_service
            .subscriptions
            .entry(key.clone())
            .or_default()
            .push(listener);

        // Notify: remove inst2, add inst3
        let new_service = Service {
            name: "test-service".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            hosts: vec![
                make_instance("10.0.0.1", 8080, true),
                make_instance("10.0.0.3", 8080, true),
            ],
            ..Default::default()
        };

        let req = NotifySubscriberRequest::for_service(
            "public",
            "DEFAULT_GROUP",
            "test-service",
            new_service,
        );

        naming_service.handle_notify_subscriber(&req);

        let diff = diff_received.lock().unwrap().clone().unwrap();
        assert_eq!(diff.added_instances.len(), 1);
        assert_eq!(diff.removed_instances.len(), 1);
        assert_eq!(diff.added_instances[0].ip, "10.0.0.3");
        assert_eq!(diff.removed_instances[0].ip, "10.0.0.2");
    }
}
