// Naming module gRPC handlers
// Implements handlers for service discovery requests

use std::sync::Arc;

use batata_core::{
    model::Connection,
    service::distro::{DistroDataType, DistroProtocol},
};
use tonic::Status;
use tracing::{debug, info, warn};

use crate::{
    api::{
        grpc::Payload,
        naming::model::{
            BATCH_DE_REGISTER_INSTANCE, BATCH_REGISTER_INSTANCE, BatchInstanceRequest,
            BatchInstanceResponse, DE_REGISTER_INSTANCE, InstanceRequest, InstanceResponse,
            NamingFuzzyWatchChangeNotifyRequest, NamingFuzzyWatchChangeNotifyResponse,
            NamingFuzzyWatchRequest, NamingFuzzyWatchResponse, NamingFuzzyWatchSyncRequest,
            NamingFuzzyWatchSyncResponse, NotifySubscriberRequest, NotifySubscriberResponse,
            PersistentInstanceRequest, QueryServiceResponse, REGISTER_INSTANCE, ServiceListRequest,
            ServiceListResponse, ServiceQueryRequest, SubscribeServiceRequest,
            SubscribeServiceResponse,
        },
        remote::model::{RequestTrait, ResponseCode, ResponseTrait},
    },
    service::{
        naming::NamingService,
        naming_fuzzy_watch::{NamingFuzzyWatchManager, NamingFuzzyWatchPattern},
        rpc::{AuthRequirement, PayloadHandler},
    },
};

// Handler for InstanceRequest - registers or deregisters a service instance
#[derive(Clone)]
pub struct InstanceRequestHandler {
    pub naming_service: Arc<NamingService>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
    /// Distro protocol for syncing ephemeral instances across cluster nodes
    pub distro_protocol: Option<Arc<DistroProtocol>>,
}

