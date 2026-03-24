// Naming module gRPC handlers
// Implements handlers for service discovery requests

use std::sync::Arc;

use batata_core::{
    GrpcResource, PermissionAction,
    handler::rpc::{AuthRequirement, PayloadHandler},
    model::Connection,
    service::distro::{DistroDataType, DistroProtocol},
};
use tonic::Status;
use tracing::{debug, info, warn};

use batata_api::{
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
};

use batata_api::naming::NamingServiceProvider;

use crate::handler::naming_fuzzy_watch::{NamingFuzzyWatchManager, NamingFuzzyWatchPattern};

// Handler for InstanceRequest - registers or deregisters a service instance
#[derive(Clone)]
pub struct InstanceRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
    /// Distro protocol for syncing ephemeral instances across cluster nodes
    pub distro_protocol: Option<Arc<DistroProtocol>>,
}

#[tonic::async_trait]
impl PayloadHandler for InstanceRequestHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = InstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let mut instance = request.instance;
        if instance.cluster_name.is_empty() {
            instance.cluster_name = "DEFAULT".to_string();
        }
        let req_type = &request.r#type;
        let connection_id = &connection.meta_info.connection_id;

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

        // Ensure source is Batata for SDK registrations
        instance.register_source = batata_api::naming::RegisterSource::Batata;

        // Build keys for tracking before the instance is moved
        let service_key = format!("{}@@{}@@{}", namespace, group_name, service_name);
        let instance_key = format!(
            "{}#{}#{}",
            instance.ip, instance.port, instance.cluster_name
        );

        let result = if req_type == REGISTER_INSTANCE {
            let ok = self.naming_service.register_instance(
                namespace,
                group_name,
                service_name,
                instance,
            );
            if ok {
                self.naming_service.add_publisher(
                    connection_id,
                    namespace,
                    group_name,
                    service_name,
                );
                self.naming_service.add_connection_instance(
                    connection_id,
                    &service_key,
                    &instance_key,
                );
            }
            ok
        } else if req_type == DE_REGISTER_INSTANCE {
            let ok = self.naming_service.deregister_instance(
                namespace,
                group_name,
                service_name,
                &instance,
            );
            if ok {
                self.naming_service.remove_publisher(
                    connection_id,
                    namespace,
                    group_name,
                    service_name,
                );
                self.naming_service.remove_connection_instance(
                    connection_id,
                    &service_key,
                    &instance_key,
                );
            }
            ok
        } else {
            warn!("Unsupported instance request type: {}", req_type);
            false
        };

        // Notify subscribers and fuzzy watchers about service change
        if result {
            // Notify regular subscribers (SubscribeServiceRequest subscribers)
            self.notify_subscribers(namespace, group_name, service_name)
                .await;

            // Notify fuzzy watchers with Nacos-compatible change types
            let fuzzy_change_type = if req_type == REGISTER_INSTANCE {
                "ADD_SERVICE"
            } else {
                "DELETE_SERVICE"
            };
            if let Err(e) = self
                .notify_fuzzy_watchers(
                    namespace,
                    group_name,
                    service_name,
                    fuzzy_change_type,
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
            response.response.error_code = batata_common::error::INSTANCE_ERROR.code;
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = InstanceRequest::from(payload);
        Some((
            GrpcResource::naming(
                &request.naming_request.namespace,
                &request.naming_request.group_name,
                &request.naming_request.service_name,
            ),
            PermissionAction::Write,
        ))
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

        // Debug: log the exact JSON being sent so we can compare with Nacos
        if let Ok(json) = serde_json::to_string(&notification) {
            info!("NotifySubscriberRequest JSON: {}", json);
        }

        let payload = notification.build_server_push_payload();

        // Debug: log payload metadata type
        if let Some(meta) = &payload.metadata {
            info!(
                "NotifySubscriberRequest payload type='{}', body_len={}",
                meta.r#type,
                payload.body.as_ref().map(|b| b.value.len()).unwrap_or(0)
            );
        }

        info!(
            "Notifying {} subscribers for service {}@@{}@@{}",
            subscribers.len(),
            namespace,
            group_name,
            service_name
        );

        // Use push_message_to_many to avoid cloning payload for each subscriber
        let sent = self
            .connection_manager
            .push_message_to_many(&subscribers, payload)
            .await;
        debug!(
            "Pushed subscriber notification to {}/{} connections",
            sent,
            subscribers.len()
        );
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

        // Build service key in Nacos format: namespace@@group@@serviceName
        let service_key = format!("{}@@{}@@{}", namespace, group, service_name);

        // Build notification payload using NamingFuzzyWatchChangeNotifyRequest
        let mut notification =
            batata_api::naming::model::NamingFuzzyWatchChangeNotifyRequest::new();
        notification.service_key = service_key.clone();
        notification.changed_type = change_type.to_string();
        let payload = notification.build_server_push_payload();

        info!(
            "Notifying {} fuzzy watchers for service {} change: {}",
            watchers.len(),
            change_type,
            service_key
        );

        // Mark all watchers as received before sending
        for connection_id in &watchers {
            self.naming_fuzzy_watch_manager
                .mark_received(connection_id, &service_key);
        }

        // Use push_message_to_many to avoid cloning payload for each watcher
        self.connection_manager
            .push_message_to_many(&watchers, payload)
            .await;

        info!(
            "Notified {} fuzzy watchers for service {} change: {}",
            watchers.len(),
            change_type,
            service_key
        );

        Ok(())
    }
}

