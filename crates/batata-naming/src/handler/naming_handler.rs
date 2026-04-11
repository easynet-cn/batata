// Naming module gRPC handlers
// Implements handlers for service discovery requests

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Semaphore;

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
        NamingContext, NamingFuzzyWatchChangeNotifyRequest, NamingFuzzyWatchChangeNotifyResponse,
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

/// Max concurrent background notification tasks to prevent unbounded spawning under load.
const MAX_CONCURRENT_PUSH_TASKS: usize = 1024;

/// Shared semaphore for limiting concurrent push notification tasks.
static PUSH_SEMAPHORE: std::sync::LazyLock<Arc<Semaphore>> =
    std::sync::LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_PUSH_TASKS)));

// Handler for InstanceRequest - registers or deregisters a service instance
#[derive(Clone)]
pub struct InstanceRequestHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
    /// Distro protocol for syncing ephemeral instances across cluster nodes
    pub distro_protocol: Option<Arc<DistroProtocol>>,
    /// Ephemeral/persistent dispatch proxy. When present, register and
    /// deregister route through the proxy so Raft writes for persistent
    /// instances happen automatically. Set by startup when both a
    /// NamingService and (optionally) RaftNode / DistroProtocol are wired.
    /// When `None`, handler falls back to direct naming_service calls —
    /// used in tests and degraded single-node scenarios.
    pub client_op_proxy: Option<Arc<crate::service::ClientOperationServiceProxy>>,
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
        // Capture ephemeral flag before `instance` is moved into the write
        // call. Distro broadcast only applies to the AP path; persistent
        // instances arriving through InstanceRequest (unusual but possible)
        // must NOT be re-synced via Distro because their authoritative
        // source is Raft.
        let is_ephemeral = instance.ephemeral;
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

        // Build keys for tracking before the instance is moved
        let service_key = crate::service::build_service_key(namespace, group_name, service_name);
        let instance_key = crate::service::build_instance_key_parts(
            &instance.ip,
            instance.port,
            &instance.cluster_name,
        );

        let result = if req_type == REGISTER_INSTANCE {
            // Route through the ClientOperationServiceProxy when available —
            // this ensures persistent instances (ephemeral=false) arriving
            // through InstanceRequest are written via Raft, not just the
            // in-memory DashMap. Falls back to direct write if no proxy
            // is wired (unit tests).
            let ok = if let Some(ref proxy) = self.client_op_proxy {
                use crate::service::ClientOperationService;
                proxy
                    .register_instance(namespace, group_name, service_name, instance)
                    .await
            } else {
                self.naming_service.register_instance(
                    namespace,
                    group_name,
                    service_name,
                    instance,
                )
            };
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
            let ok = if let Some(ref proxy) = self.client_op_proxy {
                use crate::service::ClientOperationService;
                proxy
                    .deregister_instance(namespace, group_name, service_name, &instance)
                    .await
            } else {
                self.naming_service.deregister_instance(
                    namespace,
                    group_name,
                    service_name,
                    &instance,
                )
            };
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

            // Trigger distro sync to other cluster nodes (ephemeral only).
            // Persistent instances never broadcast via Distro — their
            // authoritative source is Raft.
            if is_ephemeral {
                if let Some(ref distro) = self.distro_protocol {
                    let service_key =
                        crate::service::build_service_key(namespace, group_name, service_name);
                    let distro = distro.clone();
                    tokio::spawn(async move {
                        distro
                            .sync_data(DistroDataType::NamingInstance, &service_key)
                            .await;
                    });
                }
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
        if tracing::enabled!(tracing::Level::DEBUG)
            && let Ok(json) = serde_json::to_string(&notification)
        {
            debug!("NotifySubscriberRequest JSON: {}", json);
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

        // Push notifications in background to avoid blocking the registration response.
        // Use semaphore to bound concurrent push tasks under high load.
        let cm = self.connection_manager.clone();
        let subscriber_count = subscribers.len();
        let semaphore = PUSH_SEMAPHORE.clone();
        tokio::spawn(async move {
            let _permit = semaphore.acquire().await;
            let sent = cm.push_message_to_many(&subscribers, payload).await;
            tracing::debug!(
                "Pushed subscriber notification to {}/{} connections",
                sent,
                subscriber_count
            );
        });
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
        let service_key = crate::service::build_service_key(namespace, group, service_name);

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
        // Nacos 3.x BatchInstanceRequest is defined only for ephemeral
        // instances. If any incoming instance is non-ephemeral, skip
        // Distro broadcast for the whole batch — persistent instances
        // must not leak into the AP path.
        let batch_all_ephemeral = !instances.is_empty() && instances.iter().all(|i| i.ephemeral);
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

        let service_key = crate::service::build_service_key(namespace, group_name, service_name);

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

            // Trigger distro sync to other cluster nodes only when the
            // whole batch is ephemeral (persistent must go through Raft).
            if batch_all_ephemeral {
                if let Some(ref distro) = self.distro_protocol {
                    let service_key =
                        crate::service::build_service_key(namespace, group_name, service_name);
                    let distro = distro.clone();
                    tokio::spawn(async move {
                        distro
                            .sync_data(DistroDataType::NamingInstance, &service_key)
                            .await;
                    });
                }
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
    /// Raft node for replicating persistent instance writes. When present,
    /// register/deregister go through `RaftRequest::PersistentInstance*`
    /// so all cluster members apply the change via the state machine and
    /// the data survives process restart. When absent (single-node dev
    /// mode without Raft), falls back to direct in-memory writes.
    pub raft_node: Option<Arc<batata_consistency::raft::RaftNode>>,
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

        // Mark instance as persistent (non-ephemeral)
        instance.ephemeral = false;

        let result = if let Some(ref raft) = self.raft_node {
            // CP path: replicate via Raft. The apply-back hook installed on
            // the state machine updates NamingService DashMap on every node
            // (leader + followers) so reads see the change immediately.
            let instance_id = format!(
                "{}#{}#{}",
                instance.ip,
                instance.port,
                if instance.cluster_name.is_empty() {
                    "DEFAULT"
                } else {
                    &instance.cluster_name
                }
            );
            if req_type == REGISTER_INSTANCE {
                let metadata_json = serde_json::to_string(&instance.metadata)
                    .unwrap_or_else(|_| "{}".to_string());
                let req = batata_consistency::raft::RaftRequest::PersistentInstanceRegister(
                    Box::new(
                        batata_consistency::raft::request::PersistentInstanceRegisterPayload {
                            namespace_id: namespace.clone(),
                            group_name: group_name.clone(),
                            service_name: service_name.clone(),
                            instance_id,
                            ip: instance.ip.clone(),
                            port: instance.port as u16,
                            weight: instance.weight,
                            healthy: instance.healthy,
                            enabled: instance.enabled,
                            metadata: metadata_json,
                            cluster_name: if instance.cluster_name.is_empty() {
                                "DEFAULT".to_string()
                            } else {
                                instance.cluster_name.clone()
                            },
                        },
                    ),
                );
                match raft.write(req).await {
                    Ok(resp) => resp.success,
                    Err(e) => {
                        warn!("Raft write PersistentInstanceRegister failed: {}", e);
                        false
                    }
                }
            } else if req_type == DE_REGISTER_INSTANCE {
                let req = batata_consistency::raft::RaftRequest::PersistentInstanceDeregister {
                    namespace_id: namespace.clone(),
                    group_name: group_name.clone(),
                    service_name: service_name.clone(),
                    instance_id,
                };
                match raft.write(req).await {
                    Ok(resp) => resp.success,
                    Err(e) => {
                        warn!("Raft write PersistentInstanceDeregister failed: {}", e);
                        false
                    }
                }
            } else {
                warn!("Unsupported persistent instance request type: {}", req_type);
                false
            }
        } else if req_type == REGISTER_INSTANCE {
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
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
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

        // Trigger initial sync: find existing matching services and push them
        // Aligned with Nacos NamingFuzzyWatchRequestHandler which publishes ClientFuzzyWatchEvent
        // to trigger NamingFuzzyWatchSyncNotifier for initial data delivery.
        if request.initializing {
            info!(
                "NamingFuzzyWatch initial sync: pattern={}, connection={}",
                group_key_pattern, connection_id
            );
            let pattern = NamingFuzzyWatchPattern::from_group_key_pattern(&group_key_pattern);
            if let Some(pattern) = pattern {
                let all_keys = self.naming_service.get_all_service_keys();
                info!(
                    "NamingFuzzyWatch initial sync: {} total service keys, pattern ns={} group={} svc={}",
                    all_keys.len(),
                    pattern.namespace,
                    pattern.group_pattern,
                    pattern.service_name_pattern
                );
                let matched: Vec<String> = all_keys
                    .into_iter()
                    .filter(|key| {
                        // Service keys are in format: namespace@@group@@serviceName
                        let mut parts = key.splitn(3, "@@");
                        if let (Some(ns), Some(group), Some(svc)) =
                            (parts.next(), parts.next(), parts.next())
                        {
                            return pattern.matches(ns, group, svc);
                        }
                        false
                    })
                    .collect();

                info!(
                    "NamingFuzzyWatch initial sync: {} matched services out of {} total",
                    matched.len(),
                    self.naming_service.get_all_service_keys().len()
                );
                {
                    let batches = divide_into_batches(&matched, FUZZY_WATCH_BATCH_SIZE);
                    let total_batch = batches.len().max(1) as i32;
                    let conn_mgr = self.connection_manager.clone();
                    let conn_id = connection_id.clone();
                    let watch_mgr = self.naming_fuzzy_watch_manager.clone();
                    let ns = pattern.namespace.clone();
                    let gp = pattern.group_pattern.clone();
                    let sp = pattern.service_name_pattern.clone();
                    let gkp = group_key_pattern.clone();

                    tokio::spawn(async move {
                        // Send batches of matching services
                        for (batch_idx, batch) in batches.iter().enumerate() {
                            let current_batch = (batch_idx + 1) as i32;
                            let contexts: HashSet<NamingContext> = batch
                                .iter()
                                .map(|key| NamingContext {
                                    service_key: key.clone(),
                                    changed_type: CHANGE_TYPE_ADD.to_string(),
                                })
                                .collect();

                            let mut sync_req = NamingFuzzyWatchSyncRequest::new();
                            sync_req.group_key_pattern = gkp.clone();
                            sync_req.pattern_namespace = ns.clone();
                            sync_req.pattern_group_name = gp.clone();
                            sync_req.pattern_service_name = sp.clone();
                            sync_req.fuzzy_watch_notify_request.sync_type =
                                SYNC_TYPE_INIT_NOTIFY.to_string();
                            sync_req.total_batch = total_batch;
                            sync_req.current_batch = current_batch;
                            sync_req.contexts = contexts;

                            let push_payload = sync_req.build_server_push_payload();
                            let sent = conn_mgr.push_message(&conn_id, push_payload).await;
                            if sent {
                                for key in batch {
                                    watch_mgr.mark_received(&conn_id, key);
                                }
                            } else {
                                break;
                            }
                        }

                        // Always send FINISH notification (even when no matches)
                        // to complete SDK initialization
                        let mut finish_req = NamingFuzzyWatchSyncRequest::new();
                        finish_req.group_key_pattern = gkp;
                        finish_req.pattern_namespace = ns;
                        finish_req.pattern_group_name = gp;
                        finish_req.pattern_service_name = sp;
                        finish_req.fuzzy_watch_notify_request.sync_type =
                            SYNC_TYPE_FINISH_NOTIFY.to_string();
                        finish_req.total_batch = total_batch;
                        finish_req.current_batch = total_batch;
                        let finish_payload = finish_req.build_server_push_payload();
                        conn_mgr.push_message(&conn_id, finish_payload).await;
                    });
                }
            }
        }

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

/// Batch size for fuzzy watch sync (matches Nacos BATCH_SIZE = 10)
const FUZZY_WATCH_BATCH_SIZE: usize = 10;

/// Sync type constants
const SYNC_TYPE_ALL: &str = "all";
const SYNC_TYPE_INIT_NOTIFY: &str = "FUZZY_WATCH_INIT_NOTIFY";
const SYNC_TYPE_FINISH_NOTIFY: &str = "FINISH_FUZZY_WATCH_INIT_NOTIFY";
const CHANGE_TYPE_ADD: &str = "ADD_SERVICE";

// Handler for NamingFuzzyWatchSyncRequest - syncs fuzzy watch state with batch push
#[derive(Clone)]
pub struct NamingFuzzyWatchSyncHandler {
    pub naming_service: Arc<dyn NamingServiceProvider>,
    pub naming_fuzzy_watch_manager: Arc<NamingFuzzyWatchManager>,
    pub connection_manager: Arc<batata_core::service::remote::ConnectionManager>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchSyncHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = _connection.meta_info.connection_id.clone();
        let namespace = request.pattern_namespace.clone();
        let group_pattern = request.pattern_group_name.clone();
        let service_pattern = request.pattern_service_name.clone();
        let sync_type = request.fuzzy_watch_notify_request.sync_type.clone();

        // Build group key pattern in >> format
        let group_key_pattern = format!("{}>>{}>>{}", namespace, group_pattern, service_pattern);

        // Register the pattern if not already registered
        match self.naming_fuzzy_watch_manager.register_watch(
            &connection_id,
            &group_key_pattern,
            &sync_type,
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

        // For initial sync, find all matching services and push in batches
        if sync_type == SYNC_TYPE_ALL || request.current_batch == 0 {
            let pattern = NamingFuzzyWatchPattern::from_group_key_pattern(&group_key_pattern);

            if let Some(pattern) = pattern {
                // Get all service keys and find matches
                let all_keys = self.naming_service.get_all_service_keys();
                let matched: Vec<String> = all_keys
                    .into_iter()
                    .filter(|key| {
                        // Service keys are in format: namespace##group@@serviceName
                        if let Some((ns, rest)) = key.split_once("##")
                            && let Some((group, svc)) = rest.split_once("@@")
                        {
                            return pattern.matches(ns, group, svc);
                        }
                        false
                    })
                    .collect();

                // Check match count limit
                if let Err(e) = self
                    .naming_fuzzy_watch_manager
                    .check_match_count(matched.len(), &group_key_pattern)
                {
                    warn!("Fuzzy watch match count exceeded: {}", e);
                    // Continue with response but don't push batches
                } else {
                    // Divide into batches and push asynchronously
                    let batches = divide_into_batches(&matched, FUZZY_WATCH_BATCH_SIZE);
                    let total_batch = batches.len().max(1) as i32;

                    let conn_mgr = self.connection_manager.clone();
                    let conn_id = connection_id.clone();
                    let ns = namespace.clone();
                    let gp = group_pattern.clone();
                    let sp = service_pattern.clone();
                    let gkp = group_key_pattern.clone();
                    let watch_mgr = self.naming_fuzzy_watch_manager.clone();

                    // Spawn async task to push batches (don't block the handler)
                    tokio::spawn(async move {
                        for (batch_idx, batch) in batches.iter().enumerate() {
                            let current_batch = (batch_idx + 1) as i32;

                            let contexts: HashSet<NamingContext> = batch
                                .iter()
                                .map(|key| NamingContext {
                                    service_key: key.clone(),
                                    changed_type: CHANGE_TYPE_ADD.to_string(),
                                })
                                .collect();

                            let mut sync_req = NamingFuzzyWatchSyncRequest::new();
                            sync_req.group_key_pattern = gkp.clone();
                            sync_req.pattern_namespace = ns.clone();
                            sync_req.pattern_group_name = gp.clone();
                            sync_req.pattern_service_name = sp.clone();
                            sync_req.fuzzy_watch_notify_request.sync_type =
                                SYNC_TYPE_INIT_NOTIFY.to_string();
                            sync_req.total_batch = total_batch;
                            sync_req.current_batch = current_batch;
                            sync_req.contexts = contexts;

                            let push_payload = sync_req.build_server_push_payload();
                            let sent = conn_mgr.push_message(&conn_id, push_payload).await;

                            if sent {
                                for key in batch {
                                    watch_mgr.mark_received(&conn_id, key);
                                }
                            } else {
                                break;
                            }
                        }

                        // Always send FINISH notification to complete SDK initialization
                        let mut finish_req = NamingFuzzyWatchSyncRequest::new();
                        finish_req.group_key_pattern = gkp;
                        finish_req.pattern_namespace = ns;
                        finish_req.pattern_group_name = gp;
                        finish_req.pattern_service_name = sp;
                        finish_req.fuzzy_watch_notify_request.sync_type =
                            SYNC_TYPE_FINISH_NOTIFY.to_string();
                        finish_req.total_batch = total_batch;
                        finish_req.current_batch = total_batch;

                        let finish_payload = finish_req.build_server_push_payload();
                        conn_mgr.push_message(&conn_id, finish_payload).await;
                    });
                }
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

/// Divide a list of items into batches of the given size
fn divide_into_batches(items: &[String], batch_size: usize) -> Vec<Vec<String>> {
    items
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}