#[tonic::async_trait]
impl PayloadHandler for InstanceRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = InstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let instance = request.instance;
        let req_type = &request.r#type;

        let src_ip = payload
            .metadata
            .as_ref()
            .map(|m| m.client_ip.as_str())
            .unwrap_or("");

        info!(
            req_type = %req_type,
            namespace = %namespace,
            group_name = %group_name,
            service_name = %service_name,
            instance_ip = %instance.ip,
            instance_port = %instance.port,
            cluster_name = %instance.cluster_name,
            healthy = %instance.healthy,
            ephemeral = %instance.ephemeral,
            "Received InstanceRequest"
        );

        let result = if req_type == REGISTER_INSTANCE {
            self.naming_service
                .register_instance(namespace, group_name, service_name, instance)
        } else if req_type == DE_REGISTER_INSTANCE {
            self.naming_service
                .deregister_instance(namespace, group_name, service_name, &instance)
        } else {
            false
        };

        // Notify subscribers and fuzzy watchers about service change
        if result {
            // Notify regular subscribers (SubscribeServiceRequest subscribers)
            self.notify_subscribers(namespace, group_name, service_name)
                .await;

            // Notify fuzzy watchers
            if let Err(e) = self
                .notify_fuzzy_watchers(
                    namespace,
                    group_name,
                    service_name,
                    req_type.as_str(),
                    src_ip,
                )
                .await
            {
                warn!("Failed to notify fuzzy watchers: {}", e);
            }

            // Trigger distro sync to other cluster nodes (ephemeral instances only)
            if let Some(ref distro) = self.distro_protocol {
                let service_key = format!("{}@@{}@@{}", namespace, group_name, service_name);
                let distro = distro.clone();
                tokio::spawn(async move {
                    distro
                        .sync_data(DistroDataType::NamingInstance, &service_key)
                        .await;
                });
            }
        }

        let mut response = InstanceResponse::new();
        response.response.request_id = request_id;
        response.r#type = req_type.clone();

        if !result {
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = ResponseCode::Fail.code();
            response.response.success = false;
            response.response.message = "Operation failed".to_string();
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "InstanceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

impl InstanceRequestHandler {
    /// Notify regular subscribers about service change
    async fn notify_subscribers(&self, namespace: &str, group_name: &str, service_name: &str) {
        debug!(
            "Looking for subscribers: namespace='{}', group='{}', service='{}'",
            namespace, group_name, service_name
        );

        let subscribers = self
            .naming_service
            .get_subscribers(namespace, group_name, service_name);

        debug!(
            "Found {} subscribers for {}@@{}@@{}",
            subscribers.len(),
            namespace,
            group_name,
            service_name
        );

        if subscribers.is_empty() {
            return;
        }

        // Get current service info
        let service_info =
            self.naming_service
                .get_service(namespace, group_name, service_name, "", false);

        // Build notification
        let notification =
            NotifySubscriberRequest::for_service(namespace, group_name, service_name, service_info);
        let payload = notification.build_server_push_payload();

        info!(
            "Notifying {} subscribers for service {}@@{}@@{}",
            subscribers.len(),
            namespace,
            group_name,
            service_name
        );

        for connection_id in &subscribers {
            if self
                .connection_manager
                .push_message(connection_id, payload.clone())
                .await
            {
                debug!(
                    "Pushed subscriber notification to connection {}",
                    connection_id
                );
            } else {
                warn!(
                    "Failed to push subscriber notification to connection {}",
                    connection_id
                );
            }
        }
    }

    /// Notify fuzzy watchers about service change
    async fn notify_fuzzy_watchers(
        &self,
        namespace: &str,
        group: &str,
        service_name: &str,
        change_type: &str,
        _source_ip: &str,
    ) -> anyhow::Result<()> {
        // Get watchers for this service
        let watchers = self.naming_fuzzy_watch_manager.get_watchers_for_service(
            namespace,
            group,
            service_name,
        );

        if watchers.is_empty() {
            return Ok(());
        }

        // Build group key
        let group_key = NamingFuzzyWatchPattern::build_group_key(namespace, group, service_name);

        // Get current service info for the notification
        let service_info =
            self.naming_service
                .get_service(namespace, group, service_name, "", false);

        // Build notification payload
        let notification =
            NotifySubscriberRequest::for_service(namespace, group, service_name, service_info);
        let payload = notification.build_server_push_payload();

        info!(
            "Notifying {} fuzzy watchers for service {} change: {}",
            watchers.len(),
            change_type,
            group_key
        );

        // Push notification to each watcher
        for connection_id in &watchers {
            // Mark the group key as received by this connection
            self.naming_fuzzy_watch_manager
                .mark_received(connection_id, &group_key);

            // Push the actual notification payload
            if self
                .connection_manager
                .push_message(connection_id, payload.clone())
                .await
            {
                tracing::debug!(
                    "Pushed service {} notification to connection {}: {}",
                    change_type,
                    connection_id,
                    group_key
                );
            } else {
                warn!(
                    "Failed to push service {} notification to connection {}: {}",
                    change_type, connection_id, group_key
                );
            }
        }

        info!(
            "Notified {} fuzzy watchers for service {} change: {}",
            watchers.len(),
            change_type,
            group_key
        );

        Ok(())
    }
}

// Handler for BatchInstanceRequest - batch registers or deregisters instances
#[derive(Clone)]
pub struct BatchInstanceRequestHandler {
    pub naming_service: Arc<NamingService>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
    /// Distro protocol for syncing ephemeral instances across cluster nodes
    pub distro_protocol: Option<Arc<DistroProtocol>>,
}

#[tonic::async_trait]
impl PayloadHandler for BatchInstanceRequestHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = BatchInstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let instances = request.instances;
        let req_type = &request.r#type;

        debug!(
            "BatchInstanceRequest: type='{}', namespace='{}', group='{}', service='{}', instances_count={}",
            req_type,
            namespace,
            group_name,
            service_name,
            instances.len()
        );

        let result = if req_type == REGISTER_INSTANCE || req_type == BATCH_REGISTER_INSTANCE {
            self.naming_service.batch_register_instances(
                namespace,
                group_name,
                service_name,
                instances,
            )
        } else if req_type == DE_REGISTER_INSTANCE || req_type == BATCH_DE_REGISTER_INSTANCE {
            self.naming_service.batch_deregister_instances(
                namespace,
                group_name,
                service_name,
                &instances,
            )
        } else {
            false
        };

        // Notify subscribers about the batch change
        if result {
            self.notify_subscribers(namespace, group_name, service_name)
                .await;

            // Trigger distro sync to other cluster nodes (ephemeral instances only)
            if let Some(ref distro) = self.distro_protocol {
                let service_key = format!("{}@@{}@@{}", namespace, group_name, service_name);
                let distro = distro.clone();
                tokio::spawn(async move {
                    distro
                        .sync_data(DistroDataType::NamingInstance, &service_key)
                        .await;
                });
            }
        }

        let mut response = BatchInstanceResponse::new();
        response.response.request_id = request_id;
        response.r#type = req_type.clone();

        if !result {
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = ResponseCode::Fail.code();
            response.response.success = false;
            response.response.message = "Operation failed".to_string();
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "BatchInstanceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

impl BatchInstanceRequestHandler {
    /// Notify subscribers about service change (same logic as InstanceRequestHandler)
    async fn notify_subscribers(&self, namespace: &str, group_name: &str, service_name: &str) {
        let subscribers = self
            .naming_service
            .get_subscribers(namespace, group_name, service_name);

        if subscribers.is_empty() {
            return;
        }

        let service_info =
            self.naming_service
                .get_service(namespace, group_name, service_name, "", false);

        let notification =
            NotifySubscriberRequest::for_service(namespace, group_name, service_name, service_info);
        let payload = notification.build_server_push_payload();

        info!(
            "Notifying {} subscribers for batch service change {}@@{}@@{}",
            subscribers.len(),
            namespace,
            group_name,
            service_name
        );

        for connection_id in &subscribers {
            if !self
                .connection_manager
                .push_message(connection_id, payload.clone())
                .await
            {
                warn!(
                    "Failed to push batch notification to connection {}",
                    connection_id
                );
            }
        }
    }
}

// Handler for ServiceListRequest - lists services in a namespace
#[derive(Clone)]
pub struct ServiceListRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for ServiceListRequestHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = ServiceListRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let page_no = request.page_no.max(1);
        let page_size = request.page_size.max(10);

        let (count, service_names) = self
            .naming_service
            .list_services(namespace, group_name, page_no, page_size);

        let mut response = ServiceListResponse::new();
        response.response.request_id = request_id;
        response.count = count;
        response.service_names = service_names;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ServiceListRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

// Handler for ServiceQueryRequest - queries service details and instances
#[derive(Clone)]
pub struct ServiceQueryRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for ServiceQueryRequestHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = ServiceQueryRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let cluster = &request.cluster;
        let healthy_only = request.healthy_only;

        info!(
            "ServiceQueryRequest: namespace='{}', group='{}', service='{}', cluster='{}', healthy_only={}",
            namespace, group_name, service_name, cluster, healthy_only
        );

        let service_info = self.naming_service.get_service(
            namespace,
            group_name,
            service_name,
            cluster,
            healthy_only,
        );

        info!(
            "ServiceQueryResponse: service='{}', hosts_count={}, clusters='{}'",
            service_name,
            service_info.hosts.len(),
            service_info.clusters
        );

        let mut response = QueryServiceResponse::new();
        response.response.request_id = request_id;
        response.service_info = service_info;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ServiceQueryRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

// Handler for SubscribeServiceRequest - subscribes to service changes
#[derive(Clone)]
pub struct SubscribeServiceRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for SubscribeServiceRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = SubscribeServiceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let clusters = &request.clusters;
        let subscribe = request.subscribe;

        let connection_id = &_connection.meta_info.connection_id;

        info!(
            "SubscribeServiceRequest: subscribe={}, connection_id={}, namespace='{}', group='{}', service='{}', clusters='{}'",
            subscribe, connection_id, namespace, group_name, service_name, clusters
        );

        if subscribe {
            self.naming_service
                .subscribe(connection_id, namespace, group_name, service_name);
        } else {
            self.naming_service
                .unsubscribe(connection_id, namespace, group_name, service_name);
        }

        // Return current service info
        let service_info =
            self.naming_service
                .get_service(namespace, group_name, service_name, clusters, false);

        info!(
            "SubscribeServiceResponse: service='{}', clusters='{}', hosts_count={}",
            service_name,
            service_info.clusters,
            service_info.hosts.len()
        );

        let mut response = SubscribeServiceResponse::new();
        response.response.request_id = request_id;
        response.service_info = service_info;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "SubscribeServiceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

// Handler for PersistentInstanceRequest - handles persistent (non-ephemeral) instances
#[derive(Clone)]
pub struct PersistentInstanceRequestHandler {
    pub naming_service: Arc<NamingService>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
}

#[tonic::async_trait]
impl PayloadHandler for PersistentInstanceRequestHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = PersistentInstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let mut instance = request.instance;
        let req_type = &request.r#type;

        // Mark instance as persistent (non-ephemeral)
        instance.ephemeral = false;

        let result = if req_type == REGISTER_INSTANCE {
            self.naming_service
                .register_instance(namespace, group_name, service_name, instance)
        } else if req_type == DE_REGISTER_INSTANCE {
            self.naming_service
                .deregister_instance(namespace, group_name, service_name, &instance)
        } else {
            false
        };

        // Notify subscribers about the persistent instance change
        if result {
            self.notify_subscribers(namespace, group_name, service_name)
                .await;
        }

        let mut response = InstanceResponse::new();
        response.response.request_id = request_id;
        response.r#type = req_type.clone();

        if !result {
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = ResponseCode::Fail.code();
            response.response.success = false;
            response.response.message = "Operation failed".to_string();
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "PersistentInstanceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

impl PersistentInstanceRequestHandler {
    /// Notify subscribers about service change
    async fn notify_subscribers(&self, namespace: &str, group_name: &str, service_name: &str) {
        let subscribers = self
            .naming_service
            .get_subscribers(namespace, group_name, service_name);

        if subscribers.is_empty() {
            return;
        }

        let service_info =
            self.naming_service
                .get_service(namespace, group_name, service_name, "", false);

        let notification =
            NotifySubscriberRequest::for_service(namespace, group_name, service_name, service_info);
        let payload = notification.build_server_push_payload();

        info!(
            "Notifying {} subscribers for persistent instance change {}@@{}@@{}",
            subscribers.len(),
            namespace,
            group_name,
            service_name
        );

        for connection_id in &subscribers {
            if !self
                .connection_manager
                .push_message(connection_id, payload.clone())
                .await
            {
                warn!(
                    "Failed to push persistent instance notification to connection {}",
                    connection_id
                );
            }
        }
    }
}

// Handler for NotifySubscriberRequest - notifies subscribers of service changes
// Note: This is typically a server-push request to clients
#[derive(Clone)]
pub struct NotifySubscriberHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for NotifySubscriberHandler {
    async fn handle(
        &self,
        __connection: &Connection,
        payload: &Payload,
    ) -> Result<Payload, Status> {
        let request = NotifySubscriberRequest::from(payload);
        let request_id = request.request_id();

        // This handler acknowledges the notification
        // In client context, would update local service cache
        let mut response = NotifySubscriberResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NotifySubscriberRequest"
    }
}

// Handler for NamingFuzzyWatchRequest - handles fuzzy pattern watch for services
#[derive(Clone)]
pub struct NamingFuzzyWatchHandler {
    pub naming_service: Arc<NamingService>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &_connection.meta_info.connection_id;
        let namespace = &request.namespace;
        let group_pattern = &request.group_name_pattern;
        let service_pattern = &request.service_name_pattern;
        let watch_type = &request.watch_type;

        // Build group key pattern
        let group_key_pattern = format!("{}+{}+{}", namespace, group_pattern, service_pattern);

        // Register the fuzzy watch pattern for this connection
        let registered = self.naming_fuzzy_watch_manager.register_watch(
            connection_id,
            &group_key_pattern,
            watch_type,
        );

        if !registered {
            warn!(
                "Failed to register fuzzy watch for connection {}: {}",
                connection_id, group_key_pattern
            );
        }

        // Return matching service keys if this is an initializing request
        let mut response = NamingFuzzyWatchResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NamingFuzzyWatchRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Read
    }

    fn sign_type(&self) -> &'static str {
        "naming"
    }

    fn resource_type(&self) -> batata_core::ResourceType {
        batata_core::ResourceType::Naming
    }
}

// Handler for NamingFuzzyWatchChangeNotifyRequest - notifies fuzzy watch changes
#[derive(Clone)]
pub struct NamingFuzzyWatchChangeNotifyHandler {
    pub naming_service: Arc<NamingService>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchChangeNotifyHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchChangeNotifyRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &_connection.meta_info.connection_id;
        // service_key is the group_key (format: namespace+group+service_name)
        let group_key = &request.service_key;

        // Mark the group key as received by this connection
        self.naming_fuzzy_watch_manager
            .mark_received(connection_id, group_key);

        let mut response = NamingFuzzyWatchChangeNotifyResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NamingFuzzyWatchChangeNotifyRequest"
    }
}