// Handler for BatchInstanceRequest - batch registers or deregisters instances
#[derive(Clone)]
pub struct BatchInstanceRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
    /// Distro protocol for syncing ephemeral instances across cluster nodes
    pub distro_protocol: Option<Arc<DistroProtocol>>,
}

#[tonic::async_trait]
impl PayloadHandler for BatchInstanceRequestHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = BatchInstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let mut instances = request.instances;
        for inst in &mut instances {
            if inst.cluster_name.is_empty() {
                inst.cluster_name = "DEFAULT".to_string();
            }
        }
        let req_type = &request.r#type;
        let connection_id = &connection.meta_info.connection_id;

        debug!(
            "BatchInstanceRequest: type='{}', namespace='{}', group='{}', service='{}', instances_count={}",
            req_type,
            namespace,
            group_name,
            service_name,
            instances.len()
        );
        for (idx, inst) in instances.iter().enumerate() {
            debug!(
                "  BatchInstance[{}]: ip={}, port={}, cluster='{}', healthy={}, weight={}",
                idx, inst.ip, inst.port, inst.cluster_name, inst.healthy, inst.weight
            );
        }

        let service_key = format!("{}@@{}@@{}", namespace, group_name, service_name);

        let result = if req_type == REGISTER_INSTANCE || req_type == BATCH_REGISTER_INSTANCE {
            // Build instance keys before instances are moved
            let instance_keys: Vec<String> = instances
                .iter()
                .map(|inst| {
                    let cluster = if inst.cluster_name.is_empty() {
                        "DEFAULT"
                    } else {
                        &inst.cluster_name
                    };
                    format!("{}#{}#{}", inst.ip, inst.port, cluster)
                })
                .collect();

            let ok = self.naming_service.batch_register_instances(
                namespace,
                group_name,
                service_name,
                instances,
            );
            if ok {
                self.naming_service.add_publisher(
                    connection_id,
                    namespace,
                    group_name,
                    service_name,
                );
                for ik in &instance_keys {
                    self.naming_service
                        .add_connection_instance(connection_id, &service_key, ik);
                }
            }
            ok
        } else if req_type == DE_REGISTER_INSTANCE || req_type == BATCH_DE_REGISTER_INSTANCE {
            // Build instance keys before deregistering
            let instance_keys: Vec<String> = instances
                .iter()
                .map(|inst| {
                    let cluster = if inst.cluster_name.is_empty() {
                        "DEFAULT"
                    } else {
                        &inst.cluster_name
                    };
                    format!("{}#{}#{}", inst.ip, inst.port, cluster)
                })
                .collect();

            let ok = self.naming_service.batch_deregister_instances(
                namespace,
                group_name,
                service_name,
                instances,
            );
            if ok {
                self.naming_service.remove_publisher(
                    connection_id,
                    namespace,
                    group_name,
                    service_name,
                );
                for ik in &instance_keys {
                    self.naming_service
                        .remove_connection_instance(connection_id, &service_key, ik);
                }
            }
            ok
        } else {
            warn!("Unsupported batch instance request type: {}", req_type);
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
            response.response.error_code = batata_common::error::INSTANCE_ERROR.code;
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = BatchInstanceRequest::from(payload);
        Some((
            GrpcResource::naming(
                &request.naming_request.namespace,
                &request.naming_request.group_name,
                &request.naming_request.service_name,
            ),
            PermissionAction::Write,
        ))
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

        // Use push_message_to_many to avoid cloning payload for each subscriber
        self.connection_manager
            .push_message_to_many(&subscribers, payload)
            .await;
    }
}

// Handler for ServiceListRequest - lists services in a namespace
#[derive(Clone)]
pub struct ServiceListRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
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
        let page_size = if request.page_size <= 0 {
            10
        } else {
            request.page_size
        };

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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = ServiceListRequest::from(payload);
        Some((
            GrpcResource::naming(
                &request.naming_request.namespace,
                &request.naming_request.group_name,
                "",
            ),
            PermissionAction::Read,
        ))
    }
}

// Handler for ServiceQueryRequest - queries service details and instances
#[derive(Clone)]
pub struct ServiceQueryRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
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

        let service_info = self.naming_service.get_service_by_source(
            namespace,
            group_name,
            service_name,
            cluster,
            healthy_only,
            Some(batata_api::naming::RegisterSource::Batata),
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = ServiceQueryRequest::from(payload);
        Some((
            GrpcResource::naming(
                &request.naming_request.namespace,
                &request.naming_request.group_name,
                &request.naming_request.service_name,
            ),
            PermissionAction::Read,
        ))
    }
}

