//! Service discovery (naming) service
//!
//! Provides `BatataNamingService` for instance registration/deregistration,
//! service queries, subscriptions, and server push handling.

pub mod listener;
pub mod service_info_holder;

use std::sync::Arc;

use batata_api::{
    grpc::Payload,
    naming::model::{
        DE_REGISTER_INSTANCE, Instance, InstanceRequest, InstanceResponse, NotifySubscriberRequest,
        NotifySubscriberResponse, QueryServiceResponse, REGISTER_INSTANCE, ServiceQueryRequest,
        SubscribeServiceRequest, SubscribeServiceResponse,
    },
    remote::model::ResponseTrait,
};
use dashmap::DashMap;
use tracing::{debug, error, info};

use crate::error::Result;
use crate::grpc::{GrpcClient, ServerPushHandler};

use self::listener::{EventListener, NamingEvent};
use self::service_info_holder::{ServiceInfoHolder, build_service_key};

/// Nacos-compatible naming service backed by gRPC.
pub struct BatataNamingService {
    grpc_client: Arc<GrpcClient>,
    service_info_holder: Arc<ServiceInfoHolder>,
    /// Subscriptions: key = "groupName@@serviceName"
    subscriptions: DashMap<String, Vec<Arc<dyn EventListener>>>,
    /// Registered instances for redo on reconnect: key = "namespace#groupName#serviceName#instanceKey"
    registered_instances: DashMap<String, RegisteredInstance>,
}

/// Stored info for a registered instance (for redo on reconnect).
#[derive(Clone)]
struct RegisteredInstance {
    namespace: String,
    group_name: String,
    service_name: String,
    instance: Instance,
}

impl BatataNamingService {
    /// Create a new naming service with the given gRPC client.
    pub fn new(grpc_client: Arc<GrpcClient>) -> Self {
        Self {
            grpc_client,
            service_info_holder: Arc::new(ServiceInfoHolder::new()),
            subscriptions: DashMap::new(),
            registered_instances: DashMap::new(),
        }
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

        let _resp: InstanceResponse = self.grpc_client.request_typed(&req).await?;

        // Store for redo on reconnect
        let redo_key = build_instance_redo_key(namespace, group_name, service_name, &instance);
        self.registered_instances.insert(
            redo_key,
            RegisteredInstance {
                namespace: namespace.to_string(),
                group_name: group_name.to_string(),
                service_name: service_name.to_string(),
                instance,
            },
        );

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
        let mut req = InstanceRequest::new();
        req.naming_request.namespace = namespace.to_string();
        req.naming_request.group_name = group_name.to_string();
        req.naming_request.service_name = service_name.to_string();
        req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
        req.r#type = DE_REGISTER_INSTANCE.to_string();
        req.instance = instance.clone();

        let _resp: InstanceResponse = self.grpc_client.request_typed(&req).await?;

        // Remove from redo map
        let redo_key = build_instance_redo_key(namespace, group_name, service_name, &instance);
        self.registered_instances.remove(&redo_key);

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

        let resp: SubscribeServiceResponse = self.grpc_client.request_typed(&req).await?;

        // Cache the service info
        let key = build_service_key(group_name, service_name);
        self.service_info_holder
            .update(&key, resp.service_info.clone());

        // Store the listener
        self.subscriptions.entry(key).or_default().push(listener);

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

        // Remove listeners
        let key = build_service_key(group_name, service_name);
        self.subscriptions.remove(&key);

        debug!(
            "Unsubscribed from service: namespace={}, group={}, service={}",
            namespace, group_name, service_name
        );

        Ok(())
    }

    /// Handle a `NotifySubscriberRequest` from the server.
    ///
    /// Updates the local cache and notifies all listeners.
    pub fn handle_notify_subscriber(&self, req: &NotifySubscriberRequest) {
        let key = build_service_key(&req.group_name, &req.service_name);

        info!(
            "Service change notification: group={}, service={}, hosts={}",
            req.group_name,
            req.service_name,
            req.service_info.hosts.len()
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
            };

            for listener in listeners.iter() {
                listener.on_event(event.clone());
            }
        }
    }

    /// Re-register all instances and re-subscribe all services (called after reconnect).
    pub async fn redo(&self) -> Result<()> {
        // Re-register instances
        let instances: Vec<RegisteredInstance> = self
            .registered_instances
            .iter()
            .map(|e| e.value().clone())
            .collect();

        for reg in &instances {
            let mut req = InstanceRequest::new();
            req.naming_request.namespace = reg.namespace.clone();
            req.naming_request.group_name = reg.group_name.clone();
            req.naming_request.service_name = reg.service_name.clone();
            req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
            req.r#type = REGISTER_INSTANCE.to_string();
            req.instance = reg.instance.clone();

            match self
                .grpc_client
                .request_typed::<_, InstanceResponse>(&req)
                .await
            {
                Ok(_) => debug!("Re-registered instance: service={}", reg.service_name),
                Err(e) => error!(
                    "Failed to re-register instance: service={}, error={}",
                    reg.service_name, e
                ),
            }
        }

        // Re-subscribe services
        let sub_keys: Vec<String> = self.subscriptions.iter().map(|e| e.key().clone()).collect();

        for key in &sub_keys {
            // Parse key = "groupName@@serviceName"
            let parts: Vec<&str> = key.splitn(2, "@@").collect();
            if parts.len() != 2 {
                continue;
            }
            let group_name = parts[0];
            let service_name = parts[1];

            let mut req = SubscribeServiceRequest::new();
            req.naming_request.group_name = group_name.to_string();
            req.naming_request.service_name = service_name.to_string();
            req.naming_request.request.request_id = uuid::Uuid::new_v4().to_string();
            req.subscribe = true;

            match self
                .grpc_client
                .request_typed::<_, SubscribeServiceResponse>(&req)
                .await
            {
                Ok(resp) => {
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
            instances.len(),
            sub_keys.len()
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

#[cfg(test)]
mod tests {
    use super::service_info_holder::build_service_key;

    #[test]
    fn test_build_service_key() {
        assert_eq!(
            build_service_key("DEFAULT_GROUP", "my-service"),
            "DEFAULT_GROUP@@my-service"
        );
    }
}