// Handler for NamingFuzzyWatchSyncRequest - syncs fuzzy watch state
#[derive(Clone)]
pub struct NamingFuzzyWatchSyncHandler {
    pub naming_service: Arc<NamingService>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchSyncHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &_connection.meta_info.connection_id;
        let namespace = &request.pattern_namespace;
        let group_pattern = &request.pattern_group_name;
        let service_pattern = &request.pattern_service_name;
        let sync_type = &request.sync_type;

        // Build group key pattern
        let group_key_pattern = format!("{}+{}+{}", namespace, group_pattern, service_pattern);

        // For initial sync, get all matching services
        if sync_type == "all" || request.current_batch == 0 {
            // Get all services matching the pattern
            let _matched_services = self.naming_service.get_services_by_pattern(
                namespace,
                group_pattern,
                service_pattern,
            );
            // The matched services would be sent to client in batches
            // This handler acknowledges the sync request
        }

        // Register the pattern if not already registered
        let registered = self.naming_fuzzy_watch_manager.register_watch(
            connection_id,
            &group_key_pattern,
            sync_type,
        );

        if !registered {
            warn!(
                "Failed to register fuzzy watch sync for connection {}: {}",
                connection_id, group_key_pattern
            );
        }

        let mut response = NamingFuzzyWatchSyncResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NamingFuzzyWatchSyncRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}