// Handler for SubscribeServiceRequest - subscribes to service changes
#[derive(Clone)]
pub struct SubscribeServiceRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
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

        // Return current service info (filtered to Batata-registered instances only)
        let service_info = self.naming_service.get_service_by_source(
            namespace,
            group_name,
            service_name,
            clusters,
            false,
            Some(batata_api::naming::RegisterSource::Batata),
        );

        info!(
            "SubscribeServiceResponse: service='{}', clusters='{}', hosts_count={}",
            service_name,
            service_info.clusters,
            service_info.hosts.len()
        );

        let mut response = SubscribeServiceResponse::new();
        response.response.request_id = request_id;
        response.response.message = "success".to_string();
        response.service_info = service_info;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "SubscribeServiceRequest"
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = SubscribeServiceRequest::from(payload);
        Some((
            GrpcResource::naming(
                &request.naming_request.namespace,
                &request.naming_request.group_name,
                &request.naming_request.service_name,
            ),
            PermissionAction::Read,
        ))
    }
}

// Handler for PersistentInstanceRequest - handles persistent (non-ephemeral) instances
#[derive(Clone)]
pub struct PersistentInstanceRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
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
        if instance.cluster_name.is_empty() {
            instance.cluster_name = "DEFAULT".to_string();
        }
        let req_type = &request.r#type;

        // Mark instance as persistent (non-ephemeral) and source as Batata
        instance.ephemeral = false;
        instance.register_source = batata_api::naming::RegisterSource::Batata;

        let result = if req_type == REGISTER_INSTANCE {
            self.naming_service
                .register_instance(namespace, group_name, service_name, instance)
        } else if req_type == DE_REGISTER_INSTANCE {
            self.naming_service
                .deregister_instance(namespace, group_name, service_name, &instance)
        } else {
            warn!("Unsupported persistent instance request type: {}", req_type);
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
            response.response.error_code = batata_common::error::INSTANCE_ERROR.code;
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

    fn resource_from_payload(
        &self,
        payload: &batata_api::grpc::Payload,
    ) -> Option<(GrpcResource, PermissionAction)> {
        let request = PersistentInstanceRequest::from(payload);
        Some((
            GrpcResource::naming(
                &request.naming_request.namespace,
                &request.naming_request.group_name,
                &request.naming_request.service_name,
            ),
            PermissionAction::Write,
        ))
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

        // Use push_message_to_many to avoid cloning payload for each subscriber
        self.connection_manager
            .push_message_to_many(&subscribers, payload)
            .await;
    }
}

// Handler for NotifySubscriberRequest - notifies subscribers of service changes
// Note: This is typically a server-push request to clients
#[derive(Clone)]
pub struct NotifySubscriberHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
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
    pub naming_service: Arc<dyn NamingServiceProvider>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &_connection.meta_info.connection_id;
        let watch_type = &request.watch_type;

        // Use groupKeyPattern from request (already in ">>" format from SDK)
        let group_key_pattern = if !request.group_key_pattern.is_empty() {
            request.group_key_pattern.clone()
        } else {
            format!(
                "{}>>{}>>{}",
                request.namespace, request.group_name_pattern, request.service_name_pattern
            )
        };

        // Register the fuzzy watch pattern for this connection
        match self.naming_fuzzy_watch_manager.register_watch(
            connection_id,
            &group_key_pattern,
            watch_type,
        ) {
            Ok(true) => {
                debug!(
                    "Registered naming fuzzy watch for connection {}: pattern={}",
                    connection_id, group_key_pattern
                );
            }
            Ok(false) => {
                warn!(
                    "Failed to register fuzzy watch for connection {}: invalid pattern {}",
                    connection_id, group_key_pattern
                );
            }
            Err(e) => {
                warn!(
                    "Naming fuzzy watch registration rejected for connection {}: {}",
                    connection_id, e
                );
                return Err(Status::resource_exhausted(e.to_string()));
            }
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
    pub naming_service: Arc<dyn NamingServiceProvider>,
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
    pub naming_service: Arc<dyn NamingServiceProvider>,
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

        // Build group key pattern in >> format
        let group_key_pattern = format!("{}>>{}>>{}", namespace, group_pattern, service_pattern);

        // For initial sync, get all matching services
        if sync_type == "all" || request.current_batch == 0 {
            // TODO: The matched services would be sent to client in batches
            // This handler acknowledges the sync request
        }

        // Register the pattern if not already registered
        match self.naming_fuzzy_watch_manager.register_watch(
            connection_id,
            &group_key_pattern,
            sync_type,
        ) {
            Ok(_) => {}
            Err(e) => {
                warn!(
                    "Naming fuzzy watch sync registration rejected for connection {}: {}",
                    connection_id, e
                );
                return Err(Status::resource_exhausted(e.to_string()));
            }
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
